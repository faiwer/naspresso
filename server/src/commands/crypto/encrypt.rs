use std::path::PathBuf;

use anyhow::{Context, bail};

use super::{
    args::{key_from, parse_args},
    output::{create_output, tmp_path},
};

pub async fn encrypt(args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let (file_path, password) = parse_args(args, USAGE)?;
    let key = key_from(password)?;
    let path = PathBuf::from(&file_path);
    let file = tokio::fs::File::open(&path)
        .await
        .with_context(|| format!("opening {file_path}"))?;
    let Some(directory) = path.parent().map(PathBuf::from) else {
        bail!("{file_path} has no directory");
    };

    let encrypted = crypto::encrypt(&path, &key, file, {
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

    println!("{}", dest.display());
    Ok(())
}

const USAGE: &str = "usage: crypto encrypt [-p<password>] <file_path>";
