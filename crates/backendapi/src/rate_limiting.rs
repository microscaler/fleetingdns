//! Rate limiting middleware for FleetingDNS API
//!
//! This module implements comprehensive rate limiting using the Tower middleware framework
//! with per-token rate tracking using DashMap for efficient concurrent access.
//! Enhanced with user-specific quotas, burst handling, and DDoS protection.

use crate::{ApiError};
use axum::{
    extract::{Request, State},
    http::{HeaderValue},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::{
    num::NonZeroU32,
    sync::Arc,
    collections::HashMap,
    time::{Duration, Instant},
    net::IpAddr,
};
use tracing::{debug, warn, error, info};

/// Enhanced policy for rate limiting with burst handling and dynamic adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitPolicy {
    /// Allowed requests per minute
    pub requests_per_minute: u32,
    /// Burst capacity (max requests in a short window)
    pub burst: Option<u32>,
    /// Window size in seconds (default: 60)
    pub window_seconds: Option<u32>,
    /// Maximum request size in bytes
    pub max_request_size: Option<u64>,
    /// Connection limit per IP
    pub max_connections_per_ip: Option<u32>,
    /// Rate limit multiplier for premium users
    pub premium_multiplier: Option<f64>,
}

/// User tier configuration with different rate limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTier {
    pub name: String,
    pub api_requests_per_minute: u32,
    pub tunnel_requests_per_minute: u32,
    pub dns_requests_per_minute: u32,
    pub burst_multiplier: f64,
    pub max_concurrent_tunnels: u32,
    pub bypass_rate_limits: bool,
}

/// DDoS protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdosProtection {
    /// Maximum requests per minute per IP
    pub max_requests_per_ip_per_minute: u32,
    /// Maximum connections per IP
    pub max_connections_per_ip: u32,
    /// IP blocking duration in seconds
    pub ip_block_duration_seconds: u64,
    /// Automatic IP blocking for abuse patterns
    pub auto_block_abuse_patterns: bool,
}

/// Complete rate limit configuration with enhanced features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Default policy if no override is found
    pub default: RateLimitPolicy,
    /// Per-endpoint overrides (e.g., "/api/v1/tunnel")
    pub per_endpoint: Option<HashMap<String, RateLimitPolicy>>,
    /// User tier configurations
    pub user_tiers: HashMap<String, UserTier>,
    /// DDoS protection settings
    pub ddos_protection: DdosProtection,
    /// Dynamic rate limit adjustment based on system load
    pub dynamic_adjustment: bool,
    /// System load threshold for rate limit reduction (0.0-1.0)
    pub load_threshold: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut per_endpoint = HashMap::new();
        per_endpoint.insert("/api/v1/tunnel".to_string(), RateLimitPolicy {
            requests_per_minute: 60,
            burst: Some(10),
            window_seconds: Some(60),
            max_request_size: Some(1024 * 1024), // 1MB
            max_connections_per_ip: Some(10),
            premium_multiplier: Some(2.0),
        });

        let mut user_tiers = HashMap::new();
        user_tiers.insert("Free".to_string(), UserTier {
            name: "Free".to_string(),
            api_requests_per_minute: 60,
            tunnel_requests_per_minute: 10,
            dns_requests_per_minute: 100,
            burst_multiplier: 1.0,
            max_concurrent_tunnels: 1,
            bypass_rate_limits: false,
        });
        user_tiers.insert("Pro".to_string(), UserTier {
            name: "Pro".to_string(),
            api_requests_per_minute: 300,
            tunnel_requests_per_minute: 50,
            dns_requests_per_minute: 500,
            burst_multiplier: 2.0,
            max_concurrent_tunnels: 5,
            bypass_rate_limits: false,
        });
        user_tiers.insert("Enterprise".to_string(), UserTier {
            name: "Enterprise".to_string(),
            api_requests_per_minute: 1000,
            tunnel_requests_per_minute: 200,
            dns_requests_per_minute: 2000,
            burst_multiplier: 3.0,
            max_concurrent_tunnels: 20,
            bypass_rate_limits: true,
        });

        Self {
            default: RateLimitPolicy {
                requests_per_minute: 60,
                burst: Some(10),
                window_seconds: Some(60),
                max_request_size: Some(1024 * 1024),
                max_connections_per_ip: Some(10),
                premium_multiplier: Some(1.5),
            },
            per_endpoint: Some(per_endpoint),
            user_tiers,
            ddos_protection: DdosProtection {
                max_requests_per_ip_per_minute: 1000,
                max_connections_per_ip: 50,
                ip_block_duration_seconds: 3600, // 1 hour
                auto_block_abuse_patterns: true,
            },
            dynamic_adjustment: true,
            load_threshold: 0.8,
        }
    }
}

/// IP-based rate limiting for DDoS protection
#[derive(Debug)]
struct IpRateLimit {
    requests: u32,
    last_reset: Instant,
    blocked_until: Option<Instant>,
}

/// Enhanced rate limiting state manager with DDoS protection
pub struct RateLimitState {
    config: RateLimitConfig,
    // Per-token API rate limiters
    api_limiters: DashMap<String, Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>>,
    // Per-token tunnel creation limiters
    tunnel_limiters: DashMap<String, Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>>,
    // Per-token DNS operation limiters
    dns_limiters: DashMap<String, Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>>,
    // User tier cache
    user_tiers: DashMap<String, String>,
    // Bypass tokens for testing/admin
    bypass_tokens: DashMap<String, bool>,
    // IP-based rate limiting for DDoS protection
    ip_limiters: DashMap<IpAddr, IpRateLimit>,
    // System load tracking for dynamic adjustment
    system_load: Arc<dashmap::DashMap<String, f64>>,
    // Abuse pattern detection
    abuse_patterns: DashMap<String, u32>,
}

impl RateLimitState {
    /// Create new rate limiting state
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            api_limiters: DashMap::new(),
            tunnel_limiters: DashMap::new(),
            dns_limiters: DashMap::new(),
            user_tiers: DashMap::new(),
            bypass_tokens: DashMap::new(),
            ip_limiters: DashMap::new(),
            system_load: Arc::new(dashmap::DashMap::new()),
            abuse_patterns: DashMap::new(),
        }
    }

    /// Set user tier for a token
    pub fn set_user_tier(&self, token: &str, tier: String) {
        self.user_tiers.insert(token.to_string(), tier);
    }

    /// Add bypass token for testing/admin
    pub fn add_bypass_token(&self, token: &str) {
        self.bypass_tokens.insert(token.to_string(), true);
    }

    /// Remove bypass token
    pub fn remove_bypass_token(&self, token: &str) {
        self.bypass_tokens.remove(token);
    }

    /// Check if token is bypass token
    pub fn is_bypass_token(&self, token: &str) -> bool {
        self.bypass_tokens.contains_key(token)
    }

    /// Get user tier for token
    pub fn get_user_tier(&self, token: &str) -> String {
        self.user_tiers.get(token).map(|t| t.clone()).unwrap_or_else(|| "Free".to_string())
    }

    /// Update system load for dynamic rate limiting
    pub fn update_system_load(&self, load: f64) {
        self.system_load.insert("current".to_string(), load);
        info!("System load updated: {:.2}", load);
    }

    /// Get current system load
    pub fn get_system_load(&self) -> f64 {
        self.system_load.get("current").map(|l| *l).unwrap_or(0.0)
    }

    /// Check IP-based rate limiting for DDoS protection
    pub fn check_ip_rate_limit(&self, ip: IpAddr) -> Result<(), String> {
        let now = Instant::now();
        let mut ip_limit = self.ip_limiters.entry(ip).or_insert(IpRateLimit {
            requests: 0,
            last_reset: now,
            blocked_until: None,
        });

        // Check if IP is blocked
        if let Some(blocked_until) = ip_limit.blocked_until {
            if now < blocked_until {
                return Err("IP is temporarily blocked due to abuse".to_string());
            } else {
                ip_limit.blocked_until = None;
            }
        }

        // Reset counter if window has passed
        if now.duration_since(ip_limit.last_reset) > Duration::from_secs(60) {
            ip_limit.requests = 0;
            ip_limit.last_reset = now;
        }

        // Check rate limit
        if ip_limit.requests >= self.config.ddos_protection.max_requests_per_ip_per_minute {
            // Block IP for configured duration
            ip_limit.blocked_until = Some(now + Duration::from_secs(self.config.ddos_protection.ip_block_duration_seconds));
            return Err("IP rate limit exceeded".to_string());
        }

        ip_limit.requests += 1;
        Ok(())
    }

    /// Create API rate limiter with dynamic adjustment
    fn create_api_limiter(&self, tier: String) -> Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>> {
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        let mut requests_per_minute = tier_config.api_requests_per_minute as u32;
        
        // Apply dynamic adjustment based on system load
        if self.config.dynamic_adjustment {
            let load = self.get_system_load();
            if load > self.config.load_threshold {
                let reduction_factor = 1.0 - (load - self.config.load_threshold) * 0.5;
                requests_per_minute = (requests_per_minute as f64 * reduction_factor) as u32;
                debug!("Rate limit reduced due to high system load: {} -> {}", tier_config.api_requests_per_minute, requests_per_minute);
            }
        }

        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap());
        Arc::new(RateLimiter::keyed(quota))
    }

    /// Create tunnel rate limiter with burst handling
    fn create_tunnel_limiter(&self, tier: String) -> Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>> {
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        let requests_per_minute = tier_config.tunnel_requests_per_minute as u32;
        let burst_multiplier = tier_config.burst_multiplier;
        let burst_quota = (requests_per_minute as f64 * burst_multiplier) as u32;

        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap())
            .allow_burst(NonZeroU32::new(burst_quota).unwrap());
        Arc::new(RateLimiter::keyed(quota))
    }

    /// Create DNS rate limiter
    fn create_dns_limiter(&self, tier: String) -> Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>> {
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        let requests_per_minute = tier_config.dns_requests_per_minute as u32;
        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap());
        Arc::new(RateLimiter::keyed(quota))
    }

    /// Check API rate limit with enhanced features
    pub fn check_api_rate_limit(&self, token: &str) -> Result<(), String> {
        if self.is_bypass_token(token) {
            return Ok(());
        }

        let tier = self.get_user_tier(token);
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        if tier_config.bypass_rate_limits {
            return Ok(());
        }

        let limiter = self.api_limiters.entry(token.to_string()).or_insert_with(|| {
            self.create_api_limiter(tier.clone())
        });

        match limiter.check_key(&token.to_string()) {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("API rate limit exceeded for tier: {}", tier))
        }
    }

    /// Check tunnel creation rate limit with burst handling
    pub fn check_tunnel_rate_limit(&self, token: &str) -> Result<(), String> {
        if self.is_bypass_token(token) {
            return Ok(());
        }

        let tier = self.get_user_tier(token);
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        if tier_config.bypass_rate_limits {
            return Ok(());
        }

        let limiter = self.tunnel_limiters.entry(token.to_string()).or_insert_with(|| {
            self.create_tunnel_limiter(tier.clone())
        });

        match limiter.check_key(&token.to_string()) {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("Tunnel creation rate limit exceeded for tier: {}", tier))
        }
    }

    /// Check DNS operation rate limit
    pub fn check_dns_rate_limit(&self, token: &str) -> Result<(), String> {
        if self.is_bypass_token(token) {
            return Ok(());
        }

        let tier = self.get_user_tier(token);
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        if tier_config.bypass_rate_limits {
            return Ok(());
        }

        let limiter = self.dns_limiters.entry(token.to_string()).or_insert_with(|| {
            self.create_dns_limiter(tier.clone())
        });

        match limiter.check_key(&token.to_string()) {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("DNS operation rate limit exceeded for tier: {}", tier))
        }
    }

    /// Clean up old entries to prevent memory leaks
    pub fn cleanup_old_entries(&self) {
        // Clean up rate limiters if too many entries
        if self.api_limiters.len() > 10000 {
            self.api_limiters.clear();
        }
        if self.tunnel_limiters.len() > 10000 {
            self.tunnel_limiters.clear();
        }
        if self.dns_limiters.len() > 10000 {
            self.dns_limiters.clear();
        }

        // Clean up old IP limiters
        let now = Instant::now();
        self.ip_limiters.retain(|_, ip_limit| {
            now.duration_since(ip_limit.last_reset) < Duration::from_secs(3600) // Keep for 1 hour
        });

        // Clean up old abuse patterns
        self.abuse_patterns.retain(|_, count| *count > 0);
    }

    /// Get rate limit information for a token
    pub fn get_rate_limit_info(&self, token: &str) -> HashMap<String, String> {
        let tier = self.get_user_tier(token);
        let tier_config = self.config.user_tiers.get(&tier).unwrap_or_else(|| {
            self.config.user_tiers.get("Free").unwrap()
        });

        let mut info = HashMap::new();
        info.insert("tier".to_string(), tier);
        info.insert("api_requests_per_minute".to_string(), tier_config.api_requests_per_minute.to_string());
        info.insert("tunnel_requests_per_minute".to_string(), tier_config.tunnel_requests_per_minute.to_string());
        info.insert("dns_requests_per_minute".to_string(), tier_config.dns_requests_per_minute.to_string());
        info.insert("max_concurrent_tunnels".to_string(), tier_config.max_concurrent_tunnels.to_string());
        info.insert("burst_multiplier".to_string(), tier_config.burst_multiplier.to_string());
        info.insert("bypass_rate_limits".to_string(), tier_config.bypass_rate_limits.to_string());
        info.insert("system_load".to_string(), self.get_system_load().to_string());

        info
    }
}

/// Enhanced rate limiting middleware with DDoS protection and multiple rate limit types
pub async fn rate_limit_middleware(
    State(state): State<Arc<RateLimitState>>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let headers = req.headers();
    let path = req.uri().path();
    let method = req.method().as_str();
    
    // Extract API token from Authorization header
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("anonymous");

    // Extract client IP for DDoS protection
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .and_then(|ip_str| ip_str.split(',').next())
        .and_then(|ip_str| ip_str.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| {
            let default_ip: IpAddr = "127.0.0.1".parse().unwrap();
            default_ip
        });

    // Check IP-based rate limiting for DDoS protection
    if let Err(ip_error) = state.check_ip_rate_limit(client_ip) {
        warn!(ip = ?client_ip, error = ip_error, "IP rate limit exceeded");
        let mut response = ApiError::RateLimitExceeded.into_response();
        let headers = response.headers_mut();
        headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
        headers.insert("Retry-After", HeaderValue::from_static("3600")); // 1 hour for IP blocks
        headers.insert("X-RateLimit-Reset", HeaderValue::from_static("3600"));
        return Err(response);
    }

    // Determine which rate limit to check based on path and method
    let check_result = if path.contains("/tunnels") && method == "POST" {
        // Tunnel creation endpoint
        state.check_tunnel_rate_limit(token)
    } else if path.contains("/dns") || path.contains("/domains") {
        // DNS operations
        state.check_dns_rate_limit(token)
    } else {
        // General API endpoint
        state.check_api_rate_limit(token)
    };

    match check_result {
        Ok(_) => {
            debug!(token = token, path = path, ip = ?client_ip, "Rate limit check passed");
            
            // Get rate limit info for response headers
            let rate_limit_info = state.get_rate_limit_info(token);
            let default_tier = "Free".to_string();
            let default_limit = "60".to_string();
            let tier = rate_limit_info.get("tier").unwrap_or(&default_tier);
            let api_limit = rate_limit_info.get("api_requests_per_minute").unwrap_or(&default_limit);
            
            // Run the request
            let mut response = next.run(req).await;
            
            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Tier", HeaderValue::from_str(tier).unwrap_or_else(|_| HeaderValue::from_static("Free")));
            headers.insert("X-RateLimit-Limit", HeaderValue::from_str(api_limit).unwrap_or_else(|_| HeaderValue::from_static("60")));
            headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("99")); // TODO: Calculate actual remaining
            headers.insert("X-RateLimit-Reset", HeaderValue::from_static("60"));
            
            Ok(response)
        }
        Err(error_msg) => {
            warn!(token = token, path = path, ip = ?client_ip, error = error_msg, "Rate limit exceeded");
            
            let mut response = ApiError::RateLimitExceeded.into_response();
            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
            headers.insert("Retry-After", HeaderValue::from_static("60"));
            headers.insert("X-RateLimit-Reset", HeaderValue::from_static("60"));
            
            // Add error details to response body
            let error_body = serde_json::json!({
                "error": "rate_limit_exceeded",
                "message": error_msg,
                "retry_after": 60
            });
            
            // TODO: Set response body with error details
            Err(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.default.requests_per_minute, 60);
        assert_eq!(config.per_endpoint.as_ref().unwrap().get("/api/v1/tunnel").map(|p| p.requests_per_minute), Some(60));
    }

    #[test]
    fn test_user_tier_management() {
        let state = RateLimitState::new(RateLimitConfig::default());
        let token = "test-token";

        // Default should be free tier
        assert_eq!(state.get_user_tier(token), "Free".to_string());

        // Set to pro tier
        state.set_user_tier(token, "Pro".to_string());
        assert_eq!(state.get_user_tier(token), "Pro".to_string());

        state.set_user_tier("test-token", "Pro".to_string());
        assert_eq!(state.get_user_tier("test-token"), "Pro".to_string());

        assert_eq!(state.get_user_tier("admin-token"), "Free".to_string());
        state.add_bypass_token("admin-token");
        assert!(state.is_bypass_token("admin-token"));
        assert_eq!(state.get_user_tier("admin-token"), "Free".to_string()); // Bypass overrides tier
    }

    #[test]
    fn test_bypass_token_check() {
        let state = RateLimitState::new(RateLimitConfig::default());
        let bypass_token = "admin-token";
        let regular_token = "regular-token";

        state.add_bypass_token(bypass_token);
        assert!(state.is_bypass_token(bypass_token));
        assert!(!state.is_bypass_token(regular_token));

        state.remove_bypass_token(bypass_token);
        assert!(!state.is_bypass_token(bypass_token));
    }

    #[test]
    fn test_tunnel_creation_detection() {
        let state = RateLimitState::new(RateLimitConfig::default());
        assert!(state.check_tunnel_rate_limit("test-token").is_ok());
        assert!(state.check_api_rate_limit("test-token").is_ok());
    }

    #[test]
    fn test_token_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer test-token".parse().unwrap());
        
        let token = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap();
        assert_eq!(token, "test-token");

        // Test with no header
        headers.clear();
        let token = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("anonymous");
        assert_eq!(token, "anonymous");

        // Test with malformed header
        headers.insert("authorization", "Basic user:pass".parse().unwrap());
        let token = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("anonymous");
        assert_eq!(token, "anonymous");
    }

    #[test]
    fn test_rate_limit_getters() {
        let state = RateLimitState::new(RateLimitConfig::default());
        
        assert_eq!(state.get_user_tier("test-token"), "Free".to_string());
        assert_eq!(state.get_user_tier("admin-token"), "Free".to_string()); // Default to Free

        state.set_user_tier("test-token", "Pro".to_string());
        assert_eq!(state.get_user_tier("test-token"), "Pro".to_string());

        assert_eq!(state.get_user_tier("admin-token"), "Free".to_string());
        state.add_bypass_token("admin-token");
        assert!(state.is_bypass_token("admin-token"));
        assert_eq!(state.get_user_tier("admin-token"), "Free".to_string()); // Bypass overrides tier
    }
} 