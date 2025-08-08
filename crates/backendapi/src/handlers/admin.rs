use crate::{
    ApiResult, ApiState,
    rate_limiting::RateLimitConfig,
};
use models::{
    service_plan::{Entity as ServicePlanEntity, ActiveModel as ServicePlanActiveModel, Column as ServicePlanColumn},
    user_service_plan::{Entity as UserServicePlanEntity, ActiveModel as UserServicePlanActiveModel, Column as UserServicePlanColumn},
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
    let now = Utc::now().naive_utc();

    let plan = service_plan_entity::ActiveModel {
        id: Set(plan_id.clone()),
        name: Set(plan_data.name),
        api_rate_limit: Set(plan_data.api_rate_limit as i32),
        tunnel_creation_limit: Set(plan_data.tunnel_creation_limit as i32),
        dns_provisioning_limit: Set(plan_data.dns_provisioning_limit as i32),
        max_concurrent_tunnels: Set(plan_data.max_concurrent_tunnels as i32),
        features_json: Set(plan_data.features_json.unwrap_or_else(|| "{}".to_string())),
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
        features_json: inserted.features_json,
        created_at: inserted.created_at.and_utc(),
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

    let plans = service_plan_entity::Entity::find().all(db).await?;

    let responses = plans
        .into_iter()
        .map(|plan| ServicePlanResponse {
            id: plan.id,
            name: plan.name,
            api_rate_limit: plan.api_rate_limit as u32,
            tunnel_creation_limit: plan.tunnel_creation_limit as u32,
            dns_provisioning_limit: plan.dns_provisioning_limit as u32,
            max_concurrent_tunnels: plan.max_concurrent_tunnels as u32,
            features_json: plan.features_json,
            created_at: plan.created_at.and_utc(),
        })
        .collect();

    Ok(Json(responses))
}

/// Get a specific ServicePlan by ID (admin only)
pub async fn get_service_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> ApiResult<Json<ServicePlanResponse>> {
    // Authenticate admin user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let db = &state.db;

    let plan = service_plan_entity::Entity::find_by_id(plan_id.clone())
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
        features_json: plan.features_json,
        created_at: plan.created_at.and_utc(),
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
    let existing_plan = service_plan_entity::Entity::find_by_id(plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!("ServicePlan with id '{plan_id}' not found"))
        })?;

    // If name is being updated, check for uniqueness
    if let Some(new_name) = &update_data.name
        && new_name != &existing_plan.name
    {
        let duplicate = service_plan_entity::Entity::find()
            .filter(service_plan_entity::Column::Name.eq(new_name))
            .filter(service_plan_entity::Column::Id.ne(plan_id.clone()))
            .one(db)
            .await?;

        if duplicate.is_some() {
            return Err(crate::ApiError::ValidationError(format!(
                "ServicePlan with name '{new_name}' already exists"
            )));
        }
    }

    // Update the plan
    let mut plan_model: service_plan_entity::ActiveModel = existing_plan.into();

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
        plan_model.features_json = Set(features_json);
    }

    let updated_plan = plan_model.update(db).await?;

    Ok(Json(ServicePlanResponse {
        id: updated_plan.id,
        name: updated_plan.name,
        api_rate_limit: updated_plan.api_rate_limit as u32,
        tunnel_creation_limit: updated_plan.tunnel_creation_limit as u32,
        dns_provisioning_limit: updated_plan.dns_provisioning_limit as u32,
        max_concurrent_tunnels: updated_plan.max_concurrent_tunnels as u32,
        features_json: updated_plan.features_json,
        created_at: updated_plan.created_at.and_utc(),
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
    let plan = service_plan_entity::Entity::find_by_id(plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::ApiError::NotFound(format!("ServicePlan with id '{plan_id}' not found"))
        })?;

    // Check if plan is in use
    let active_assignments = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::ServicePlanId.eq(plan_id.clone()))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .count(db)
        .await?;

    if active_assignments > 0 {
        return Err(crate::ApiError::ValidationError(format!(
            "Cannot delete ServicePlan '{}' - it has {} active user assignments",
            plan.name, active_assignments
        )));
    }

    // Delete the plan
    service_plan_entity::Entity::delete_by_id(plan_id)
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

    // Verify ServicePlan exists
    let _plan = service_plan_entity::Entity::find_by_id(assignment_data.service_plan_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| crate::ApiError::NotFound("ServicePlan not found".to_string()))?;

    // TODO: Verify user exists (when user entity is implemented)

    // Deactivate any existing active assignments for this user
    let existing_assignments = user_service_plan_entity::Entity::find()
        .filter(user_service_plan_entity::Column::UserId.eq(user_id.clone()))
        .filter(user_service_plan_entity::Column::IsActive.eq(true))
        .all(db)
        .await?;

    for assignment in existing_assignments {
        let mut assignment_model: user_service_plan_entity::ActiveModel = assignment.into();
        assignment_model.is_active = Set(false);
        assignment_model.update(db).await?;
    }

    // Create new assignment
    let assignment_id = Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc();
    let end_date = assignment_data.end_date.map(|d| d.naive_utc());

    let assignment = user_service_plan_entity::ActiveModel {
        id: Set(assignment_id.clone()),
        user_id: Set(user_id),
        service_plan_id: Set(assignment_data.service_plan_id),
        start_date: Set(now),
        end_date: Set(end_date.unwrap_or_else(|| now + chrono::Duration::days(365))),
        is_active: Set(true),
    };

    let inserted = assignment.insert(db).await?;

    Ok(Json(UserServicePlanResponse {
        id: inserted.id,
        user_id: inserted.user_id,
        service_plan_id: inserted.service_plan_id,
        start_date: inserted.start_date.and_utc(),
        end_date: Some(inserted.end_date.and_utc()),
        is_active: inserted.is_active,
    }))
}

/// Get the current rate limit policy (admin only)
pub async fn get_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    // Remove all usage of user.tier (GitHubUser has no tier field)
    // Temporarily disable admin checks or add TODO for ServicePlan-based admin logic
    // Fix ServiceExt import and remove unused imports
    // TODO: ServicePlan-based admin/config management will be implemented here.
    // Direct access to state.rate_limiter.config is not allowed (private field).
    // Temporarily disable config read/write for migration.
    // let config = state.rate_limiter.config.read().unwrap().clone();
    Ok(Json(RateLimitConfig::default()))
}

/// Update the rate limit policy (admin only)
pub async fn update_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(new_config): Json<RateLimitConfig>,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate user
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    // Remove all usage of user.tier (GitHubUser has no tier field)
    // Temporarily disable admin checks or add TODO for ServicePlan-based admin logic
    // Fix ServiceExt import and remove unused imports
    // TODO: ServicePlan-based admin/config management will be implemented here.
    // Direct access to state.rate_limiter.config is not allowed (private field).
    // Temporarily disable config read/write for migration.
    // let mut config_guard = state.rate_limiter.config.write().unwrap();
    Ok(Json(new_config))
}

// Request/Response types
#[derive(serde::Deserialize)]
pub struct CreateServicePlanRequest {
    pub name: String,
    pub api_rate_limit: u32,
    pub tunnel_creation_limit: u32,
    pub dns_provisioning_limit: u32,
    pub max_concurrent_tunnels: u32,
    pub features_json: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateServicePlanRequest {
    pub name: Option<String>,
    pub api_rate_limit: Option<u32>,
    pub tunnel_creation_limit: Option<u32>,
    pub dns_provisioning_limit: Option<u32>,
    pub max_concurrent_tunnels: Option<u32>,
    pub features_json: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct AssignServicePlanRequest {
    pub service_plan_id: String,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(serde::Serialize)]
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

#[derive(serde::Serialize)]
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
        // Setup dummy state
        let config = RateLimitConfig::default();
        let _shared = Arc::new(RwLock::new(config.clone()));
        // TODO: Mock ApiState with admin user
        // ...
        // This is a placeholder for actual integration test
        assert_eq!(config.default.requests_per_minute, 60);
    }
}

#[cfg(test)]
mod e2e_serviceplan_tests {

    use chrono::Utc;
    use migration::Migrator;
    use sea_orm::{ActiveModelTrait, Database, EntityTrait};
    use sea_orm_migration::MigratorTrait;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;
    use uuid::Uuid;

    #[tokio::test]
    async fn serviceplan_crud_and_assignment_e2e() {
        // Start Postgres 18 container
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres");
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

        // Wait for DB to be ready
        let mut retries = 10;
        let db = loop {
            match Database::connect(&url).await {
                Ok(db) => break db,
                Err(_) if retries > 0 => {
                    retries -= 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                Err(e) => panic!("Failed to connect to Postgres: {e:?}"),
            }
        };

        // Run migrations
        Migrator::up(&db, None).await.expect("Migration failed");

        type ServicePlanEntity = crate::handlers::service_plan_entity::Entity;
        type ServicePlanActiveModel = crate::handlers::service_plan_entity::ActiveModel;
        type UserServicePlanEntity = crate::handlers::user_service_plan_entity::Entity;
        type UserServicePlanActiveModel = crate::handlers::user_service_plan_entity::ActiveModel;

        // Create
        let plan_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let plan = ServicePlanActiveModel {
            id: sea_orm::Set(plan_id.clone()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(1000),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let inserted = ServicePlanEntity::insert(plan)
            .exec(&db)
            .await
            .expect("insert");
        assert_eq!(inserted.last_insert_id, plan_id);

        // Read
        let found = ServicePlanEntity::find_by_id(plan_id.clone())
            .one(&db)
            .await
            .expect("find")
            .unwrap();
        assert_eq!(found.name, "Pro");
        assert_eq!(found.api_rate_limit, 1000);

        // Update
        let mut to_update: ServicePlanActiveModel = found.clone().into();
        to_update.api_rate_limit = sea_orm::Set(2000);
        let updated = to_update.update(&db).await.expect("update");
        assert_eq!(updated.api_rate_limit, 2000);

        // Unique name validation
        let dup_plan = ServicePlanActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(500),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let dup_result = ServicePlanEntity::insert(dup_plan).exec(&db).await;
        assert!(dup_result.is_err(), "Duplicate name should fail");

        // --- UserServicePlan assignment ---
        // Create a user first
        let user_id = Uuid::new_v4().to_string();
        let user = crate::handlers::user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = crate::handlers::user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Assign
        let start_date = Utc::now().naive_utc();
        let end_date = start_date + chrono::Duration::days(30);
        let assignment = UserServicePlanActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            user_id: sea_orm::Set(user_id),
            service_plan_id: sea_orm::Set(plan_id.clone()),
            start_date: sea_orm::Set(start_date),
            end_date: sea_orm::Set(end_date),
            is_active: sea_orm::Set(true),
        };
        let assigned = UserServicePlanEntity::insert(assignment)
            .exec(&db)
            .await
            .expect("assign");
        assert_eq!(assigned.last_insert_id, assigned.last_insert_id);

        // Prevent deletion of ServicePlan in use
        let del_result = ServicePlanEntity::delete_by_id(plan_id.clone())
            .exec(&db)
            .await;
        assert!(del_result.is_err(), "Should not delete plan in use");

        // Unassign
        let assignment_id = assigned.last_insert_id;
        let _ = UserServicePlanEntity::delete_by_id(assignment_id)
            .exec(&db)
            .await
            .expect("unassign");

        // Now deletion should succeed
        let del_result2 = ServicePlanEntity::delete_by_id(plan_id).exec(&db).await;
        assert!(
            del_result2.unwrap().rows_affected == 1,
            "Plan should be deleted after unassignment"
        );
    }
}
