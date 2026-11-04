use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::io::SyncIoBridge;

use crate::{EncryptError, FilenameError, Key, encrypt_content, encrypt_filename};

/// Ciphertext written for one input file.
pub struct EncryptedFile {
    /// Encrypted file name, with a `.npe` suffix. The directory is the caller's.
    pub file_name: PathBuf,
}

/// Encrypt `reader` into an `<name>.npe` through `open_writer`.
///
/// `open_writer` receives that file name. `key.names` encrypts the name and
/// `key.contents` encrypts the bytes. A failure can leave a partial ciphertext
/// in the writer; publish that output only after this returns [`Ok`].
pub async fn encrypt<R, W, Fut>(
    file_path: impl AsRef<Path>,
    key: &Key,
    reader: R,
    open_writer: impl FnOnce(PathBuf) -> Fut,
) -> Result<EncryptedFile, EncryptError>
where
    R: AsyncRead + Unpin + Send + 'static,
    Fut: Future<Output = io::Result<W>>,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let file_name = encrypted_name(file_path.as_ref(), key)?;
    let writer = open_writer(file_name.clone()).await?;
    let key = key.clone();
    // The bridge blocks on the runtime, so the encryption runs off the runtime thread.
    let mut reader = SyncIoBridge::new(reader);
    let mut writer = SyncIoBridge::new(writer);
    tokio::task::spawn_blocking(move || -> Result<(), EncryptError> {
        encrypt_content(&key, &mut reader, &mut writer)?;
        writer.shutdown()?;
        Ok(())
    })
    .await
    .map_err(io::Error::other)??;

    Ok(EncryptedFile { file_name })
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn encrypts_reader_into_an_npe_name() {
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

        let bytes = captured.lock().expect("writer lock").clone();
        assert_eq!(encrypted.file_name, expected);
        assert_ne!(bytes, source);
        let mut decoded = Vec::new();
        crate::decrypt_content(
            &key,
            &mut io::Cursor::new(&bytes),
            bytes.len() as u64,
            &mut decoded,
        )
        .unwrap();
        assert_eq!(decoded, source);
    }
}
