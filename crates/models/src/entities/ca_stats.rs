use sea_orm::entity::prelude::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Certificate Authority statistics entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ca_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,

    pub certificates_issued: i32,
    pub active_certificates: i32,
    pub expired_certificates: i32,
    pub issuance_rate: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::api_stats::Entity")]
    ApiStats,
}

impl Related<super::api_stats::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ApiStats.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
