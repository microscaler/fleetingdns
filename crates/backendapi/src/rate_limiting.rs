//! Rate limiting middleware for FleetingDNS API
//!
//! This module implements comprehensive rate limiting using the Tower middleware framework
//! with per-token rate tracking using DashMap for efficient concurrent access.

use crate::{ApiError};
use crate::models::UserTier;
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
};
use tracing::{debug, warn};

/// Rate limit configuration for different user tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Free tier API calls per minute
    pub free_tier_limit: u32,
    /// Pro tier API calls per minute
    pub pro_tier_limit: u32,
    /// Enterprise tier API calls per minute
    pub enterprise_tier_limit: u32,
    /// Default rate limit for unauthenticated requests
    pub default_limit: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            free_tier_limit: 60,     // 1 per second
            pro_tier_limit: 300,     // 5 per second
            enterprise_tier_limit: 600, // 10 per second
            default_limit: 20,       // Very low for unauthenticated
        }
    }
}

/// Rate limiting state manager
pub struct RateLimitState {
    config: RateLimitConfig,
    // Per-token API rate limiters
    api_limiters: DashMap<String, Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>>,
    // Per-token tunnel creation limiters
    tunnel_limiters: DashMap<String, Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>>,
    // User tier cache
    user_tiers: DashMap<String, UserTier>,
    // Bypass tokens for testing/admin
    bypass_tokens: DashMap<String, bool>,
}

impl RateLimitState {
    /// Create new rate limiting state
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            api_limiters: DashMap::new(),
            tunnel_limiters: DashMap::new(),
            user_tiers: DashMap::new(),
            bypass_tokens: DashMap::new(),
        }
    }

    /// Set user tier for a token
    pub fn set_user_tier(&self, token: &str, tier: UserTier) {
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

    /// Check if token has bypass privileges
    pub fn is_bypass_token(&self, token: &str) -> bool {
        self.bypass_tokens.contains_key(token)
    }

    /// Get user tier for a token
    pub fn get_user_tier(&self, token: &str) -> UserTier {
        self.user_tiers.get(token).map(|v| *v).unwrap_or(UserTier::Free)
    }

    /// Create API rate limiter for a user tier
    fn create_api_limiter(&self, tier: UserTier) -> Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>> {
        let limit = tier.api_rate_limit();
        let quota = Quota::per_minute(NonZeroU32::new(limit).unwrap());
        Arc::new(RateLimiter::keyed(quota))
    }

    /// Create tunnel rate limiter for a user tier  
    fn create_tunnel_limiter(&self, tier: UserTier) -> Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>> {
        let limit = tier.tunnel_creation_limit().min(60); // Max 60 per minute
        let quota = Quota::per_minute(NonZeroU32::new(limit).unwrap());
        Arc::new(RateLimiter::keyed(quota))
    }

    /// Check API rate limit for a token
    pub fn check_api_rate_limit(&self, token: &str) -> Result<(), String> {
        // Check bypass tokens first
        if self.is_bypass_token(token) {
            return Ok(());
        }

        let tier = self.get_user_tier(token);
        
        // Get or create limiter for this token
        let limiter = if let Some(existing) = self.api_limiters.get(token) {
            existing.clone()
        } else {
            let new_limiter = self.create_api_limiter(tier);
            self.api_limiters.insert(token.to_string(), new_limiter.clone());
            new_limiter
        };

        match limiter.check_key(&token.to_string()) {
            Ok(_) => Ok(()),
            Err(_negative) => {
                Err("Rate limit exceeded".to_string())
            }
        }
    }

    /// Check tunnel rate limit for a token
    pub fn check_tunnel_rate_limit(&self, token: &str) -> Result<(), String> {
        // Check bypass tokens first
        if self.is_bypass_token(token) {
            return Ok(());
        }

        let tier = self.get_user_tier(token);
        
        // Get or create limiter for this token
        let limiter = if let Some(existing) = self.tunnel_limiters.get(token) {
            existing.clone()
        } else {
            let new_limiter = self.create_tunnel_limiter(tier);
            self.tunnel_limiters.insert(token.to_string(), new_limiter.clone());
            new_limiter
        };

        match limiter.check_key(&token.to_string()) {
            Ok(_) => Ok(()),
            Err(_negative) => {
                Err("Tunnel creation rate limit exceeded".to_string())
            }
        }
    }

    /// Clean up old rate limiter entries to prevent memory leaks
    pub fn cleanup_old_entries(&self) {
        // Simple cleanup - remove entries for tokens not seen recently
        // In production, this would be more sophisticated
        if self.api_limiters.len() > 10000 {
            self.api_limiters.clear();
        }
        if self.tunnel_limiters.len() > 10000 {
            self.tunnel_limiters.clear();
        }
    }
}

/// Rate limiting middleware function
pub async fn rate_limit_middleware(
    State(state): State<Arc<RateLimitState>>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let headers = req.headers();
    let path = req.uri().path();
    
    // Extract API token from Authorization header
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("anonymous");

    // Determine which rate limit to check based on path
    let check_result = if path.contains("/tunnels") && req.method() == "POST" {
        // Tunnel creation endpoint
        state.check_tunnel_rate_limit(token)
    } else {
        // General API endpoint
        state.check_api_rate_limit(token)
    };

    match check_result {
        Ok(_) => {
            debug!(token = token, path = path, "Rate limit check passed");
            Ok(next.run(req).await)
        }
        Err(error_msg) => {
            warn!(token = token, path = path, error = error_msg, "Rate limit exceeded");
            
            let mut response = ApiError::RateLimitExceeded.into_response();
            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
            headers.insert("Retry-After", HeaderValue::from_static("60"));
            // Optionally, add the error message to the response body as JSON
            // (requires custom error response type if desired)
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
        assert_eq!(config.free_tier_limit, 60);
        assert_eq!(config.pro_tier_limit, 300);
        assert_eq!(config.enterprise_tier_limit, 600);
        assert_eq!(config.default_limit, 20);
    }

    #[test]
    fn test_user_tier_management() {
        let state = RateLimitState::new(RateLimitConfig::default());
        let token = "test-token";

        // Default should be free tier
        assert_eq!(state.get_user_tier(token), UserTier::Free);

        // Set to pro tier
        state.set_user_tier(token, UserTier::Pro);
        assert_eq!(state.get_user_tier(token), UserTier::Pro);

        state.set_user_tier("test-token", UserTier::Pro);
        assert_eq!(state.get_user_tier("test-token"), UserTier::Pro);

        assert_eq!(state.get_user_tier("admin-token"), UserTier::Free);
        state.add_bypass_token("admin-token");
        assert!(state.is_bypass_token("admin-token"));
        assert_eq!(state.get_user_tier("admin-token"), UserTier::Free); // Bypass overrides tier
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
        
        assert_eq!(state.get_user_tier("test-token"), UserTier::Free);
        assert_eq!(state.get_user_tier("admin-token"), UserTier::Free); // Default to Free

        state.set_user_tier("test-token", UserTier::Pro);
        assert_eq!(state.get_user_tier("test-token"), UserTier::Pro);

        assert_eq!(state.get_user_tier("admin-token"), UserTier::Free);
        state.add_bypass_token("admin-token");
        assert!(state.is_bypass_token("admin-token"));
        assert_eq!(state.get_user_tier("admin-token"), UserTier::Free); // Bypass overrides tier
    }
} 