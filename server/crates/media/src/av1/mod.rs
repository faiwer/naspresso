//! AV1, the codec inside AVIF files.

mod encode;

pub use encode::{Av1Image, BIT_DEPTH, EncodeError, encode, encode_alpha, encode_pq};
