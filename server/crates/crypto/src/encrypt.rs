use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, copy};

use crate::{EncryptError, FilenameError, Key, encrypt_filename};

/// Ciphertext written for one input file.
pub struct EncryptedFile {
    /// Bytes written to [`Self::file_name`].
    pub size: u64,
    /// Encrypted file name, with a `.npe` suffix. The directory is the caller's.
    pub file_name: PathBuf,
}

/// Copy `reader` into an encrypted `<name>.npe` through `open_writer`.
///
/// `open_writer` receives that file name. `key.names` encrypts it.
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
    let file_name = encrypted_name(file_path.as_ref(), key)?;
    let mut writer = open_writer(file_name.clone()).await?;
    let size = copy(&mut reader, &mut writer).await?;
    writer.shutdown().await?;

    Ok(EncryptedFile { size, file_name })
}

fn encrypted_name(file_path: &Path, key: &Key) -> Result<PathBuf, EncryptError> {
    let Some(name) = file_path.file_name().and_then(|name| name.to_str()) else {
        return Err(FilenameError::Invalid.into());
    };
    Ok(PathBuf::from(encrypt_filename(key, name)?))
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
        let expected = PathBuf::from(encrypt_filename(&key, "photo.jpg").unwrap());
        let encrypted = encrypt(
            Path::new("album/photo.jpg"),
            &key,
            io::Cursor::new(&source[..]),
            {
                let captured = Arc::clone(&captured);
                let expected = expected.clone();
                move |name| {
                    let captured = Arc::clone(&captured);
                    async move {
                        assert_eq!(name, expected);
                        Ok(Mem(captured))
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(encrypted.size, source.len() as u64);
        assert_eq!(encrypted.file_name, expected);
        assert_eq!(*captured.lock().expect("writer lock"), source);
    }
}
