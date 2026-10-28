use std::{
    io::{self, SeekFrom},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
};
use twox_hash::XxHash3_64;

use super::StorageError;

/// How much of the file is read to recognize its type by magic bytes.
const SNIFF_LEN: u64 = 8 * 1024;

const FALLBACK_MIME: &str = "application/octet-stream";

pub struct OpenedFile {
    pub file: fs::File,
    /// Bytes.
    pub size: u64,
    /// Changes whenever the file is replaced, modified or moved.
    pub fingerprint: String,
    path: PathBuf,
}

/// Open a regular file for reading. Directories and special files are rejected
/// before opening, so a FIFO can't block the call.
pub async fn open_file(path: &Path) -> Result<OpenedFile, StorageError> {
    let meta = fs::metadata(path).await.map_err(|e| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => StorageError::NotFound,
        _ => StorageError::Io(e),
    })?;
    if !meta.is_file() {
        return Err(StorageError::NotAFile);
    }

    let file = fs::File::open(path).await?;
    let meta = file.metadata().await?;
    Ok(OpenedFile {
        file,
        size: meta.len(),
        fingerprint: fingerprint(path, meta.modified()?, meta.created().ok()),
        path: path.to_path_buf(),
    })
}

impl OpenedFile {
    /// The MIME type by magic bytes, then by extension. Leaves the file at the start.
    pub async fn mime(&mut self) -> Result<&'static str, StorageError> {
        let mut head = Vec::with_capacity(SNIFF_LEN as usize);
        (&mut self.file)
            .take(SNIFF_LEN)
            .read_to_end(&mut head)
            .await?;
        self.file.seek(SeekFrom::Start(0)).await?;

        Ok(infer::get(&head)
            .map(|kind| kind.mime_type())
            .or_else(|| mime_guess::from_path(&self.path).first_raw())
            .unwrap_or(FALLBACK_MIME))
    }
}

fn fingerprint(path: &Path, modified: SystemTime, created: Option<SystemTime>) -> String {
    let mut seed = path.as_os_str().as_encoded_bytes().to_vec();
    seed.extend_from_slice(&nanos(modified).to_le_bytes());
    seed.extend_from_slice(&created.map_or(0, nanos).to_le_bytes());
    format!("{:016x}", XxHash3_64::oneshot(&seed))
}

fn nanos(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn fingerprint_tracks_path_and_times() {
        let path = Path::new("/srv/a.txt");
        let t1 = UNIX_EPOCH + Duration::from_secs(1);
        let t2 = UNIX_EPOCH + Duration::from_secs(2);

        let base = fingerprint(path, t1, Some(t1));
        assert_eq!(base, fingerprint(path, t1, Some(t1)));
        assert_ne!(base, fingerprint(Path::new("/srv/b.txt"), t1, Some(t1)));
        assert_ne!(base, fingerprint(path, t2, Some(t1)));
        assert_ne!(base, fingerprint(path, t1, Some(t2)));
        assert_ne!(base, fingerprint(path, t1, None));
    }
}
