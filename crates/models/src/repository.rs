//! Repository pattern for database operations
//!
//! This module provides repository traits and implementations for database operations
//! using SeaORM entities.

use crate::entities::{service_plan, tunnel, user};
use crate::{ModelError, ModelResult};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

/// Repository trait for User entity
#[async_trait]
pub trait UserRepository {
    async fn find_by_github_user_id(
        &self,
        github_user_id: &str,
    ) -> ModelResult<Option<user::Model>>;
    async fn create(&self, user: user::ActiveModel) -> ModelResult<user::Model>;
    async fn update(&self, user: user::ActiveModel) -> ModelResult<user::Model>;
    async fn delete(&self, id: &str) -> ModelResult<bool>;
}

/// Repository trait for ServicePlan entity
#[async_trait]
pub trait ServicePlanRepository {
    async fn find_by_id(&self, id: &str) -> ModelResult<Option<service_plan::Model>>;
    async fn find_all(&self) -> ModelResult<Vec<service_plan::Model>>;
    async fn create(&self, plan: service_plan::ActiveModel) -> ModelResult<service_plan::Model>;
    async fn update(&self, plan: service_plan::ActiveModel) -> ModelResult<service_plan::Model>;
}

/// Repository trait for Tunnel entity
#[async_trait]
pub trait TunnelRepository {
    async fn find_by_id(&self, id: uuid::Uuid) -> ModelResult<Option<tunnel::Model>>;
    async fn find_by_github_user_id(&self, github_user_id: &str)
        -> ModelResult<Vec<tunnel::Model>>;
    async fn find_active_by_github_user_id(
        &self,
        github_user_id: &str,
    ) -> ModelResult<Vec<tunnel::Model>>;
    async fn create(&self, tunnel: tunnel::ActiveModel) -> ModelResult<tunnel::Model>;
    async fn update(&self, tunnel: tunnel::ActiveModel) -> ModelResult<tunnel::Model>;
    async fn delete(&self, id: uuid::Uuid) -> ModelResult<bool>;
}

/// SeaORM implementation of UserRepository
pub struct SeaOrmUserRepository {
    pub db: DatabaseConnection,
}

impl SeaOrmUserRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UserRepository for SeaOrmUserRepository {
    async fn find_by_github_user_id(
        &self,
        github_user_id: &str,
    ) -> ModelResult<Option<user::Model>> {
        let user = user::Entity::find()
            .filter(user::Column::GithubUserId.eq(github_user_id))
            .one(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    async fn create(&self, user: user::ActiveModel) -> ModelResult<user::Model> {
        let user = user
            .insert(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    async fn update(&self, user: user::ActiveModel) -> ModelResult<user::Model> {
        let user = user
            .update(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    async fn delete(&self, id: &str) -> ModelResult<bool> {
        let result = user::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected > 0)
    }
}

/// SeaORM implementation of ServicePlanRepository
pub struct SeaOrmServicePlanRepository {
    pub db: DatabaseConnection,
}

impl SeaOrmServicePlanRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ServicePlanRepository for SeaOrmServicePlanRepository {
    async fn find_by_id(&self, id: &str) -> ModelResult<Option<service_plan::Model>> {
        let plan = service_plan::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(plan)
    }

    async fn find_all(&self) -> ModelResult<Vec<service_plan::Model>> {
        let plans = service_plan::Entity::find()
            .all(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(plans)
    }

    async fn create(&self, plan: service_plan::ActiveModel) -> ModelResult<service_plan::Model> {
        let plan = plan
            .insert(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(plan)
    }

    async fn update(&self, plan: service_plan::ActiveModel) -> ModelResult<service_plan::Model> {
        let plan = plan
            .update(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(plan)
    }
}

/// SeaORM implementation of TunnelRepository
pub struct SeaOrmTunnelRepository {
    pub db: DatabaseConnection,
}

impl SeaOrmTunnelRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TunnelRepository for SeaOrmTunnelRepository {
    async fn find_by_id(&self, id: uuid::Uuid) -> ModelResult<Option<tunnel::Model>> {
        let tunnel = tunnel::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(tunnel)
    }

    async fn find_by_github_user_id(
        &self,
        github_user_id: &str,
    ) -> ModelResult<Vec<tunnel::Model>> {
        let tunnels = tunnel::Entity::find()
            .filter(tunnel::Column::GithubUserId.eq(github_user_id))
            .all(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(tunnels)
    }

    async fn find_active_by_github_user_id(
        &self,
        github_user_id: &str,
    ) -> ModelResult<Vec<tunnel::Model>> {
        let tunnels = tunnel::Entity::find()
            .filter(tunnel::Column::GithubUserId.eq(github_user_id))
            .filter(tunnel::Column::Status.eq("active"))
            .filter(tunnel::Column::ExpiresAt.gt(chrono::Utc::now()))
            .all(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(tunnels)
    }

    async fn create(&self, tunnel: tunnel::ActiveModel) -> ModelResult<tunnel::Model> {
        let tunnel = tunnel
            .insert(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(tunnel)
    }

    async fn update(&self, tunnel: tunnel::ActiveModel) -> ModelResult<tunnel::Model> {
        let tunnel = tunnel
            .update(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(tunnel)
    }

    async fn delete(&self, id: uuid::Uuid) -> ModelResult<bool> {
        let result = tunnel::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected > 0)
    }
}
