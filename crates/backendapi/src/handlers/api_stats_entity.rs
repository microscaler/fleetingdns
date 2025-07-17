use sea_orm::{EntityTrait, PrimaryKeyTrait, DeriveColumn, DerivePrimaryKey, DeriveRelation, EnumIter, Set, ActiveModelBehavior};

#[derive(Clone, Debug, sea_orm::DeriveEntityModel)]
#[sea_orm(table_name = "api_stats")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub active_tunnels: i32,
    pub tunnels_created_today: i32,
    pub bytes_transferred_today: i64,
    pub uptime_seconds: i32,
    pub ca_stats_id: String,
}

#[derive(Clone, Debug, Default, sea_orm::DeriveActiveModel)]
pub struct ActiveModel {
    pub id: Set<String>,
    pub active_tunnels: Set<i32>,
    pub tunnels_created_today: Set<i32>,
    pub bytes_transferred_today: Set<i64>,
    pub uptime_seconds: Set<i32>,
    pub ca_stats_id: Set<String>,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column { Id, ActiveTunnels, TunnelsCreatedToday, BytesTransferredToday, UptimeSeconds, CaStatsId }
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