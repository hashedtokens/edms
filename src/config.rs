use serde::Deserialize;


pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

pub struct LimitsConfig {
    pub max_endpoints: usize,
    pub max_requests_per_minute: usize,
    pub max_response_size_bytes: usize,
}

pub struct AppConfig {
    pub server: ServerConfig,
    pub limits: LimitsConfig,
}

impl AppConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let cfg: AppConfig = serde_yaml::from_str(&contents)?;
        Ok(cfg)
    }
}
