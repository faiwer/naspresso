//! Subcommands. The server starts when none is given.

use std::{io, path::PathBuf};

use anyhow::{Context, bail};

use crate::{config::Config, storage};

const USAGE: &str = "usage: encrypt [-p<password>] <file_path>";

pub async fn encrypt(args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let (file_path, password) = parse_encrypt_args(args)?;
    let password = match password {
        Some(password) => password,
        None => rpassword::prompt_password("Password: ").context("reading password")?,
    };
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

fn parse_encrypt_args(
    args: impl Iterator<Item = String>,
) -> anyhow::Result<(String, Option<String>)> {
    let mut file_path = None;
    let mut password = None;
    for arg in args {
        if let Some(value) = arg.strip_prefix("-p") {
            if password.is_some() {
                bail!(USAGE);
            }
            password = Some(value.to_string());
        } else if file_path.is_some() {
            bail!(USAGE);
        } else {
            file_path = Some(arg);
        }
    }
    file_path
        .ok_or_else(|| anyhow::anyhow!(USAGE))
        .map(|file_path| (file_path, password))
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

#[cfg(test)]
mod tests {
    use super::parse_encrypt_args;

    fn parse(args: &[&str]) -> anyhow::Result<(String, Option<String>)> {
        parse_encrypt_args(args.iter().map(|arg| (*arg).to_string()))
    }

    #[test]
    fn reads_a_glued_password() {
        let (path, password) = parse(&["-psecret", "note.txt"]).unwrap();
        assert_eq!(path, "note.txt");
        assert_eq!(password.as_deref(), Some("secret"));

        let (path, password) = parse(&["note.txt", "-psecret"]).unwrap();
        assert_eq!(path, "note.txt");
        assert_eq!(password.as_deref(), Some("secret"));
    }

    #[test]
    fn prompts_when_the_password_is_absent() {
        let (path, password) = parse(&["note.txt"]).unwrap();
        assert_eq!(path, "note.txt");
        assert_eq!(password, None);
    }

    #[test]
    fn rejects_a_bad_shape() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["-psecret"]).is_err());
        assert!(parse(&["a.txt", "b.txt"]).is_err());
        assert!(parse(&["-pone", "-ptwo", "a.txt"]).is_err());
    }
}
