//! What a file is: its MIME type and, for images, size and HDR kind. Images
//! are recognized from their headers alone, in one open and usually one page.

mod bytes;
mod cicp;
mod heif;
mod jpeg;
mod mime;
mod png;
mod probe;
#[cfg(test)]
mod test_support;

pub(super) use mime::{SNIFF_LEN, sniff_mime};
pub(super) use probe::probe_file;
