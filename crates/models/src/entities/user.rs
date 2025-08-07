use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// User entity representing GitHub users
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    
    #[sea_orm(unique)]
    pub github_user_id: String,
    
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub public_repos: Option<i32>,
    pub followers: Option<i32>,
    pub following: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_at_db: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_service_plan::Entity")]
    UserServicePlan,
    
    #[sea_orm(has_many = "super::tunnel::Entity")]
    Tunnel,
    
    #[sea_orm(has_many = "super::auth_token::Entity")]
    AuthToken,
    
    #[sea_orm(has_many = "super::payment_info::Entity")]
    PaymentInfo,
    
    #[sea_orm(has_many = "super::user_usage::Entity")]
    UserUsage,
    
    #[sea_orm(has_many = "super::audit_log::Entity")]
    AuditLog,
    
    #[sea_orm(has_many = "super::billing_event::Entity")]
    BillingEvent,
}

impl Related<super::user_service_plan::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServicePlan.def()
    }
}

impl Related<super::tunnel::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tunnel.def()
    }
}

impl Related<super::auth_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuthToken.def()
    }
}

impl Related<super::payment_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PaymentInfo.def()
    }
}

impl Related<super::user_usage::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserUsage.def()
    }
}

impl Related<super::audit_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuditLog.def()
    }
}

impl Related<super::billing_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillingEvent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 