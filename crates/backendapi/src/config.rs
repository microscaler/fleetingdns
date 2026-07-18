use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;

/// Configuration for the FleetingDNS API server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Address to bind the API server
    pub bind_address: SocketAddr,

    /// GitHub OAuth client ID
    pub github_client_id: String,

    /// GitHub OAuth client secret
    pub github_client_secret: String,

    /// Redis URL for tunnel metadata storage
    pub redis_url: String,

    /// Base domain for tunnel subdomains
    pub base_domain: String,

    /// Default tunnel TTL in seconds
    pub default_tunnel_ttl: u64,

    /// Maximum tunnel TTL in seconds
    pub max_tunnel_ttl: u64,

    /// EdgeHub SSH server address
    pub edgehub_address: String,

    /// JWT secret for token signing
    pub jwt_secret: String,

    /// Database URL for PostgreSQL
    pub database_url: String,

    /// Base URL for the API (used for OAuth redirects)
    pub base_url: String,

    /// Development mode flag - bypasses authentication for testing
    pub development_mode: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            // 8880: 8080 is chronically contested on shared dev hosts.
            bind_address: "0.0.0.0:8880".parse().unwrap(),
            github_client_id: "your-github-client-id".to_string(),
            github_client_secret: "your-github-client-secret".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            base_domain: "fleetingdns.run".to_string(),
            default_tunnel_ttl: 1800, // 30 minutes
            max_tunnel_ttl: 7200,     // 2 hours
            edgehub_address: "edgehub.fleetingdns.com:443".to_string(),
            jwt_secret: "your-jwt-secret-key".to_string(),
            database_url: "postgres://postgres:postgres@localhost:5432/fleetingdns".to_string(),
            base_url: "http://localhost:8880".to_string(),
            development_mode: false,
        }
    }
}

impl ApiConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let bind_addr_str =
            env::var("API_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8880".to_string());

        let bind_address = bind_addr_str
            .parse()
            .map_err(|e| format!("Invalid API bind address '{}': {}", bind_addr_str, e))?;

        Ok(Self {
            bind_address,
            github_client_id: env::var("GITHUB_CLIENT_ID")
                .unwrap_or_else(|_| "your-github-client-id".to_string()),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| "your-github-client-secret".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            base_domain: env::var("BASE_DOMAIN").unwrap_or_else(|_| "fleetingdns.run".to_string()),
            default_tunnel_ttl: env::var("DEFAULT_TUNNEL_TTL")
                .unwrap_or_else(|_| "1800".to_string())
                .parse()?,
            max_tunnel_ttl: env::var("MAX_TUNNEL_TTL")
                .unwrap_or_else(|_| "7200".to_string())
                .parse()?,
            edgehub_address: env::var("EDGEHUB_ADDRESS")
                .unwrap_or_else(|_| "edgehub.fleetingdns.com:443".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-jwt-secret-key".to_string()),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/fleetingdns".to_string()
            }),
            base_url: env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8880".to_string()),
            development_mode: env::var("DEVELOPMENT_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.github_client_id == "your-github-client-id" {
            return Err("GitHub client ID not configured".to_string());
        }

        if self.github_client_secret == "your-github-client-secret" {
            return Err("GitHub client secret not configured".to_string());
        }

        if self.jwt_secret == "your-jwt-secret-key" {
            return Err("JWT secret not configured".to_string());
        }

        if self.default_tunnel_ttl > self.max_tunnel_ttl {
            return Err("Default TTL cannot be greater than max TTL".to_string());
        }

        Ok(())
    }
}
