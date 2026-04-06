use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::fs::File;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub repo_path: String,
    pub per_md_file: usize,
}

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }


    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<AppConfig, Box<dyn std::error::Error + Send + Sync>> {
        let file = File::open(path)?;
        let config: AppConfig = serde_yaml::from_reader(file)?;
        Ok(config)
    }

}
