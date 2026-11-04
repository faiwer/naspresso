use std::io::{Read, Seek, SeekFrom, sink};

use hmac::Mac;

use crate::Key;

use super::{
    error::ContentError,
    format::{self, EOF_SIZE},
    keys, mac,
    read::decrypt,
};

/// Check the header, the stored chunk tags, and the end-of-file authenticator.
///
/// This does not read chunk ciphertext, so it does not prove the file contents are intact.
pub fn verify_tag_sequence<R: Read + Seek>(
    key: &Key,
    file: &mut R,
    file_len: u64,
) -> Result<(), ContentError> {
    let layout = format::layout(file_len)?;
    let mut header = [0; format::HEADER_SIZE as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header).map_err(short_read)?;
    format::check_header(&header)?;
    let derived = keys::derive(&key.contents);
    let mut hmac = mac::start(&derived, &header);
    for index in 0..layout.chunk_count {
        let plain_len = format::chunk_plain_len(&layout, index);
        let tag_at = format::physical_offset(index)? + plain_len;
        file.seek(SeekFrom::Start(tag_at))?;
        let mut tag = [0; 16];
        file.read_exact(&mut tag).map_err(short_read)?;
        hmac.update(&tag);
    }
    let mut eof = [0; EOF_SIZE as usize];
    file.seek(SeekFrom::Start(file_len - EOF_SIZE))?;
    file.read_exact(&mut eof).map_err(short_read)?;
    mac::matches(
        &eof,
        &mac::finish(hmac, layout.plain_len, layout.chunk_count),
    )
}

/// Authenticate every chunk and the end-of-file authenticator.
pub fn verify_all<R: Read + Seek>(
    key: &Key,
    file: &mut R,
    file_len: u64,
) -> Result<(), ContentError> {
    file.seek(SeekFrom::Start(0))?;
    decrypt(key, file, file_len, &mut sink())
}

fn short_read(error: std::io::Error) -> ContentError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        ContentError::Layout
    } else {
        ContentError::Io(error)
    }
}
