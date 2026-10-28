use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    /// Interface to listen on. `HOST` env var; an IP or a hostname like `localhost`.
    pub host: String,
    /// `PORT` env var.
    pub port: u16,
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
}

impl Config {
    /// Read the config from the environment, after loading `server/.env` into it.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Resolved at compile time so the dev `.env` is found from any working directory.
        // A missing file is fine: deploys inject env vars directly. Variables already
        // set in the environment win over the file.
        let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));

        Ok(Self {
            host: parse_env("HOST")?,
            port: parse_env("PORT")?,
        })
    }
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
