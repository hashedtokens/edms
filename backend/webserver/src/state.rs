use crate::events::ServerEvent;
use edms::core::EdmsCore;
use edms::query_loader::QueryMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<EdmsCore>,
    pub queries: Arc<QueryMap>,
    pub events_tx: broadcast::Sender<ServerEvent>,
    pub active_folder: Arc<RwLock<Option<String>>>,
}

impl AppState {
    pub fn new(core: Arc<EdmsCore>, queries: Arc<QueryMap>) -> Self {
        let (events_tx, _) = broadcast::channel(256);

        Self {
            core,
            queries,
            events_tx,
            active_folder: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn emit(&self, evt: ServerEvent) {
        let _ = self.events_tx.send(evt);
    }
}