use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use chrono::NaiveDateTime;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "billing_event")]
pub struct Model {
    #[sea_orm(primary_key, column_name = "id")]
    pub id: String,
    #[sea_orm(column_name = "user_id")]
    pub user_id: String,
    #[sea_orm(column_name = "service_plan_id")]
    pub service_plan_id: String,
    #[sea_orm(column_name = "event_type")]
    pub event_type: String,
    #[sea_orm(column_name = "amount")]
    pub amount: f64,
    #[sea_orm(column_name = "event_time")]
    pub event_time: NaiveDateTime,
    #[sea_orm(column_name = "details_json")]
    pub details_json: String,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub user_id: Set<String>,
    pub service_plan_id: Set<String>,
    pub event_type: Set<String>,
    pub amount: Set<f64>,
    pub event_time: Set<NaiveDateTime>,
    pub details_json: Set<String>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, UserId, ServicePlanId, EventType, Amount, EventTime, DetailsJson }
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