use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tunnel")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub user_id: String,
    pub remote_addr: String,
    pub local_addr: String,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub status: String,
    pub bytes_transferred: i64,
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
