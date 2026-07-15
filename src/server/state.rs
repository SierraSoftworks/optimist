use std::sync::Arc;

use tokio::sync::RwLock;

use crate::project::ProjectCatalog;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) catalog: Arc<RwLock<ProjectCatalog>>,
}

impl AppState {
    pub(super) fn new(catalog: ProjectCatalog) -> Self {
        Self {
            catalog: Arc::new(RwLock::new(catalog)),
        }
    }
}
