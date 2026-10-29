use thiserror::Error;
use tokio_util::sync::{CancellationToken, DropGuard};

/// Fires when whoever asked for the work, like an HTTP client, has gone away.
/// Nothing stops on its own: work checks it only where giving up makes sense.
#[derive(Clone, Default)]
pub struct Cancel(CancellationToken);

#[derive(Debug, Error)]
#[error("cancelled")]
pub struct Cancelled;

impl Cancel {
    /// Cancels when the returned guard is dropped.
    pub fn cancel_on_drop(&self) -> DropGuard {
        self.0.clone().drop_guard()
    }

    /// `future`'s output, unless cancelled first.
    pub async fn until<T>(&self, future: impl Future<Output = T>) -> Result<T, Cancelled> {
        tokio::select! {
            value = future => Ok(value),
            () = self.0.cancelled() => Err(Cancelled),
        }
    }
}
