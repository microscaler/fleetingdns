use sea_orm::entity::prelude::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// API statistics entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,

    pub active_tunnels: i32,
    pub tunnels_created_today: i32,
    pub bytes_transferred_today: i64,
    pub uptime_seconds: i64,
    pub ca_stats_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::ca_stats::Entity",
        from = "Column::CaStatsId",
        to = "super::ca_stats::Column::Id"
    )]
    CaStats,
}

impl Related<super::ca_stats::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CaStats.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
