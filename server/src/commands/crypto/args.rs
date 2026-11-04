use anyhow::{Context, bail};

pub(super) fn parse_args(
    args: impl Iterator<Item = String>,
    usage: &str,
) -> anyhow::Result<(String, Option<String>)> {
    let mut path = None;
    let mut password = None;
    for arg in args {
        if let Some(value) = arg.strip_prefix("-p") {
            if password.is_some() {
                bail!("{usage}");
            }
            password = Some(value.to_string());
        } else if path.is_some() {
            bail!("{usage}");
        } else {
            path = Some(arg);
        }
    }
    let Some(path) = path else {
        bail!("{usage}");
    };
    Ok((path, password))
}

pub(super) fn key_from(password: Option<String>) -> anyhow::Result<crypto::Key> {
    let password = match password {
        Some(password) => password,
        None => rpassword::prompt_password("Password: ").context("reading password")?,
    };
    crypto::validate_password(&password)?;
    Ok(crypto::prepare(&password)?)
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    const USAGE: &str = "usage";

    fn parse(args: &[&str]) -> anyhow::Result<(String, Option<String>)> {
        parse_args(args.iter().map(|arg| (*arg).to_string()), USAGE)
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
