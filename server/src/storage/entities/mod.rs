//! What lives under the `STORAGE` root: directories and files.

mod dir;
mod entry;
mod file;

pub use dir::Dir;
pub use entry::{Entry, EntryKind};
pub use file::File;
