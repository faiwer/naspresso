use std::path::Path;

/// How much of a file is read to recognize its type: one memory page.
pub const SNIFF_LEN: usize = 4096;

const FALLBACK_MIME: &str = "application/octet-stream";

/// The MIME type by magic bytes in `head`, the start of the file at `path`,
/// then by `path`'s extension.
pub fn sniff_mime(head: &[u8], path: &Path) -> &'static str {
    infer::get(head).map_or_else(|| guess_mime(path), |kind| kind.mime_type())
}

/// The MIME type by `path`'s extension alone, for files that aren't opened.
pub fn guess_mime(path: &Path) -> &'static str {
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or(FALLBACK_MIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_bytes_win_over_the_extension() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert_eq!(sniff_mime(png, Path::new("photo.jpg")), "image/png");
    }

    #[test]
    fn unknown_bytes_fall_back_to_the_extension() {
        assert_eq!(sniff_mime(b"hello", Path::new("notes.txt")), "text/plain");
        assert_eq!(sniff_mime(b"hello", Path::new("blob")), FALLBACK_MIME);
    }
}
