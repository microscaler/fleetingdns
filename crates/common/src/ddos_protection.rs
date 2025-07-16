//! DDoS protection and connection limiting utilities

use dashmap::DashMap;
use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, warn};
use serde::{Serialize, Deserialize};

/// DDoS protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdosConfig {
    /// Maximum connections per IP address
    pub max_connections_per_ip: u32,
    /// Connection rate limit per IP (connections per minute)
    pub connection_rate_per_minute: u32,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// IP blocking duration for abuse
    pub block_duration: Duration,
    /// Time window for connection rate tracking
    pub rate_window: Duration,
}

impl Default for DdosConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip: 10,
            connection_rate_per_minute: 100,
            max_request_size: 1024 * 1024, // 1MB
            block_duration: Duration::from_secs(300), // 5 minutes
            rate_window: Duration::from_secs(60), // 1 minute
        }
    }
}

/// Connection tracking information per IP
#[derive(Debug, Clone)]
struct IpConnectionInfo {
    /// Current active connections
    active_connections: u32,
    /// Recent connection timestamps for rate limiting
    recent_connections: Vec<Instant>,
    /// Block expiry time (if blocked)
    blocked_until: Option<Instant>,
}

/// DDoS protection state manager
#[derive(Debug, Clone)]
pub struct DdosProtection {
    config: DdosConfig,
    ip_info: Arc<DashMap<IpAddr, IpConnectionInfo>>,
}

impl DdosProtection {
    /// Create new DDoS protection with configuration
    pub fn new(config: DdosConfig) -> Self {
        Self {
            config,
            ip_info: Arc::new(DashMap::new()),
        }
    }

    /// Check if an IP address is currently blocked
    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        if let Some(info) = self.ip_info.get(&ip) 
            && info.blocked_until.is_some_and(|blocked_until| Instant::now() < blocked_until) 
        {
            debug!(ip = %ip, "IP is blocked");
            return true;
        }
        false
    }

    /// Check if a new connection from an IP should be allowed
    pub fn check_connection_limit(&self, ip: IpAddr) -> Result<(), String> {
        if self.is_blocked(ip) {
            return Err("IP address is blocked".to_string());
        }

        let now = Instant::now();
        let mut should_block = false;

        // Get or create IP info
        let mut info_ref = self.ip_info.entry(ip).or_insert_with(|| IpConnectionInfo {
            active_connections: 0,
            recent_connections: Vec::new(),
            blocked_until: None,
        });

        // Clean up old connection timestamps
        info_ref.recent_connections.retain(|&timestamp| {
            now.duration_since(timestamp) < self.config.rate_window
        });

        // Check connection rate limit
        if info_ref.recent_connections.len() >= self.config.connection_rate_per_minute as usize {
            warn!(
                ip = %ip,
                rate = info_ref.recent_connections.len(),
                limit = self.config.connection_rate_per_minute,
                "Connection rate limit exceeded"
            );
            should_block = true;
        }

        // Check concurrent connection limit
        if info_ref.active_connections >= self.config.max_connections_per_ip {
            warn!(
                ip = %ip,
                active = info_ref.active_connections,
                limit = self.config.max_connections_per_ip,
                "Concurrent connection limit exceeded"
            );
            should_block = true;
        }

        if should_block {
            // Block the IP
            info_ref.blocked_until = Some(now + self.config.block_duration);
            crate::counter!("ddos_protection_blocks_total", "ip" => ip.to_string()).increment(1);
            return Err("Connection limit exceeded - IP blocked".to_string());
        }

        // Allow the connection
        info_ref.active_connections += 1;
        info_ref.recent_connections.push(now);
        crate::counter!("ddos_protection_connections_allowed_total", "ip" => ip.to_string()).increment(1);

        Ok(())
    }

    /// Record connection closure for an IP
    pub fn connection_closed(&self, ip: IpAddr) {
        if let Some(mut info_ref) = self.ip_info.get_mut(&ip) {
            info_ref.active_connections = info_ref.active_connections.saturating_sub(1);
        }
    }

    /// Check if request size is within limits
    pub fn check_request_size(&self, size: usize) -> Result<(), String> {
        if size > self.config.max_request_size {
            crate::counter!("ddos_protection_oversized_requests_total").increment(1);
            return Err(format!(
                "Request size {} exceeds maximum allowed size {}",
                size, self.config.max_request_size
            ));
        }
        Ok(())
    }

    /// Get current statistics for monitoring
    pub fn get_stats(&self) -> DdosStats {
        let total_ips = self.ip_info.len();
        let mut blocked_ips = 0;
        let mut total_active_connections = 0;

        let now = Instant::now();
        for entry in self.ip_info.iter() {
            let info = entry.value();
            if info.blocked_until.is_some_and(|blocked_until| now < blocked_until) {
                blocked_ips += 1;
            }
            total_active_connections += info.active_connections;
        }

        DdosStats {
            total_tracked_ips: total_ips,
            blocked_ips,
            total_active_connections,
        }
    }

    /// Clean up old entries to prevent memory leaks
    pub fn cleanup_old_entries(&self) {
        let now = Instant::now();
        let cleanup_threshold = self.config.rate_window * 2; // Keep entries for 2x the rate window

        self.ip_info.retain(|_ip, info| {
            // Remove if no active connections and no recent activity
            if info.active_connections == 0 {
                // Check if there are any recent connections
                let has_recent_activity = info.recent_connections.iter().any(|&timestamp| {
                    now.duration_since(timestamp) < cleanup_threshold
                });

                // Check if still blocked
                let is_still_blocked = info.blocked_until
                    .map(|blocked_until| now < blocked_until)
                    .unwrap_or(false);

                // Keep only if there's recent activity or still blocked
                has_recent_activity || is_still_blocked
            } else {
                // Always keep if there are active connections
                true
            }
        });
    }
}

/// DDoS protection statistics
#[derive(Debug, Clone)]
pub struct DdosStats {
    pub total_tracked_ips: usize,
    pub blocked_ips: usize,
    pub total_active_connections: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ddos_config_default() {
        let config = DdosConfig::default();
        assert_eq!(config.max_connections_per_ip, 10);
        assert_eq!(config.connection_rate_per_minute, 100);
        assert_eq!(config.max_request_size, 1024 * 1024);
    }

    #[test]
    fn test_connection_limit_enforcement() {
        let config = DdosConfig {
            max_connections_per_ip: 2,
            connection_rate_per_minute: 100,
            block_duration: Duration::from_millis(50), // Short block for test
            ..Default::default()
        };
        let ddos = DdosProtection::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // First two connections should succeed
        assert!(ddos.check_connection_limit(ip).is_ok());
        assert!(ddos.check_connection_limit(ip).is_ok());

        // Third connection should be blocked
        assert!(ddos.check_connection_limit(ip).is_err());

        // After closing a connection, should allow another after block expires
        ddos.connection_closed(ip);
        std::thread::sleep(Duration::from_millis(60)); // Wait for block to expire
        assert!(ddos.check_connection_limit(ip).is_ok());
    }

    #[test]
    fn test_request_size_limit() {
        let ddos = DdosProtection::new(DdosConfig::default());

        // Normal size should pass
        assert!(ddos.check_request_size(1000).is_ok());

        // Oversized request should fail
        assert!(ddos.check_request_size(2 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_ip_blocking() {
        let config = DdosConfig {
            max_connections_per_ip: 1,
            block_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let ddos = DdosProtection::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Fill up the connection limit to trigger blocking
        assert!(ddos.check_connection_limit(ip).is_ok());
        assert!(ddos.check_connection_limit(ip).is_err()); // This should trigger blocking

        // Should be blocked now
        assert!(ddos.is_blocked(ip));

        // Wait for block to expire (in a real test, you might want to mock time)
        std::thread::sleep(Duration::from_millis(150));

        // Should no longer be blocked (though connection limits still apply)
        assert!(!ddos.is_blocked(ip));
    }

    #[test]
    fn test_stats_collection() {
        let ddos = DdosProtection::new(DdosConfig::default());
        let ip1: IpAddr = "192.168.1.1".parse().unwrap();
        let ip2: IpAddr = "192.168.1.2".parse().unwrap();

        // Add some connections
        let _ = ddos.check_connection_limit(ip1);
        let _ = ddos.check_connection_limit(ip2);

        let stats = ddos.get_stats();
        assert_eq!(stats.total_tracked_ips, 2);
        assert_eq!(stats.total_active_connections, 2);
    }
} 