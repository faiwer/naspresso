use std::{
    collections::HashMap,
    fs,
    io::{self, Cursor},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use crypto::ContentError;
use media::preview::{PreviewError, PreviewSize, Rendered, render, render_reader};
use thiserror::Error;
use tokio::{
    io::AsyncReadExt,
    sync::{OnceCell, Semaphore},
};

use super::bundle::{self, BundledAvif};
use crate::cancel::{Cancel, Cancelled};

/// A preview, from the cache or just made.
pub enum Preview {
    Cached(BundledAvif),
    Fresh(Vec<u8>),
}

#[derive(Debug, Error)]
pub enum PreviewCacheError {
    #[error(transparent)]
    Preview(#[from] PreviewError),

    /// Cancelled while waiting for a render slot.
    #[error(transparent)]
    Cancelled(#[from] Cancelled),

    /// The vault key does not open this `.npe` file.
    #[error("cannot decrypt the file")]
    Decrypt,
}

impl From<io::Error> for PreviewCacheError {
    fn from(err: io::Error) -> Self {
        Self::Preview(err.into())
    }
}

/// Bundle path and source fingerprint.
type RenderKey = (PathBuf, String);

/// Vault key and decrypted name for one `.npe` source.
pub struct OpenNpe {
    pub key: ::crypto::Key,
    pub decrypted_name: String,
}

/// Previews of files under `STORAGE`, made on demand and kept under `CACHE`
/// in a tree mirroring it: `a/b/c.jpg` is cached as `preview/a/b/c.jpg.preview`.
/// An `.npe` bundle keeps that layout; each size's AVIF is encrypted.
pub struct PreviewCache {
    storage: PathBuf,
    root: PathBuf,
    /// AVIF quality, `1..=100`.
    quality: f32,
    /// AVIF encoding speed, `1..=10`.
    speed: u8,
    /// One permit per render running at once; waiters are served in order.
    slots: Arc<Semaphore>,
    /// Renders in progress. Requests for the same source wait for one render
    /// instead of starting their own.
    rendering: Mutex<HashMap<RenderKey, Arc<OnceCell<Arc<Rendered>>>>>,
}

impl PreviewCache {
    pub fn new(
        storage: PathBuf,
        cache: &Path,
        quality: f32,
        speed: u8,
        workers: NonZeroUsize,
    ) -> Self {
        Self {
            storage,
            root: cache.join("preview"),
            quality,
            speed,
            slots: Arc::new(Semaphore::new(workers.get())),
            rendering: Mutex::default(),
        }
    }

    /// The `size` preview of `source`, a file under `STORAGE` whose current
    /// fingerprint is `fingerprint`. `npe` decrypts an `.npe` source and
    /// encrypts each cached AVIF with its content key. Gives up on `cancel`
    /// only while waiting for a render slot: a started render is finished and
    /// cached.
    pub async fn get(
        &self,
        source: &Path,
        fingerprint: &str,
        size: PreviewSize,
        cancel: &Cancel,
        npe: Option<&OpenNpe>,
    ) -> Result<Preview, PreviewCacheError> {
        let bundle_path = self.bundle_path(source)?;
        if let Some(hit) = open_cached(&bundle_path, fingerprint, size, npe).await? {
            return Ok(hit);
        }

        let rendered = self
            .render_once(source, bundle_path, fingerprint, cancel, npe)
            .await?;
        let avif = rendered
            .avif(size)
            .ok_or_else(|| io::Error::other("a size is missing from a render"))?;
        Ok(Preview::Fresh(avif.to_vec()))
    }

    /// Whether `source`, last modified at `modified`, has previews cached that
    /// are at least as new. Blocks on a `stat`.
    pub fn is_fresh(&self, source: &Path, modified: SystemTime) -> bool {
        self.bundle_path(source)
            .and_then(fs::metadata)
            .and_then(|meta| meta.modified())
            .is_ok_and(|cached| cached >= modified)
    }

    fn bundle_path(&self, source: &Path) -> io::Result<PathBuf> {
        let relative = source
            .strip_prefix(&self.storage)
            .map_err(|_| io::Error::other("preview source outside STORAGE"))?;
        Ok(bundle::with_suffix(&self.root.join(relative), ".preview"))
    }

    async fn render_once(
        &self,
        source: &Path,
        bundle_path: PathBuf,
        fingerprint: &str,
        cancel: &Cancel,
        npe: Option<&OpenNpe>,
    ) -> Result<Arc<Rendered>, PreviewCacheError> {
        let key = (bundle_path, fingerprint.to_owned());
        let cell = self
            .rendering
            .lock()
            .expect("rendering lock poisoned")
            .entry(key.clone())
            .or_default()
            .clone();

        // A cancelled waiter fails its own attempt only; the next one in line
        // tries again.
        let result = cell
            .get_or_try_init(|| self.render_and_store(source, &key.0, fingerprint, cancel, npe))
            .await
            .cloned();

        let mut rendering = self.rendering.lock().expect("rendering lock poisoned");
        if rendering
            .get(&key)
            .is_some_and(|current| Arc::ptr_eq(current, &cell))
        {
            rendering.remove(&key);
        }
        result
    }

    async fn render_and_store(
        &self,
        source: &Path,
        bundle_path: &Path,
        fingerprint: &str,
        cancel: &Cancel,
        npe: Option<&OpenNpe>,
    ) -> Result<Arc<Rendered>, PreviewCacheError> {
        let slot = cancel
            .until(Arc::clone(&self.slots).acquire_owned())
            .await?
            .expect("render slots are never closed");

        let (source, quality, speed) = (source.to_path_buf(), self.quality, self.speed);
        let sealed = npe.map(|open| (open.key.clone(), open.decrypted_name.to_owned()));
        let rendered =
            tokio::task::spawn_blocking(move || -> Result<Rendered, PreviewCacheError> {
                let _slot = slot;
                if let Some((key, decrypted_name)) = &sealed {
                    render_npe(&source, key, decrypted_name, quality, speed)
                } else {
                    render(&source, quality, speed).map_err(PreviewCacheError::from)
                }
            })
            .await
            .map_err(io::Error::other)??;

        let bytes = if let Some(open) = npe {
            encode_sealed(fingerprint, &rendered, &open.key)?
        } else {
            bundle::encode(fingerprint, &rendered)
        };
        if let Err(e) = bundle::write(bundle_path, &bytes).await {
            tracing::warn!(error = %e, path = %bundle_path.display(), "can't cache previews");
        }
        Ok(Arc::new(rendered))
    }
}

async fn open_cached(
    bundle_path: &Path,
    fingerprint: &str,
    size: PreviewSize,
    npe: Option<&OpenNpe>,
) -> io::Result<Option<Preview>> {
    if let Some(open) = npe {
        return Ok(read_encrypted(bundle_path, fingerprint, size, &open.key)
            .await?
            // It can't be read right from the file, since it's encrypted, so we
            // read it from the decrypted Vec<u8>.
            .map(Preview::Fresh));
    }
    Ok(bundle::open(bundle_path, fingerprint, size)
        .await?
        .map(Preview::Cached))
}

/// Each payload is ciphertext. The header and the size table stay plaintext.
fn encode_sealed(
    fingerprint: &str,
    rendered: &Rendered,
    key: &::crypto::Key,
) -> io::Result<Vec<u8>> {
    let avifs = rendered
        .avifs()
        .map(|(size, avif)| Ok((size, encrypt_payload(key, avif)?)))
        .collect::<io::Result<Vec<_>>>()?;
    Ok(bundle::encode(fingerprint, &Rendered::new(avifs)))
}

async fn read_encrypted(
    path: &Path,
    fingerprint: &str,
    size: PreviewSize,
    key: &::crypto::Key,
) -> io::Result<Option<Vec<u8>>> {
    let Some(mut bundled) = bundle::open(path, fingerprint, size).await? else {
        return Ok(None);
    };

    let mut encrypted = Vec::new();
    // It doesn't read the whole .preview file. `bundled.reader` is a
    // `file.take(size's chunk length)`, it's like a slice of the file.
    bundled.reader.read_to_end(&mut encrypted).await?;

    if encrypted.len() as u64 != bundled.len {
        tracing::debug!(path = %path.display(), "discarding a short preview cache");
        return Ok(None);
    }

    let Some(plain) = decrypt_payload(key, &encrypted) else {
        tracing::debug!(path = %path.display(), "discarding an unreadable preview cache");
        return Ok(None);
    };

    Ok(Some(plain))
}

fn render_npe(
    npe_file_path: &Path,
    key: &::crypto::Key,
    decrypted_name: &str,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewCacheError> {
    let file = open_source(npe_file_path)?;
    let reader = ::crypto::ContentFile::open(key, file).map_err(sealed_err)?;
    render_reader(Path::new(decrypted_name), reader, quality, speed).map_err(sealed_io_err)
}

/// A regular file, without following a symlink that replaced it.
fn open_source(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(file)
}

fn encrypt_payload(key: &::crypto::Key, plain: &[u8]) -> io::Result<Vec<u8>> {
    let mut input = Cursor::new(plain);
    let mut encrypted = Vec::new();
    ::crypto::encrypt_content(key, &mut input, &mut encrypted).map_err(content_io_err)?;
    Ok(encrypted)
}

fn decrypt_payload(key: &::crypto::Key, encrypted: &[u8]) -> Option<Vec<u8>> {
    let mut input = Cursor::new(encrypted);
    let mut plain = Vec::new();
    ::crypto::decrypt_content(key, &mut input, &mut plain).ok()?;
    Some(plain)
}

fn sealed_err(error: ContentError) -> PreviewCacheError {
    match error {
        ContentError::Io(error) => error.into(),
        other => {
            tracing::debug!(error = %other, "can't decrypt a preview source");
            PreviewCacheError::Decrypt
        }
    }
}

fn sealed_io_err(error: PreviewError) -> PreviewCacheError {
    match error {
        PreviewError::Io(error) if error.kind() == io::ErrorKind::InvalidData => {
            tracing::debug!(error = %error, "can't decrypt a preview source");
            PreviewCacheError::Decrypt
        }
        other => other.into(),
    }
}

fn content_io_err(error: ContentError) -> io::Error {
    match error {
        ContentError::Io(error) => error,
        other => io::Error::other(other),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        time::{SystemTime, UNIX_EPOCH},
    };

    use media::preview::Rendered;

    use super::*;

    const FINGERPRINT: &str = "0123456789abcdef";
    /// A 1×1 red PNG.
    const PNG: &str = "89504e470d0a1a0a0000000d4948445200000001000000010802000000907753de0000000c49444154789c63f8cfc0000003010100c9fe92ef0000000049454e44ae426082";

    fn contents_key(contents: u8) -> ::crypto::Key {
        ::crypto::Key {
            names: [1; 32],
            contents: [contents; 32],
        }
    }

    #[test]
    fn an_encrypted_bundle_roundtrips_one_size() {
        let key = contents_key(2);
        let rendered = Rendered::new(vec![
            (PreviewSize::S512, b"large".to_vec()),
            (PreviewSize::S256, b"mid".to_vec()),
            (PreviewSize::S128, b"s".to_vec()),
        ]);
        let bytes = encode_sealed(FINGERPRINT, &rendered, &key).unwrap();
        assert_eq!(&bytes[..4], b"NSPM");

        let mid = bundle::read_avif(&bytes, FINGERPRINT, PreviewSize::S256).unwrap();
        assert_ne!(mid, b"mid");
        assert_eq!(decrypt_payload(&key, &mid).unwrap(), b"mid");
        let large = bundle::read_avif(&bytes, FINGERPRINT, PreviewSize::S512).unwrap();
        assert_eq!(decrypt_payload(&key, &large).unwrap(), b"large");
        assert!(decrypt_payload(&contents_key(9), &mid).is_none());
    }

    #[tokio::test]
    async fn an_npe_preview_is_cached_encrypted() {
        let tmp = Temp::new();
        let storage = tmp.0.join("storage");
        let cache_root = tmp.0.join("cache");
        std::fs::create_dir_all(&storage).unwrap();

        let key = contents_key(4);
        let disk = ::crypto::encrypt_filename(&key, "dot.png").unwrap();
        let source = storage.join(&disk);
        let mut plain = Cursor::new(hex(PNG));
        let mut encrypted = Vec::new();
        ::crypto::encrypt_content(&key, &mut plain, &mut encrypted).unwrap();
        std::fs::write(&source, encrypted).unwrap();

        let cache = PreviewCache::new(
            storage,
            &cache_root,
            40.0,
            10,
            NonZeroUsize::new(1).unwrap(),
        );
        let cancel = Cancel::default();
        let open = OpenNpe {
            key: key.clone(),
            decrypted_name: "dot.png".to_owned(),
        };
        let first = cache
            .get(
                &source,
                FINGERPRINT,
                PreviewSize::S128,
                &cancel,
                Some(&open),
            )
            .await
            .unwrap();
        let Preview::Fresh(avif) = first else {
            panic!("a rendered preview is bytes");
        };
        assert!(!avif.is_empty());

        let bundle = cache_root.join("preview").join(format!("{disk}.preview"));
        let stored = std::fs::read(&bundle).unwrap();
        assert_eq!(&stored[..4], b"NSPM");
        let sealed = bundle::read_avif(&stored, FINGERPRINT, PreviewSize::S128).unwrap();
        assert_ne!(sealed, avif);
        assert_eq!(decrypt_payload(&key, &sealed).unwrap(), avif);

        let second = cache
            .get(
                &source,
                FINGERPRINT,
                PreviewSize::S128,
                &cancel,
                Some(&open),
            )
            .await
            .unwrap();
        let Preview::Fresh(again) = second else {
            panic!("a sealed cache hit is bytes");
        };
        assert_eq!(again, avif);
        assert_eq!(std::fs::read(&bundle).unwrap(), stored);

        let wrong_key = contents_key(9);
        let wrong_open = OpenNpe {
            key: wrong_key,
            decrypted_name: "dot.png".to_owned(),
        };
        let wrong = cache
            .get(
                &source,
                FINGERPRINT,
                PreviewSize::S128,
                &cancel,
                Some(&wrong_open),
            )
            .await;
        let Err(wrong) = wrong else {
            panic!("a wrong key does not open the preview");
        };
        assert!(matches!(wrong, PreviewCacheError::Decrypt));
        assert_eq!(std::fs::read(&bundle).unwrap(), stored);
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("npe-preview-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
