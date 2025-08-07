use sea_orm::entity::prelude::*;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Billing event entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "billing_event")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    
    pub user_id: String,
    pub service_plan_id: String,
    pub event_type: String,
    pub amount: f64,
    pub event_time: DateTime<Utc>,
    pub details_json: Option<String>,
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
    
    #[sea_orm(
        belongs_to = "super::service_plan::Entity",
        from = "Column::ServicePlanId",
        to = "super::service_plan::Column::Id"
    )]
    ServicePlan,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::service_plan::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ServicePlan.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 