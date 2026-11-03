//! Subcommands. The server starts when none is given.

use std::{io, path::PathBuf};

use anyhow::{Context, bail};

use crate::{config::Config, storage};

pub async fn encrypt(mut args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let Some(file_path) = args.next() else {
        bail!("usage: encrypt <file_path>");
    };
    if args.next().is_some() {
        bail!("usage: encrypt <file_path>");
    }

    let password = rpassword::prompt_password("Password: ").context("reading password")?;
    crypto::validate_password(&password)?;
    let key = crypto::prepare(&password)?;

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

    // The ciphertext does not use a key yet.
    let encrypted = crypto::encrypt(&path, &key, opened.file, {
        let directory = directory.clone();
        move |file_name| {
            let output = directory.join(file_name);
            async move { create_output(output).await }
        }
    })
    .await
    .with_context(|| format!("encrypting {file_path}"))?;

    println!(
        "{} {}",
        encrypted.size,
        directory.join(&encrypted.file_name).display()
    );
    Ok(())
}

/// Create `path` for writing. An existing file is left untouched.
async fn create_output(path: PathBuf) -> io::Result<tokio::fs::File> {
    if tokio::fs::try_exists(&path).await? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
}
