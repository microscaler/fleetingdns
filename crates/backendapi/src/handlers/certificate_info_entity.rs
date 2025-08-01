use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "certificate_info")]
#[allow(dead_code)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub serial: String,
    pub certificate: String,
    pub private_key: String,
    pub fingerprint: String,
    pub issued_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub subject: String,
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
