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
    
  
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(core: Arc<EdmsCore>, queries: Arc<QueryMap>) -> Self {
        let (events_tx, _) = broadcast::channel(256);
        
        // Create a shared HTTP client with sensible defaults
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            core,
            queries,
            events_tx,
            active_folder: Arc::new(RwLock::new(None)),
            http_client,
        }
    }
    
    pub async fn emit(&self, evt: ServerEvent) {
        let _ = self.events_tx.send(evt);
    }
}
