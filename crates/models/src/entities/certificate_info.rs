use sea_orm::entity::prelude::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Certificate information entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "certificate_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub serial: String,

    pub certificate: String,
    pub private_key: String,
    pub fingerprint: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub subject: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::tunnel::Entity")]
    Tunnel,
}

impl Related<super::tunnel::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tunnel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
