use sea_orm::entity::prelude::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tunnel entity representing ephemeral tunnels
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tunnels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,

    pub github_user_id: String,
    pub github_username: String,
    pub subdomain: String,
    pub fqdn: String,
    pub local_port: i32,
    pub slot: i32,
    pub certificate_serial: Option<String>,
    pub ssh_key_pair_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: String,
    pub bytes_transferred: i64,
    pub request_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::GithubUserId",
        to = "super::user::Column::GithubUserId"
    )]
    User,

    #[sea_orm(
        belongs_to = "super::certificate_info::Entity",
        from = "Column::CertificateSerial",
        to = "super::certificate_info::Column::Serial"
    )]
    CertificateInfo,

    #[sea_orm(
        belongs_to = "super::ssh_key_pair::Entity",
        from = "Column::SshKeyPairId",
        to = "super::ssh_key_pair::Column::Id"
    )]
    SshKeyPair,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::certificate_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CertificateInfo.def()
    }
}

impl Related<super::ssh_key_pair::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SshKeyPair.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
