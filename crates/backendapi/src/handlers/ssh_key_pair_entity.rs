use sea_orm::entity::prelude::*;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ssh_key_pair")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub private_key: String,
    pub public_key: String,
    pub fingerprint: String,
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
