//! Mime type and image headers of one file.

use std::{path::Path, time::SystemTime};

use media::{ImageMetadata, is_image_path, probe_file};

use super::super::{
    mime::{self, file_mime},
    xattr::{self, Pending},
};
use super::{npe, sort::SortKey};

/// Mime type and image header data. `pending` is set when this call opened the file.
pub(super) struct Described {
    pub(super) mime: String,
    pub(super) image: Option<ImageMetadata>,
    pub(super) pending: Option<Pending>,
}

pub(super) fn describe(
    path: &Path,
    sorted: &SortKey,
    file_size: u64,
    modified: SystemTime,
    key: Option<&::crypto::Key>,
) -> Described {
    if sorted.decoded
        && let Some(key) = key
    {
        return npe::describe_open_npe(path, &sorted.name, modified, key);
    }
    describe_regular_file(path, file_size, modified)
}

fn describe_regular_file(path: &Path, file_size: u64, modified: SystemTime) -> Described {
    // A sealed `.npe` xattr might have been written while the vault was open.
    // We cannot read it without decrypting anyway.
    if mime::declared_mime(path) == mime::NPE_MIME {
        return Described {
            mime: mime::NPE_MIME.to_owned(),
            image: None,
            pending: None,
        };
    }

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
