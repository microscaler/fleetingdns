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
pub use cache::{CacheError, CacheResult, RedisPool, get_slot, set_slot, del_slot, new_pool};
pub use client::{
    MonitoringConfig, PerformanceConfig, PerformanceError, PipelineConfig, PoolConfig,
    RedisPerformanceClient, PerformanceStats, PipelineStats, PoolStats
};
pub use tunnel::{get_tunnel_by_subdomain, store_tunnel_data, store_user_tunnel_lookup}; 