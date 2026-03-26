use serde::Deserialize;


#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LimitsConfig {
    pub max_endpoints: usize,
    pub max_requests_per_minute: usize,
    pub max_response_size_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
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
    
    
    pub fn from_file_or_default(path: &str) -> Self {
        Self::from_file(path).unwrap_or_else(|_| Self::default())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
            },
            limits: LimitsConfig {
                max_endpoints: 1000,
                max_requests_per_minute: 100,
                max_response_size_bytes: 10 * 1024 * 1024, // 10 MB
            },
        }
    }
}
```
