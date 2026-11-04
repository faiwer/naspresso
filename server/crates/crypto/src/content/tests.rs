use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

use crate::Key;

use super::{
    AsyncContentFile, ContentFile,
    format::{CHUNK_SIZE, EOF_SIZE, FULL_RECORD_SIZE, HEADER_SIZE},
    write::encrypt_with_prefix,
};

const PREFIX: [u8; 16] = [0x42; 16];

fn key(contents: u8) -> Key {
    Key {
        names: [0x22; 32],
        contents: [contents; 32],
    }
}

fn seal(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encrypt_with_prefix(&key(0x11), &mut Cursor::new(plain), &mut out, &PREFIX).unwrap();
    out
}

fn read_at(file: &mut (impl Read + Seek), offset: u64, buf: &mut [u8]) -> io::Result<usize> {
    file.seek(SeekFrom::Start(offset))?;
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

async fn read_at_async(
    file: &mut (impl AsyncRead + AsyncSeek + Unpin),
    offset: u64,
    buf: &mut [u8],
) -> io::Result<usize> {
    file.seek(SeekFrom::Start(offset)).await?;
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]).await? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

fn round_trip(plain: &[u8]) {
    let encrypted = seal(plain);
    let chunks = if plain.is_empty() {
        0
    } else {
        plain.len().div_ceil(CHUNK_SIZE as usize)
    };
    assert_eq!(
        encrypted.len() as u64,
        HEADER_SIZE + plain.len() as u64 + 16 * chunks as u64 + EOF_SIZE
    );
    let mut decoded = Vec::new();
    super::decrypt_content(
        &key(0x11),
        &mut Cursor::new(&encrypted),
        encrypted.len() as u64,
        &mut decoded,
    )
    .unwrap();
    assert_eq!(decoded, plain);

    let mut file = ContentFile::open(&key(0x11), Cursor::new(encrypted)).unwrap();
    assert_eq!(file.plaintext_len(), plain.len() as u64);
    let mut back = vec![0; plain.len()];
    assert_eq!(read_at(&mut file, 0, &mut back).unwrap(), plain.len());
    assert_eq!(back, plain);
}

#[test]
fn round_trips_the_size_table() {
    round_trip(b"");
    for size in [
        1,
        15,
        16,
        CHUNK_SIZE - 1,
        CHUNK_SIZE,
        CHUNK_SIZE + 1,
        2 * CHUNK_SIZE - 1,
        2 * CHUNK_SIZE,
        2 * CHUNK_SIZE + 1,
    ] {
        round_trip(&vec![0x5a; size as usize]);
    }
}

#[test]
fn round_trips_irregular_reads_and_binary_bytes() {
    let mut plain = vec![0; (8 * CHUNK_SIZE + 1) as usize];
    for (index, byte) in plain.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    plain[10] = 0;
    let mut reader = Short {
        data: &plain,
        pos: 0,
        steps: [1, 3, 8, 2],
        step: 0,
    };
    let mut encrypted = Vec::new();
    encrypt_with_prefix(&key(0x11), &mut reader, &mut encrypted, &PREFIX).unwrap();
    let mut decoded = Vec::new();
    super::decrypt_content(
        &key(0x11),
        &mut Cursor::new(&encrypted),
        encrypted.len() as u64,
        &mut decoded,
    )
    .unwrap();
    assert_eq!(decoded, plain);
}

#[test]
fn a_fresh_nonce_changes_the_ciphertext() {
    let mut first = Vec::new();
    let mut second = Vec::new();
    super::encrypt_content(&key(0x11), &mut Cursor::new(b"same"), &mut first).unwrap();
    super::encrypt_content(&key(0x11), &mut Cursor::new(b"same"), &mut second).unwrap();
    assert_ne!(first[..21], second[..21]);
    assert_ne!(first, second);
}

#[test]
fn names_do_not_affect_the_ciphertext() {
    let mut other = key(0x11);
    other.names = [0x99; 32];
    let mut out = Vec::new();
    encrypt_with_prefix(&other, &mut Cursor::new(b"a"), &mut out, &PREFIX).unwrap();
    assert_eq!(out, seal(b"a"));
}

#[test]
fn reads_ranges_without_scanning_the_whole_file() {
    let plain = vec![0xab; (2 * CHUNK_SIZE + 8) as usize];
    let encrypted = seal(&plain);
    let read = Arc::new(AtomicUsize::new(0));
    let mut file = ContentFile::open(
        &key(0x11),
        Counted {
            inner: Cursor::new(encrypted),
            read: Arc::clone(&read),
        },
    )
    .unwrap();
    let after_open = read.load(Ordering::Relaxed);
    let mut buf = vec![0; 16];
    read_at(&mut file, CHUNK_SIZE + 4, &mut buf).unwrap();
    assert_eq!(
        &buf,
        &plain[CHUNK_SIZE as usize + 4..CHUNK_SIZE as usize + 20]
    );
    let fetched = read.load(Ordering::Relaxed) - after_open;
    assert!(fetched < FULL_RECORD_SIZE as usize * 2);
    assert!(fetched >= CHUNK_SIZE as usize);

    let mut across = [0; 10];
    read_at(&mut file, CHUNK_SIZE - 4, &mut across).unwrap();
    assert_eq!(across, [0xab; 10]);
    assert_eq!(file.seek(SeekFrom::Start(0)).unwrap(), 0);
    assert_eq!(file.seek(SeekFrom::End(0)).unwrap(), plain.len() as u64);
    assert_eq!(file.read(&mut [0; 4]).unwrap(), 0);
    assert!(file.seek(SeekFrom::Start(0)).is_ok());
    assert!(file.seek(SeekFrom::Current(-1)).is_err());
}

#[test]
fn an_empty_file_has_no_plaintext() {
    let encrypted = seal(b"");
    assert_eq!(encrypted.len() as u64, HEADER_SIZE + EOF_SIZE);
    let mut file = ContentFile::open(&key(0x11), Cursor::new(&encrypted)).unwrap();
    assert_eq!(read_at(&mut file, 0, &mut [0; 4]).unwrap(), 0);
    super::verify_all(
        &key(0x11),
        &mut Cursor::new(encrypted),
        HEADER_SIZE + EOF_SIZE,
    )
    .unwrap();
}

#[test]
fn a_wrong_key_fails_closed() {
    let encrypted = seal(b"hello");
    let mut buf = [0; 5];
    let error = read_at(
        &mut ContentFile::open(&key(0x22), Cursor::new(&encrypted)).unwrap(),
        0,
        &mut buf,
    )
    .unwrap_err();
    assert!(matches!(
        error.get_ref().and_then(|err| err.downcast_ref()),
        Some(super::ContentError::Authentication)
    ));

    let empty = seal(b"");
    assert!(
        super::verify_all(&key(0x22), &mut Cursor::new(empty), HEADER_SIZE + EOF_SIZE).is_err()
    );
}

#[test]
fn corruption_is_rejected_by_the_matching_check() {
    let mut encrypted = seal(b"hello world");
    encrypted[25] ^= 0x01;
    assert!(
        super::verify_tag_sequence(
            &key(0x11),
            &mut Cursor::new(encrypted.clone()),
            encrypted.len() as u64
        )
        .is_ok()
    );
    assert!(
        super::verify_all(
            &key(0x11),
            &mut Cursor::new(encrypted.clone()),
            encrypted.len() as u64
        )
        .is_err()
    );

    let mut tag = seal(b"hello world");
    let tag_at = HEADER_SIZE as usize + b"hello world".len();
    tag[tag_at] ^= 0x01;
    assert!(
        super::verify_tag_sequence(&key(0x11), &mut Cursor::new(&tag), tag.len() as u64).is_err()
    );

    let mut eof = seal(b"hello world");
    let last = eof.len() - 1;
    eof[last] ^= 0x01;
    let mut opened = ContentFile::open(&key(0x11), Cursor::new(eof.clone())).unwrap();
    assert_eq!(read_at(&mut opened, 0, &mut [0; 11]).unwrap(), 11);
    assert!(
        super::verify_tag_sequence(&key(0x11), &mut Cursor::new(&eof), eof.len() as u64).is_err()
    );

    let mut magic = seal(b"hello world");
    magic[0] ^= 0x01;
    assert!(matches!(
        ContentFile::open(&key(0x11), Cursor::new(&magic)),
        Err(super::ContentError::Magic)
    ));
    let mut version = seal(b"hello world");
    version[4] = 2;
    assert!(matches!(
        ContentFile::open(&key(0x11), Cursor::new(version)),
        Err(super::ContentError::Version)
    ));

    let mut truncated = seal(b"hello world");
    truncated.pop();
    assert!(
        super::decrypt_content(
            &key(0x11),
            &mut Cursor::new(&truncated),
            truncated.len() as u64,
            &mut Vec::new()
        )
        .is_err()
    );

    let mut extra = seal(b"hello world");
    extra.push(0);
    assert!(super::verify_all(&key(0x11), &mut Cursor::new(&extra), extra.len() as u64).is_err());
}

#[test]
fn swapped_and_foreign_chunks_fail_authentication() {
    let plain = vec![0x11; (CHUNK_SIZE + 1) as usize];
    let mut encrypted = seal(&plain);
    let full = FULL_RECORD_SIZE as usize;
    let header = HEADER_SIZE as usize;
    let first: Vec<u8> = encrypted[header..header + full].to_vec();
    let second: Vec<u8> = encrypted[header + full..encrypted.len() - EOF_SIZE as usize].to_vec();
    encrypted[header..header + second.len()].copy_from_slice(&second);
    encrypted[header + second.len()..header + second.len() + full].copy_from_slice(&first);
    assert!(
        read_at(
            &mut ContentFile::open(&key(0x11), Cursor::new(encrypted)).unwrap(),
            0,
            &mut [0; 8],
        )
        .is_err()
    );

    let mut ours = seal(b"hello");
    let foreign = {
        let mut out = Vec::new();
        encrypt_with_prefix(
            &key(0x11),
            &mut Cursor::new(b"hello"),
            &mut out,
            &[0x07; 16],
        )
        .unwrap();
        out
    };
    let record = header + b"hello".len() + 16;
    ours[header..record].copy_from_slice(&foreign[header..record]);
    assert!(
        read_at(
            &mut ContentFile::open(&key(0x11), Cursor::new(ours)).unwrap(),
            0,
            &mut [0; 5],
        )
        .is_err()
    );
}

#[test]
fn impossible_layouts_are_rejected() {
    use super::format::layout;
    assert!(layout(HEADER_SIZE + EOF_SIZE + FULL_RECORD_SIZE + 1).is_err());
    assert!(layout(HEADER_SIZE).is_err());
    assert_eq!(layout(HEADER_SIZE + EOF_SIZE).unwrap().plain_len, 0);
    assert!(super::format::physical_offset(u64::MAX).is_err());
}

#[test]
fn matches_fixed_vectors() {
    let vectors = [
        (
            &b""[..],
            "4e5350450142424242424242424242424242424242784f83188d1a74a198830bd28ccca70f",
        ),
        (
            &b"a"[..],
            "4e53504501424242424242424242424242424242427a68bacb13e11f2a3262f65c572a4c325d3e5ee39fe12f44c1aa2820ec94342dc0",
        ),
        (
            &b"0123456789abcdef"[..],
            "4e53504501424242424242424242424242424242422b273274cde617d56a372b9de1310564624680496b5c0afdd6b4587729b42b020d9305feb98528a975d21c8a7ae3ce11",
        ),
    ];
    for (plain, expected) in vectors {
        assert_eq!(hex(&seal(plain)), expected);
        let encrypted = seal(plain);
        let mut decoded = Vec::new();
        super::decrypt_content(
            &key(0x11),
            &mut Cursor::new(&encrypted),
            encrypted.len() as u64,
            &mut decoded,
        )
        .unwrap();
        assert_eq!(decoded, plain);
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

struct Short<'a> {
    data: &'a [u8],
    pos: usize,
    steps: [usize; 4],
    step: usize,
}

impl Read for Short<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let n = self.steps[self.step % self.steps.len()]
            .min(buf.len())
            .min(self.data.len() - self.pos);
        self.step += 1;
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

struct Counted<T> {
    inner: T,
    read: Arc<AtomicUsize>,
}

impl<T: Read> Read for Counted<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
}

impl<T: Seek> Seek for Counted<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[tokio::test]
async fn async_reads_match_the_plaintext() {
    let plain = vec![0x3c; (CHUNK_SIZE + 32) as usize];
    let encrypted = seal(&plain);
    let mut file = AsyncContentFile::open(&key(0x11), Cursor::new(encrypted))
        .await
        .unwrap();
    let mut across = vec![0; 40];
    let n = read_at_async(&mut file, CHUNK_SIZE - 8, &mut across)
        .await
        .unwrap();
    assert_eq!(n, 40);
    assert_eq!(
        across,
        plain[CHUNK_SIZE as usize - 8..CHUNK_SIZE as usize + 32]
    );
    file.seek(SeekFrom::End(0)).await.unwrap();
    let mut rest = [0; 4];
    assert_eq!(file.read(&mut rest).await.unwrap(), 0);
}
