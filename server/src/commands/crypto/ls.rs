use anyhow::{Context, bail};

use super::args::{key_from, parse_args};
use crate::{config::Config, storage};

pub async fn ls(args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let (dir_path, password) = parse_args(args, USAGE)?;
    let key = key_from(password)?;

    let config = Config::from_env().context("loading config from environment")?;
    let path = storage::resolve(&config.storage, &dir_path)
        .await
        .with_context(|| format!("{dir_path} in STORAGE"))?;
    let meta = tokio::fs::metadata(&path)
        .await
        .with_context(|| format!("{dir_path} in STORAGE"))?;
    if !meta.is_dir() {
        bail!("{dir_path} is not a directory");
    }

    let mut rows = Vec::new();
    let mut entries = tokio::fs::read_dir(&path)
        .await
        .with_context(|| format!("reading {dir_path}"))?;
    while let Some(entry) = entries.next_entry().await? {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(crypto::EXTENSION) else {
            continue;
        };
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let size = entry.metadata().await?.len();
        let stem = stem.to_string();
        let plain = crypto::decrypt_filename(&key, &name).unwrap_or_else(|_| stem.clone());
        rows.push(Row {
            name: plain,
            size,
            stem,
        });
    }
    rows.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.stem.cmp(&right.stem))
    });

    print!("{}", format_listing(&display_dir(&dir_path), &rows));
    Ok(())
}

const USAGE: &str = "usage: crypto ls [-p<password>] <dir_path>";

struct Row {
    name: String,
    size: u64,
    stem: String,
}

fn display_dir(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "./".to_string();
    }
    format!("./{trimmed}")
}

fn format_listing(dir: &str, rows: &[Row]) -> String {
    let mut out = format!("Encrypted files in {dir}:\n\n");
    let name_width = rows
        .iter()
        .map(|row| row.name.chars().count())
        .max()
        .unwrap_or(0);
    let size_width = rows
        .iter()
        .map(|row| format!("{}b", row.size).chars().count())
        .max()
        .unwrap_or(0);
    for row in rows {
        let size = format!("{}b", row.size);
        out.push_str(&format!(
            "- {:<name_width$}  {size:<size_width$}  {}\n",
            row.name, row.stem
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_storage_path_with_a_dot_slash() {
        assert_eq!(display_dir("crypto.nped"), "./crypto.nped");
        assert_eq!(display_dir("/crypto.nped/"), "./crypto.nped");
        assert_eq!(display_dir("/"), "./");
    }

    #[test]
    fn aligns_the_name_and_size_columns() {
        let text = format_listing(
            "./album",
            &[
                Row {
                    name: "1.txt".to_string(),
                    size: 121,
                    stem: "4h6356u467u467".to_string(),
                },
                Row {
                    name: "Hey.md".to_string(),
                    size: 5130,
                    stem: "g5h467j546j567k".to_string(),
                },
            ],
        );
        assert_eq!(
            text,
            "\
Encrypted files in ./album:

- 1.txt   121b   4h6356u467u467
- Hey.md  5130b  g5h467j546j567k
"
        );
    }
}
