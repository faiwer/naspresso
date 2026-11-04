use std::io::{self, Read, Write};

use hmac::Mac;

use crate::Key;

use super::{
    chunk,
    error::ContentError,
    format::{CHUNK_SIZE, header},
    keys, mac,
};

/// Encrypt `plaintext` into `encrypted`.
///
/// A failure can leave a partial ciphertext in `encrypted`. Publish that output
/// only after this returns `Ok`.
pub fn encrypt<R: Read, W: Write>(
    key: &Key,
    plaintext: &mut R,
    encrypted: &mut W,
) -> Result<(), ContentError> {
    let mut prefix = [0; 16];
    getrandom::fill(&mut prefix).map_err(|_| ContentError::Random)?;
    encrypt_with_prefix(key, plaintext, encrypted, &prefix)
}

pub(super) fn encrypt_with_prefix<R: Read, W: Write>(
    key: &Key,
    plaintext: &mut R,
    encrypted: &mut W,
    prefix: &[u8; 16],
) -> Result<(), ContentError> {
    let derived = keys::derive(&key.contents);
    let head = header(prefix);
    encrypted.write_all(&head)?;
    let mut hmac = mac::start(&derived, &head);

    let mut buf = vec![0; CHUNK_SIZE as usize];
    let mut plain_len = 0u64;
    let mut chunk_count = 0u64;
    loop {
        let filled = read_chunk(plaintext, &mut buf)?;
        if filled == 0 {
            break;
        }

        let tag = chunk::seal(&derived, &head, chunk_count, &mut buf[..filled])?;
        encrypted.write_all(&buf[..filled])?;
        encrypted.write_all(&tag)?;
        hmac.update(&tag);
        plain_len = plain_len
            .checked_add(filled as u64)
            .ok_or(ContentError::Layout)?;
        chunk_count = chunk_count.checked_add(1).ok_or(ContentError::Layout)?;
    }

    encrypted.write_all(&mac::finish(hmac, plain_len, chunk_count))?;
    encrypted.flush()?;
    Ok(())
}

/// Fill `buf` until it is full or the reader is exhausted. A short read is not EOF.
fn read_chunk(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}
