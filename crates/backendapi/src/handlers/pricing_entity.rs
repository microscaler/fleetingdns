use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use chrono::NaiveDateTime;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "pricing")]
pub struct Model {
    #[sea_orm(primary_key, column_name = "id")]
    pub id: String,
    #[sea_orm(column_name = "service_plan_id")]
    pub service_plan_id: String,
    #[sea_orm(column_name = "price")]
    pub price: f64,
    #[sea_orm(column_name = "currency")]
    pub currency: String,
    #[sea_orm(column_name = "region")]
    pub region: String,
    #[sea_orm(column_name = "valid_from")]
    pub valid_from: NaiveDateTime,
    #[sea_orm(column_name = "valid_to")]
    pub valid_to: NaiveDateTime,
    #[sea_orm(column_name = "description")]
    pub description: String,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub service_plan_id: Set<String>,
    pub price: Set<f64>,
    pub currency: Set<String>,
    pub region: Set<String>,
    pub valid_from: Set<NaiveDateTime>,
    pub valid_to: Set<NaiveDateTime>,
    pub description: Set<String>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, ServicePlanId, Price, Currency, Region, ValidFrom, ValidTo, Description }
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