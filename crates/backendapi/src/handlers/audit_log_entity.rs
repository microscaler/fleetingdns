use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use chrono::NaiveDateTime;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, column_name = "id")]
    pub id: String,
    #[sea_orm(column_name = "user_id")]
    pub user_id: String,
    #[sea_orm(column_name = "action")]
    pub action: String,
    #[sea_orm(column_name = "resource")]
    pub resource: String,
    #[sea_orm(column_name = "timestamp")]
    pub timestamp: NaiveDateTime,
    #[sea_orm(column_name = "details_json")]
    pub details_json: String,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub user_id: Set<String>,
    pub action: Set<String>,
    pub resource: Set<String>,
    pub timestamp: Set<NaiveDateTime>,
    pub details_json: Set<String>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, UserId, Action, Resource, Timestamp, DetailsJson }
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