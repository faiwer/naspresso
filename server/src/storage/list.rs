use std::{
    cmp::Ordering,
    fs::{self, Metadata},
    io,
    path::Path,
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use media::{ImageMetadata, is_image_path, probe_file, probe_reader};
use serde::Serialize;

use super::{
    Dir, Entry, EntryKind, File, StorageError,
    mime::{self, file_mime},
    xattr::{self, Pending},
};

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

/// The order of [`EntryKey`]. `disk_name` is how the file is stored and is not part of the order.
#[derive(Debug)]
struct SortKey {
    kind: EntryKind,
    folded: String,
    /// Name the listing shows. Decoded for an `.npe` file that opened.
    name: String,
    disk_name: String,
    /// `name` was decoded from `disk_name`.
    decoded: bool,
}

impl SortKey {
    fn new(kind: EntryKind, name: String, disk_name: String, decoded: bool) -> Self {
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

type HasPreview = dyn Fn(&Path, SystemTime) -> bool;

fn is_os_metadata(name: &str) -> bool {
    matches!(name, ".DS_Store" | ".apdisk" | "Thumbs.db")
}

fn list_dir_blocking(
    dir: &Path,
    after: Option<EntryKey>,
    limit: usize,
    encryption: Encryption,
    key: Option<&::crypto::Key>,
    has_preview: &HasPreview,
) -> Result<ListPage, StorageError> {
    let mut keys = read_keys(dir, key)?;
    keys.sort_unstable();
    let total = keys.len();
    let start = after.map_or(0, |after| {
        let after = SortKey::new(after.kind, after.name.clone(), after.name, false);
        keys.partition_point(|key| *key <= after)
    });

    // Only the page gets a `stat` per entry; the rest of the directory is names only.
    let mut entries = Vec::with_capacity(limit.min(total - start));
    let mut pending = Vec::new();
    let mut remaining = keys.into_iter().skip(start);
    while entries.len() < limit
        && let Some(sorted) = remaining.next()
    {
        if let Some((entry, record)) = read_entry(dir, &sorted, has_preview, key)? {
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

fn read_keys(dir: &Path, key: Option<&::crypto::Key>) -> Result<Vec<SortKey>, StorageError> {
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

/// `None` when `name` is gone or is no longer a plain file or directory.
fn read_entry(
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
        let described = describe(path, sorted, meta.len(), modified, key);
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

/// Mime type and image header data. `pending` is set when this call opened the file.
struct Described {
    mime: String,
    image: Option<ImageMetadata>,
    pending: Option<Pending>,
}

fn describe(
    path: &Path,
    sorted: &SortKey,
    file_size: u64,
    modified: SystemTime,
    key: Option<&::crypto::Key>,
) -> Described {
    if sorted.decoded
        && let Some(key) = key
    {
        return describe_open_npe(path, Path::new(&sorted.name), modified, key);
    }
    describe_regular_file(path, file_size, modified)
}

/// Mime and image headers of an unlocked `.npe` file, read through a decrypting reader.
/// The decoded name supplies the extension. A reader that does not open leaves the
/// ciphertext's own type.
fn describe_open_npe(
    path: &Path,
    display_path: &Path,
    modified: SystemTime,
    key: &::crypto::Key,
) -> Described {
    if is_image_path(display_path) {
        return describe_open_image(path, display_path, modified, key);
    }
    // A known extension is the type. `file_mime` would sniff, and `display` is the
    // decoded name, not a path on disk.
    if !mime::needs_sniff(display_path, 1) {
        return declared(display_path);
    }
    if let Some(cached) = xattr::load(path, modified) {
        return Described {
            mime: cached.mime,
            image: None,
            pending: None,
        };
    }

    let Some((plain_len, mut reader)) = open_reader(path, key) else {
        return describe_regular_file(path, 0, modified);
    };
    if mime::needs_sniff(display_path, plain_len)
        && let Some(mime) = sniff_plain(&mut reader, plain_len, display_path)
    {
        return Described {
            pending: xattr::pending(path, mime, None, modified),
            mime: mime.to_owned(),
            image: None,
        };
    }
    declared(display_path)
}

fn describe_open_image(
    path: &Path,
    display: &Path,
    modified: SystemTime,
    key: &::crypto::Key,
) -> Described {
    if let Some(cached) = xattr::load(path, modified) {
        return Described {
            mime: cached.mime,
            image: cached.image,
            pending: None,
        };
    }
    let Some((_, reader)) = open_reader(path, key) else {
        return describe_regular_file(path, 0, modified);
    };
    match probe_reader(display, reader) {
        Ok(Some(probe)) => Described {
            pending: xattr::pending(path, probe.mime, probe.image, modified),
            mime: probe.mime.to_owned(),
            image: probe.image,
        },
        _ => declared(display),
    }
}

fn declared(display: &Path) -> Described {
    Described {
        mime: mime::declared_mime(display).to_owned(),
        image: None,
        pending: None,
    }
}

fn open_reader(
    path: &Path,
    key: &::crypto::Key,
) -> Option<(u64, std::io::BufReader<::crypto::ContentFile<fs::File>>)> {
    let plain = match open_plain(path, key) {
        Ok(plain) => plain,
        Err(error) => {
            tracing::debug!(error = %error, path = %path.display(), "can't open encrypted content");
            return None;
        }
    };
    let plain_len = plain.plaintext_len();
    Some((
        plain_len,
        std::io::BufReader::with_capacity(mime::SNIFF_LEN, plain),
    ))
}

fn open_plain(path: &Path, key: &::crypto::Key) -> io::Result<::crypto::ContentFile<fs::File>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    ::crypto::ContentFile::open(key, file).map_err(io::Error::other)
}

fn sniff_plain<R: std::io::Read>(
    reader: &mut R,
    plain_len: u64,
    display: &Path,
) -> Option<&'static str> {
    let mut head = [0; mime::SNIFF_LEN];
    let read = reader.read(&mut head).ok()?;
    if read == 0 {
        return None;
    }
    Some(mime::sniff_mime(&head[..read], plain_len, display))
}

fn describe_regular_file(path: &Path, file_size: u64, modified: SystemTime) -> Described {
    let cacheable = is_image_path(path) || mime::needs_sniff(path, file_size);
    if cacheable && let Some(cached) = xattr::load(path, modified) {
        return Described {
            mime: cached.mime,
            image: cached.image,
            pending: None,
        };
    }

    if is_image_path(path)
        && let Ok(Some(probe)) = probe_file(path)
    {
        return Described {
            pending: xattr::pending(path, probe.mime, probe.image, modified),
            mime: probe.mime.to_owned(),
            image: probe.image,
        };
    }

    if mime::needs_sniff(path, file_size)
        && let Some(mime) = mime::sniffed_mime(path, file_size)
    {
        return Described {
            pending: xattr::pending(path, mime, None, modified),
            mime: mime.to_owned(),
            image: None,
        };
    }

    Described {
        mime: file_mime(path, file_size).to_owned(),
        image: None,
        pending: None,
    }
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
        list_dir_blocking(dir, after, limit, Encryption::None, None, &|_, _| false)
    }

    #[test]
    fn an_opened_npe_name_is_shown_decoded() {
        let tmp = TempDir::new("names");
        let key = ::crypto::Key {
            names: [9; 32],
            contents: [0; 32],
        };
        let disk = ::crypto::encrypt_filename(&key, "note.txt").unwrap();
        fs::write(tmp.0.join(&disk), b"x").unwrap();
        fs::write(tmp.0.join("readme.txt"), b"x").unwrap();
        fs::write(tmp.0.join("zz.npe"), b"x").unwrap();

        let page = list_dir_blocking(&tmp.0, None, 10, Encryption::None, Some(&key), &|_, _| {
            false
        })
        .unwrap();
        let note = file(&page, "note.txt");
        assert_eq!(note.original_name.as_deref(), Some(disk.as_str()));
        let readme = file(&page, "readme.txt");
        assert_eq!(readme.original_name, None);
        let broken = file(&page, "zz.npe");
        assert_eq!(broken.original_name, None);
        assert_eq!(names(&page.entries), ["note.txt", "readme.txt", "zz.npe"]);
    }

    #[test]
    fn an_opened_npe_image_is_probed_through_its_plaintext() {
        let tmp = TempDir::new("npe-image");
        let key = ::crypto::Key {
            names: [4; 32],
            contents: [5; 32],
        };
        let gif = b"GIF89a\x03\x00\x02\x00\x00\x00\x00;";
        let disk = write_npe(&tmp.0, &key, "tiny.gif", gif);
        write_npe(&tmp.0, &key, "hello.txt", b"hello");
        let broken = ::crypto::encrypt_filename(&key, "bad.gif").unwrap();
        fs::write(tmp.0.join(&broken), b"not an npe").unwrap();

        let page = list_dir_blocking(&tmp.0, None, 10, Encryption::None, Some(&key), &|_, _| {
            false
        })
        .unwrap();

        let image = file(&page, "tiny.gif");
        assert_eq!(image.mime, "image/gif");
        assert_eq!(image.original_name.as_deref(), Some(disk.as_str()));
        let meta = image.image.unwrap();
        assert_eq!((meta.width, meta.height), (3, 2));

        let text = file(&page, "hello.txt");
        assert_eq!(text.mime, "text/plain");
        assert!(text.image.is_none());

        let failed = file(&page, "bad.gif");
        assert_eq!(failed.mime, "naspresso/npe");
        assert!(failed.image.is_none());
    }

    fn write_npe(dir: &Path, key: &::crypto::Key, name: &str, plain: &[u8]) -> String {
        let disk = ::crypto::encrypt_filename(key, name).unwrap();
        let mut input = std::io::Cursor::new(plain.to_vec());
        let mut output = std::io::Cursor::new(Vec::new());
        ::crypto::encrypt_content(key, &mut input, &mut output).unwrap();
        fs::write(dir.join(&disk), output.into_inner()).unwrap();
        disk
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
            SortKey::new(EntryKind::File, "a".into(), "a".into(), false),
            SortKey::new(EntryKind::File, "B".into(), "B".into(), false),
            SortKey::new(EntryKind::File, "A".into(), "A".into(), false),
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

    #[test]
    fn skips_os_metadata_files() {
        let tmp = TempDir::new("metadata");
        tmp.file(".DS_Store")
            .file(".apdisk")
            .file("Thumbs.db")
            .file(".gitignore")
            .file("photo.jpg");

        let page = list(&tmp.0, None, 100).unwrap();
        assert_eq!(names(&page.entries), [".gitignore", "photo.jpg"]);
        assert_eq!(page.total, 2);
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

    #[test]
    fn a_cached_read_is_reused_until_the_file_is_newer() {
        let tmp = TempDir::new("xattr");
        fs::write(tmp.0.join("photo.jpg"), tiny_jpeg(4, 5)).unwrap();
        fs::write(tmp.0.join("notes"), b"hello\n").unwrap();
        tmp.file("readme.txt");

        let first = list(&tmp.0, None, 100).unwrap();
        assert_eq!(file(&first, "photo.jpg").image.unwrap().width, 4);
        assert_eq!(file(&first, "photo.jpg").image.unwrap().height, 5);
        assert_eq!(file(&first, "notes").mime, "text/plain");
        assert!(file(&first, "notes").image.is_none());
        assert_eq!(file(&first, "readme.txt").mime, "text/plain");
        let mut stored: Vec<_> = first
            .pending
            .iter()
            .map(|record| {
                record
                    .path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        stored.sort();
        assert_eq!(stored, ["notes", "photo.jpg"]);
        crate::storage::write_records(first.pending);

        fs::write(tmp.0.join("photo.jpg"), b"nope").unwrap();
        fs::write(tmp.0.join("notes"), [0xff, 0xfe]).unwrap();
        rewind(&tmp.0.join("photo.jpg"));
        rewind(&tmp.0.join("notes"));

        let cached = list(&tmp.0, None, 100).unwrap();
        assert!(cached.pending.is_empty());
        assert_eq!(file(&cached, "photo.jpg").image.unwrap().width, 4);
        assert_eq!(file(&cached, "notes").mime, "text/plain");

        fs::write(tmp.0.join("photo.jpg"), b"nope").unwrap();
        fs::write(tmp.0.join("notes"), [0xff, 0xfe]).unwrap();
        let reread = list(&tmp.0, None, 100).unwrap();
        assert!(file(&reread, "photo.jpg").image.is_none());
        assert_eq!(file(&reread, "notes").mime, "application/octet-stream");
        assert_eq!(reread.pending.len(), 2);
    }

    fn file<'a>(page: &'a ListPage, name: &str) -> &'a File {
        match page.entries.iter().find(|entry| entry.name() == name) {
            Some(Entry::File(file)) => file,
            _ => panic!("{name} missing"),
        }
    }

    fn rewind(path: &Path) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH)
            .unwrap();
    }

    /// A JPEG whose frame header says `width` by `height` and nothing else.
    fn tiny_jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x07, 8];
        data.extend(height.to_be_bytes());
        data.extend(width.to_be_bytes());
        data
    }
}
