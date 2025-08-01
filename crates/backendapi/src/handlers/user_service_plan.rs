use crate::handlers::{service_plan_entity, user_service_plan_entity};
use crate::{
    ApiResult, ApiState,
    auth::{extract_bearer_token, validate_jwt_token},
};
use axum::{Json, extract::State, http::HeaderMap};
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
        description: format!(
            "Service plan with {} API calls per hour",
            service_plan.api_rate_limit
        ),
        features,
        quotas,
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
    let service_plans = service_plan_entity::Entity::find().all(&state.db).await?;

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
    let _target_plan = service_plan_entity::Entity::find_by_id(&service_plan_id)
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
    use crate::handlers::service_plan_entity;
    use crate::handlers::user_entity;
    use crate::handlers::user_service_plan_entity;
    use crate::test_utils::postgres_test_container::PostgresTestContainer;
    use chrono::Utc;

    use uuid::Uuid;

    #[tokio::test]
    async fn test_database_operations_with_real_postgres() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create a test user
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let user = user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Create a test service plan
        let plan_id = Uuid::new_v4().to_string();
        let plan = service_plan_entity::ActiveModel {
            id: sea_orm::Set(plan_id.clone()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(1000),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = service_plan_entity::Entity::insert(plan)
            .exec(&db)
            .await
            .expect("create service plan");

        // Assign service plan to user
        let assignment = user_service_plan_entity::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            user_id: sea_orm::Set(user_id.clone()),
            service_plan_id: sea_orm::Set(plan_id.clone()),
            start_date: sea_orm::Set(now),
            end_date: sea_orm::Set(now + chrono::Duration::days(30)),
            is_active: sea_orm::Set(true),
        };
        let _ = user_service_plan_entity::Entity::insert(assignment)
            .exec(&db)
            .await
            .expect("assign service plan");

        // Verify the data was created correctly
        let user_service_plan = user_service_plan_entity::Entity::find()
            .filter(user_service_plan_entity::Column::UserId.eq(user_id.clone()))
            .one(&db)
            .await
            .expect("find user service plan")
            .expect("user service plan should exist");

        assert_eq!(user_service_plan.user_id, user_id);
        assert_eq!(user_service_plan.service_plan_id, plan_id);
        assert!(user_service_plan.is_active);

        // Verify the service plan exists
        let service_plan = service_plan_entity::Entity::find_by_id(plan_id.clone())
            .one(&db)
            .await
            .expect("find service plan")
            .expect("service plan should exist");

        assert_eq!(service_plan.name, "Pro");
        assert_eq!(service_plan.api_rate_limit, 1000);
    }

    #[tokio::test]
    async fn test_service_plan_queries_with_real_postgres() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create multiple service plans
        let now = Utc::now().naive_utc();
        let plan1 = service_plan_entity::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            name: sea_orm::Set("Basic".to_string()),
            api_rate_limit: sea_orm::Set(100),
            tunnel_creation_limit: sea_orm::Set(5),
            dns_provisioning_limit: sea_orm::Set(2),
            max_concurrent_tunnels: sea_orm::Set(1),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = service_plan_entity::Entity::insert(plan1)
            .exec(&db)
            .await
            .expect("create service plan 1");

        let plan2 = service_plan_entity::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(1000),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = service_plan_entity::Entity::insert(plan2)
            .exec(&db)
            .await
            .expect("create service plan 2");

        // Query all service plans
        let all_plans = service_plan_entity::Entity::find()
            .all(&db)
            .await
            .expect("find all service plans");

        assert_eq!(all_plans.len(), 2);
        assert!(all_plans.iter().any(|p| p.name == "Basic"));
        assert!(all_plans.iter().any(|p| p.name == "Pro"));
    }

    #[tokio::test]
    async fn test_user_service_plan_queries_with_real_postgres() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create a test user
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let user = user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Create a test service plan
        let plan_id = Uuid::new_v4().to_string();
        let plan = service_plan_entity::ActiveModel {
            id: sea_orm::Set(plan_id.clone()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(1000),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = service_plan_entity::Entity::insert(plan)
            .exec(&db)
            .await
            .expect("create service plan");

        // Assign service plan to user
        let assignment = user_service_plan_entity::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            user_id: sea_orm::Set(user_id.clone()),
            service_plan_id: sea_orm::Set(plan_id.clone()),
            start_date: sea_orm::Set(now),
            end_date: sea_orm::Set(now + chrono::Duration::days(30)),
            is_active: sea_orm::Set(true),
        };
        let _ = user_service_plan_entity::Entity::insert(assignment)
            .exec(&db)
            .await
            .expect("assign service plan");

        // Query user's active service plan
        let active_plan = user_service_plan_entity::Entity::find()
            .filter(user_service_plan_entity::Column::UserId.eq(user_id.clone()))
            .filter(user_service_plan_entity::Column::IsActive.eq(true))
            .one(&db)
            .await
            .expect("find active user service plan")
            .expect("active user service plan should exist");

        assert_eq!(active_plan.user_id, user_id);
        assert_eq!(active_plan.service_plan_id, plan_id);
        assert!(active_plan.is_active);
    }
}
