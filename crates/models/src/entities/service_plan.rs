use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Service plan entity for different subscription tiers
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "service_plans")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    
    pub name: String,
    pub api_rate_limit: i32,
    pub tunnel_creation_limit: i32,
    pub dns_provisioning_limit: i32,
    pub max_concurrent_tunnels: i32,
    pub features_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_service_plan::Entity")]
    UserServicePlan,
    
    #[sea_orm(has_many = "super::billing_event::Entity")]
    BillingEvent,
}

impl Related<super::user_service_plan::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServicePlan.def()
    }
}

impl Related<super::billing_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillingEvent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 