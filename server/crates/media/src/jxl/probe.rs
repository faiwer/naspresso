//! JPEG XL: size, orientation and transfer function from the image header, which comes
//! before the image data along with the embedded ICC profile, if any; a gain
//! map from its box. A packed JPEG's gain map is out of reach, so those report
//! [`Hdr::Unknown`].

use std::io::{self, BufRead, BufReader, Read, Seek};

use jxl_oxide::{HdrType, InitializeResult, JxlImage};

use crate::{
    bytes::{invalid_data, read_array, read_u32, read_u64, skip},
    exif::upright,
    metadata::{Hdr, ImageMetadata},
};

/// Headers past this are taken for a broken file. ICC profiles are the bulk.
const MAX_HEADER_LEN: usize = 1 << 20;
/// The first box of a container file; a bare codestream has no boxes.
const CONTAINER_SIGNATURE: [u8; 12] = *b"\0\0\0\x0cJXL \r\n\x87\n";

/// Expects the reader at the start of the file.
pub fn read<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<ImageMetadata> {
    let image = read_header(reader)?;
    let header = image.image_header();
    let (width, height) = upright(
        (header.size.width, header.size.height),
        header.metadata.orientation,
    );
    reader.rewind()?;
    let boxes = find_boxes(reader)?;
    let hdr = match image.hdr_type() {
        _ if boxes.packed_jpeg => Hdr::Unknown,
        Some(HdrType::Pq) => Hdr::Pq,
        Some(HdrType::Hlg) => Hdr::Hlg,
        // An HDR base image's gain map is only for SDR displays.
        None if boxes.gain_map => Hdr::GainMap,
        None => Hdr::None,
    };
    Ok(ImageMetadata { width, height, hdr })
}

/// The image with its header parsed, having read little past it.
fn read_header(reader: &mut impl BufRead) -> io::Result<JxlImage> {
    let mut uninit = JxlImage::builder().build_uninit();
    // The parser may leave the tail of what it's fed for the next call.
    let mut pending = Vec::new();
    let mut read = 0;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        pending.extend_from_slice(chunk);
        let len = chunk.len();
        reader.consume(len);
        read += len;

        let consumed = uninit.feed_bytes(&pending).map_err(io::Error::other)?;
        pending.drain(..consumed);
        match uninit.try_init().map_err(io::Error::other)? {
            InitializeResult::Initialized(image) => return Ok(image),
            InitializeResult::NeedMoreData(more) => uninit = more,
        }
        if read > MAX_HEADER_LEN {
            return Err(invalid_data("JPEG XL header too long"));
        }
    }
}

/// Boxes that change what a file holds.
#[derive(Default)]
struct Boxes {
    /// `jbrd`: the data to rebuild a packed JPEG.
    packed_jpeg: bool,
    /// `jhgm`.
    gain_map: bool,
}

/// Reads box headers only, skipping their bodies.
fn find_boxes<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<Boxes> {
    let mut found = Boxes::default();
    if read_array(reader)? != CONTAINER_SIGNATURE {
        return Ok(found);
    }
    loop {
        let len = match read_u32(reader) {
            Ok(len) => u64::from(len),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(found),
            Err(e) => return Err(e),
        };
        let kind: [u8; 4] = read_array(reader)?;
        match &kind {
            b"jbrd" => found.packed_jpeg = true,
            b"jhgm" => found.gain_map = true,
            _ => {}
        }
        let (len, header_len) = match len {
            // The last box, to the end of the file.
            0 => return Ok(found),
            1 => (read_u64(reader)?, 16),
            len => (len, 8),
        };
        let body_len = len
            .checked_sub(header_len)
            .ok_or_else(|| invalid_data("JPEG XL box shorter than its header"))?;
        skip(reader, body_len)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::page_reader;

    fn jxl_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let len = u32::try_from(8 + body.len()).unwrap();
        [&len.to_be_bytes()[..], kind, body].concat()
    }

    fn container(boxes: &[Vec<u8>]) -> Vec<u8> {
        [&CONTAINER_SIGNATURE[..], &boxes.concat()].concat()
    }

    fn boxes(file: Vec<u8>) -> (Boxes, usize) {
        let (mut reader, read) = page_reader(file);
        (find_boxes(&mut reader).unwrap(), read.get())
    }

    #[test]
    fn finds_boxes_past_big_ones() {
        let (found, read) = boxes(container(&[
            jxl_box(b"ftyp", b"jxl \0\0\0\0jxl "),
            jxl_box(b"jxlp", &vec![0; 100_000]),
            jxl_box(b"jhgm", &vec![0; 100_000]),
            jxl_box(b"jxlp", &vec![0; 100_000]),
            jxl_box(b"jbrd", &[0; 10]),
        ]));
        assert!(found.packed_jpeg && found.gain_map);
        assert!(read < 20_000, "read {read} bytes");
    }

    #[test]
    fn finds_neither() {
        let (found, _) = boxes(container(&[
            jxl_box(b"ftyp", b"jxl \0\0\0\0jxl "),
            jxl_box(b"jxlc", &[0; 100]),
        ]));
        assert!(!found.packed_jpeg && !found.gain_map);
    }

    #[test]
    fn a_bare_codestream_has_no_boxes() {
        let (found, _) = boxes(b"\xff\x0a\0\0\0\0\0\0\0\0\0\0\0\0".to_vec());
        assert!(!found.packed_jpeg && !found.gain_map);
    }
}
