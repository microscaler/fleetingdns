use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CliConfig {
    pub api_url: String,
    pub hub_url: String,
    pub key_directory: PathBuf,
    pub default_ttl: u32,
    pub log_level: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.edf.run".to_string(),
            hub_url: "https://hub.edf.run".to_string(),
            key_directory: Self::get_default_key_directory(),
            default_ttl: 1800,
            log_level: "info".to_string(),
        }
    }
}

impl CliConfig {
    fn get_default_key_directory() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".edf")
            .join("keys")
    }
    
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Implement config file loading
        // For now, return default config
        Ok(Self::default())
    }
    
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement config file saving
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = CliConfig::default();
        assert_eq!(config.default_ttl, 1800);
        assert_eq!(config.log_level, "info");
    }
} 