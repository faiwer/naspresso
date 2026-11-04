use anyhow::bail;

use super::{encrypt, ls};

const USAGE: &str = "usage: crypto <encrypt|ls> [-p<password>] <path>";

pub async fn run(mut args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    match args.next().as_deref() {
        Some("encrypt") => encrypt::encrypt(args).await,
        Some("ls") => ls::ls(args).await,
        _ => bail!("{USAGE}"),
    }
}
