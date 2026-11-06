//! Listing order: directories first, then the name.

use std::{cmp::Ordering, fs, io, path::Path};

use super::super::{EntryKind, StorageError};

/// The order of [`super::EntryKey`]. `disk_name` is how the file is stored and is not part of the order.
#[derive(Debug)]
pub(super) struct SortKey {
    kind: EntryKind,
    folded: String,
    /// Name the listing shows. Decoded for an `.npe` file that opened.
    pub(super) name: String,
    pub(super) disk_name: String,
    /// `name` was decoded from `disk_name`.
    pub(super) decoded: bool,
}

impl SortKey {
    pub(super) fn new(kind: EntryKind, name: String, disk_name: String, decoded: bool) -> Self {
        Self {
            kind,
            folded: name.to_lowercase(),
            name,
            disk_name,
            decoded,
        }
    }
}

impl PartialEq for SortKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SortKey {}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.folded.cmp(&other.folded))
            .then_with(|| self.name.cmp(&other.name))
    }
}

fn is_os_metadata(name: &str) -> bool {
    matches!(name, ".DS_Store" | ".apdisk" | "Thumbs.db")
}

pub(super) fn read_keys(
    dir: &Path,
    key: Option<&::crypto::Key>,
) -> Result<Vec<SortKey>, StorageError> {
    let read_dir = fs::read_dir(dir).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => StorageError::NotFound,
        io::ErrorKind::NotADirectory => StorageError::NotADirectory,
        _ => StorageError::Io(e),
    })?;

    let mut keys = Vec::new();
    for dir_entry in read_dir {
        let dir_entry = dir_entry?;
        let name = match dir_entry.file_name().into_string() {
            Ok(name) => name,
            Err(name) => {
                tracing::warn!(?name, dir = %dir.display(), "skipping a non-UTF-8 name");
                continue;
            }
        };
        // Finder and Explorer write these beside real files; they hold no user content.
        if is_os_metadata(&name) {
            continue;
        }
        // Comes with the directory read on most filesystems, so no `stat`. Doesn't
        // follow symlinks: a link reports as a symlink, so it's skipped below.
        let file_type = match dir_entry.file_type() {
            Ok(file_type) => file_type,
            // Deleted between `read_dir` and here.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let kind = if file_type.is_dir() {
            EntryKind::Dir
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            continue;
        };
        keys.push(sort_key(kind, name, key));
    }

    Ok(keys)
}

/// `.npe` files open into their plaintext name when `key` can decode them.
/// A name that does not decode stays the on-disk name.
fn sort_key(kind: EntryKind, disk_name: String, key: Option<&::crypto::Key>) -> SortKey {
    let decoded = if kind == EntryKind::File {
        key.and_then(|key| ::crypto::decrypt_filename(key, &disk_name).ok())
    } else {
        None
    };
    match decoded {
        Some(name) => SortKey::new(kind, name, disk_name, true),
        None => SortKey::new(kind, disk_name.clone(), disk_name, false),
    }
}
