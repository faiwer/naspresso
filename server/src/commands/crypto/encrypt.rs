use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};

use super::args::{key_from, parse_args};
use crate::{config::Config, storage};

pub async fn encrypt(args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let (file_path, password) = parse_args(args, USAGE)?;
    let key = key_from(password)?;

    let config = Config::from_env().context("loading config from environment")?;
    // A storage-relative path, same as `/storage/*`: inside `STORAGE`, no `..`,
    // and every component already exists.
    let path = storage::resolve(&config.storage, &file_path)
        .await
        .with_context(|| format!("{file_path} in STORAGE"))?;
    let opened = storage::open_file(&path)
        .await
        .with_context(|| format!("{file_path} in STORAGE"))?;
    let Some(directory) = path.parent().map(PathBuf::from) else {
        bail!("{file_path} has no directory");
    };

    let encrypted = crypto::encrypt(&path, &key, opened.file, {
        let directory = directory.clone();
        move |file_name| {
            let output = directory.join(file_name);
            async move { create_output(output).await }
        }
    })
    .await
    .with_context(|| format!("encrypting {file_path}"))?;

    let dest = directory.join(&encrypted.file_name);
    tokio::fs::rename(tmp_path(&dest), &dest)
        .await
        .with_context(|| format!("renaming {}", dest.display()))?;

    println!("{}", directory.join(&encrypted.file_name).display());
    Ok(())
}

const USAGE: &str = "usage: crypto encrypt [-p<password>] <file_path>";

/// Create the `.tmp` beside `path`. An existing final file is left untouched.
async fn create_output(path: PathBuf) -> io::Result<tokio::fs::File> {
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

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path);
    name.push(".tmp");
    PathBuf::from(name)
}
