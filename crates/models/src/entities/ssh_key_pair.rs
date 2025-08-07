use sea_orm::entity::prelude::*;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// SSH key pair entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ssh_key_pairs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    
    pub private_key: String,
    pub public_key: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
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