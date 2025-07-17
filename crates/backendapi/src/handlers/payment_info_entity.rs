use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use chrono::Utc;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "payment_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub user_id: String,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: String,
    pub last_payment_date: chrono::DateTime<Utc>,
    pub next_payment_date: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub user_id: Set<String>,
    pub stripe_customer_id: Set<String>,
    pub stripe_subscription_id: Set<String>,
    pub last_payment_date: Set<chrono::DateTime<Utc>>,
    pub next_payment_date: Set<chrono::DateTime<Utc>>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, UserId, StripeCustomerId, StripeSubscriptionId, LastPaymentDate, NextPaymentDate }
#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey { Id }
impl PrimaryKeyTrait for PrimaryKey { type ValueType = String; fn auto_increment() -> bool { false } }
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
pub struct Entity;
impl EntityTrait for Entity {
    type Model = Model;
    type Column = Column;
    type PrimaryKey = PrimaryKey;
    type Relation = Relation;
    type ActiveModel = ActiveModel;
} 