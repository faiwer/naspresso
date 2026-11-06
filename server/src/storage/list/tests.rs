use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::super::{Entry, EntryKind, File, StorageError};
use super::page::{ListPage, list_dir_blocking};
use super::sort::SortKey;
use super::*;

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("naspresso-{label}-{}", std::process::id()));
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

#[test]
fn an_unlocked_npe_keeps_its_extension_mime_and_seals_a_record() {
    let tmp = TempDir::new("npe-cache");
    let key = ::crypto::Key {
        names: [4; 32],
        contents: [5; 32],
    };
    write_npe(&tmp.0, &key, "hello.txt", b"hello");
    let page = list_dir_blocking(&tmp.0, None, 10, Encryption::None, Some(&key), &|_, _| {
        false
    })
    .unwrap();
    assert_eq!(file(&page, "hello.txt").mime, "text/plain");
    assert_eq!(page.pending.len(), 1);
    assert_eq!(page.pending[0].bytes.len(), 49);
    assert_eq!(page.pending[0].bytes[0], 1);
    crate::storage::write_records(page.pending);

    let again = list_dir_blocking(&tmp.0, None, 10, Encryption::None, Some(&key), &|_, _| {
        false
    })
    .unwrap();
    assert!(again.pending.is_empty());
    assert_eq!(file(&again, "hello.txt").mime, "text/plain");
}

#[test]
fn a_sealed_npe_is_not_given_an_xattr() {
    let tmp = TempDir::new("npe-sealed");
    tmp.file("secret.npe");
    let page = list(&tmp.0, None, 10).unwrap();
    assert_eq!(file(&page, "secret.npe").mime, "naspresso/npe");
    assert!(page.pending.is_empty());
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
