use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "auth_token")]
#[allow(dead_code)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub token: String,
    pub token_type: String,
    pub expires_at: NaiveDateTime,
    pub user_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
#[allow(dead_code)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No relations defined")
    }
}

impl ActiveModelBehavior for ActiveModel {}
