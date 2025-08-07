use sea_orm::entity::prelude::*;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Payment information entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    
    pub user_id: String,
    pub stripe_customer_id: Option<String>,
    pub stripe_subscription_id: Option<String>,
    pub last_payment_date: Option<DateTime<Utc>>,
    pub next_payment_date: Option<DateTime<Utc>>,
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