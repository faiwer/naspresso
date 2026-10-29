use std::{
    collections::HashMap,
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use media::preview::{PreviewError, PreviewSize, Rendered, render};
use thiserror::Error;
use tokio::sync::{OnceCell, Semaphore};

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
}

impl From<io::Error> for PreviewCacheError {
    fn from(err: io::Error) -> Self {
        Self::Preview(err.into())
    }
}

/// Bundle path and source fingerprint.
type RenderKey = (PathBuf, String);

/// Previews of files under `STORAGE`, made on demand and kept under `CACHE`
/// in a tree mirroring it: `a/b/c.jpg` is cached as `preview/a/b/c.jpg.preview`.
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
    /// fingerprint is `fingerprint`. Gives up on `cancel` only while waiting
    /// for a render slot: a started render is finished and cached.
    pub async fn get(
        &self,
        source: &Path,
        fingerprint: &str,
        size: PreviewSize,
        cancel: &Cancel,
    ) -> Result<Preview, PreviewCacheError> {
        let bundle_path = self.bundle_path(source)?;
        if let Some(cached) = bundle::open(&bundle_path, fingerprint, size).await? {
            return Ok(Preview::Cached(cached));
        }

        let rendered = self
            .render_once(source, bundle_path, fingerprint, cancel)
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
            .get_or_try_init(|| self.render_and_store(source, &key.0, fingerprint, cancel))
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
    ) -> Result<Arc<Rendered>, PreviewCacheError> {
        let slot = cancel
            .until(Arc::clone(&self.slots).acquire_owned())
            .await?
            .expect("render slots are never closed");

        let (source, quality, speed) = (source.to_path_buf(), self.quality, self.speed);
        let rendered = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            render(&source, quality, speed)
        })
        .await
        .map_err(io::Error::other)??;

        let bytes = bundle::encode(fingerprint, &rendered);
        if let Err(e) = bundle::write(bundle_path, &bytes).await {
            tracing::warn!(error = %e, path = %bundle_path.display(), "can't cache previews");
        }
        Ok(Arc::new(rendered))
    }
}
