use crate::{ApiResult, ApiState, auth::{extract_bearer_token, validate_jwt_token}};
use axum::{Json, extract::State, http::HeaderMap};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::handlers::{service_plan_entity, user_service_plan_entity};

/// Get current user's ServicePlan information
pub async fn get_my_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<MyServicePlanResponse>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Get user's current ServicePlan assignment
    let user_service_plan = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::UserId.eq(user_id))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("No active ServicePlan found".to_string()))?;

    // Get the associated ServicePlan details
    let service_plan = service_plan_entity::Entity::find_by_id(user_service_plan.service_plan_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("ServicePlan not found".to_string()))?;

    // Parse features JSON
    let features: serde_json::Value = serde_json::from_str(&service_plan.features_json)
        .map_err(|_| crate::ApiError::InternalError("Invalid features JSON".to_string()))?;

    // Create quotas from the actual fields
    let quotas = serde_json::json!({
        "api_rate_limit": service_plan.api_rate_limit,
        "tunnel_creation_limit": service_plan.tunnel_creation_limit,
        "dns_provisioning_limit": service_plan.dns_provisioning_limit,
        "max_concurrent_tunnels": service_plan.max_concurrent_tunnels
    });

    Ok(Json(MyServicePlanResponse {
        service_plan_id: service_plan.id,
        name: service_plan.name,
        description: format!("Service plan with {} API calls per hour", service_plan.api_rate_limit),
        features: features,
        quotas: quotas,
        pricing: 0.0, // TODO: Add pricing field to entity
        assignment_date: user_service_plan.start_date,
        end_date: Some(user_service_plan.end_date),
        is_active: user_service_plan.is_active,
    }))
}

/// Get current user's ServicePlan usage statistics
pub async fn get_my_service_plan_usage(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<ServicePlanUsageResponse>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Get user's current ServicePlan assignment
    let user_service_plan = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::UserId.eq(user_id))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("No active ServicePlan found".to_string()))?;

    // Get the associated ServicePlan details
    let service_plan = service_plan_entity::Entity::find_by_id(user_service_plan.service_plan_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("ServicePlan not found".to_string()))?;

    // Create quotas from the actual fields
    let quotas = serde_json::json!({
        "api_rate_limit": service_plan.api_rate_limit,
        "tunnel_creation_limit": service_plan.tunnel_creation_limit,
        "dns_provisioning_limit": service_plan.dns_provisioning_limit,
        "max_concurrent_tunnels": service_plan.max_concurrent_tunnels
    });

    // TODO: Implement actual usage tracking
    // For now, return placeholder usage data
    let usage = ServicePlanUsage {
        tunnels_created: 0,
        tunnels_active: 0,
        dns_queries: 0,
        data_transferred_mb: 0,
        certificates_issued: 0,
        quota_limits: quotas,
    };

    Ok(Json(ServicePlanUsageResponse {
        service_plan_id: service_plan.id,
        service_plan_name: service_plan.name,
        usage,
        last_updated: chrono::Utc::now(),
    }))
}

/// Get available ServicePlans for upgrade/downgrade
pub async fn get_available_service_plans(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<AvailableServicePlanResponse>>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Get user's current ServicePlan
    let current_assignment = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::UserId.eq(user_id))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .one(&state.db)
        .await?;

    // Get all available ServicePlans
    let service_plans = service_plan_entity::Entity::find()
        .all(&state.db)
        .await?;

    let mut available_plans = Vec::new();

    for plan in service_plans {
        // Parse features JSON
        let features: serde_json::Value = serde_json::from_str(&plan.features_json)
            .map_err(|_| crate::ApiError::InternalError("Invalid features JSON".to_string()))?;
        
        // Create quotas from the actual fields
        let quotas = serde_json::json!({
            "api_rate_limit": plan.api_rate_limit,
            "tunnel_creation_limit": plan.tunnel_creation_limit,
            "dns_provisioning_limit": plan.dns_provisioning_limit,
            "max_concurrent_tunnels": plan.max_concurrent_tunnels
        });

        // Determine if this plan is available for upgrade/downgrade
        let is_current_plan = current_assignment
            .as_ref()
            .map(|assignment| assignment.service_plan_id == plan.id)
            .unwrap_or(false);

        let can_upgrade = !is_current_plan;
        let can_downgrade = !is_current_plan;

        available_plans.push(AvailableServicePlanResponse {
            id: plan.id,
            name: plan.name,
            description: format!("Service plan with {} API calls per hour", plan.api_rate_limit),
            features,
            quotas,
            pricing: 0.0, // TODO: Add pricing field to entity
            is_current_plan,
            can_upgrade,
            can_downgrade,
        });
    }

    Ok(Json(available_plans))
}

/// Request ServicePlan upgrade/downgrade
pub async fn request_service_plan_change(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ServicePlanChangeRequest>,
) -> ApiResult<Json<ServicePlanChangeResponse>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Validate the target ServicePlan exists
    let service_plan_id = request.service_plan_id.clone();
    let target_plan = service_plan_entity::Entity::find_by_id(&service_plan_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("Target ServicePlan not found".to_string()))?;

    // Check if user already has this ServicePlan
    let current_assignment = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::UserId.eq(user_id))
        .filter(user_service_plan_entity::Column::ServicePlanId.eq(service_plan_id))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .one(&state.db)
        .await?;

    if current_assignment.is_some() {
        return Err(crate::ApiError::ValidationError("User already has this ServicePlan".to_string()));
    }

    // TODO: Implement actual ServicePlan change logic
    // For now, return a success response indicating the request was received
    Ok(Json(ServicePlanChangeResponse {
        message: "ServicePlan change request received. An admin will review and process your request.".to_string(),
        request_id: Uuid::new_v4(),
        status: "pending".to_string(),
        estimated_processing_time: "24-48 hours".to_string(),
    }))
}

// Response types
#[derive(Serialize)]
pub struct MyServicePlanResponse {
    pub service_plan_id: String,
    pub name: String,
    pub description: String,
    pub features: serde_json::Value,
    pub quotas: serde_json::Value,
    pub pricing: f64,
    pub assignment_date: chrono::NaiveDateTime,
    pub end_date: Option<chrono::NaiveDateTime>,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct ServicePlanUsage {
    pub tunnels_created: i32,
    pub tunnels_active: i32,
    pub dns_queries: i64,
    pub data_transferred_mb: i64,
    pub certificates_issued: i32,
    pub quota_limits: serde_json::Value,
}

#[derive(Serialize)]
pub struct ServicePlanUsageResponse {
    pub service_plan_id: String,
    pub service_plan_name: String,
    pub usage: ServicePlanUsage,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct AvailableServicePlanResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub features: serde_json::Value,
    pub quotas: serde_json::Value,
    pub pricing: f64,
    pub is_current_plan: bool,
    pub can_upgrade: bool,
    pub can_downgrade: bool,
}

#[derive(Deserialize)]
pub struct ServicePlanChangeRequest {
    pub service_plan_id: String,
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct ServicePlanChangeResponse {
    pub message: String,
    pub request_id: Uuid,
    pub status: String,
    pub estimated_processing_time: String,
} 

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_service_plan_response_creation() {
        let now = chrono::Utc::now().naive_utc();
        let response = MyServicePlanResponse {
            service_plan_id: "test_plan".to_string(),
            name: "Test Plan".to_string(),
            description: "A test service plan".to_string(),
            features: serde_json::json!({
                "feature1": true,
                "feature2": "value"
            }),
            quotas: serde_json::json!({
                "api_rate_limit": 1000,
                "tunnel_creation_limit": 100
            }),
            pricing: 29.99,
            assignment_date: now,
            end_date: Some(now + chrono::Duration::days(30)),
            is_active: true,
        };

        assert_eq!(response.service_plan_id, "test_plan");
        assert_eq!(response.name, "Test Plan");
        assert_eq!(response.description, "A test service plan");
        assert_eq!(response.pricing, 29.99);
        assert!(response.is_active);
    }

    #[tokio::test]
    async fn test_service_plan_usage_response_creation() {
        let now = chrono::Utc::now();
        let usage = ServicePlanUsage {
            tunnels_created: 75,
            tunnels_active: 5,
            dns_queries: 1000,
            data_transferred_mb: 500,
            certificates_issued: 30,
            quota_limits: serde_json::json!({
                "api_rate_limit": 1000,
                "tunnel_creation_limit": 100
            }),
        };

        let response = ServicePlanUsageResponse {
            service_plan_id: "test_plan".to_string(),
            service_plan_name: "Test Plan".to_string(),
            usage,
            last_updated: now,
        };

        assert_eq!(response.service_plan_id, "test_plan");
        assert_eq!(response.service_plan_name, "Test Plan");
        assert_eq!(response.usage.tunnels_created, 75);
        assert_eq!(response.usage.tunnels_active, 5);
    }

    #[tokio::test]
    async fn test_available_service_plan_response_creation() {
        let response = AvailableServicePlanResponse {
            id: "test_plan".to_string(),
            name: "Test Plan".to_string(),
            description: "A test service plan".to_string(),
            features: serde_json::json!({
                "feature1": true,
                "feature2": "value"
            }),
            quotas: serde_json::json!({
                "api_rate_limit": 1000,
                "tunnel_creation_limit": 100
            }),
            pricing: 29.99,
            is_current_plan: false,
            can_upgrade: true,
            can_downgrade: false,
        };

        assert_eq!(response.id, "test_plan");
        assert_eq!(response.name, "Test Plan");
        assert_eq!(response.description, "A test service plan");
        assert_eq!(response.pricing, 29.99);
        assert!(!response.is_current_plan);
        assert!(response.can_upgrade);
        assert!(!response.can_downgrade);
    }

    #[tokio::test]
    async fn test_service_plan_change_request_creation() {
        let request = ServicePlanChangeRequest {
            service_plan_id: "new_plan".to_string(),
            reason: Some("Need more features".to_string()),
        };

        assert_eq!(request.service_plan_id, "new_plan");
        assert_eq!(request.reason, Some("Need more features".to_string()));
    }

    #[tokio::test]
    async fn test_service_plan_change_response_creation() {
        let response = ServicePlanChangeResponse {
            message: "ServicePlan change request received".to_string(),
            request_id: Uuid::new_v4(),
            status: "pending".to_string(),
            estimated_processing_time: "24-48 hours".to_string(),
        };

        assert!(response.message.contains("ServicePlan change request received"));
        assert_eq!(response.status, "pending");
        assert_eq!(response.estimated_processing_time, "24-48 hours");
    }

    #[tokio::test]
    async fn test_features_json_parsing() {
        let valid_features = r#"{"feature1": true, "feature2": "value", "feature3": 123}"#;
        let parsed: serde_json::Value = serde_json::from_str(valid_features).unwrap();
        
        assert!(parsed["feature1"].as_bool().unwrap());
        assert_eq!(parsed["feature2"].as_str().unwrap(), "value");
        assert_eq!(parsed["feature3"].as_i64().unwrap(), 123);
    }

    #[tokio::test]
    async fn test_quotas_json_creation() {
        let quotas = serde_json::json!({
            "api_rate_limit": 1000,
            "tunnel_creation_limit": 100,
            "dns_provisioning_limit": 50,
            "max_concurrent_tunnels": 10
        });
        
        assert_eq!(quotas["api_rate_limit"].as_i64().unwrap(), 1000);
        assert_eq!(quotas["tunnel_creation_limit"].as_i64().unwrap(), 100);
        assert_eq!(quotas["dns_provisioning_limit"].as_i64().unwrap(), 50);
        assert_eq!(quotas["max_concurrent_tunnels"].as_i64().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_usage_stats_json_creation() {
        let usage_stats = serde_json::json!({
            "tunnels_created": 75,
            "tunnels_active": 5,
            "dns_queries": 1000,
            "data_transferred_mb": 500,
            "certificates_issued": 30
        });
        
        assert_eq!(usage_stats["tunnels_created"].as_i64().unwrap(), 75);
        assert_eq!(usage_stats["tunnels_active"].as_i64().unwrap(), 5);
        assert_eq!(usage_stats["dns_queries"].as_i64().unwrap(), 1000);
        assert_eq!(usage_stats["data_transferred_mb"].as_i64().unwrap(), 500);
        assert_eq!(usage_stats["certificates_issued"].as_i64().unwrap(), 30);
    }

    #[tokio::test]
    async fn test_plan_comparison_logic() {
        // Test current plan detection
        let current_assignment = Some("plan_a".to_string());
        
        let plan_a = "plan_a";
        let plan_b = "plan_b";
        
        let is_current_plan_a = current_assignment.as_ref().map(|assignment| assignment == plan_a).unwrap_or(false);
        let is_current_plan_b = current_assignment.as_ref().map(|assignment| assignment == plan_b).unwrap_or(false);
        
        assert!(is_current_plan_a);
        assert!(!is_current_plan_b);
        
        // Test upgrade/downgrade logic
        let can_upgrade_a = !is_current_plan_a;
        let can_downgrade_a = !is_current_plan_a;
        let can_upgrade_b = !is_current_plan_b;
        let can_downgrade_b = !is_current_plan_b;
        
        assert!(!can_upgrade_a);
        assert!(!can_downgrade_a);
        assert!(can_upgrade_b);
        assert!(can_downgrade_b);
    }

    #[tokio::test]
    async fn test_uuid_generation() {
        let request_id = Uuid::new_v4();
        
        // Test that UUID is valid
        assert!(!request_id.to_string().is_empty());
        assert_eq!(request_id.to_string().len(), 36); // Standard UUID length
    }

    #[tokio::test]
    async fn test_chrono_datetime_operations() {
        let now = chrono::Utc::now();
        let future = now + chrono::Duration::days(30);
        
        // Test that future date is after current date
        assert!(future > now);
        
        // Test duration calculation
        let duration = future - now;
        assert!(duration.num_days() >= 29); // Allow for slight timing differences
    }

    #[tokio::test]
    async fn test_optional_fields_handling() {
        // Test with reason provided
        let request_with_reason = ServicePlanChangeRequest {
            service_plan_id: "new_plan".to_string(),
            reason: Some("Need more features".to_string()),
        };
        
        // Test without reason
        let request_without_reason = ServicePlanChangeRequest {
            service_plan_id: "new_plan".to_string(),
            reason: None,
        };
        
        assert_eq!(request_with_reason.reason, Some("Need more features".to_string()));
        assert_eq!(request_without_reason.reason, None);
    }

    #[tokio::test]
    async fn test_response_message_generation() {
        let response = ServicePlanChangeResponse {
            message: "ServicePlan change request received. An admin will review and process your request.".to_string(),
            request_id: Uuid::new_v4(),
            status: "pending".to_string(),
            estimated_processing_time: "24-48 hours".to_string(),
        };
        
        assert!(response.message.contains("ServicePlan change request received"));
        assert!(response.message.contains("admin will review"));
        assert_eq!(response.status, "pending");
        assert_eq!(response.estimated_processing_time, "24-48 hours");
    }
} 