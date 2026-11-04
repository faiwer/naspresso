use anyhow::bail;

use super::{decrypt, encrypt, ls};

const USAGE: &str = "usage: crypto <encrypt|decrypt|ls> [-p<password>] <path>";

pub async fn run(mut args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    match args.next().as_deref() {
        Some("encrypt") => encrypt::encrypt(args).await,
        Some("decrypt") => decrypt::decrypt(args).await,
        Some("ls") => ls::ls(args).await,
        _ => bail!("{USAGE}"),
    }
}
