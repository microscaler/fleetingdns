use sea_orm::entity::prelude::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Authentication token entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_tokens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub token: String,

    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub user_id: String,
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
