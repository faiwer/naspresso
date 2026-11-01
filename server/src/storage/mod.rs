//! Files under the `STORAGE` root. Knows nothing about HTTP.

mod entities;
mod error;
mod list;
mod mime;
mod path;
mod read;
mod xattr;

pub use entities::{Dir, Entry, EntryKind, File};
pub use error::StorageError;
pub use list::{EntryKey, list_dir};
pub use path::resolve;
pub use read::open_file;
pub(crate) use xattr::write_records;
