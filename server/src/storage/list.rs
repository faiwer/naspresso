use std::{
    fs::{self, Metadata},
    io,
    path::Path,
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use media::probe_file;

use super::{Dir, Entry, EntryKind, File, StorageError};

/// One page of a directory listing.
pub struct ListPage {
    pub entries: Vec<Entry>,
    /// Whether the listing goes on after the last of `entries`.
    pub has_more: bool,
    /// Listable entries in the whole directory, across all pages.
    pub total: usize,
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
/// Symlinks, special files and names that aren't UTF-8 are skipped. `after`
/// doesn't have to exist anymore. `has_preview` is asked about image files,
/// with their path and modification time, and may block.
pub async fn list_dir(
    dir: &Path,
    after: Option<EntryKey>,
    limit: usize,
    has_preview: impl Fn(&Path, SystemTime) -> bool + Send + 'static,
) -> Result<ListPage, StorageError> {
    let dir = dir.to_path_buf();
    // One blocking-pool handoff for the whole listing instead of one per syscall.
    tokio::task::spawn_blocking(move || list_dir_blocking(&dir, after, limit, &has_preview))
        .await
        .map_err(|e| StorageError::Io(io::Error::other(e)))?
}

/// The order of [`EntryKey`]; the folded name is computed once, not per comparison.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    kind: EntryKind,
    folded: String,
    name: String,
}

impl SortKey {
    fn new(kind: EntryKind, name: String) -> Self {
        Self {
            kind,
            folded: name.to_lowercase(),
            name,
        }
    }
}

type HasPreview = dyn Fn(&Path, SystemTime) -> bool;

fn list_dir_blocking(
    dir: &Path,
    after: Option<EntryKey>,
    limit: usize,
    has_preview: &HasPreview,
) -> Result<ListPage, StorageError> {
    let mut keys = read_keys(dir)?;
    keys.sort_unstable();
    let total = keys.len();
    let start = after.map_or(0, |after| {
        let after = SortKey::new(after.kind, after.name);
        keys.partition_point(|key| *key <= after)
    });

    // Only the page gets a `stat` per entry; the rest of the directory is names only.
    let mut entries = Vec::with_capacity(limit.min(total - start));
    let mut remaining = keys.into_iter().skip(start);
    while entries.len() < limit
        && let Some(key) = remaining.next()
    {
        if let Some(entry) = read_entry(dir, key.name, has_preview)? {
            entries.push(entry);
        }
    }

    Ok(ListPage {
        entries,
        has_more: remaining.next().is_some(),
        total,
    })
}

fn read_keys(dir: &Path) -> Result<Vec<SortKey>, StorageError> {
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
        keys.push(SortKey::new(kind, name));
    }

    Ok(keys)
}

/// `None` when `name` is gone or is no longer a plain file or directory.
fn read_entry(dir: &Path, name: String, has_preview: &HasPreview) -> io::Result<Option<Entry>> {
    let path = dir.join(&name);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    to_entry(name, &meta, &path, has_preview)
}

fn to_entry(
    name: String,
    meta: &Metadata,
    path: &Path,
    has_preview: &HasPreview,
) -> io::Result<Option<Entry>> {
    let modified = meta.modified()?;
    let modified_at = DateTime::<Utc>::from(modified);
    let created_at = meta.created().ok().map(DateTime::<Utc>::from);
    let file_type = meta.file_type();

    Ok(if file_type.is_dir() {
        Some(Entry::Dir(Dir {
            name,
            modified_at,
            created_at,
        }))
    } else if file_type.is_file() {
        let probe = probe_file(path);
        Some(Entry::File(File {
            name,
            modified_at,
            created_at,
            size: meta.len(),
            mime: probe.mime,
            has_preview: probe.image.is_some() && has_preview(path, modified),
            image: probe.image,
        }))
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("naspresso-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn dir(&self, name: &str) -> &Self {
            fs::create_dir(self.0.join(name)).unwrap();
            self
        }

        fn file(&self, name: &str) -> &Self {
            fs::write(self.0.join(name), name).unwrap();
            self
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn list(dir: &Path, after: Option<EntryKey>, limit: usize) -> Result<ListPage, StorageError> {
        list_dir_blocking(dir, after, limit, &|_, _| false)
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(Entry::name).collect()
    }

    fn after(entry: &Entry) -> Option<EntryKey> {
        Some(EntryKey::of(entry))
    }

    #[test]
    fn lists_dirs_first_then_by_name_ignoring_case() {
        let tmp = TempDir::new("order");
        tmp.file("b.txt")
            .file("A.txt")
            .file("c.txt")
            .dir("Zed")
            .dir("alpha");

        let page = list(&tmp.0, None, 100).unwrap();
        assert_eq!(
            names(&page.entries),
            ["alpha", "Zed", "A.txt", "b.txt", "c.txt"]
        );
        assert_eq!(page.total, 5);
        assert!(!page.has_more);
    }

    #[test]
    fn exact_name_breaks_case_ties() {
        let mut keys = [
            SortKey::new(EntryKind::File, "a".into()),
            SortKey::new(EntryKind::File, "B".into()),
            SortKey::new(EntryKind::File, "A".into()),
        ];
        keys.sort_unstable();
        let names: Vec<_> = keys.iter().map(|key| key.name.as_str()).collect();
        assert_eq!(names, ["A", "a", "B"]);
    }

    #[test]
    fn pages_through_everything() {
        let tmp = TempDir::new("pages");
        tmp.dir("d1").dir("d2").file("f1").file("f2").file("f3");

        let first = list(&tmp.0, None, 2).unwrap();
        assert_eq!(names(&first.entries), ["d1", "d2"]);
        assert!(first.has_more);

        let second = list(&tmp.0, after(&first.entries[1]), 2).unwrap();
        assert_eq!(names(&second.entries), ["f1", "f2"]);
        assert!(second.has_more);

        let last = list(&tmp.0, after(&second.entries[1]), 2).unwrap();
        assert_eq!(names(&last.entries), ["f3"]);
        assert!(!last.has_more);
        assert_eq!(last.total, 5);
    }

    #[test]
    fn resumes_after_a_deleted_entry() {
        let tmp = TempDir::new("deleted");
        tmp.file("a").file("b").file("c");

        let first = list(&tmp.0, None, 2).unwrap();
        assert_eq!(names(&first.entries), ["a", "b"]);
        fs::remove_file(tmp.0.join("b")).unwrap();

        let next = list(&tmp.0, after(&first.entries[1]), 2).unwrap();
        assert_eq!(names(&next.entries), ["c"]);
        assert!(!next.has_more);
    }

    #[test]
    fn an_exactly_full_last_page_has_no_more() {
        let tmp = TempDir::new("full");
        tmp.file("a").file("b");

        let page = list(&tmp.0, None, 2).unwrap();
        assert_eq!(names(&page.entries), ["a", "b"]);
        assert!(!page.has_more);
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinks() {
        let tmp = TempDir::new("symlinks");
        tmp.file("real");
        std::os::unix::fs::symlink(tmp.0.join("real"), tmp.0.join("link")).unwrap();

        let page = list(&tmp.0, None, 100).unwrap();
        assert_eq!(names(&page.entries), ["real"]);
        assert_eq!(page.total, 1);
    }
}
