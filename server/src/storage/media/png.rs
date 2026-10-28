//! PNG: size from IHDR, PQ or HLG from cICP. Both come before the image data.

use std::io::{self, BufReader, Read, Seek};

use super::{
    bytes::{invalid_data, read_array, read_u32, skip},
    cicp::hdr_from_transfer,
};
use crate::storage::{Hdr, ImageMetadata};

const SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";
/// Every chunk ends with a CRC.
const CRC_LEN: u64 = 4;

/// Expects the reader at the start of the file.
pub fn read<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<ImageMetadata> {
    if read_array(reader)? != SIGNATURE {
        return Err(invalid_data("not a PNG"));
    }

    let (len, kind) = chunk_header(reader)?;
    if kind != *b"IHDR" || len < 8 {
        return Err(invalid_data("PNG doesn't start with IHDR"));
    }
    let width = read_u32(reader)?;
    let height = read_u32(reader)?;
    if width == 0 || height == 0 {
        return Err(invalid_data("zero-sized PNG"));
    }
    skip(reader, len - 8 + CRC_LEN)?;

    let mut hdr = Hdr::None;
    loop {
        let (len, kind) = chunk_header(reader)?;
        match &kind {
            b"cICP" if len >= 4 => {
                let [_primaries, transfer, _matrix, _full_range] = read_array(reader)?;
                hdr = hdr_from_transfer(transfer.into());
                break;
            }
            b"IDAT" | b"IEND" => break,
            _ => skip(reader, len + CRC_LEN)?,
        }
    }

    Ok(ImageMetadata { width, height, hdr })
}

fn chunk_header(reader: &mut impl Read) -> io::Result<(u64, [u8; 4])> {
    Ok((read_u32(reader)?.into(), read_array(reader)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::media::test_support::page_reader;

    fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let len = u32::try_from(body.len()).unwrap();
        [&len.to_be_bytes()[..], kind, body, &[0; 4]].concat()
    }

    fn png(width: u32, height: u32, chunks: &[Vec<u8>]) -> Vec<u8> {
        let ihdr = [
            &width.to_be_bytes()[..],
            &height.to_be_bytes(),
            &[8, 6, 0, 0, 0],
        ]
        .concat();
        let mut data = SIGNATURE.to_vec();
        data.extend_from_slice(&chunk(b"IHDR", &ihdr));
        for chunk in chunks {
            data.extend_from_slice(chunk);
        }
        data.extend_from_slice(&chunk(b"IDAT", &[0; 100]));
        data.extend_from_slice(&chunk(b"IEND", &[]));
        data
    }

    #[test]
    fn reads_the_size_of_a_plain_png() {
        let (mut reader, _) = page_reader(png(800, 600, &[chunk(b"sRGB", &[0])]));
        let meta = read(&mut reader).unwrap();
        assert_eq!((meta.width, meta.height, meta.hdr), (800, 600, Hdr::None));
    }

    #[test]
    fn reads_the_transfer_from_cicp() {
        let icc = chunk(b"iCCP", &[0; 3000]);
        let pq = png(8, 8, &[icc.clone(), chunk(b"cICP", &[9, 16, 0, 1])]);
        let hlg = png(8, 8, &[icc, chunk(b"cICP", &[9, 18, 0, 1])]);

        let (mut reader, _) = page_reader(pq);
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::Pq);
        let (mut reader, _) = page_reader(hlg);
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::Hlg);
    }

    #[test]
    fn rejects_non_pngs() {
        let (mut reader, _) = page_reader(b"\xFF\xD8\xFF\xE0".to_vec());
        assert!(read(&mut reader).is_err());
    }
}
