//! On-disk v1 layout. The header is `NSPE`, one version byte, 16 nonce bytes,
//! then the plaintext length as 8 big-endian bytes.

use super::error::ContentError;

pub(super) const MAGIC: &[u8; 4] = b"NSPE";
pub(super) const VERSION: u8 = 1;
pub(super) const CHUNK_SIZE: u64 = 256 * 1024;
pub(super) const TAG_SIZE: u64 = 16;
pub(super) const HEADER_SIZE: u64 = 29;
pub(super) const EOF_SIZE: u64 = 16;
pub(super) const FULL_RECORD_SIZE: u64 = CHUNK_SIZE + TAG_SIZE;
pub(super) const NONCE_PREFIX: usize = 16;

pub(super) const HKDF_SALT: &[u8] = b"naspresso-content-v1";
pub(super) const AEAD_INFO: &[u8] = b"chunk-aead-key-v1";
pub(super) const EOF_INFO: &[u8] = b"eof-hmac-key-v1";
pub(super) const CHUNK_DOMAIN: &[u8] = b"naspresso-chunk-v1\0";
pub(super) const EOF_DOMAIN: &[u8] = b"naspresso-eof-v1\0";

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub chunk_count: u64,
    pub plain_len: u64,
}

/// Prepare a header for the NPE file.
pub(super) fn header(prefix: &[u8; NONCE_PREFIX], plain_len: u64) -> [u8; HEADER_SIZE as usize] {
    let mut out = [0; HEADER_SIZE as usize];
    out[..4].copy_from_slice(MAGIC);
    out[4] = VERSION;
    out[5..21].copy_from_slice(prefix);
    out[21..].copy_from_slice(&plain_len.to_be_bytes());
    out
}

pub(super) fn plain_len(header: &[u8; HEADER_SIZE as usize]) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&header[21..29]);
    u64::from_be_bytes(bytes)
}

pub(super) fn check_header(header: &[u8; HEADER_SIZE as usize]) -> Result<(), ContentError> {
    if &header[..4] != MAGIC {
        return Err(ContentError::Magic);
    }
    if header[4] != VERSION {
        return Err(ContentError::Version);
    }
    Ok(())
}

/// Chunk count implied by a plaintext of `plain_len` bytes.
pub(super) fn layout(plain_len: u64) -> Result<Layout, ContentError> {
    if plain_len == 0 {
        return Ok(Layout {
            chunk_count: 0,
            plain_len: 0,
        });
    }

    let chunk_count = plain_len.div_ceil(CHUNK_SIZE);
    let previous = chunk_count
        .checked_sub(1)
        .and_then(|count| count.checked_mul(CHUNK_SIZE))
        .ok_or(ContentError::Layout)?;
    let limit = chunk_count
        .checked_mul(CHUNK_SIZE)
        .ok_or(ContentError::Layout)?;
    if plain_len <= previous || plain_len > limit {
        return Err(ContentError::Layout);
    }

    Ok(Layout {
        chunk_count,
        plain_len,
    })
}

pub(super) fn chunk_plain_len(layout: &Layout, index: u64) -> u64 {
    if index + 1 == layout.chunk_count {
        layout.plain_len - index * CHUNK_SIZE
    } else {
        CHUNK_SIZE
    }
}

pub(super) fn eof_offset(layout: &Layout) -> Result<u64, ContentError> {
    if layout.chunk_count == 0 {
        return Ok(HEADER_SIZE);
    }
    let index = layout.chunk_count - 1;
    physical_offset(index)?
        .checked_add(chunk_plain_len(layout, index))
        .and_then(|offset| offset.checked_add(TAG_SIZE))
        .ok_or(ContentError::Layout)
}

pub(super) fn physical_offset(index: u64) -> Result<u64, ContentError> {
    HEADER_SIZE
        .checked_add(
            index
                .checked_mul(FULL_RECORD_SIZE)
                .ok_or(ContentError::Layout)?,
        )
        .ok_or(ContentError::Layout)
}

pub(super) fn nonce(header: &[u8; HEADER_SIZE as usize], index: u64) -> [u8; 24] {
    let mut nonce = [0; 24];
    nonce[..16].copy_from_slice(&header[5..21]);
    nonce[16..].copy_from_slice(&index.to_be_bytes());
    nonce
}

pub(super) fn aad(header: &[u8; HEADER_SIZE as usize], index: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(CHUNK_DOMAIN.len() + header.len() + 8);
    out.extend_from_slice(CHUNK_DOMAIN);
    out.extend_from_slice(header);
    out.extend_from_slice(&index.to_be_bytes());
    out
}
