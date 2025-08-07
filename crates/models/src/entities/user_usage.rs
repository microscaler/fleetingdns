use sea_orm::entity::prelude::*;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// User usage tracking entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_usage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    
    pub user_id: String,
    pub period_start: DateTime<Utc>,
    pub api_calls_count: i32,
    pub tunnels_created_count: i32,
    pub dns_operations_count: i32,
    pub active_tunnels_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 