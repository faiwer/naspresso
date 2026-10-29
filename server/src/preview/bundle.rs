//! One cache file per source image with every size in it:
//!
//! ```text
//! "NSPM" | version: u8 | count: u8 | source fingerprint: 16 ASCII bytes
//! count × (longest side: u16 LE | length: u32 LE)
//! the AVIFs, back to back in the same order
//! ```

use std::{
    ffi::OsString,
    io::{self, SeekFrom},
    path::{Path, PathBuf},
};

use media::preview::{PreviewSize, Rendered};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt, Take},
};

const MAGIC: &[u8; 4] = b"NSPM";
const VERSION: u8 = 1;
const FINGERPRINT_LEN: usize = 16;
const FIXED_LEN: usize = MAGIC.len() + 2 + FINGERPRINT_LEN;
const ENTRY_LEN: usize = 2 + 4;

/// One AVIF of a bundle file, positioned to be read to the end.
pub struct BundledAvif {
    pub reader: Take<fs::File>,
    /// Bytes.
    pub len: u64,
}

pub fn encode(fingerprint: &str, rendered: &Rendered) -> Vec<u8> {
    debug_assert_eq!(fingerprint.len(), FINGERPRINT_LEN);
    let avifs: Vec<_> = rendered.avifs().collect();
    let data_len: usize = avifs.iter().map(|(_, avif)| avif.len()).sum();
    let mut bytes = Vec::with_capacity(FIXED_LEN + avifs.len() * ENTRY_LEN + data_len);

    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.push(avifs.len() as u8);
    bytes.extend_from_slice(fingerprint.as_bytes());
    for (size, avif) in &avifs {
        bytes.extend_from_slice(&(size.px() as u16).to_le_bytes());
        bytes.extend_from_slice(&(avif.len() as u32).to_le_bytes());
    }
    for (_, avif) in &avifs {
        bytes.extend_from_slice(avif);
    }
    bytes
}

/// The AVIF of `size` from the bundle at `path`. `None` when there's no bundle,
/// or it was made from another version of the source, or it's damaged.
pub async fn open(
    path: &Path,
    fingerprint: &str,
    size: PreviewSize,
) -> io::Result<Option<BundledAvif>> {
    let mut file = match fs::File::open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let file_len = file.metadata().await?.len();

    let mut fixed = [0; FIXED_LEN];
    if !read_exact(&mut file, &mut fixed).await? {
        return Ok(None);
    }
    let Some(count) = parse_fixed(&fixed, fingerprint) else {
        return Ok(None);
    };
    let mut table = vec![0; count * ENTRY_LEN];
    if !read_exact(&mut file, &mut table).await? {
        return Ok(None);
    }
    let Some((offset, len)) = locate(&table, size, file_len) else {
        return Ok(None);
    };

    file.seek(SeekFrom::Start(offset)).await?;
    Ok(Some(BundledAvif {
        reader: file.take(len),
        len,
    }))
}

/// Replaces the file at `path` all at once, so a crash midway can't leave half
/// a bundle behind.
pub async fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp = with_suffix(path, ".tmp");
    fs::write(&tmp, bytes).await?;
    fs::rename(&tmp, path).await
}

pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path);
    name.push(suffix);
    PathBuf::from(name)
}

/// `false` when the file ends first.
async fn read_exact(file: &mut fs::File, buf: &mut [u8]) -> io::Result<bool> {
    match file.read_exact(buf).await {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}

/// The entry count, if the header is ours and made from `fingerprint`.
fn parse_fixed(fixed: &[u8; FIXED_LEN], fingerprint: &str) -> Option<usize> {
    let (magic, rest) = fixed.split_at(MAGIC.len());
    let [version, count, stored @ ..] = rest else {
        return None;
    };
    (magic == MAGIC && *version == VERSION && stored == fingerprint.as_bytes())
        .then_some(usize::from(*count))
}

/// The offset and length of `size`'s AVIF. `None` if it's missing or the
/// lengths don't add up to the file's.
fn locate(table: &[u8], size: PreviewSize, file_len: u64) -> Option<(u64, u64)> {
    let mut offset = (FIXED_LEN + table.len()) as u64;
    let mut found = None;
    for entry in table.chunks_exact(ENTRY_LEN) {
        let side = u16::from_le_bytes([entry[0], entry[1]]);
        let len = u64::from(u32::from_le_bytes([entry[2], entry[3], entry[4], entry[5]]));
        if u32::from(side) == size.px() {
            found = Some((offset, len));
        }
        offset += len;
    }
    found.filter(|_| offset == file_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FINGERPRINT: &str = "0123456789abcdef";

    fn sample() -> Vec<u8> {
        let rendered = Rendered::new(vec![
            (PreviewSize::S512, b"large".to_vec()),
            (PreviewSize::S256, b"mid".to_vec()),
            (PreviewSize::S128, b"s".to_vec()),
        ]);
        encode(FINGERPRINT, &rendered)
    }

    /// What [`open`] reads from `bytes`, without the file.
    fn read(bytes: &[u8], fingerprint: &str, size: PreviewSize) -> Option<Vec<u8>> {
        let count = parse_fixed(bytes.get(..FIXED_LEN)?.try_into().ok()?, fingerprint)?;
        let table = bytes.get(FIXED_LEN..FIXED_LEN + count * ENTRY_LEN)?;
        let (offset, len) = locate(table, size, bytes.len() as u64)?;
        Some(bytes[offset as usize..(offset + len) as usize].to_vec())
    }

    #[test]
    fn finds_every_size() {
        let bytes = sample();
        assert_eq!(
            read(&bytes, FINGERPRINT, PreviewSize::S512).unwrap(),
            b"large"
        );
        assert_eq!(
            read(&bytes, FINGERPRINT, PreviewSize::S256).unwrap(),
            b"mid"
        );
        assert_eq!(read(&bytes, FINGERPRINT, PreviewSize::S128).unwrap(), b"s");
    }

    #[test]
    fn rejects_another_version_of_the_source() {
        assert!(read(&sample(), "fedcba9876543210", PreviewSize::S256).is_none());
    }

    #[test]
    fn rejects_damaged_files() {
        let bytes = sample();
        assert!(read(&bytes[..bytes.len() - 1], FINGERPRINT, PreviewSize::S128).is_none());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(read(&longer, FINGERPRINT, PreviewSize::S128).is_none());
        let mut bad_magic = bytes;
        bad_magic[0] = b'X';
        assert!(read(&bad_magic, FINGERPRINT, PreviewSize::S128).is_none());
    }
}
