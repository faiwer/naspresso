//! `user.naspresso.file`: the mime type, and image header data, from an earlier read.

use std::{
    ffi::{CString, OsStr},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use media::{Hdr, ImageMetadata};

const NAME: &[u8] = b"user.naspresso.file\0";
const VERSION: u8 = 1;
const HAS_IMAGE: u8 = 1;
/// Sealed `.npe` record, before the clear version byte.
const NPE_BODY: usize = 48;
const NPE_MIME_CAP: usize = 24;
const OCTET_STREAM: &str = "application/octet-stream";

/// What an earlier listing stored on the file.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Cached {
    pub mime: String,
    pub image: Option<ImageMetadata>,
}

/// One attribute to write after the listing response has been returned.
pub(crate) struct Pending {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// The stored record when it is version 1 and was captured at or after `modified`.
pub(crate) fn load(path: &Path, modified: SystemTime) -> Option<Cached> {
    decode(&read_attr(path)?, modified)
}

/// Bytes captured at `stored_at`. `None` when `mime` is empty or longer than 255 bytes.
pub(crate) fn encode(
    mime: &str,
    image: Option<ImageMetadata>,
    stored_at: SystemTime,
) -> Option<Vec<u8>> {
    if mime.is_empty() {
        return None;
    }
    let (secs, nanos) = parts(stored_at)?;
    let mime_bytes = mime.as_bytes();
    let mime_len = u8::try_from(mime_bytes.len()).ok()?;
    let mut bytes = Vec::with_capacity(24 + mime_bytes.len());
    bytes.push(VERSION);
    bytes.push(u8::from(image.is_some()));
    bytes.extend_from_slice(&secs.to_le_bytes());
    bytes.extend_from_slice(&nanos.to_le_bytes());
    bytes.push(mime_len);
    bytes.extend_from_slice(mime_bytes);
    if let Some(image) = image {
        bytes.extend_from_slice(&image.width.to_le_bytes());
        bytes.extend_from_slice(&image.height.to_le_bytes());
        bytes.push(u8::from(image.hdr));
        bytes.push(u8::from(image.animated));
    }
    Some(bytes)
}

/// The sealed record on an `.npe` file. `decrypted_file_name` is the cipher tweak.
pub(crate) fn load_npe(
    path: &Path,
    modified: SystemTime,
    key: &[u8; 32],
    decrypted_file_name: &str,
) -> Option<Cached> {
    open_sealed(&read_attr(path)?, modified, key, decrypted_file_name)
}

/// A sealed record for an `.npe` file, captured now. A mime longer than [`NPE_MIME_CAP`]
/// is stored as [`OCTET_STREAM`].
pub(crate) fn pending_npe(
    npe_file_path: &Path,
    mime: &str,
    image: Option<ImageMetadata>,
    key: &[u8; 32],
    decrypted_file_name: &str,
) -> Option<Pending> {
    let stored_at = SystemTime::now();
    Some(Pending {
        path: npe_file_path.to_owned(),
        bytes: seal_record(mime, image, stored_at, key, decrypted_file_name)?,
    })
}

fn cap_mime(mime: &str) -> &str {
    if mime.len() > NPE_MIME_CAP {
        OCTET_STREAM
    } else {
        mime
    }
}

/// A record for `path`, captured now. `None` when it would be older than `modified`
/// or the mime type cannot be stored.
pub(crate) fn pending(
    path: &Path,
    mime: &str,
    image: Option<ImageMetadata>,
    modified: SystemTime,
) -> Option<Pending> {
    let stored_at = SystemTime::now();
    if stored_at < modified {
        return None;
    }
    Some(Pending {
        path: path.to_owned(),
        bytes: encode(mime, image, stored_at)?,
    })
}

/// Writes each record. A failure is logged; the listing already returned the data.
pub(crate) fn write_records(pending: Vec<Pending>) {
    for record in pending {
        if let Err(error) = store(&record.path, &record.bytes) {
            tracing::debug!(
                error = %error,
                path = %record.path.display(),
                "can't store file metadata"
            );
        }
    }
}

fn seal_record(
    mime: &str,
    image: Option<ImageMetadata>,
    stored_at: SystemTime,
    key: &[u8; 32],
    decrypted_file_name: &str,
) -> Option<Vec<u8>> {
    let plain = encode_body(mime, image, stored_at)?;
    let cipher = ::crypto::seal_xattr(key, decrypted_file_name.as_bytes(), &plain);
    let mut bytes = Vec::with_capacity(1 + NPE_BODY);
    bytes.push(VERSION);
    bytes.extend_from_slice(&cipher);
    Some(bytes)
}

fn open_sealed(
    bytes: &[u8],
    modified: SystemTime,
    key: &[u8; 32],
    decrypted_file_name: &str,
) -> Option<Cached> {
    if bytes.len() != 1 + NPE_BODY || bytes[0] != VERSION {
        return None;
    }
    let plain = ::crypto::open_xattr(key, decrypted_file_name.as_bytes(), &bytes[1..]);
    decode_body(&plain, modified)
}

fn encode_body(
    mime: &str,
    image: Option<ImageMetadata>,
    stored_at: SystemTime,
) -> Option<[u8; NPE_BODY]> {
    let mime = cap_mime(mime);
    if mime.is_empty() {
        return None;
    }

    let (secs, nanos) = parts(stored_at)?;

    let mime_bytes = mime.as_bytes();
    let mime_len = u8::try_from(mime_bytes.len()).ok()?;

    let mut body = [0u8; NPE_BODY];
    let mut at = 0;

    body[at] = u8::from(image.is_some());
    at += 1;

    body[at..at + 8].copy_from_slice(&secs.to_le_bytes());
    at += 8;
    body[at..at + 4].copy_from_slice(&nanos.to_le_bytes());
    at += 4;

    body[at] = mime_len;
    at += 1;
    body[at..at + mime_bytes.len()].copy_from_slice(mime_bytes);
    at += mime_bytes.len();

    if let Some(image) = image {
        body[at..at + 4].copy_from_slice(&image.width.to_le_bytes());
        at += 4;
        body[at..at + 4].copy_from_slice(&image.height.to_le_bytes());
        at += 4;
        body[at] = u8::from(image.hdr);
        at += 1;
        body[at] = u8::from(image.animated);
        at += 1;
    }

    if at < NPE_BODY {
        getrandom::fill(&mut body[at..]).ok()?;
    }

    Some(body)
}

/// Like [`decode`], and the bytes after the logical record are padding.
fn decode_body(bytes: &[u8], modified: SystemTime) -> Option<Cached> {
    let mut rest = bytes;
    let flags = take_u8(&mut rest)?;
    if flags & !HAS_IMAGE != 0 {
        return None;
    }

    let stored_at = time_from_parts(take_u64(&mut rest)?, take_u32(&mut rest)?)?;
    if stored_at < modified {
        return None;
    }

    let mime_len = usize::from(take_u8(&mut rest)?);
    if mime_len == 0 || mime_len > NPE_MIME_CAP {
        return None;
    }
    let mime = std::str::from_utf8(take(&mut rest, mime_len)?).ok()?;
    let mime = mime.to_owned();

    let image = if flags & HAS_IMAGE == 0 {
        None
    } else {
        Some(ImageMetadata {
            width: take_u32(&mut rest)?,
            height: take_u32(&mut rest)?,
            hdr: Hdr::try_from(take_u8(&mut rest)?).ok()?,
            animated: match take_u8(&mut rest)? {
                0 => false,
                1 => true,
                _ => return None,
            },
        })
    };

    Some(Cached { mime, image })
}

fn decode(bytes: &[u8], modified: SystemTime) -> Option<Cached> {
    let mut rest = bytes;
    let version = take_u8(&mut rest)?;
    if version != VERSION {
        return None;
    }
    let flags = take_u8(&mut rest)?;
    if flags & !HAS_IMAGE != 0 {
        return None; // Invalid flags
    }
    let stored_at = time_from_parts(take_u64(&mut rest)?, take_u32(&mut rest)?)?;
    if stored_at < modified {
        return None;
    }
    let mime_len = usize::from(take_u8(&mut rest)?);
    let mime = std::str::from_utf8(take(&mut rest, mime_len)?).ok()?;
    if mime.is_empty() {
        return None;
    }
    let mime = mime.to_owned();
    let image = if flags & HAS_IMAGE == 0 {
        None
    } else {
        Some(ImageMetadata {
            width: take_u32(&mut rest)?,
            height: take_u32(&mut rest)?,
            hdr: Hdr::try_from(take_u8(&mut rest)?).ok()?,
            animated: match take_u8(&mut rest)? {
                0 => false,
                1 => true,
                _ => return None,
            },
        })
    };
    rest.is_empty().then_some(Cached { mime, image })
}

fn parts(time: SystemTime) -> Option<(u64, u32)> {
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    Some((since.as_secs(), since.subsec_nanos()))
}

fn time_from_parts(secs: u64, nanos: u32) -> Option<SystemTime> {
    if nanos >= 1_000_000_000 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}

fn take<'a>(rest: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    let (head, tail) = rest.split_at_checked(n)?;
    *rest = tail;
    Some(head)
}

fn take_u8(rest: &mut &[u8]) -> Option<u8> {
    let (head, tail) = rest.split_first()?;
    *rest = tail;
    Some(*head)
}

fn take_u32(rest: &mut &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(take(rest, 4)?.try_into().ok()?))
}

fn take_u64(rest: &mut &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(take(rest, 8)?.try_into().ok()?))
}

fn c_path(path: &Path) -> Option<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(OsStr::as_bytes(path.as_os_str())).ok()
}

#[cfg(unix)]
fn read_attr(path: &Path) -> Option<Vec<u8>> {
    let path = c_path(path)?;
    let mut buf = [0u8; 256];
    let read = get_attr(path.as_ptr(), buf.as_mut_ptr(), buf.len());
    if read < 0 {
        let error = io::Error::last_os_error();
        if !is_absent(&error) {
            tracing::debug!(error = %error, "can't read file metadata");
        }
        return None;
    }
    Some(buf[..usize::try_from(read).ok()?].to_vec())
}

#[cfg(not(unix))]
fn read_attr(_path: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(unix)]
fn store(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = c_path(path).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path"))?;
    let rc = set_attr(path.as_ptr(), bytes.as_ptr(), bytes.len());
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn store(_path: &Path, _bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "extended attributes",
    ))
}

#[cfg(target_os = "linux")]
fn get_attr(path: *const libc::c_char, buf: *mut u8, len: usize) -> isize {
    // SAFETY: `path` and the attribute name are nul-terminated. `buf` holds `len` writable bytes.
    unsafe { libc::lgetxattr(path, NAME.as_ptr().cast(), buf.cast(), len) }
}

#[cfg(target_os = "linux")]
fn set_attr(path: *const libc::c_char, bytes: *const u8, len: usize) -> i32 {
    // SAFETY: `path` and the attribute name are nul-terminated. `bytes` holds `len` readable bytes.
    unsafe { libc::lsetxattr(path, NAME.as_ptr().cast(), bytes.cast(), len, 0) }
}

#[cfg(target_os = "macos")]
fn get_attr(path: *const libc::c_char, buf: *mut u8, len: usize) -> isize {
    // SAFETY: `path` and the attribute name are nul-terminated. `buf` holds `len` writable bytes.
    unsafe {
        libc::getxattr(
            path,
            NAME.as_ptr().cast(),
            buf.cast(),
            len,
            0,
            libc::XATTR_NOFOLLOW,
        )
    }
}

#[cfg(target_os = "macos")]
fn set_attr(path: *const libc::c_char, bytes: *const u8, len: usize) -> i32 {
    // SAFETY: `path` and the attribute name are nul-terminated. `bytes` holds `len` readable bytes.
    unsafe {
        libc::setxattr(
            path,
            NAME.as_ptr().cast(),
            bytes.cast(),
            len,
            0,
            libc::XATTR_NOFOLLOW,
        )
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn get_attr(path: *const libc::c_char, buf: *mut u8, len: usize) -> isize {
    let _ = (path, buf, len);
    -1
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn set_attr(path: *const libc::c_char, bytes: *const u8, len: usize) -> i32 {
    let _ = (path, bytes, len);
    -1
}

fn is_absent(error: &io::Error) -> bool {
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    #[cfg(target_os = "linux")]
    return code == libc::ENODATA || code == libc::EOPNOTSUPP;
    #[cfg(target_os = "macos")]
    return code == libc::ENOATTR || code == libc::ENOTSUP;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = code;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn round_trips_an_image_and_a_text_sniff() {
        let image = ImageMetadata {
            width: 4,
            height: 5,
            hdr: Hdr::GainMap,
            animated: true,
        };
        let stored = at(50);
        let bytes = encode("image/jpeg", Some(image), stored).unwrap();
        assert_eq!(
            decode(&bytes, at(50)),
            Some(Cached {
                mime: "image/jpeg".to_owned(),
                image: Some(image),
            })
        );
        assert_eq!(decode(&bytes, at(51)), None);

        let text = encode("text/plain", None, stored).unwrap();
        assert_eq!(
            decode(&text, at(10)),
            Some(Cached {
                mime: "text/plain".to_owned(),
                image: None,
            })
        );
    }

    #[test]
    fn a_wrong_version_or_mime_is_a_miss() {
        let stored = at(50);
        let mut bytes = encode("image/png", None, stored).unwrap();
        bytes[0] = 2;
        assert_eq!(decode(&bytes, at(1)), None);

        let mut unknown = encode("text/plain", None, stored).unwrap();
        let start = unknown.len() - "text/plain".len();
        unknown[start] = 0xff;
        assert_eq!(decode(&unknown, at(1)), None);
    }

    #[test]
    fn a_sealed_image_and_a_sealed_mime_have_one_length() {
        let names = [0x11; 32];
        let stored = at(50);
        let image = ImageMetadata {
            width: 4,
            height: 5,
            hdr: Hdr::GainMap,
            animated: true,
        };
        let picture = seal_record("image/jpeg", Some(image), stored, &names, "a.jpg").unwrap();
        let text = seal_record("text/plain", None, stored, &names, "a.jpg").unwrap();
        let again = seal_record("text/plain", None, stored, &names, "a.jpg").unwrap();
        assert_eq!(picture.len(), 1 + NPE_BODY);
        assert_eq!(text.len(), picture.len());
        assert_eq!(picture[0], VERSION);
        assert_ne!(picture, text);
        assert_ne!(text, again);
        assert_eq!(
            open_sealed(&picture, at(50), &names, "a.jpg"),
            Some(Cached {
                mime: "image/jpeg".to_owned(),
                image: Some(image),
            })
        );
        assert_eq!(
            open_sealed(&text, at(10), &names, "a.jpg"),
            Some(Cached {
                mime: "text/plain".to_owned(),
                image: None,
            })
        );
        assert_eq!(open_sealed(&picture, at(51), &names, "a.jpg"), None);
        assert_ne!(
            open_sealed(&picture, at(50), &names, "b.jpg"),
            Some(Cached {
                mime: "image/jpeg".to_owned(),
                image: Some(image),
            })
        );
    }

    #[test]
    fn a_mime_past_the_cap_is_stored_as_octet_stream() {
        let names = [0x11; 32];
        let long = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
        let bytes = seal_record(long, None, at(50), &names, "deck.pptx").unwrap();
        assert_eq!(
            open_sealed(&bytes, at(1), &names, "deck.pptx"),
            Some(Cached {
                mime: OCTET_STREAM.to_owned(),
                image: None,
            })
        );
    }
}
