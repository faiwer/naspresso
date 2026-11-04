use std::io::{self, Read, Seek, SeekFrom, Write};

use hmac::Mac;

use crate::Key;

use super::{chunk, error::ContentError, format::header, keys, mac};

/// Encrypt `plaintext` into `encrypted`.
///
/// A failure can leave a partial ciphertext in `encrypted`. Publish that output
/// only after this returns `Ok`.
pub fn encrypt<R: Read + Seek, W: Write>(
    key: &Key,
    plaintext: &mut R,
    encrypted: &mut W,
) -> Result<(), ContentError> {
    let mut prefix = [0; 16];
    getrandom::fill(&mut prefix).map_err(|_| ContentError::Random)?;
    encrypt_with_prefix(key, plaintext, encrypted, &prefix)
}

pub(super) fn encrypt_with_prefix<R: Read + Seek, W: Write>(
    key: &Key,
    plaintext: &mut R,
    encrypted: &mut W,
    prefix: &[u8; 16],
) -> Result<(), ContentError> {
    let plain_len = plaintext.seek(SeekFrom::End(0))?;
    plaintext.seek(SeekFrom::Start(0))?;
    let derived = keys::derive(&key.contents);
    let layout = super::format::layout(plain_len)?;
    let head = header(prefix, plain_len);
    encrypted.write_all(&head)?;
    let mut hmac = mac::start(&derived, &head);

    for index in 0..layout.chunk_count {
        let len = super::format::chunk_plain_len(&layout, index) as usize;
        let mut buf = vec![0; len];
        read_exact(plaintext, &mut buf)?;
        let tag = chunk::seal(&derived, &head, index, &mut buf)?;
        encrypted.write_all(&buf)?;
        encrypted.write_all(&tag)?;
        hmac.update(&tag);
    }

    encrypted.write_all(&mac::finish(hmac, plain_len, layout.chunk_count))?;
    encrypted.flush()?;
    Ok(())
}

fn read_exact(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    reader.read_exact(buf)
}
