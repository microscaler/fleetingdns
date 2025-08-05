use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;

/// Utility functions for environment variable parsing
mod env_utils {
    use std::env;
    use std::net::SocketAddr;
    use std::fmt::Display;

    /// Parse an environment variable with a default value
    pub fn parse_env<T>(key: &str, default: T) -> T
    where
        T: std::str::FromStr + Clone + Display,
    {
        env::var(key)
            .unwrap_or_else(|_| default.to_string())
            .parse()
            .unwrap_or(default)
    }

    /// Parse an environment variable as a string with a default value
    pub fn parse_env_str(key: &str, default: &str) -> String {
        env::var(key).unwrap_or_else(|_| default.to_string())
    }

    /// Parse an environment variable as an optional string
    pub fn parse_env_opt(key: &str) -> Option<String> {
        env::var(key).ok()
    }

    /// Parse an environment variable as a boolean with a default value
    pub fn parse_env_bool(key: &str, default: bool) -> bool {
        match env::var(key) {
            Ok(value) => {
                match value.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => true,
                    "false" | "0" | "no" | "off" => false,
                    _ => default,
                }
            }
            Err(_) => default,
        }
    }

    /// Parse an environment variable as a SocketAddr with a default value
    pub fn parse_env_socket_addr(key: &str, port_key: &str, default_addr: &str, default_port: u16) -> SocketAddr {
        let bind_addr_str = parse_env_str(key, default_addr);
        let port = parse_env(port_key, default_port);
        
        format!("{}:{}", bind_addr_str, port)
            .parse()
            .unwrap_or_else(|_| format!("{}:{}", default_addr, default_port).parse().unwrap())
    }
}

use env_utils::{parse_env, parse_env_bool, parse_env_str, parse_env_opt, parse_env_socket_addr};

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
            url: parse_env_str("REDIS_URL", "redis://localhost:6379"),
            pool_size: parse_env("REDIS_POOL_SIZE", 10u32),
            timeout_secs: parse_env("REDIS_TIMEOUT_SECS", 5u64),
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
            url: parse_env_str("DATABASE_URL", "postgresql://fdns:fdns@localhost:5432/fdns"),
            pool_size: parse_env("DATABASE_POOL_SIZE", 5u32),
            timeout_secs: parse_env("DATABASE_TIMEOUT_SECS", 10u64),
        }
    }
}

/// DNS server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// DNS server bind address
    pub bind_addr: SocketAddr,
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
            bind_addr: parse_env_socket_addr("DNS_BIND_ADDR", "DNS_PORT", "0.0.0.0", 6353),
            enable_dnssec: parse_env_bool("DNS_ENABLE_DNSSEC", true),
            enable_ddos_protection: parse_env_bool("DNS_ENABLE_DDOS_PROTECTION", true),
            cache_ttl: parse_env("DNS_CACHE_TTL", 300u64),
            max_cache_size: parse_env("DNS_MAX_CACHE_SIZE", 5000usize),
        }
    }
}

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API server bind address
    pub bind_addr: SocketAddr,
    /// Enable CORS
    pub enable_cors: bool,
    /// Rate limiting requests per minute
    pub rate_limit_per_minute: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: parse_env_socket_addr("API_BIND_ADDR", "API_PORT", "0.0.0.0", 8080),
            enable_cors: parse_env_bool("API_ENABLE_CORS", true),
            rate_limit_per_minute: parse_env("API_RATE_LIMIT_PER_MINUTE", 100u32),
        }
    }
}

/// EdgeHub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeHubConfig {
    /// EdgeHub bind address
    pub bind_addr: SocketAddr,
    /// SSH key path
    pub ssh_key_path: Option<String>,
    /// Enable certificate validation
    pub enable_cert_validation: bool,
}

impl Default for EdgeHubConfig {
    fn default() -> Self {
        Self {
            bind_addr: parse_env_socket_addr("EDGEHUB_BIND_ADDR", "EDGEHUB_PORT", "0.0.0.0", 2222),
            ssh_key_path: parse_env_opt("EDGEHUB_SSH_KEY_PATH"),
            enable_cert_validation: parse_env_bool("EDGEHUB_ENABLE_CERT_VALIDATION", true),
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
            level: parse_env_str("RUST_LOG", "info"),
            structured: parse_env_bool("LOG_STRUCTURED", true),
            format: parse_env_str("LOG_FORMAT", "json"),
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
            enabled: parse_env_bool("METRICS_ENABLED", true),
            endpoint: parse_env_opt("METRICS_ENDPOINT"),
            port: parse_env("METRICS_PORT", 9090u16),
        }
    }
}

impl FleetingDnsConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Get DNS server address as SocketAddr
    pub fn dns_addr(&self) -> SocketAddr {
        self.dns.bind_addr
    }

    /// Get API server address as SocketAddr
    pub fn api_addr(&self) -> SocketAddr {
        self.api.bind_addr
    }

    /// Get EdgeHub address as SocketAddr
    pub fn edgehub_addr(&self) -> SocketAddr {
        self.edgehub.bind_addr
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

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
        assert_eq!(config.dns.bind_addr.port(), 6353);
        assert_eq!(config.api.bind_addr.port(), 8080);
        assert_eq!(config.edgehub.bind_addr.port(), 2222);
    }

    #[test]
    fn test_config_validation() {
        let config = FleetingDnsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_dns_addr_parsing() {
        let config = FleetingDnsConfig::default();
        let dns_addr = config.dns_addr();
        assert_eq!(dns_addr.port(), 6353);
    }

    #[test]
    fn test_api_addr_parsing() {
        let config = FleetingDnsConfig::default();
        let api_addr = config.api_addr();
        assert_eq!(api_addr.port(), 8080);
    }

    #[test]
    fn test_edgehub_addr_parsing() {
        let config = FleetingDnsConfig::default();
        let edgehub_addr = config.edgehub_addr();
        assert_eq!(edgehub_addr.port(), 2222);
    }
}

#[cfg(test)]
mod env_utils_tests {
    use super::*;
    use std::env;

    #[test]
    fn test_parse_env_with_default() {
        // Test with existing environment variable
        unsafe { env::set_var("TEST_U32", "42"); }
        assert_eq!(parse_env("TEST_U32", 10u32), 42);
        
        // Test with non-existent environment variable
        unsafe { env::remove_var("TEST_NONEXISTENT"); }
        assert_eq!(parse_env("TEST_NONEXISTENT", 100u32), 100);
        
        // Test with invalid value (should fall back to default)
        unsafe { env::set_var("TEST_INVALID", "not_a_number"); }
        assert_eq!(parse_env("TEST_INVALID", 50u32), 50);
        
        // Cleanup
        unsafe {
            env::remove_var("TEST_U32");
            env::remove_var("TEST_INVALID");
        }
    }

    #[test]
    fn test_parse_env_str() {
        // Test with existing environment variable
        unsafe { env::set_var("TEST_STR", "custom_value"); }
        assert_eq!(parse_env_str("TEST_STR", "default"), "custom_value");
        
        // Test with non-existent environment variable
        unsafe { env::remove_var("TEST_STR_NONEXISTENT"); }
        assert_eq!(parse_env_str("TEST_STR_NONEXISTENT", "default"), "default");
        
        // Cleanup
        unsafe { env::remove_var("TEST_STR"); }
    }

    #[test]
    fn test_parse_env_opt() {
        // Test with existing environment variable
        unsafe { env::set_var("TEST_OPT", "some_value"); }
        assert_eq!(parse_env_opt("TEST_OPT"), Some("some_value".to_string()));
        
        // Test with non-existent environment variable
        unsafe { env::remove_var("TEST_OPT_NONEXISTENT"); }
        assert_eq!(parse_env_opt("TEST_OPT_NONEXISTENT"), None);
        
        // Cleanup
        unsafe { env::remove_var("TEST_OPT"); }
    }

    #[test]
    fn test_parse_env_bool() {
        // Test with true values
        unsafe { env::set_var("TEST_BOOL_TRUE", "true"); }
        assert_eq!(parse_env_bool("TEST_BOOL_TRUE", false), true);
        
        unsafe { env::set_var("TEST_BOOL_1", "1"); }
        assert_eq!(parse_env_bool("TEST_BOOL_1", false), true);
        
        // Test with false values
        unsafe { env::set_var("TEST_BOOL_FALSE", "false"); }
        assert_eq!(parse_env_bool("TEST_BOOL_FALSE", true), false);
        
        unsafe { env::set_var("TEST_BOOL_0", "0"); }
        assert_eq!(parse_env_bool("TEST_BOOL_0", true), false);
        
        // Test with non-existent environment variable
        unsafe { env::remove_var("TEST_BOOL_NONEXISTENT"); }
        assert_eq!(parse_env_bool("TEST_BOOL_NONEXISTENT", true), true);
        
        // Test with invalid value (should fall back to default)
        unsafe { env::set_var("TEST_BOOL_INVALID", "not_a_bool"); }
        assert_eq!(parse_env_bool("TEST_BOOL_INVALID", false), false);
        
        // Cleanup
        unsafe {
            env::remove_var("TEST_BOOL_TRUE");
            env::remove_var("TEST_BOOL_1");
            env::remove_var("TEST_BOOL_FALSE");
            env::remove_var("TEST_BOOL_0");
            env::remove_var("TEST_BOOL_INVALID");
        }
    }

    #[test]
    fn test_parse_env_socket_addr() {
        // Test with existing environment variables
        unsafe {
            env::set_var("TEST_ADDR", "127.0.0.1");
            env::set_var("TEST_PORT", "8080");
        }
        let addr = parse_env_socket_addr("TEST_ADDR", "TEST_PORT", "0.0.0.0", 3000);
        assert_eq!(addr.port(), 8080);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        
        // Test with non-existent environment variables
        unsafe {
            env::remove_var("TEST_ADDR_NONEXISTENT");
            env::remove_var("TEST_PORT_NONEXISTENT");
        }
        let addr = parse_env_socket_addr("TEST_ADDR_NONEXISTENT", "TEST_PORT_NONEXISTENT", "0.0.0.0", 3000);
        assert_eq!(addr.port(), 3000);
        assert_eq!(addr.ip().to_string(), "0.0.0.0");
        
        // Test with invalid port (should fall back to default)
        unsafe {
            env::set_var("TEST_ADDR_VALID", "127.0.0.1");
            env::set_var("TEST_PORT_INVALID", "not_a_port");
        }
        let addr = parse_env_socket_addr("TEST_ADDR_VALID", "TEST_PORT_INVALID", "0.0.0.0", 3000);
        assert_eq!(addr.port(), 3000);
        
        // Cleanup
        unsafe {
            env::remove_var("TEST_ADDR");
            env::remove_var("TEST_PORT");
            env::remove_var("TEST_ADDR_VALID");
            env::remove_var("TEST_PORT_INVALID");
        }
    }
}
