//! An unlocked `.npe` file, read through a decrypting reader.

use std::{fs, io, path::Path, time::SystemTime};

use media::{is_image_path, probe_reader};

use super::super::{mime, xattr};
use super::describe::Described;

/// Mime and image headers of an unlocked `.npe` file, read through a decrypting reader.
/// The decoded name supplies the extension. A reader that does not open leaves the
/// ciphertext's own type.
pub(super) fn describe_open_npe(
    npe_file_path: &Path,
    decrypted_file_name: &str,
    modified: SystemTime,
    key: &::crypto::Key,
) -> Described {
    let decrypted_path = Path::new(decrypted_file_name);
    if is_image_path(decrypted_path) {
        return describe_npe_image(npe_file_path, decrypted_file_name, modified, key);
    }

    // A known extension is the type. `file_mime` would sniff, and
    // `decrypted_path` is the decoded name, not a path on disk.
    if !mime::needs_sniff(decrypted_path, 1) {
        return declared_npe(npe_file_path, decrypted_file_name, modified, key);
    }

    if let Some(cached) = xattr::load_npe(npe_file_path, modified, &key.names, decrypted_file_name)
    {
        return Described {
            mime: cached.mime,
            image: None,
            pending: None,
        };
    }

    let Some((plain_len, mut reader)) = open_npe_reader(npe_file_path, key) else {
        return unreadable_npe(npe_file_path);
    };
    if mime::needs_sniff(decrypted_path, plain_len)
        && let Some(mime) = sniff_plain(&mut reader, plain_len, decrypted_path)
    {
        return Described {
            pending: xattr::pending_npe(npe_file_path, mime, None, &key.names, decrypted_file_name),
            // If `mime` is too long, it's stored as `application/octet-stream`.
            // Why? To avoid the xattr being too long. Why? On ext4 if we keep
            // it below 56 Bytes the metadata is fast-available.
            mime: mime.to_owned(),
            image: None,
        };
    }
    declared_npe(npe_file_path, decrypted_file_name, modified, key)
}

fn describe_npe_image(
    npe_file_path: &Path,
    decrypted_file_name: &str,
    modified: SystemTime,
    key: &::crypto::Key,
) -> Described {
    if let Some(cached) = xattr::load_npe(npe_file_path, modified, &key.names, decrypted_file_name)
    {
        return Described {
            mime: cached.mime,
            image: cached.image,
            pending: None,
        };
    }
    let Some((_, reader)) = open_npe_reader(npe_file_path, key) else {
        return unreadable_npe(npe_file_path);
    };
    let decrypted_file_path = Path::new(decrypted_file_name);

    match probe_reader(decrypted_file_path, reader) {
        Ok(Some(probe)) => Described {
            pending: xattr::pending_npe(
                npe_file_path,
                probe.mime,
                probe.image,
                &key.names,
                decrypted_file_name,
            ),
            mime: probe.mime.to_owned(),
            image: probe.image,
        },
        _ => declared_npe(npe_file_path, decrypted_file_name, modified, key),
    }
}

/// The extension's mime, plus a sealed attribute when this file does not have a fresh one.
fn declared_npe(
    npe_file_path: &Path,
    decrypted_file_name: &str,
    modified: SystemTime,
    key: &::crypto::Key,
) -> Described {
    let mime = mime::declared_mime(Path::new(decrypted_file_name));

    if xattr::load_npe(npe_file_path, modified, &key.names, decrypted_file_name).is_some() {
        // Discard the xattr value, since is has nothing but the mime type. And
        // if the source file's mime is too long the mime from the xattr is a
        // fallback. Thus, derive it from the decrypted name instead.
        return Described {
            mime: mime.to_owned(),
            image: None,
            pending: None,
        };
    }

    Described {
        mime: mime.to_owned(),
        image: None,
        pending: xattr::pending_npe(npe_file_path, mime, None, &key.names, decrypted_file_name),
    }
}

fn unreadable_npe(original_path: &Path) -> Described {
    Described {
        mime: mime::declared_mime(original_path).to_owned(),
        image: None,
        pending: None,
    }
}

fn open_npe_reader(
    npe_file_path: &Path,
    key: &::crypto::Key,
) -> Option<(u64, std::io::BufReader<::crypto::ContentFile<fs::File>>)> {
    let plain = match open_plain(npe_file_path, key) {
        Ok(plain) => plain,
        Err(error) => {
            tracing::debug!(error = %error, path = %npe_file_path.display(), "can't open encrypted content");
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
