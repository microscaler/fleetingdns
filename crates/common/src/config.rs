use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;

/// Global configuration for all FleetingDNS services
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetingDnsConfig {
    /// Redis configuration
    pub redis: RedisConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// DNS server configuration
    pub dns: DnsConfig,
    /// API server configuration
    pub api: ApiConfig,
    /// EdgeHub configuration
    pub edgehub: EdgeHubConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Metrics configuration
    pub metrics: MetricsConfig,
}

// impl Default for FleetingDnsConfig {
//     fn default() -> Self {
//         Self {
//             redis: RedisConfig::default(),
//             database: DatabaseConfig::default(),
//             dns: DnsConfig::default(),
//             api: ApiConfig::default(),
//             edgehub: EdgeHubConfig::default(),
//             logging: LoggingConfig::default(),
//             metrics: MetricsConfig::default(),
//         }
//     }
// }

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            pool_size: env::var("REDIS_POOL_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            timeout_secs: env::var("REDIS_TIMEOUT_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://fdns:fdns@localhost:5432/fdns".to_string()),
            pool_size: env::var("DATABASE_POOL_SIZE")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            timeout_secs: env::var("DATABASE_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
        }
    }
}

/// DNS server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// DNS server bind address
    pub bind_addr: String,
    /// DNS server port
    pub port: u16,
    /// Enable DNSSEC signing
    pub enable_dnssec: bool,
    /// Enable DDoS protection
    pub enable_ddos_protection: bool,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Maximum cache size
    pub max_cache_size: usize,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            bind_addr: env::var("DNS_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("DNS_PORT")
                .unwrap_or_else(|_| "6353".to_string())
                .parse()
                .unwrap_or(6353),
            enable_dnssec: env::var("DNS_ENABLE_DNSSEC")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_ddos_protection: env::var("DNS_ENABLE_DDOS_PROTECTION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            cache_ttl: env::var("DNS_CACHE_TTL")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            max_cache_size: env::var("DNS_MAX_CACHE_SIZE")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .unwrap_or(5000),
        }
    }
}

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API server bind address
    pub bind_addr: String,
    /// API server port
    pub port: u16,
    /// Enable CORS
    pub enable_cors: bool,
    /// Rate limiting requests per minute
    pub rate_limit_per_minute: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("API_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            enable_cors: env::var("API_ENABLE_CORS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            rate_limit_per_minute: env::var("API_RATE_LIMIT_PER_MINUTE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
        }
    }
}

/// EdgeHub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeHubConfig {
    /// EdgeHub bind address
    pub bind_addr: String,
    /// EdgeHub port
    pub port: u16,
    /// SSH key path
    pub ssh_key_path: Option<String>,
    /// Enable certificate validation
    pub enable_cert_validation: bool,
}

impl Default for EdgeHubConfig {
    fn default() -> Self {
        Self {
            bind_addr: env::var("EDGEHUB_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("EDGEHUB_PORT")
                .unwrap_or_else(|_| "2222".to_string())
                .parse()
                .unwrap_or(2222),
            ssh_key_path: env::var("EDGEHUB_SSH_KEY_PATH").ok(),
            enable_cert_validation: env::var("EDGEHUB_ENABLE_CERT_VALIDATION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Enable structured logging
    pub structured: bool,
    /// Log format (json, text)
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            structured: env::var("LOG_STRUCTURED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            format: env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string()),
        }
    }
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics endpoint
    pub endpoint: Option<String>,
    /// Metrics port
    pub port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: env::var("METRICS_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            endpoint: env::var("METRICS_ENDPOINT").ok(),
            port: env::var("METRICS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()
                .unwrap_or(9090),
        }
    }
}

impl FleetingDnsConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Get DNS server address as SocketAddr
    pub fn dns_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.dns.bind_addr, self.dns.port);
        Ok(addr.parse()?)
    }

    /// Get API server address as SocketAddr
    pub fn api_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.api.bind_addr, self.api.port);
        Ok(addr.parse()?)
    }

    /// Get EdgeHub address as SocketAddr
    pub fn edgehub_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.edgehub.bind_addr, self.edgehub.port);
        Ok(addr.parse()?)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate DNS configuration
        if let Err(e) = self.dns_addr() {
            errors.push(format!("Invalid DNS address: {}", e));
        }

        // Validate API configuration
        if let Err(e) = self.api_addr() {
            errors.push(format!("Invalid API address: {}", e));
        }

        // Validate EdgeHub configuration
        if let Err(e) = self.edgehub_addr() {
            errors.push(format!("Invalid EdgeHub address: {}", e));
        }

        // Validate Redis URL
        if !self.redis.url.starts_with("redis://") {
            errors.push("Invalid Redis URL format".to_string());
        }

        // Validate Database URL
        if !self.database.url.starts_with("postgresql://") {
            errors.push("Invalid Database URL format".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FleetingDnsConfig::default();
        assert!(!config.redis.url.is_empty());
        assert!(!config.database.url.is_empty());
        assert!(!config.dns.bind_addr.is_empty());
        assert!(config.dns.port > 0);
    }

    #[test]
    fn test_config_validation() {
        let config = FleetingDnsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_dns_addr_parsing() {
        let config = FleetingDnsConfig::default();
        assert!(config.dns_addr().is_ok());
    }
}
