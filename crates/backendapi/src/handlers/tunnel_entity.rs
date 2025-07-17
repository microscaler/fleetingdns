use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "tunnel")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub subdomain: String,
    pub fqdn: String,
    pub local_port: i32,
    pub slot: i32,
    pub certificate_serial: String,
    pub ssh_key_pair_id: String,
    pub created_at: chrono::DateTime<Utc>,
    pub expires_at: chrono::DateTime<Utc>,
    pub status: String,
    pub bytes_transferred: i64,
    pub request_count: i64,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<Uuid>,
    pub user_id: Set<Uuid>,
    pub subdomain: Set<String>,
    pub fqdn: Set<String>,
    pub local_port: Set<i32>,
    pub slot: Set<i32>,
    pub certificate_serial: Set<String>,
    pub ssh_key_pair_id: Set<String>,
    pub created_at: Set<chrono::DateTime<Utc>>,
    pub expires_at: Set<chrono::DateTime<Utc>>,
    pub status: Set<String>,
    pub bytes_transferred: Set<i64>,
    pub request_count: Set<i64>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, UserId, Subdomain, Fqdn, LocalPort, Slot, CertificateSerial, SshKeyPairId, CreatedAt, ExpiresAt, Status, BytesTransferred, RequestCount }
#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey { Id }
impl PrimaryKeyTrait for PrimaryKey { type ValueType = Uuid; fn auto_increment() -> bool { false } }
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