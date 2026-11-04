use std::{
    future::Future,
    io,
    path::{Component, Path, PathBuf},
};

use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use tokio_util::io::SyncIoBridge;

use crate::{
    EncryptError, FilenameError, Key, decrypt_content, decrypt_filename, encrypt_content,
    encrypt_filename,
};

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
    R: AsyncRead + AsyncSeek + Unpin + Send + 'static,
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

/// Plaintext written for one encrypted file.
pub struct DecryptedFile {
    /// Original file name. The directory is the caller's.
    pub file_name: PathBuf,
}

/// Decrypt `reader` into the original file name through `open_writer`.
///
/// `open_writer` receives that file name. `key.names` decrypts the name and
/// `key.contents` decrypts the bytes. A failure can leave a partial plaintext
/// in the writer; publish that output only after this returns [`Ok`].
pub async fn decrypt<R, W, Fut>(
    file_path: impl AsRef<Path>,
    key: &Key,
    reader: R,
    open_writer: impl FnOnce(PathBuf) -> Fut,
) -> Result<DecryptedFile, EncryptError>
where
    R: AsyncRead + Unpin + Send + 'static,
    Fut: Future<Output = io::Result<W>>,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let file_name = decrypted_name(file_path.as_ref(), key)?;
    let writer = open_writer(file_name.clone()).await?;

    let key = key.clone();
    let mut reader = SyncIoBridge::new(reader);
    let mut writer = SyncIoBridge::new(writer);

    tokio::task::spawn_blocking(move || -> Result<(), EncryptError> {
        decrypt_content(&key, &mut reader, &mut writer)?;
        writer.shutdown()?;
        Ok(())
    })
    .await
    .map_err(io::Error::other)??;

    Ok(DecryptedFile { file_name })
}

fn encrypted_name(file_path: &Path, key: &Key) -> Result<PathBuf, EncryptError> {
    let Some(name) = file_path.file_name().and_then(|name| name.to_str()) else {
        return Err(FilenameError::Invalid.into());
    };
    Ok(PathBuf::from(encrypt_filename(key, name)?))
}

fn decrypted_name(file_path: &Path, key: &Key) -> Result<PathBuf, EncryptError> {
    let Some(name) = file_path.file_name().and_then(|name| name.to_str()) else {
        return Err(FilenameError::Invalid.into());
    };
    let plain = decrypt_filename(key, name)?;

    // Check if the filename is valid.
    let mut components = Path::new(&plain).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(FilenameError::Invalid.into());
    }

    Ok(PathBuf::from(plain))
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
        crate::decrypt_content(&key, &mut io::Cursor::new(&bytes), &mut decoded).unwrap();
        assert_eq!(decoded, source);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn decrypts_an_npe_back_to_the_original_name() {
        let source = b"album bytes";
        let encrypted_bytes = Arc::new(Mutex::new(Vec::new()));
        let key = Key {
            names: [0; 32],
            contents: [0; 32],
        };
        let encrypted_name = PathBuf::from(encrypt_filename(&key, "photo.jpg").unwrap());
        encrypt(
            Path::new("album/photo.jpg"),
            &key,
            io::Cursor::new(&source[..]),
            {
                let encrypted_bytes = Arc::clone(&encrypted_bytes);
                move |_| {
                    let encrypted_bytes = Arc::clone(&encrypted_bytes);
                    async move { Ok(Mem(encrypted_bytes)) }
                }
            },
        )
        .await
        .unwrap();

        let ciphertext = encrypted_bytes.lock().expect("writer lock").clone();
        let plain_bytes = Arc::new(Mutex::new(Vec::new()));
        let decrypted = decrypt(
            Path::new("album").join(&encrypted_name),
            &key,
            io::Cursor::new(ciphertext),
            {
                let plain_bytes = Arc::clone(&plain_bytes);
                move |name| {
                    let plain_bytes = Arc::clone(&plain_bytes);
                    async move {
                        assert_eq!(name, PathBuf::from("photo.jpg"));
                        Ok(Mem(plain_bytes))
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(decrypted.file_name, PathBuf::from("photo.jpg"));
        assert_eq!(*plain_bytes.lock().expect("writer lock"), source);
    }
}
