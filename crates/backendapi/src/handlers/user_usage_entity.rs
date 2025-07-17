use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use chrono::Utc;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "user_usage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub user_id: String,
    pub period_start: chrono::DateTime<Utc>,
    pub api_calls_count: i32,
    pub tunnels_created_count: i32,
    pub dns_operations_count: i32,
    pub active_tunnels_count: i32,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub user_id: Set<String>,
    pub period_start: Set<chrono::DateTime<Utc>>,
    pub api_calls_count: Set<i32>,
    pub tunnels_created_count: Set<i32>,
    pub dns_operations_count: Set<i32>,
    pub active_tunnels_count: Set<i32>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, UserId, PeriodStart, ApiCallsCount, TunnelsCreatedCount, DnsOperationsCount, ActiveTunnelsCount }
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