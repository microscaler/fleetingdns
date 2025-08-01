use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ca_stats")]
#[allow(dead_code)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub certificates_issued: i32,
    pub active_certificates: i32,
    pub expired_certificates: i32,
    pub issuance_rate: f64,
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
