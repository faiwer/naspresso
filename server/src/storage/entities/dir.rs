use chrono::{
    DateTime, Utc,
    serde::{ts_milliseconds, ts_milliseconds_option},
};
use serde::Serialize;

#[derive(Serialize)]
pub struct Dir {
    pub name: String,
    #[serde(with = "ts_milliseconds")]
    pub modified_at: DateTime<Utc>,
    /// `None` when the filesystem doesn't record it.
    #[serde(with = "ts_milliseconds_option")]
    pub created_at: Option<DateTime<Utc>>,
}
