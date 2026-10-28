use serde::Serialize;

use super::{Dir, File};

/// Anything a directory can contain.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Dir(Dir),
    File(File),
}

/// Declaration order is the listing order: directories first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Dir,
    File,
}

impl Entry {
    pub fn kind(&self) -> EntryKind {
        match self {
            Entry::Dir(_) => EntryKind::Dir,
            Entry::File(_) => EntryKind::File,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Entry::Dir(dir) => &dir.name,
            Entry::File(file) => &file.name,
        }
    }
}
