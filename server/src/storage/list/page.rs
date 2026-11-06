use std::{io, path::Path, time::SystemTime};

use serde::Serialize;

use super::super::{Entry, EntryKind, StorageError, xattr::Pending};
use super::{entry, sort};

/// One page of a directory listing.
pub struct ListPage {
    pub entries: Vec<Entry>,
    /// Whether the listing goes on after the last of `entries`.
    pub has_more: bool,
    /// Listable entries in the whole directory, across all pages.
    pub total: usize,
    /// Metadata for files this page had to open. The caller stores it after
    /// returning the page; reading the page does not.
    pub pending: Vec<Pending>,
    /// Whether `dir` sits inside an `.nped` directory, and whether that directory is open.
    pub encryption: Encryption,
}

/// `none` outside an `.nped` directory. `locked` and `unlocked` say whether that directory is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encryption {
    None,
    Locked,
    Unlocked,
}

/// An entry's place in the listing order: directories first, then by name
/// ignoring case, then by exact name. A page starts right after one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryKey {
    pub kind: EntryKind,
    pub name: String,
}

impl EntryKey {
    pub fn of(entry: &Entry) -> Self {
        Self {
            kind: entry.kind(),
            name: entry.name().to_owned(),
        }
    }
}

/// Up to `limit` entries of `dir` in listing order, starting after `after`.
/// Symlinks, special files, names that aren't UTF-8, and OS metadata files
/// (`.DS_Store`, `.apdisk`, `Thumbs.db`) are skipped. `after` doesn't have to
/// exist anymore. `has_preview` is asked about image files, with their path
/// and modification time, and may block. `pending` is the attributes to store
/// for files this page opened.
pub async fn list_dir(
    dir: &Path,
    after: Option<EntryKey>,
    limit: usize,
    encryption: Encryption,
    key: Option<::crypto::Key>,
    has_preview: impl Fn(&Path, SystemTime) -> bool + Send + 'static,
) -> Result<ListPage, StorageError> {
    let dir = dir.to_path_buf();
    // One blocking-pool handoff for the whole listing instead of one per syscall.
    tokio::task::spawn_blocking(move || {
        list_dir_blocking(&dir, after, limit, encryption, key.as_ref(), &has_preview)
    })
    .await
    .map_err(|e| StorageError::Io(io::Error::other(e)))?
}

pub(super) type HasPreview = dyn Fn(&Path, SystemTime) -> bool;

pub(super) fn list_dir_blocking(
    dir: &Path,
    after: Option<EntryKey>,
    limit: usize,
    encryption: Encryption,
    key: Option<&::crypto::Key>,
    has_preview: &HasPreview,
) -> Result<ListPage, StorageError> {
    let mut keys = sort::read_keys(dir, key)?;
    keys.sort_unstable();
    let total = keys.len();
    let start = after.map_or(0, |after| {
        let after = sort::SortKey::new(after.kind, after.name.clone(), after.name, false);
        keys.partition_point(|key| *key <= after)
    });

    // Only the page gets a `stat` per entry; the rest of the directory is names only.
    let mut entries = Vec::with_capacity(limit.min(total - start));
    let mut pending = Vec::new();
    let mut remaining = keys.into_iter().skip(start);
    while entries.len() < limit
        && let Some(sorted) = remaining.next()
    {
        if let Some((entry, record)) = entry::read_entry(dir, &sorted, has_preview, key)? {
            if let Some(record) = record {
                pending.push(record);
            }
            entries.push(entry);
        }
    }

    Ok(ListPage {
        entries,
        has_more: remaining.next().is_some(),
        total,
        pending,
        encryption,
    })
}
