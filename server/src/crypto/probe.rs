use std::{
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use crypto::{AsyncContentFile, ContentError};
use tokio::{fs, io::AsyncReadExt};

use super::CryptoError;
use crate::storage::StorageError;

/// Whether `key` opens the encrypted file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Opened,
    Rejected,
}

/// `path` has to be a directory whose name ends in `.nped`.
pub async fn ensure_encrypted_dir(path: &Path) -> Result<(), CryptoError> {
    let encrypted = path
        .file_name()
        .is_some_and(|name| name.as_bytes().ends_with(b".nped"));
    if !encrypted {
        return Err(CryptoError::NotEncryptedDirectory);
    }
    let meta = fs::metadata(path).await.map_err(storage_io)?;
    if !meta.is_dir() {
        return Err(CryptoError::Storage(StorageError::NotADirectory));
    }
    Ok(())
}

/// The first regular `*.npe` file under `dir`, at any depth. Symlinks are skipped.
pub async fn find_encrypted_file(dir: &Path) -> Result<Option<PathBuf>, CryptoError> {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let mut entries = fs::read_dir(&dir).await.map_err(storage_io)?;
        while let Some(entry) = entries.next_entry().await.map_err(storage_io)? {
            let file_type = entry.file_type().await.map_err(storage_io)?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if file_type.is_file() && entry.file_name().as_bytes().ends_with(b".npe") {
                return Ok(Some(entry.path()));
            }
        }
    }
    Ok(None)
}

/// Decrypt one chunk of `path`, or the end-of-file authenticator when it is empty.
pub async fn probe(path: &Path, key: &::crypto::Key) -> Result<Probe, CryptoError> {
    let file = fs::File::open(path).await.map_err(storage_io)?;
    let mut content = AsyncContentFile::open(key, file)
        .await
        .map_err(content_error)?;
    if content.plaintext_len() == 0 {
        // An empty file has no chunk, so the end-of-file authenticator is the check.
        return verify_empty(path.to_path_buf(), key.clone()).await;
    }
    let mut byte = [0; 1];
    match content.read_exact(&mut byte).await {
        Ok(_) => Ok(Probe::Opened),
        Err(error) => read_outcome(error),
    }
}

async fn verify_empty(path: PathBuf, key: ::crypto::Key) -> Result<Probe, CryptoError> {
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path).map_err(storage_io)?;
        match ::crypto::verify_tag_sequence(&key, &mut file) {
            Ok(()) => Ok(Probe::Opened),
            Err(error) => content_probe(error),
        }
    })
    .await
    .map_err(|error| CryptoError::Storage(StorageError::Io(io::Error::other(error))))?
}

fn read_outcome(error: io::Error) -> Result<Probe, CryptoError> {
    match content_of(&error) {
        Some(content) => content_probe(content),
        None => Err(storage_io(error)),
    }
}

fn content_of(error: &io::Error) -> Option<ContentError> {
    match error.get_ref()?.downcast_ref::<ContentError>()? {
        ContentError::Authentication => Some(ContentError::Authentication),
        ContentError::Magic => Some(ContentError::Magic),
        ContentError::Version => Some(ContentError::Version),
        ContentError::Layout => Some(ContentError::Layout),
        ContentError::Random => Some(ContentError::Random),
        ContentError::Io(error) => Some(ContentError::Io(io::Error::new(
            error.kind(),
            error.to_string(),
        ))),
    }
}

fn content_probe(error: ContentError) -> Result<Probe, CryptoError> {
    match error {
        ContentError::Authentication => Ok(Probe::Rejected),
        other => Err(content_error(other)),
    }
}

fn content_error(error: ContentError) -> CryptoError {
    match error {
        ContentError::Io(error) => CryptoError::Storage(StorageError::Io(error)),
        other => CryptoError::Content(other),
    }
}

fn storage_io(error: io::Error) -> CryptoError {
    CryptoError::Storage(match error.kind() {
        io::ErrorKind::NotFound => StorageError::NotFound,
        io::ErrorKind::NotADirectory => StorageError::NotADirectory,
        _ => StorageError::Io(error),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn key(byte: u8) -> ::crypto::Key {
        ::crypto::Key {
            names: [byte; 32],
            contents: [byte; 32],
        }
    }

    fn encrypt(key: &::crypto::Key, plain: &[u8]) -> Vec<u8> {
        let mut input = Cursor::new(plain.to_vec());
        let mut output = Cursor::new(Vec::new());
        ::crypto::encrypt_content(key, &mut input, &mut output).unwrap();
        output.into_inner()
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("naspresso-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn finds_a_nested_file_and_checks_the_key() {
        let tmp = TempDir::new("probe");
        let dir = tmp.0.join("vault.nped");
        let nested = dir.join("album");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("note.npe"), encrypt(&key(1), b"hi")).unwrap();
        std::fs::write(dir.join("skip.txt"), b"no").unwrap();
        std::os::unix::fs::symlink(dir.join("skip.txt"), dir.join("link.npe")).unwrap();

        let found = find_encrypted_file(&dir).await.unwrap().unwrap();
        assert_eq!(found.file_name().unwrap(), "note.npe");
        assert_eq!(probe(&found, &key(1)).await.unwrap(), Probe::Opened);
        assert_eq!(probe(&found, &key(2)).await.unwrap(), Probe::Rejected);
    }

    #[tokio::test]
    async fn an_empty_file_uses_the_end_authenticator() {
        let tmp = TempDir::new("probe-empty");
        let dir = tmp.0.join("empty.nped");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.npe");
        std::fs::write(&path, encrypt(&key(1), b"")).unwrap();

        assert_eq!(probe(&path, &key(1)).await.unwrap(), Probe::Opened);
        assert_eq!(probe(&path, &key(2)).await.unwrap(), Probe::Rejected);
    }

    #[tokio::test]
    async fn reports_a_directory_with_no_encrypted_file() {
        let tmp = TempDir::new("probe-none");
        let dir = tmp.0.join("vault.nped");
        std::fs::create_dir_all(dir.join("album")).unwrap();
        assert!(find_encrypted_file(&dir).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_a_directory_whose_name_is_not_encrypted() {
        let tmp = TempDir::new("probe-dir");
        let dir = tmp.0.join("photos");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            ensure_encrypted_dir(&dir).await,
            Err(CryptoError::NotEncryptedDirectory)
        ));
        let file = tmp.0.join("file.nped");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(
            ensure_encrypted_dir(&file).await,
            Err(CryptoError::Storage(StorageError::NotADirectory))
        ));
    }
}
