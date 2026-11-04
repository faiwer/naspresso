use std::{
    env,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use thiserror::Error;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Debug, Clone)]
pub struct Config {
    /// Interface to listen on. `HOST` env var; an IP or a hostname like `localhost`.
    pub host: String,
    /// `PORT` env var.
    pub port: u16,
    /// Public mount of the API, used as the cookie `Path`. `API_PREFIX` env
    /// var. A trailing slash is dropped, except when the prefix is `/`.
    pub api_prefix: String,
    /// Key that seals an unlocked password key into the `npe.` cookie.
    /// `NPE_SEAL_KEY` env var: standard base64 of exactly 32 bytes.
    pub npe_seal_key: NpeSealKey,
    /// Root directory served by `/storage/*`. `STORAGE` env var. A relative
    /// path is resolved against the `server/` directory, where `.env` lives.
    /// Canonicalized at startup, so it's absolute and known to be an existing
    /// directory.
    pub storage: PathBuf,
    /// Where derived files, like thumbnails, are kept. `CACHE` env var. Resolved
    /// like `storage`, but created at startup if missing: it's disposable.
    pub cache: PathBuf,
    /// AVIF quality of previews, `1..=100`. `PREVIEW_QUALITY` env var.
    pub preview_quality: f32,
    /// AVIF encoding speed of previews, `1..=10`, slowest to fastest.
    /// `PREVIEW_SPEED` env var.
    pub preview_speed: u8,
    /// How many previews may render at once; the rest wait in order.
    /// `PREVIEW_WORKERS` env var.
    pub preview_workers: NonZeroUsize,
}

/// 32-byte server key. `Debug` redacts it.
#[derive(Clone)]
pub struct NpeSealKey([u8; 32]);

impl NpeSealKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for NpeSealKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NpeSealKey(..)")
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid value for {var}: {source}")]
    InvalidValue {
        var: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("required env var {0} is not set")]
    Missing(&'static str),

    #[error("{var} must point to an existing directory, got {path:?}")]
    NotADirectory { var: &'static str, path: PathBuf },
}

impl Config {
    /// Read the config from the environment, after loading `server/.env` into it.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Resolved at compile time so the dev `.env` is found from any working directory.
        // A missing file is fine: deploys inject env vars directly. Variables already
        // set in the environment win over the file.
        let _ = dotenvy::from_filename(Path::new(MANIFEST_DIR).join(".env"));

        Ok(Self {
            host: parse_env("HOST")?,
            port: parse_env("PORT")?,
            api_prefix: api_prefix(&parse_env::<String>("API_PREFIX")?)?,
            npe_seal_key: npe_seal_key(&parse_env::<String>("NPE_SEAL_KEY")?)?,
            storage: parse_dir_env("STORAGE")?,
            cache: parse_created_dir_env("CACHE")?,
            preview_quality: parse_ranged_env("PREVIEW_QUALITY", 1.0, 100.0)?,
            preview_speed: parse_ranged_env("PREVIEW_SPEED", 1, 10)?,
            preview_workers: parse_env("PREVIEW_WORKERS")?,
        })
    }
}

/// Like [`parse_env`], but only `min..=max` is valid.
fn parse_ranged_env<T>(var: &'static str, min: T, max: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let value = parse_env::<T>(var)?;
    if !(min..=max).contains(&value) {
        return Err(ConfigError::InvalidValue {
            var,
            source: format!("must be {min} to {max}, got {value}").into(),
        });
    }
    Ok(value)
}

fn api_prefix(raw: &str) -> Result<String, ConfigError> {
    let trimmed = raw.trim();
    let invalid = || ConfigError::InvalidValue {
        var: "API_PREFIX",
        source: "must be an absolute path".into(),
    };
    if !trimmed.starts_with('/')
        || trimmed
            .chars()
            .any(|char| char.is_whitespace() || char == ';' || char.is_control())
    {
        return Err(invalid());
    }
    if trimmed == "/" {
        return Ok(trimmed.to_owned());
    }
    Ok(trimmed.trim_end_matches('/').to_owned())
}

fn npe_seal_key(raw: &str) -> Result<NpeSealKey, ConfigError> {
    let bytes = STANDARD
        .decode(raw.trim())
        .map_err(|error| ConfigError::InvalidValue {
            var: "NPE_SEAL_KEY",
            source: Box::new(error),
        })?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| ConfigError::InvalidValue {
            var: "NPE_SEAL_KEY",
            source: format!("must decode to 32 bytes, got {}", bytes.len()).into(),
        })?;
    Ok(NpeSealKey(bytes))
}

/// Read and parse a required env var. Unset and blank both count as missing.
fn parse_env<T>(var: &'static str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw = env::var(var)
        .ok()
        .filter(|raw| !raw.trim().is_empty())
        .ok_or(ConfigError::Missing(var))?;
    raw.trim()
        .parse::<T>()
        .map_err(|e| ConfigError::InvalidValue {
            var,
            source: Box::new(e),
        })
}

/// Read a required directory path, resolve it against `server/`, and canonicalize it.
fn parse_dir_env(var: &'static str) -> Result<PathBuf, ConfigError> {
    let path = Path::new(MANIFEST_DIR).join(parse_env::<PathBuf>(var)?);
    let canonical = path.canonicalize().map_err(|e| ConfigError::InvalidValue {
        var,
        source: Box::new(e),
    })?;
    if !canonical.is_dir() {
        return Err(ConfigError::NotADirectory {
            var,
            path: canonical,
        });
    }

    Ok(canonical)
}

/// Like [`parse_dir_env`], but creates the directory first.
fn parse_created_dir_env(var: &'static str) -> Result<PathBuf, ConfigError> {
    let path = Path::new(MANIFEST_DIR).join(parse_env::<PathBuf>(var)?);
    std::fs::create_dir_all(&path).map_err(|e| ConfigError::InvalidValue {
        var,
        source: Box::new(e),
    })?;
    parse_dir_env(var)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_absolute_prefix_and_drops_a_trailing_slash() {
        assert_eq!(api_prefix("/api").unwrap(), "/api");
        assert_eq!(api_prefix("/api/").unwrap(), "/api");
        assert_eq!(api_prefix("/").unwrap(), "/");
    }

    #[test]
    fn rejects_a_prefix_that_is_not_an_absolute_path() {
        assert!(api_prefix("api").is_err());
        assert!(api_prefix("").is_err());
        assert!(api_prefix("/api;").is_err());
    }

    #[test]
    fn accepts_32_bytes_of_standard_base64() {
        let key = npe_seal_key("AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=").unwrap();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn rejects_a_seal_key_of_the_wrong_length() {
        assert!(npe_seal_key("YQ==").is_err());
        assert!(npe_seal_key("not base64").is_err());
    }
}
