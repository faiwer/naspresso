//! One directory entry from its sorted key.

use std::{
    fs::{self, Metadata},
    io,
    path::Path,
};

use chrono::{DateTime, Utc};

use super::super::{Dir, Entry, File, xattr::Pending};
use super::{describe, page::HasPreview, sort::SortKey};

/// `None` when `name` is gone or is no longer a plain file or directory.
pub(super) fn read_entry(
    dir: &Path,
    sorted: &SortKey,
    has_preview: &HasPreview,
    key: Option<&::crypto::Key>,
) -> io::Result<Option<(Entry, Option<Pending>)>> {
    let path = dir.join(&sorted.disk_name);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    to_entry(sorted, &meta, &path, has_preview, key)
}

fn to_entry(
    sorted: &SortKey,
    meta: &Metadata,
    path: &Path,
    has_preview: &HasPreview,
    key: Option<&::crypto::Key>,
) -> io::Result<Option<(Entry, Option<Pending>)>> {
    let modified = meta.modified()?;
    let modified_at = DateTime::<Utc>::from(modified);
    let created_at = meta.created().ok().map(DateTime::<Utc>::from);
    let file_type = meta.file_type();

    Ok(if file_type.is_dir() {
        Some((
            Entry::Dir(Dir {
                name: sorted.disk_name.clone(),
                modified_at,
                created_at,
            }),
            None,
        ))
    } else if file_type.is_file() {
        let described = describe::describe(path, sorted, meta.len(), modified, key);
        Some((
            Entry::File(File {
                name: sorted.name.clone(),
                original_name: sorted.decoded.then(|| sorted.disk_name.clone()),
                modified_at,
                created_at,
                size: meta.len(),
                mime: described.mime,
                has_preview: described.image.is_some() && has_preview(path, modified),
                image: described.image,
            }),
            described.pending,
        ))
    } else {
        None
    })
}
