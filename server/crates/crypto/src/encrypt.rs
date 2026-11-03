use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, copy};

use crate::{EncryptError, Key};

/// Ciphertext written for one input file.
pub struct EncryptedFile {
    /// Bytes written to [`Self::file_name`].
    pub size: u64,
    /// The input file's name with `.npe` appended. The directory is the caller's.
    pub file_name: PathBuf,
}

/// Copy `reader` into `<file name>.npe` through `open_writer`.
///
/// `open_writer` receives that file name (`photo.jpg` becomes `photo.jpg.npe`)
/// and returns the writer to fill. `key` is the derived key for the file.
pub async fn encrypt<R, W, Fut>(
    file_path: impl AsRef<Path>,
    key: &Key,
    mut reader: R,
    open_writer: impl FnOnce(PathBuf) -> Fut,
) -> Result<EncryptedFile, EncryptError>
where
    R: AsyncRead + Unpin,
    Fut: Future<Output = io::Result<W>>,
    W: AsyncWrite + Unpin,
{
    // Applied once the ciphertext is real; the caller already has to pass it.
    let _ = key;

    let file_name = npe_name(file_path.as_ref());
    let mut writer = open_writer(file_name.clone()).await?;
    let size = copy(&mut reader, &mut writer).await?;
    writer.shutdown().await?;

    Ok(EncryptedFile { size, file_name })
}

/// The input file's name with `.npe` appended, without its directory.
fn npe_name(file_path: &Path) -> PathBuf {
    let mut name = file_path
        .file_name()
        .unwrap_or(file_path.as_os_str())
        .to_os_string();
    name.push(".npe");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        sync::{Arc, Mutex},
        task::{Context, Poll},
    };

    use tokio::io::AsyncWrite;

    use super::*;

    struct Mem(Arc<Mutex<Vec<u8>>>);

    impl AsyncWrite for Mem {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.0.lock().expect("writer lock").extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn copies_reader_to_npe_name() {
        let source = b"album bytes";
        let captured = Arc::new(Mutex::new(Vec::new()));
        let key = Key {
            names: [0; 32],
            contents: [0; 32],
        };
        let encrypted = encrypt(
            Path::new("album/photo.jpg"),
            &key,
            io::Cursor::new(&source[..]),
            {
                let captured = Arc::clone(&captured);
                move |name| {
                    let captured = Arc::clone(&captured);
                    async move {
                        assert_eq!(name, PathBuf::from("photo.jpg.npe"));
                        Ok(Mem(captured))
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(encrypted.size, source.len() as u64);
        assert_eq!(encrypted.file_name, PathBuf::from("photo.jpg.npe"));
        assert_eq!(*captured.lock().expect("writer lock"), source);
    }
}
