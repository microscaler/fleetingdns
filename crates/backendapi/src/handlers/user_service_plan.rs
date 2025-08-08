use crate::{
    ApiResult, ApiState,
};
use auth::{extract_bearer_token, validate_jwt_token};
use axum::{Json, extract::State, http::HeaderMap};
use models::{
    service_plan::{Entity as ServicePlanEntity, Column as ServicePlanColumn},
    user_service_plan::{Entity as UserServicePlanEntity, Column as UserServicePlanColumn},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    let user_service_plan = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::UserId.eq(user_id))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("No active ServicePlan found".to_string()))?;

    // Get the associated ServicePlan details
    let service_plan = ServicePlanEntity::find_by_id(user_service_plan.service_plan_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("ServicePlan not found".to_string()))?;

    // Parse features JSON
    let features: serde_json::Value = serde_json::from_str(
        &service_plan.features_json.unwrap_or_else(|| "{}".to_string())
    )
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
        description: format!(
            "Service plan with {} API calls per hour",
            service_plan.api_rate_limit
        ),
        features,
        quotas,
        pricing: 0.0, // TODO: Add pricing field to entity
        assignment_date: user_service_plan.start_date,
        end_date: user_service_plan.end_date,
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
    let user_service_plan = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::UserId.eq(user_id))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("No active ServicePlan found".to_string()))?;

    // Get the associated ServicePlan details
    let service_plan = ServicePlanEntity::find_by_id(user_service_plan.service_plan_id)
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
    let current_assignment = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::UserId.eq(user_id))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .one(&state.db)
        .await?;

    // Get all available ServicePlans
    let service_plans = ServicePlanEntity::find().all(&state.db).await?;

    let mut available_plans = Vec::new();

    for plan in service_plans {
        // Parse features JSON
        let features: serde_json::Value = serde_json::from_str(
            &plan.features_json.unwrap_or_else(|| "{}".to_string())
        )
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
            description: format!(
                "Service plan with {} API calls per hour",
                plan.api_rate_limit
            ),
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
    let _target_plan = ServicePlanEntity::find_by_id(&service_plan_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("Target ServicePlan not found".to_string()))?;

    // Check if user already has this ServicePlan
    let current_assignment = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::UserId.eq(user_id))
        .filter(UserServicePlanColumn::ServicePlanId.eq(service_plan_id))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .one(&state.db)
        .await?;

    if current_assignment.is_some() {
        return Err(crate::ApiError::ValidationError(
            "User already has this ServicePlan".to_string(),
        ));
    }

    // TODO: Implement actual ServicePlan change logic
    // For now, return a success response indicating the request was received
    Ok(Json(ServicePlanChangeResponse {
        message:
            "ServicePlan change request received. An admin will review and process your request."
                .to_string(),
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
    pub assignment_date: chrono::DateTime<chrono::Utc>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
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
    #[allow(dead_code)]
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
    async fn test_service_plan_entities_compile() {
        // Test that we can use the models crate entities
        let _service_plan_entity = ServicePlanEntity;
        let _user_service_plan_entity = UserServicePlanEntity;
        
        // Test that we can use the column types
        let _service_plan_column = ServicePlanColumn::Id;
        let _user_service_plan_column = UserServicePlanColumn::UserId;
        
        assert!(true);
    }
}
