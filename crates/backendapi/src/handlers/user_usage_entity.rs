use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_usage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub user_id: String,
    pub period_start: NaiveDateTime,
    pub api_calls_count: i32,
    pub tunnels_created_count: i32,
    pub dns_operations_count: i32,
    pub active_tunnels_count: i32,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No relations defined")
    }
}

impl ActiveModelBehavior for ActiveModel {}
