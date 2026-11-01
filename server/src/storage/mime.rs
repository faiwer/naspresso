use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::Path,
};

/// How much of a file is read to recognize its type: one memory page.
pub(crate) const SNIFF_LEN: usize = 4096;

/// Largest file whose bytes may promote an unknown extension to `text/plain`.
const TEXT_MAX_LEN: u64 = 512 * 1024;

const FALLBACK_MIME: &str = "application/octet-stream";

/// The type of a file that isn't probed as an image. A known extension comes
/// from the table. Anything else is sniffed when it is small enough to be text.
pub(crate) fn file_mime(path: &Path, file_size: u64) -> &'static str {
    if let Some(mime) = extension_mime(path) {
        return mime;
    }
    sniffed_mime(path, file_size).unwrap_or(FALLBACK_MIME)
}

/// Whether [`sniffed_mime`] would open `path`. A known extension, an empty
/// file, and a file over [`TEXT_MAX_LEN`] are typed without a read.
pub(crate) fn needs_sniff(path: &Path, file_size: u64) -> bool {
    extension_mime(path).is_none() && file_size > 0 && file_size <= TEXT_MAX_LEN
}

/// The MIME type by magic bytes in `head`, the start of a file of `file_size`
/// bytes at `path`, then by `path`'s extension. An extension the table does not
/// know becomes `text/plain` when the file is non-empty, at most
/// [`TEXT_MAX_LEN`] bytes, and `head` is text.
pub(crate) fn sniff_mime(head: &[u8], file_size: u64, path: &Path) -> &'static str {
    if let Some(kind) = infer::get(head) {
        return kind.mime_type();
    }
    extension_mime(path).unwrap_or_else(|| {
        if is_text(head, file_size) {
            "text/plain"
        } else {
            FALLBACK_MIME
        }
    })
}

/// The table's type for `path`'s extension. `None` when it has no row there.
fn extension_mime(path: &Path) -> Option<&'static str> {
    mime_guess::from_path(path).first_raw()
}

/// The sniffed type, when the file was actually read. `None` when
/// [`needs_sniff`] is false or the file can't be opened.
pub(crate) fn sniffed_mime(path: &Path, file_size: u64) -> Option<&'static str> {
    if !needs_sniff(path, file_size) {
        return None;
    }
    let file = open(path).ok()?;
    disable_read_ahead(&file);
    let mut reader = BufReader::with_capacity(SNIFF_LEN, file);
    let head = reader.fill_buf().ok()?;
    Some(sniff_mime(head, file_size, path))
}

/// `head` is the start of a file of `file_size` bytes: UTF-8, with no NUL. A
/// character cut off by the sample still counts when the file goes on past it.
fn is_text(head: &[u8], file_size: u64) -> bool {
    if file_size == 0
        // Skip too big files. They are likely binary. Not worth the effort to sniff.
        || file_size > TEXT_MAX_LEN
        // Head is the whole prefix, not a short read.
        || head.len() as u64 != file_size.min(SNIFF_LEN as u64)
    {
        return false;
    }

    if head.contains(&0) {
        return false; // NUL bytes are not text. Nailed it.
    }

    match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(error) => {
            // Just a symbol whose UTF-8 is incomplete (cut off by the sample).
            file_size > SNIFF_LEN as u64 && error.error_len().is_none() && error.valid_up_to() > 0
        }
    }
}

/// Opens a regular file only: the entry was a file when listed, but may have
/// been swapped for a symlink or a FIFO since.
fn open(path: &Path) -> io::Result<fs::File> {
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
            io::ErrorKind::InvalidData,
            "not a regular file",
        ));
    }
    Ok(file)
}

/// A sniff needs one page; the kernel's read-ahead would pull in far more.
#[cfg(target_os = "linux")]
fn disable_read_ahead(file: &fs::File) {
    use std::os::fd::AsRawFd;
    // SAFETY: an advisory call on a descriptor `file` keeps open.
    unsafe {
        libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_RANDOM);
    }
}

/// A sniff needs one page; the kernel's read-ahead would pull in far more.
#[cfg(target_os = "macos")]
fn disable_read_ahead(file: &fs::File) {
    use std::os::fd::AsRawFd;
    // SAFETY: an advisory call on a descriptor `file` keeps open.
    unsafe {
        libc::fcntl(file.as_raw_fd(), libc::F_RDAHEAD, 0);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn disable_read_ahead(_file: &fs::File) {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn magic_bytes_win_over_the_extension() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert_eq!(
            sniff_mime(png, png.len() as u64, Path::new("photo.jpg")),
            "image/png"
        );
    }

    #[test]
    fn unknown_bytes_fall_back_to_the_extension() {
        assert_eq!(
            sniff_mime(b"hello", 5, Path::new("notes.txt")),
            "text/plain"
        );
        assert_eq!(
            sniff_mime(b"hello", 5, Path::new("data.bin")),
            FALLBACK_MIME
        );
        assert_eq!(
            sniff_mime(&[0xff, 0xfe], 2, Path::new("blob")),
            FALLBACK_MIME
        );
    }

    #[test]
    fn small_unknown_text_is_plain() {
        let sample = b"export const n = 1;\n";
        assert_eq!(
            sniff_mime(sample, sample.len() as u64, Path::new("FileIcon.tsx")),
            "text/plain"
        );
        assert_eq!(sniff_mime(b"a\0b", 3, Path::new("blob")), FALLBACK_MIME);
        assert_eq!(sniff_mime(b"", 0, Path::new("blob")), FALLBACK_MIME);

        let head = vec![b'a'; SNIFF_LEN];
        assert_eq!(
            sniff_mime(&head, TEXT_MAX_LEN, Path::new("blob")),
            "text/plain"
        );
        assert_eq!(
            sniff_mime(&head, TEXT_MAX_LEN + 1, Path::new("blob")),
            FALLBACK_MIME
        );
    }

    #[test]
    fn a_character_cut_off_by_the_sample_is_still_text() {
        let mut head = vec![b'a'; SNIFF_LEN];
        head[SNIFF_LEN - 2..].copy_from_slice(&[0xe2, 0x82]);
        assert_eq!(
            sniff_mime(&head, TEXT_MAX_LEN, Path::new("blob")),
            "text/plain"
        );

        let partial = b"price \xe2\x82";
        assert_eq!(
            sniff_mime(partial, partial.len() as u64, Path::new("blob")),
            FALLBACK_MIME
        );
    }

    #[test]
    fn a_known_extension_is_typed_without_opening() {
        // Doesn't exist, so opening it would fail.
        assert_eq!(
            file_mime(Path::new("/nonexistent/report.pdf"), 0),
            "application/pdf"
        );
    }

    #[test]
    fn small_unknown_text_is_sniffed() {
        let path = TempFile::new("icon.tsx");
        fs::write(&path, b"export const n = 1;\n").unwrap();
        let len = fs::metadata(&path).unwrap().len();
        assert_eq!(file_mime(path.as_ref(), len), "text/plain");
    }

    #[test]
    fn a_known_binary_extension_is_not_sniffed() {
        let path = TempFile::new("blob.bin");
        fs::write(&path, b"export const n = 1;\n").unwrap();
        assert_eq!(file_mime(path.as_ref(), 1), FALLBACK_MIME);
    }

    #[test]
    fn binary_bytes_stay_octet_stream() {
        let path = TempFile::new("blob");
        fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).unwrap();
        assert_eq!(file_mime(path.as_ref(), 4), FALLBACK_MIME);
    }

    #[test]
    fn an_empty_unknown_file_is_not_text() {
        let path = TempFile::new("empty");
        fs::write(&path, b"").unwrap();
        assert_eq!(file_mime(path.as_ref(), 0), FALLBACK_MIME);
    }

    #[test]
    fn text_past_the_size_cap_stays_octet_stream() {
        let path = TempFile::new("long");
        let mut file = fs::File::create(&path).unwrap();
        std::io::Write::write_all(&mut file, &[b'a'; SNIFF_LEN]).unwrap();
        file.set_len(TEXT_MAX_LEN + 1).unwrap();
        assert_eq!(file_mime(path.as_ref(), TEXT_MAX_LEN + 1), FALLBACK_MIME);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_not_sniffed() {
        let target = TempFile::new("target");
        let link = TempFile::new("link");
        fs::write(&target, b"fn main() {}\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let len = fs::symlink_metadata(&link).unwrap().len();
        assert_eq!(file_mime(link.as_ref(), len), FALLBACK_MIME);
    }

    /// Removed on drop. The name is unique per test, which run in parallel.
    struct TempFile(std::path::PathBuf);

    impl TempFile {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("naspresso-mime-{}-{name}", std::process::id()));
            let _ = fs::remove_file(&path);
            Self(path)
        }
    }

    impl AsRef<Path> for TempFile {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
}
