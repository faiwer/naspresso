use std::sync::Arc;

use crate::config::Config;

/// Shared by every handler through axum's `State` extractor. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
}
