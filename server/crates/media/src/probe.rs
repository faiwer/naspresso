use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    bytes::invalid_data,
    exif::{self, upright},
    gif, heif, jpeg, jxl,
    metadata::{Hdr, ImageMetadata},
    png, webp,
};

/// How much of an image is read to recognize its type: one memory page.
pub(crate) const SNIFF_LEN: usize = 4096;

pub struct Probe {
    pub mime: &'static str,
    pub image: Option<ImageMetadata>,
}

/// Image metadata for `path`, when its extension is one this crate reads.
/// `Ok(None)` for any other extension, without opening the file. The MIME type
/// is magic bytes, or the extension when the bytes aren't recognized.
pub fn probe_file(path: &Path) -> io::Result<Option<Probe>> {
    if image_extension_mime(path).is_none() {
        return Ok(None);
    }
    let file = open(path).inspect_err(
        |e| tracing::debug!(error = %e, path = %path.display(), "can't open an image"),
    )?;
    disable_read_ahead(&file);
    probe_reader(path, BufReader::with_capacity(SNIFF_LEN, file))
}

/// [`probe_file`] for a reader already at the start of the plaintext. `name`
/// supplies the extension; it does not have to be the file that was opened.
pub fn probe_reader<R: Read + Seek>(
    name: &Path,
    mut reader: BufReader<R>,
) -> io::Result<Option<Probe>> {
    if image_extension_mime(name).is_none() {
        return Ok(None);
    }
    let mime = image_mime(reader.fill_buf()?, name);
    let image = read_image(&mut reader, mime)
        .inspect_err(
            |e| tracing::debug!(error = %e, path = %name.display(), "unreadable image header"),
        )
        .ok();
    Ok(Some(Probe { mime, image }))
}

/// Magic bytes, or the image extension when those bytes aren't a known type.
fn image_mime(head: &[u8], path: &Path) -> &'static str {
    infer::get(head)
        .map(|kind| kind.mime_type())
        .or_else(|| image_extension_mime(path))
        .unwrap_or("application/octet-stream")
}

/// Whether [`probe_file`](probe_file) would open `path`.
pub fn is_image_path(path: &Path) -> bool {
    image_extension_mime(path).is_some()
}

fn image_extension_mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "apng" => "image/apng",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        "heic" => "image/heic",
        "heif" | "hif" => "image/heif",
        "jxl" => "image/jxl",
        _ => return None,
    })
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
        return Err(invalid_data("not a regular file"));
    }
    Ok(file)
}

/// A probe needs a page or two; the kernel's read-ahead would pull in far more.
#[cfg(target_os = "linux")]
fn disable_read_ahead(file: &fs::File) {
    use std::os::fd::AsRawFd;
    // SAFETY: an advisory call on a descriptor `file` keeps open.
    unsafe {
        libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_RANDOM);
    }
}

/// A probe needs a page or two; the kernel's read-ahead would pull in far more.
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

fn read_image<R: Read + Seek>(reader: &mut BufReader<R>, mime: &str) -> io::Result<ImageMetadata> {
    match mime {
        "image/jpeg" => jpeg::probe::read(reader),
        "image/png" => png::probe::read(reader),
        "image/avif" | "image/heif" => heif::probe::read(reader),
        "image/jxl" => jxl::probe::read(reader),
        // GIF, WebP, BMP and TIFF: size, orientation and animation; HDR isn't detected
        _ => {
            let size = read_size(reader)?;
            reader.rewind()?;
            let (orientation, animated) = match mime {
                "image/gif" => (1, gif::probe::read_animated(reader)?),
                "image/webp" => {
                    let extended = webp::probe::read_extended(reader)?;
                    (extended.orientation, extended.animated)
                }
                "image/tiff" => {
                    let len = reader.seek(SeekFrom::End(0))?;
                    reader.rewind()?;
                    (exif::read_orientation(reader, len)?, false)
                }
                _ => (1, false),
            };
            let (width, height) = upright(size, orientation);
            Ok(ImageMetadata {
                width,
                height,
                hdr: Hdr::None,
                animated,
            })
        }
    }
}

/// The stored size, by the `imagesize` crate.
fn read_size<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<(u32, u32)> {
    let size = imagesize::reader_size(reader).map_err(|e| io::Error::other(e.to_string()))?;
    let dimension =
        |value: usize| u32::try_from(value).map_err(|_| invalid_data("image too large"));
    Ok((dimension(size.width)?, dimension(size.height)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_image_is_not_opened() {
        // Doesn't exist, so opening it would fail.
        assert!(
            probe_file(Path::new("/nonexistent/report.pdf"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_unopenable_image_is_an_error() {
        assert!(probe_file(Path::new("/nonexistent/photo.JPG")).is_err());
    }

    #[test]
    fn probes_an_image_file() {
        let path = std::env::temp_dir().join(format!("naspresso-probe-{}.gif", std::process::id()));
        // A GIF header claiming 3×2.
        fs::write(&path, b"GIF89a\x03\x00\x02\x00\x00\x00\x00;").unwrap();
        let probe = probe_file(&path).unwrap().unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(probe.mime, "image/gif");
        let image = probe.image.unwrap();
        assert_eq!((image.width, image.height, image.hdr), (3, 2, Hdr::None));
    }
}
