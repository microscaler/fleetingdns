use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "pricing")]
#[allow(dead_code)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub service_plan_id: String,
    pub price: f64,
    pub currency: String,
    pub region: String,
    pub valid_from: NaiveDateTime,
    pub valid_to: NaiveDateTime,
    pub description: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No relations defined")
    }
}

impl ActiveModelBehavior for ActiveModel {}
