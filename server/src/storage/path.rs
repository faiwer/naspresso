use std::{
    io,
    path::{Path, PathBuf},
};

use tokio::fs;

use super::StorageError;

/// Join a client-supplied path onto `root`, making sure it stays inside: bad
/// segments are rejected, and so is any symlink along the way.
pub async fn resolve(root: &Path, path: &str) -> Result<PathBuf, StorageError> {
    let resolved = join(root, path)?;
    reject_symlinks(root, &resolved).await?;
    Ok(resolved)
}

/// Leading and trailing slashes are ignored; empty, `.` and `..` segments are
/// rejected rather than normalized.
fn join(root: &Path, path: &str) -> Result<PathBuf, StorageError> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(root.to_path_buf());
    }

    let mut resolved = root.to_path_buf();
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(['\\', '\0'])
        {
            return Err(StorageError::InvalidPath(path.to_owned()));
        }
        resolved.push(segment);
    }

    Ok(resolved)
}

/// Symlinks are hidden from listings, so one on the path reads as not found.
async fn reject_symlinks(root: &Path, resolved: &Path) -> Result<(), StorageError> {
    let Ok(relative) = resolved.strip_prefix(root) else {
        return Err(StorageError::NotFound);
    };

    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let meta = fs::symlink_metadata(&current)
            .await
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => StorageError::NotFound,
                _ => StorageError::Io(e),
            })?;
        if meta.file_type().is_symlink() {
            return Err(StorageError::NotFound);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_valid_paths() {
        let root = Path::new("/srv");
        assert_eq!(join(root, "").unwrap(), root);
        assert_eq!(join(root, "/").unwrap(), root);
        assert_eq!(join(root, "/a/b").unwrap(), Path::new("/srv/a/b"));
        assert_eq!(join(root, "a/b/").unwrap(), Path::new("/srv/a/b"));
        assert_eq!(
            join(root, "/a/.hidden").unwrap(),
            Path::new("/srv/a/.hidden")
        );
    }

    #[test]
    fn rejects_traversal() {
        let root = Path::new("/srv");
        for path in [
            "/..",
            "/a/../b",
            "/./a",
            "/a/.",
            "/a//b",
            "/a\\..\\b",
            "/a\0",
        ] {
            assert!(join(root, path).is_err(), "{path:?} should be rejected");
        }
    }
}
