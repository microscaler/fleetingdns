use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "api_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub active_tunnels: i32,
    pub tunnels_created_today: i32,
    pub bytes_transferred_today: i64,
    pub uptime_seconds: i64,
    pub ca_stats_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No relations defined")
    }
}

impl ActiveModelBehavior for ActiveModel {} 