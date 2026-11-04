use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// Create the `.tmp` beside `path`. An existing final file is left untouched.
pub(super) async fn create_output(path: PathBuf) -> io::Result<tokio::fs::File> {
    if tokio::fs::try_exists(&path).await? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(tmp_path(&path))
        .await
}

pub(super) fn tmp_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path);
    name.push(".tmp");
    PathBuf::from(name)
}
