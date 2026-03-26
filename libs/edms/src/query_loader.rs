use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct QueryConfig {
    pub help: String,
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryMap {
    pub endpoints: HashMap<String, QueryConfig>,
    pub requests: HashMap<String, QueryConfig>,
    pub responses: HashMap<String, QueryConfig>,
    pub tags: HashMap<String, QueryConfig>,
}

impl QueryMap {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let yaml_content = fs::read_to_string("queries.yaml")?;
        let query_map: QueryMap = serde_yaml::from_str(&yaml_content)?;
        Ok(query_map)
    }

    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|e| {
            eprintln!("[WARN] Failed to load queries.yaml: {e}");
            eprintln!("[WARN] Using embedded YAML as fallback");
            Self::load_embedded()
        })
    }

    fn load_embedded() -> Self {
        const EMBEDDED_YAML: &str = include_str!("queries.yaml");
        serde_yaml::from_str(EMBEDDED_YAML)
            .expect("Embedded queries.yaml is invalid - this is a compile-time bug!")
    }

    pub fn get_endpoint_query(&self, key: &str) -> Option<&str> {
        self.endpoints.get(key).map(|c| c.query.as_str())
    }

    pub fn get_request_query(&self, key: &str) -> Option<&str> {
        self.requests.get(key).map(|c| c.query.as_str())
    }

    pub fn get_response_query(&self, key: &str) -> Option<&str> {
        self.responses.get(key).map(|c| c.query.as_str())
    }

    pub fn get_tag_query(&self, key: &str) -> Option<&str> {
        self.tags.get(key).map(|c| c.query.as_str())
    }
}
