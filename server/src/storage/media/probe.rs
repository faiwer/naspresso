use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::Path,
};

use super::{
    bytes::invalid_data,
    heif, jpeg,
    mime::{SNIFF_LEN, guess_mime, sniff_mime},
    png,
};
use crate::storage::{Hdr, ImageMetadata};

/// Files with other extensions aren't opened.
const IMAGE_EXTENSIONS: [&str; 16] = [
    "apng", "avif", "bmp", "gif", "heic", "heif", "hif", "jfif", "jpe", "jpeg", "jpg", "jxl",
    "png", "tif", "tiff", "webp",
];

pub struct Probe {
    pub mime: &'static str,
    pub image: Option<ImageMetadata>,
}

/// The type of the file at `path` and, for an image, its size and HDR kind.
/// Only files named like images are opened. Never fails: whatever can't be
/// read is typed by extension.
pub fn probe_file(path: &Path) -> Probe {
    if has_image_extension(path) {
        match probe_image(path) {
            Ok(probe) => return probe,
            Err(e) => tracing::debug!(error = %e, path = %path.display(), "can't open an image"),
        }
    }
    Probe {
        mime: guess_mime(path),
        image: None,
    }
}

fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

fn probe_image(path: &Path) -> io::Result<Probe> {
    let file = open(path)?;
    disable_read_ahead(&file);
    let mut reader = BufReader::with_capacity(SNIFF_LEN, file);
    let mime = sniff_mime(reader.fill_buf()?, path);
    let image = read_image(&mut reader, mime)
        .inspect_err(
            |e| tracing::debug!(error = %e, path = %path.display(), "unreadable image header"),
        )
        .ok();
    Ok(Probe { mime, image })
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

fn read_image(reader: &mut BufReader<fs::File>, mime: &str) -> io::Result<ImageMetadata> {
    match mime {
        "image/jpeg" => jpeg::read(reader),
        "image/png" => png::read(reader),
        "image/avif" | "image/heif" => heif::read(reader),
        // GIF, WebP, BMP, TIFF and JPEG XL: sizes only.
        _ => {
            let size =
                imagesize::reader_size(reader).map_err(|e| io::Error::other(e.to_string()))?;
            let dimension =
                |value: usize| u32::try_from(value).map_err(|_| invalid_data("image too large"));
            Ok(ImageMetadata {
                width: dimension(size.width)?,
                height: dimension(size.height)?,
                hdr: Hdr::None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_images_are_typed_without_opening() {
        // Doesn't exist, so opening it would fail.
        let probe = probe_file(Path::new("/nonexistent/report.pdf"));
        assert_eq!(probe.mime, "application/pdf");
        assert!(probe.image.is_none());
    }

    #[test]
    fn unopenable_images_fall_back_to_the_extension() {
        let probe = probe_file(Path::new("/nonexistent/photo.JPG"));
        assert_eq!(probe.mime, "image/jpeg");
        assert!(probe.image.is_none());
    }

    #[test]
    fn probes_an_image_file() {
        let path = std::env::temp_dir().join(format!("naspresso-probe-{}.gif", std::process::id()));
        // A GIF header claiming 3×2.
        fs::write(&path, b"GIF89a\x03\x00\x02\x00\x00\x00\x00;").unwrap();
        let probe = probe_file(&path);
        let _ = fs::remove_file(&path);

        assert_eq!(probe.mime, "image/gif");
        let image = probe.image.unwrap();
        assert_eq!((image.width, image.height, image.hdr), (3, 2, Hdr::None));
    }
}
