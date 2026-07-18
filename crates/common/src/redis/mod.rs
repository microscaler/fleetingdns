//! Redis Module
//!
//! This module provides unified Redis functionality for FleetingDNS including:
//! - Cache operations (get_slot, set_slot, del_slot)
//! - High-performance client with monitoring
//! - Tunnel-specific operations
//! - Authentication and session management

pub mod auth;
pub mod cache;
pub mod client;
pub mod tunnel;

// Re-export commonly used types and functions for convenience
pub use auth::RedisAuthHandler;
pub use cache::{CacheError, CacheResult, RedisPool, del_slot, get_slot, new_pool, set_slot};
pub use client::{
    MonitoringConfig, PerformanceConfig, PerformanceError, PerformanceStats, PipelineConfig,
    PipelineStats, PoolConfig, PoolStats, RedisPerformanceClient,
};
pub use tunnel::{
    SESSION_COOKIE_NAME, TeardownPolicy, check_session_grant, get_tunnel_by_slot,
    get_tunnel_by_subdomain, session_grant_key, store_tunnel_data, store_user_tunnel_lookup,
};
