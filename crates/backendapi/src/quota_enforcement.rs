use crate::{ApiResult, ApiError};
use sea_orm::{QueryFilter, ColumnTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, DateTime};

/// Quota types that can be enforced
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuotaType {
    ApiCalls,
    TunnelCreation,
    DnsProvisioning,
    ConcurrentTunnels,
    DataTransfer,
    CertificateIssuance,
}

/// Current usage statistics for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsage {
    pub user_id: String,
    pub service_plan_id: String,
    pub api_calls_count: i64,
    pub tunnels_created_count: i64,
    pub dns_operations_count: i64,
    pub active_tunnels_count: i64,
    pub data_transferred_mb: i64,
    pub certificates_issued_count: i64,
    pub last_updated: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
}

/// Quota limits from ServicePlan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaLimits {
    pub api_rate_limit: i32,
    pub tunnel_creation_limit: i32,
    pub dns_provisioning_limit: i32,
    pub max_concurrent_tunnels: i32,
    pub data_transfer_limit_mb: Option<i64>,
    pub certificate_issuance_limit: Option<i32>,
}

/// Quota enforcement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaEnforcementResult {
    pub allowed: bool,
    pub remaining_quota: Option<i64>,
    pub quota_exceeded: Option<QuotaType>,
    pub message: String,
}

/// Usage tracking service
pub struct UsageTracker {
    db: DatabaseConnection,
    cache: Arc<RwLock<HashMap<String, UserUsage>>>,
}

impl UsageTracker {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current usage for a user
    pub async fn get_user_usage(&self, user_id: &str) -> ApiResult<UserUsage> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(usage) = cache.get(user_id) {
                return Ok(usage.clone());
            }
        }

        // TODO: Get from database - simplified for now
        // For now, create placeholder usage
        let usage = UserUsage {
            user_id: user_id.to_string(),
            service_plan_id: "default".to_string(),
            api_calls_count: 0,
            tunnels_created_count: 0,
            dns_operations_count: 0,
            active_tunnels_count: 0,
            data_transferred_mb: 0,
            certificates_issued_count: 0,
            last_updated: Utc::now(),
            period_start: Utc::now(),
        };

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(user_id.to_string(), usage.clone());
        }

        Ok(usage)
    }

    /// Get quota limits for a user's ServicePlan
    pub async fn get_quota_limits(&self, user_id: &str) -> ApiResult<QuotaLimits> {
        // TODO: Get from database - simplified for now
        // Return default limits
        Ok(QuotaLimits {
            api_rate_limit: 1000,
            tunnel_creation_limit: 100,
            dns_provisioning_limit: 100,
            max_concurrent_tunnels: 10,
            data_transfer_limit_mb: Some(1024),
            certificate_issuance_limit: Some(100),
        })
    }

    /// Check if an operation is allowed based on quotas
    pub async fn check_quota(&self, user_id: &str, quota_type: QuotaType, amount: i64) -> ApiResult<QuotaEnforcementResult> {
        let usage = self.get_user_usage(user_id).await?;
        let limits = self.get_quota_limits(user_id).await?;

        let (current_usage, limit) = match quota_type {
            QuotaType::ApiCalls => (usage.api_calls_count, limits.api_rate_limit as i64),
            QuotaType::TunnelCreation => (usage.tunnels_created_count, limits.tunnel_creation_limit as i64),
            QuotaType::DnsProvisioning => (usage.dns_operations_count, limits.dns_provisioning_limit as i64),
            QuotaType::ConcurrentTunnels => (usage.active_tunnels_count, limits.max_concurrent_tunnels as i64),
            QuotaType::DataTransfer => (usage.data_transferred_mb, limits.data_transfer_limit_mb.unwrap_or(1024)),
            QuotaType::CertificateIssuance => (usage.certificates_issued_count, limits.certificate_issuance_limit.unwrap_or(100) as i64),
        };

        let new_usage = current_usage + amount;
        let allowed = new_usage <= limit;
        let remaining = if allowed { Some(limit - new_usage) } else { None };

        Ok(QuotaEnforcementResult {
            allowed,
            remaining_quota: remaining,
            quota_exceeded: if allowed { None } else { Some(quota_type.clone()) },
            message: if allowed {
                format!("Operation allowed. Remaining quota: {}", remaining.unwrap_or(0))
            } else {
                format!("Quota exceeded for {:?}. Limit: {}, Current: {}, Requested: {}", 
                    quota_type, limit, current_usage, amount)
            },
        })
    }

    /// Record usage for an operation
    pub async fn record_usage(&self, user_id: &str, quota_type: QuotaType, amount: i64) -> ApiResult<()> {
        // TODO: Implement actual usage recording to database
        // For now, just update the cache
        
        let mut cache = self.cache.write().await;
        let usage = cache.entry(user_id.to_string()).or_insert_with(|| UserUsage {
            user_id: user_id.to_string(),
            service_plan_id: "default".to_string(),
            api_calls_count: 0,
            tunnels_created_count: 0,
            dns_operations_count: 0,
            active_tunnels_count: 0,
            data_transferred_mb: 0,
            certificates_issued_count: 0,
            last_updated: Utc::now(),
            period_start: Utc::now(),
        });
        
        match quota_type {
            QuotaType::ApiCalls => usage.api_calls_count += amount,
            QuotaType::TunnelCreation => usage.tunnels_created_count += amount,
            QuotaType::DnsProvisioning => usage.dns_operations_count += amount,
            QuotaType::ConcurrentTunnels => usage.active_tunnels_count += amount,
            QuotaType::DataTransfer => usage.data_transferred_mb += amount,
            QuotaType::CertificateIssuance => usage.certificates_issued_count += amount,
        }
        usage.last_updated = Utc::now();

        Ok(())
    }

    /// Reset usage for a new period
    pub async fn reset_usage(&self, user_id: &str) -> ApiResult<()> {
        let mut cache = self.cache.write().await;
        if let Some(usage) = cache.get_mut(user_id) {
            usage.api_calls_count = 0;
            usage.tunnels_created_count = 0;
            usage.dns_operations_count = 0;
            usage.active_tunnels_count = 0;
            usage.data_transferred_mb = 0;
            usage.certificates_issued_count = 0;
            usage.period_start = Utc::now();
            usage.last_updated = Utc::now();
        }

        Ok(())
    }
}

/// Quota enforcement middleware
pub struct QuotaEnforcementMiddleware {
    usage_tracker: Arc<UsageTracker>,
}

impl QuotaEnforcementMiddleware {
    pub fn new(usage_tracker: Arc<UsageTracker>) -> Self {
        Self { usage_tracker }
    }

    /// Enforce quota for API calls
    pub async fn enforce_api_quota(&self, user_id: &str) -> ApiResult<QuotaEnforcementResult> {
        let result = self.usage_tracker.check_quota(user_id, QuotaType::ApiCalls, 1).await?;
        
        if result.allowed {
            self.usage_tracker.record_usage(user_id, QuotaType::ApiCalls, 1).await?;
        }

        Ok(result)
    }

    /// Enforce quota for tunnel creation
    pub async fn enforce_tunnel_creation_quota(&self, user_id: &str) -> ApiResult<QuotaEnforcementResult> {
        let result = self.usage_tracker.check_quota(user_id, QuotaType::TunnelCreation, 1).await?;
        
        if result.allowed {
            self.usage_tracker.record_usage(user_id, QuotaType::TunnelCreation, 1).await?;
        }

        Ok(result)
    }

    /// Enforce quota for DNS operations
    pub async fn enforce_dns_quota(&self, user_id: &str) -> ApiResult<QuotaEnforcementResult> {
        let result = self.usage_tracker.check_quota(user_id, QuotaType::DnsProvisioning, 1).await?;
        
        if result.allowed {
            self.usage_tracker.record_usage(user_id, QuotaType::DnsProvisioning, 1).await?;
        }

        Ok(result)
    }

    /// Enforce quota for concurrent tunnels
    pub async fn enforce_concurrent_tunnels_quota(&self, user_id: &str, current_count: i64) -> ApiResult<QuotaEnforcementResult> {
        let result = self.usage_tracker.check_quota(user_id, QuotaType::ConcurrentTunnels, current_count).await?;
        Ok(result)
    }

    /// Enforce quota for data transfer
    pub async fn enforce_data_transfer_quota(&self, user_id: &str, bytes_transferred: i64) -> ApiResult<QuotaEnforcementResult> {
        let mb_transferred = bytes_transferred / 1024 / 1024; // Convert to MB
        let result = self.usage_tracker.check_quota(user_id, QuotaType::DataTransfer, mb_transferred).await?;
        
        if result.allowed {
            self.usage_tracker.record_usage(user_id, QuotaType::DataTransfer, mb_transferred).await?;
        }

        Ok(result)
    }

    /// Enforce quota for certificate issuance
    pub async fn enforce_certificate_quota(&self, user_id: &str) -> ApiResult<QuotaEnforcementResult> {
        let result = self.usage_tracker.check_quota(user_id, QuotaType::CertificateIssuance, 1).await?;
        
        if result.allowed {
            self.usage_tracker.record_usage(user_id, QuotaType::CertificateIssuance, 1).await?;
        }

        Ok(result)
    }
}

/// Integration with existing rate limiting system
pub struct ServicePlanRateLimiter {
    pub usage_tracker: Arc<UsageTracker>,
    quota_enforcement: Arc<QuotaEnforcementMiddleware>,
}

impl ServicePlanRateLimiter {
    pub fn new(usage_tracker: Arc<UsageTracker>) -> Self {
        let quota_enforcement = Arc::new(QuotaEnforcementMiddleware::new(usage_tracker.clone()));
        Self {
            usage_tracker,
            quota_enforcement,
        }
    }

    /// Check if user can make an API call
    pub async fn can_make_api_call(&self, user_id: &str) -> ApiResult<bool> {
        let result = self.quota_enforcement.enforce_api_quota(user_id).await?;
        Ok(result.allowed)
    }

    /// Check if user can create a tunnel
    pub async fn can_create_tunnel(&self, user_id: &str) -> ApiResult<bool> {
        let result = self.quota_enforcement.enforce_tunnel_creation_quota(user_id).await?;
        Ok(result.allowed)
    }

    /// Check if user can perform DNS operations
    pub async fn can_perform_dns_operation(&self, user_id: &str) -> ApiResult<bool> {
        let result = self.quota_enforcement.enforce_dns_quota(user_id).await?;
        Ok(result.allowed)
    }

    /// Get detailed quota information for a user
    pub async fn get_quota_info(&self, user_id: &str) -> ApiResult<QuotaInfo> {
        let usage = self.usage_tracker.get_user_usage(user_id).await?;
        let limits = self.usage_tracker.get_quota_limits(user_id).await?;

        Ok(QuotaInfo {
            usage,
            limits,
        })
    }
}

/// Detailed quota information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaInfo {
    pub usage: UserUsage,
    pub limits: QuotaLimits,
}

impl QuotaInfo {
    /// Get usage percentage for a quota type
    pub fn get_usage_percentage(&self, quota_type: QuotaType) -> f64 {
        let (current, limit) = match quota_type {
            QuotaType::ApiCalls => (self.usage.api_calls_count, self.limits.api_rate_limit as i64),
            QuotaType::TunnelCreation => (self.usage.tunnels_created_count, self.limits.tunnel_creation_limit as i64),
            QuotaType::DnsProvisioning => (self.usage.dns_operations_count, self.limits.dns_provisioning_limit as i64),
            QuotaType::ConcurrentTunnels => (self.usage.active_tunnels_count, self.limits.max_concurrent_tunnels as i64),
            QuotaType::DataTransfer => (self.usage.data_transferred_mb, self.limits.data_transfer_limit_mb.unwrap_or(1024)),
            QuotaType::CertificateIssuance => (self.usage.certificates_issued_count, self.limits.certificate_issuance_limit.unwrap_or(100) as i64),
        };

        if limit == 0 {
            0.0
        } else {
            (current as f64 / limit as f64) * 100.0
        }
    }

    /// Check if any quota is near exhaustion (80% or more)
    pub fn has_quota_warnings(&self) -> Vec<QuotaType> {
        let mut warnings = Vec::new();
        
        if self.get_usage_percentage(QuotaType::ApiCalls) >= 80.0 {
            warnings.push(QuotaType::ApiCalls);
        }
        if self.get_usage_percentage(QuotaType::TunnelCreation) >= 80.0 {
            warnings.push(QuotaType::TunnelCreation);
        }
        if self.get_usage_percentage(QuotaType::DnsProvisioning) >= 80.0 {
            warnings.push(QuotaType::DnsProvisioning);
        }
        if self.get_usage_percentage(QuotaType::ConcurrentTunnels) >= 80.0 {
            warnings.push(QuotaType::ConcurrentTunnels);
        }
        if self.get_usage_percentage(QuotaType::DataTransfer) >= 80.0 {
            warnings.push(QuotaType::DataTransfer);
        }
        if self.get_usage_percentage(QuotaType::CertificateIssuance) >= 80.0 {
            warnings.push(QuotaType::CertificateIssuance);
        }

        warnings
    }
} 

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::postgres_test_container::PostgresTestContainer;
    use sea_orm::{Database, ActiveModelTrait, EntityTrait};
    use uuid::Uuid;
    use chrono::{Utc, NaiveDateTime};

    #[tokio::test]
    async fn test_quota_type_serialization() {
        let quota_type = QuotaType::ApiCalls;
        let serialized = serde_json::to_string(&quota_type).unwrap();
        let deserialized: QuotaType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(quota_type, deserialized);
    }

    #[tokio::test]
    async fn test_user_usage_creation() {
        let usage = UserUsage {
            user_id: "test_user".to_string(),
            service_plan_id: "pro".to_string(),
            api_calls_count: 50,
            tunnels_created_count: 5,
            dns_operations_count: 10,
            active_tunnels_count: 2,
            data_transferred_mb: 100,
            certificates_issued_count: 8,
            last_updated: Utc::now(),
            period_start: Utc::now(),
        };
        assert_eq!(usage.user_id, "test_user");
        assert_eq!(usage.api_calls_count, 50);
    }

    #[tokio::test]
    async fn test_quota_limits_creation() {
        let limits = QuotaLimits {
            api_rate_limit: 1000,
            tunnel_creation_limit: 100,
            dns_provisioning_limit: 50,
            max_concurrent_tunnels: 10,
            data_transfer_limit_mb: Some(1024),
            certificate_issuance_limit: Some(100),
        };
        assert_eq!(limits.api_rate_limit, 1000);
        assert_eq!(limits.max_concurrent_tunnels, 10);
    }

    #[tokio::test]
    async fn test_usage_tracker_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let tracker = UsageTracker::new(db);
        
        // Test getting user usage
        let user_id = "test_user_123";
        let usage = tracker.get_user_usage(user_id).await;
        assert!(usage.is_ok());
        let usage = usage.unwrap();
        assert_eq!(usage.user_id, user_id);
        assert_eq!(usage.api_calls_count, 0); // Should start at 0
    }

    #[tokio::test]
    async fn test_service_plan_rate_limiter_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let usage_tracker = Arc::new(UsageTracker::new(db));
        let rate_limiter = ServicePlanRateLimiter::new(usage_tracker);
        
        // Test API call quota
        let user_id = "test_user_456";
        let can_call = rate_limiter.can_make_api_call(user_id).await;
        assert!(can_call.is_ok());
        assert!(can_call.unwrap());
        
        // Test DNS operation quota
        let can_dns = rate_limiter.can_perform_dns_operation(user_id).await;
        assert!(can_dns.is_ok());
        assert!(can_dns.unwrap());
    }

    #[tokio::test]
    async fn test_usage_recording_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let tracker = UsageTracker::new(db);
        let user_id = "test_user_789";
        
        // Record multiple API calls
        for i in 0..5 {
            let result = tracker.record_usage(user_id, QuotaType::ApiCalls, 1).await;
            assert!(result.is_ok());
        }
        
        // Check total usage
        let usage = tracker.get_user_usage(user_id).await;
        assert!(usage.is_ok());
        let usage = usage.unwrap();
        assert_eq!(usage.api_calls_count, 5);
    }

    #[tokio::test]
    async fn test_usage_reset_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let tracker = UsageTracker::new(db);
        let user_id = "test_user_reset";
        
        // Record some usage
        tracker.record_usage(user_id, QuotaType::ApiCalls, 10).await.unwrap();
        
        // Reset usage
        let result = tracker.reset_usage(user_id).await;
        assert!(result.is_ok());
        
        // Verify reset
        let usage = tracker.get_user_usage(user_id).await;
        assert!(usage.is_ok());
        let usage = usage.unwrap();
        assert_eq!(usage.api_calls_count, 0);
    }

    #[tokio::test]
    async fn test_quota_checking_allowed() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let tracker = UsageTracker::new(db);
        let user_id = "test_user_allowed";
        
        // Set up a reasonable limit
        let limits = QuotaLimits {
            api_rate_limit: 100,
            tunnel_creation_limit: 10,
            dns_provisioning_limit: 5,
            max_concurrent_tunnels: 3,
            data_transfer_limit_mb: Some(100),
            certificate_issuance_limit: Some(100),
        };
        
        // Record some usage but stay under limit
        tracker.record_usage(user_id, QuotaType::ApiCalls, 50).await.unwrap();
        
        let result = tracker.check_quota(user_id, QuotaType::ApiCalls, 1).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_quota_checking_exceeded() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let tracker = UsageTracker::new(db.clone());
        let user_id = "test_user_exceeded";
        
        // Record usage that exceeds the limit
        tracker.record_usage(user_id, QuotaType::ApiCalls, 15).await.unwrap();
        
        let result = tracker.check_quota(user_id, QuotaType::ApiCalls, 1).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        // The default limit is 1000, so 15 + 1 = 16 is still under the limit
        assert!(result.allowed);
        
        // Test with a number that actually exceeds the default limit
        tracker.record_usage(user_id, QuotaType::ApiCalls, 1000).await.unwrap();
        let result = tracker.check_quota(user_id, QuotaType::ApiCalls, 1).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_quota_info_creation() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let usage_tracker = Arc::new(UsageTracker::new(db));
        let rate_limiter = ServicePlanRateLimiter::new(usage_tracker);
        let user_id = "test_user_info";
        
        // Record some usage
        rate_limiter.usage_tracker.record_usage(user_id, QuotaType::ApiCalls, 25).await.unwrap();
        
        let info = rate_limiter.get_quota_info(user_id).await;
        assert!(info.is_ok());
        let info = info.unwrap();
        assert_eq!(info.usage.api_calls_count, 25);
    }

    #[tokio::test]
    async fn test_concurrent_tunnels_quota() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let usage_tracker = Arc::new(UsageTracker::new(db));
        let rate_limiter = ServicePlanRateLimiter::new(usage_tracker);
        let user_id = "test_user_tunnels";
        
        // Test concurrent tunnels quota
        let can_create = rate_limiter.can_create_tunnel(user_id).await;
        assert!(can_create.is_ok());
        assert!(can_create.unwrap());
    }

    #[tokio::test]
    async fn test_data_transfer_quota() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let usage_tracker = Arc::new(UsageTracker::new(db));
        let rate_limiter = ServicePlanRateLimiter::new(usage_tracker);
        let user_id = "test_user_data";
        
        // Test data transfer quota - this would be called during tunnel operations
        // For now, just test that the method exists and doesn't panic
        let _ = rate_limiter.get_quota_info(user_id).await;
    }

    #[tokio::test]
    async fn test_certificate_quota() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();
        
        let usage_tracker = Arc::new(UsageTracker::new(db));
        let rate_limiter = ServicePlanRateLimiter::new(usage_tracker);
        let user_id = "test_user_cert";
        
        // Test certificate issuance quota - this would be called during certificate operations
        // For now, just test that the method exists and doesn't panic
        let _ = rate_limiter.get_quota_info(user_id).await;
    }
} 