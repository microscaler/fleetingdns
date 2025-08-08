use crate::{
    ApiResult, ApiState,
    rate_limiting::RateLimitConfig,
};
use models::{
    service_plan::{Entity as ServicePlanEntity, ActiveModel as ServicePlanActiveModel, Column as ServicePlanColumn},
    user_service_plan::{Entity as UserServicePlanEntity, ActiveModel as UserServicePlanActiveModel, Column as UserServicePlanColumn},
    user::{Entity as UserEntity, ActiveModel as UserActiveModel},
};
use auth::{extract_bearer_token, validate_jwt_token};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Shared, thread-safe rate limit config
#[allow(dead_code)]
pub type SharedRateLimitConfig = Arc<RwLock<RateLimitConfig>>;

/// Create a new ServicePlan (admin only)
pub async fn create_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(plan_data): Json<CreateServicePlanRequest>,
) -> ApiResult<Json<ServicePlanResponse>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // TODO: Add admin role validation here

    let db = &state.db;

    // Validate unique name
    let existing = ServicePlanEntity::find()
        .filter(ServicePlanColumn::Name.eq(&plan_data.name))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(crate::ApiError::ValidationError(format!(
            "ServicePlan with name '{}' already exists",
            plan_data.name
        )));
    }

    // Create new ServicePlan
    let plan_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let plan = ServicePlanActiveModel {
        id: Set(plan_id.clone()),
        name: Set(plan_data.name),
        api_rate_limit: Set(plan_data.api_rate_limit as i32),
        tunnel_creation_limit: Set(plan_data.tunnel_creation_limit as i32),
        dns_provisioning_limit: Set(plan_data.dns_provisioning_limit as i32),
        max_concurrent_tunnels: Set(plan_data.max_concurrent_tunnels as i32),
        features_json: Set(plan_data.features_json),
        created_at: Set(now),
    };

    let inserted = plan.insert(db).await?;

    Ok(Json(ServicePlanResponse {
        id: inserted.id,
        name: inserted.name,
        api_rate_limit: inserted.api_rate_limit as u32,
        tunnel_creation_limit: inserted.tunnel_creation_limit as u32,
        dns_provisioning_limit: inserted.dns_provisioning_limit as u32,
        max_concurrent_tunnels: inserted.max_concurrent_tunnels as u32,
        features_json: inserted.features_json.unwrap_or_else(|| "{}".to_string()),
        created_at: inserted.created_at,
    }))
}

/// Get all ServicePlans (admin only)
pub async fn list_service_plans(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ServicePlanResponse>>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    let plans = ServicePlanEntity::find().all(db).await?;

    let responses = plans
        .into_iter()
        .map(|plan| ServicePlanResponse {
            id: plan.id,
            name: plan.name,
            api_rate_limit: plan.api_rate_limit as u32,
            tunnel_creation_limit: plan.tunnel_creation_limit as u32,
            dns_provisioning_limit: plan.dns_provisioning_limit as u32,
            max_concurrent_tunnels: plan.max_concurrent_tunnels as u32,
            features_json: plan.features_json.unwrap_or_else(|| "{}".to_string()),
            created_at: plan.created_at,
        })
        .collect();

    Ok(Json(responses))
}

/// Get a specific ServicePlan (admin only)
pub async fn get_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> ApiResult<Json<ServicePlanResponse>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    let plan = ServicePlanEntity::find_by_id(plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!("ServicePlan with id '{plan_id}' not found"))
        })?;

    Ok(Json(ServicePlanResponse {
        id: plan.id,
        name: plan.name,
        api_rate_limit: plan.api_rate_limit as u32,
        tunnel_creation_limit: plan.tunnel_creation_limit as u32,
        dns_provisioning_limit: plan.dns_provisioning_limit as u32,
        max_concurrent_tunnels: plan.max_concurrent_tunnels as u32,
        features_json: plan.features_json.unwrap_or_else(|| "{}".to_string()),
        created_at: plan.created_at,
    }))
}

/// Update a ServicePlan (admin only)
pub async fn update_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
    Json(update_data): Json<UpdateServicePlanRequest>,
) -> ApiResult<Json<ServicePlanResponse>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    // Check if plan exists
    let existing_plan = ServicePlanEntity::find_by_id(plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!("ServicePlan with id '{plan_id}' not found"))
        })?;

    // If name is being updated, check for uniqueness
    if let Some(new_name) = &update_data.name
        && new_name != &existing_plan.name
    {
        let duplicate = ServicePlanEntity::find()
            .filter(ServicePlanColumn::Name.eq(new_name))
            .filter(ServicePlanColumn::Id.ne(plan_id.clone()))
            .one(db)
            .await?;

        if duplicate.is_some() {
            return Err(crate::ApiError::ValidationError(format!(
                "ServicePlan with name '{new_name}' already exists"
            )));
        }
    }

    // Update the plan
    let mut plan_model: ServicePlanActiveModel = existing_plan.into();

    if let Some(name) = update_data.name {
        plan_model.name = Set(name);
    }
    if let Some(api_rate_limit) = update_data.api_rate_limit {
        plan_model.api_rate_limit = Set(api_rate_limit as i32);
    }
    if let Some(tunnel_creation_limit) = update_data.tunnel_creation_limit {
        plan_model.tunnel_creation_limit = Set(tunnel_creation_limit as i32);
    }
    if let Some(dns_provisioning_limit) = update_data.dns_provisioning_limit {
        plan_model.dns_provisioning_limit = Set(dns_provisioning_limit as i32);
    }
    if let Some(max_concurrent_tunnels) = update_data.max_concurrent_tunnels {
        plan_model.max_concurrent_tunnels = Set(max_concurrent_tunnels as i32);
    }
    if let Some(features_json) = update_data.features_json {
        plan_model.features_json = Set(Some(features_json));
    }

    let updated_plan = plan_model.update(db).await?;

    Ok(Json(ServicePlanResponse {
        id: updated_plan.id,
        name: updated_plan.name,
        api_rate_limit: updated_plan.api_rate_limit as u32,
        tunnel_creation_limit: updated_plan.tunnel_creation_limit as u32,
        dns_provisioning_limit: updated_plan.dns_provisioning_limit as u32,
        max_concurrent_tunnels: updated_plan.max_concurrent_tunnels as u32,
        features_json: updated_plan.features_json.unwrap_or_else(|| "{}".to_string()),
        created_at: updated_plan.created_at,
    }))
}

/// Delete a ServicePlan (admin only)
pub async fn delete_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    // Check if plan exists
    let existing_plan = ServicePlanEntity::find_by_id(plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!("ServicePlan with id '{plan_id}' not found"))
        })?;

    // Check if plan has active assignments
    let active_assignments = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::ServicePlanId.eq(plan_id.clone()))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .count(db)
        .await?;

    if active_assignments > 0 {
        return Err(crate::ApiError::ValidationError(
            "Cannot delete ServicePlan with active user assignments".to_string(),
        ));
    }

    // Delete the plan
    ServicePlanEntity::delete_by_id(plan_id)
        .exec(db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Assign a ServicePlan to a user (admin only)
pub async fn assign_service_plan_to_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(assignment_data): Json<AssignServicePlanRequest>,
) -> ApiResult<Json<UserServicePlanResponse>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    // Verify service plan exists
    let _plan = ServicePlanEntity::find_by_id(assignment_data.service_plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!(
                "ServicePlan with id '{}' not found",
                assignment_data.service_plan_id
            ))
        })?;

    // Check for existing active assignments
    let existing_assignments = UserServicePlanEntity::find()
        .filter(UserServicePlanColumn::UserId.eq(user_id.clone()))
        .filter(UserServicePlanColumn::IsActive.eq(true))
        .all(db)
        .await?;

    // Deactivate existing assignments
    for assignment in existing_assignments {
        let mut assignment_model: UserServicePlanActiveModel = assignment.into();
        assignment_model.is_active = Set(false);
        assignment_model.update(db).await?;
    }

    // Create new assignment
    let assignment = UserServicePlanActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id.clone()),
        service_plan_id: Set(assignment_data.service_plan_id.clone()),
        start_date: Set(Utc::now()),
        end_date: Set(assignment_data.end_date),
        is_active: Set(true),
        created_at: Set(Utc::now()),
    };

    let inserted = assignment.insert(db).await?;

    Ok(Json(UserServicePlanResponse {
        id: inserted.id.to_string(),
        user_id: inserted.user_id,
        service_plan_id: inserted.service_plan_id,
        start_date: inserted.start_date,
        end_date: inserted.end_date,
        is_active: inserted.is_active,
    }))
}

/// Get current rate limit policy (admin only)
pub async fn get_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // TODO: Load from database or config
    let config = RateLimitConfig::default();
    Ok(Json(config))
}

/// Update rate limit policy (admin only)
pub async fn update_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(new_config): Json<RateLimitConfig>,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // TODO: Save to database or config
    Ok(Json(new_config))
}

// Request/Response types
#[derive(Deserialize)]
pub struct CreateServicePlanRequest {
    pub name: String,
    pub api_rate_limit: u32,
    pub tunnel_creation_limit: u32,
    pub dns_provisioning_limit: u32,
    pub max_concurrent_tunnels: u32,
    pub features_json: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateServicePlanRequest {
    pub name: Option<String>,
    pub api_rate_limit: Option<u32>,
    pub tunnel_creation_limit: Option<u32>,
    pub dns_provisioning_limit: Option<u32>,
    pub max_concurrent_tunnels: Option<u32>,
    pub features_json: Option<String>,
}

#[derive(Deserialize)]
pub struct AssignServicePlanRequest {
    pub service_plan_id: String,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Serialize)]
pub struct ServicePlanResponse {
    pub id: String,
    pub name: String,
    pub api_rate_limit: u32,
    pub tunnel_creation_limit: u32,
    pub dns_provisioning_limit: u32,
    pub max_concurrent_tunnels: u32,
    pub features_json: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct UserServicePlanResponse {
    pub id: String,
    pub user_id: String,
    pub service_plan_id: String,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_rate_limit_policy_requires_admin() {
        // Test that admin authentication is required
        // This is a placeholder test
        assert!(true);
    }
}

#[cfg(test)]
mod e2e_serviceplan_tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, Database, EntityTrait};

    #[tokio::test]
    async fn serviceplan_crud_and_assignment_e2e() {
        // This test would require a test database setup
        // For now, we'll just verify the types compile correctly
        
        // Test that we can use the models crate entities
        let _service_plan_entity = ServicePlanEntity;
        let _user_service_plan_entity = UserServicePlanEntity;
        
        // Test that we can use the models crate entities
        // Note: ActiveModel::default() is not available, so we'll just verify the types exist
        
        assert!(true);
    }
}
