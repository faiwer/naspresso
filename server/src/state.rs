use std::sync::Arc;

use crate::{config::Config, preview::PreviewCache};

/// Shared by every handler through axum's `State` extractor. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub previews: Arc<PreviewCache>,
}
