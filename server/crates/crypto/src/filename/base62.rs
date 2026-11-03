use super::error::FilenameError;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const RADIX: u64 = 62;
/// Bytes in one block. A shorter final block uses [`WIDTH`].
const BLOCK: usize = 8;
/// Base62 digits for a block of `0..=8` bytes. Each entry is the smallest width that can hold `256^k` values.
const WIDTH: [usize; BLOCK + 1] = [0, 2, 3, 5, 6, 7, 9, 10, 11];
const FULL_WIDTH: usize = WIDTH[BLOCK];

pub(crate) fn index(symbol: u8) -> Option<u8> {
    let index = match symbol {
        b'0'..=b'9' => symbol - b'0',
        b'A'..=b'Z' => symbol - b'A' + 10,
        b'a'..=b'z' => symbol - b'a' + 36,
        _ => return None,
    };
    Some(index)
}

pub(crate) fn symbol(index: u8) -> Option<char> {
    ALPHABET
        .get(usize::from(index))
        .map(|byte| char::from(*byte))
}

/// Encode `bytes` as fixed-width Base62 blocks of [`BLOCK`] bytes.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(BLOCK) * FULL_WIDTH);
    for chunk in bytes.chunks(BLOCK) {
        write_block(&mut out, block_value(chunk), WIDTH[chunk.len()]);
    }
    out
}

/// Invert [`encode`]. A block value that does not fit its byte length is [`FilenameError::Length`].
pub(crate) fn decode(text: &str) -> Result<Vec<u8>, FilenameError> {
    let digits = text.as_bytes();
    let tail_width = digits.len() % FULL_WIDTH;
    let Some(tail_len) = block_len(tail_width) else {
        return Err(FilenameError::Length);
    };
    let full = digits.len() / FULL_WIDTH;
    let mut out = Vec::with_capacity(full * BLOCK + tail_len);
    let split = full * FULL_WIDTH;
    for chunk in digits[..split].chunks(FULL_WIDTH) {
        out.extend_from_slice(&decode_value(chunk)?.to_be_bytes());
    }
    if tail_len > 0 {
        let value = decode_value(&digits[split..])?;
        let limit = 1u64 << (8 * tail_len);
        if value >= limit {
            return Err(FilenameError::Length);
        }
        let bytes = value.to_be_bytes();
        out.extend_from_slice(&bytes[BLOCK - tail_len..]);
    }
    Ok(out)
}

fn block_value(bytes: &[u8]) -> u64 {
    let mut value = 0u64;
    for &byte in bytes {
        value = (value << 8) | u64::from(byte);
    }
    value
}

fn write_block(out: &mut String, mut value: u64, width: usize) {
    let mut chars = ['0'; FULL_WIDTH];
    for slot in (0..width).rev() {
        let digit = u8::try_from(value % RADIX).expect("base62 digit fits");
        chars[slot] = symbol(digit).expect("base62 index");
        value /= RADIX;
    }
    debug_assert_eq!(value, 0);
    out.extend(chars[..width].iter().copied());
}

fn decode_value(digits: &[u8]) -> Result<u64, FilenameError> {
    let mut value = 0u64;
    for &symbol in digits {
        let digit = index(symbol).ok_or(FilenameError::Alphabet)?;
        value = value
            .checked_mul(RADIX)
            .and_then(|value| value.checked_add(u64::from(digit)))
            .ok_or(FilenameError::Length)?;
    }
    Ok(value)
}

fn block_len(width: usize) -> Option<usize> {
    WIDTH.iter().position(|&entry| entry == width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_single_byte() {
        assert_eq!(encode(b"a"), "1Z");
        assert_eq!(decode("1Z").unwrap(), b"a");
    }

    #[test]
    fn keeps_leading_zero_bytes() {
        assert_eq!(encode(&[0x00, b'a']), "01Z");
        assert_eq!(decode("01Z").unwrap(), [0x00, b'a']);
    }

    #[test]
    fn round_trips_every_final_block_size() {
        assert_eq!(encode(&[]), "");
        assert_eq!(decode("").unwrap(), b"");
        for final_len in 1..=BLOCK {
            for full_blocks in 0..3 {
                let len = full_blocks * BLOCK + final_len;
                let bytes: Vec<u8> = (0..len).map(|index| u8::MAX - index as u8).collect();
                assert_eq!(decode(&encode(&bytes)).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn round_trips_utf8() {
        for name in ["a", "1.txt", "你好世界.txt", "😀🚀🔥.txt", "e\u{301}.txt"] {
            let encoded = encode(name.as_bytes());
            assert_eq!(decode(&encoded).unwrap(), name.as_bytes());
        }
    }

    #[test]
    fn rejects_invalid_blocks() {
        for text in ["0", "0000", "00000000", &"0".repeat(12)] {
            assert_eq!(decode(text), Err(FilenameError::Length));
        }
        assert_eq!(decode("1Z!"), Err(FilenameError::Alphabet));
        assert_eq!(decode(&"z".repeat(FULL_WIDTH)), Err(FilenameError::Length));
        assert_eq!(
            decode(&increment(&encode(&[0xff; BLOCK]))),
            Err(FilenameError::Length)
        );

        for len in 1..BLOCK {
            let limit = 1u64 << (8 * len);
            let mut too_big = String::new();
            write_block(&mut too_big, limit, WIDTH[len]);
            assert_eq!(decode(&too_big), Err(FilenameError::Length));

            let mut max = String::new();
            write_block(&mut max, limit - 1, WIDTH[len]);
            let encoded = (limit - 1).to_be_bytes();
            assert_eq!(decode(&max).unwrap(), encoded[BLOCK - len..]);
        }
    }

    fn increment(text: &str) -> String {
        let mut digits = text.as_bytes().to_vec();
        for byte in digits.iter_mut().rev() {
            let digit = index(*byte).expect("test digit");
            if digit < 61 {
                *byte = ALPHABET[usize::from(digit + 1)];
                return String::from_utf8(digits).expect("ascii");
            }
            *byte = b'0';
        }
        panic!("carry escaped the block");
    }
}
