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
pub use auth::{RedisAuthHandler, SessionData};
pub use cache::{
    CacheError, CacheResult, RedisPool, del_key, del_slot, get_slot, get_string, new_pool,
    set_slot, set_string_ex,
};
pub use client::{
    MonitoringConfig, PerformanceConfig, PerformanceError, PerformanceStats, PipelineConfig,
    PipelineStats, PoolConfig, PoolStats, RedisPerformanceClient,
};
pub use tunnel::{
    SESSION_COOKIE_NAME, TeardownPolicy, check_session_grant, clear_tunnel_live,
    get_tunnel_by_slot, get_tunnel_by_subdomain, is_tunnel_live, mark_tunnel_live,
    session_grant_key, store_tunnel_data, store_user_tunnel_lookup, tunnel_live_key,
};
