use std::{
    fs::{self, Metadata},
    io,
    path::Path,
};

use chrono::{DateTime, Utc};

use super::{Dir, Entry, File, StorageError};

/// The entries of `dir`, directories first, then by name. Symlinks, special
/// files and names that aren't UTF-8 are skipped.
pub async fn list_dir(dir: &Path) -> Result<Vec<Entry>, StorageError> {
    let dir = dir.to_path_buf();
    // One blocking-pool handoff for the whole listing instead of one per syscall.
    tokio::task::spawn_blocking(move || list_dir_blocking(&dir))
        .await
        .map_err(|e| StorageError::Io(io::Error::other(e)))?
}

fn list_dir_blocking(dir: &Path) -> Result<Vec<Entry>, StorageError> {
    let read_dir = fs::read_dir(dir).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => StorageError::NotFound,
        io::ErrorKind::NotADirectory => StorageError::NotADirectory,
        _ => StorageError::Io(e),
    })?;

    let mut entries = Vec::new();
    for dir_entry in read_dir {
        let dir_entry = dir_entry?;
        let name = match dir_entry.file_name().into_string() {
            Ok(name) => name,
            Err(name) => {
                tracing::warn!(?name, dir = %dir.display(), "skipping a non-UTF-8 name");
                continue;
            }
        };
        // Doesn't follow symlinks: a link reports as a symlink, so it's skipped below.
        let meta = match dir_entry.metadata() {
            Ok(meta) => meta,
            // Deleted between `read_dir` and here.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        if let Some(entry) = to_entry(name, &meta)? {
            entries.push(entry);
        }
    }
    entries.sort_by(|a, b| a.kind().cmp(&b.kind()).then_with(|| a.name().cmp(b.name())));

    Ok(entries)
}

fn to_entry(name: String, meta: &Metadata) -> io::Result<Option<Entry>> {
    let modified_at = DateTime::<Utc>::from(meta.modified()?);
    let created_at = meta.created().ok().map(DateTime::<Utc>::from);
    let file_type = meta.file_type();

    Ok(if file_type.is_dir() {
        Some(Entry::Dir(Dir {
            name,
            modified_at,
            created_at,
        }))
    } else if file_type.is_file() {
        Some(Entry::File(File {
            name,
            modified_at,
            created_at,
            size: meta.len(),
        }))
    } else {
        None
    })
}
