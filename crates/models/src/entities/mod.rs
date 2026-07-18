//! SeaORM entities for FleetingDNS database models

pub mod api_stats;
pub mod audit_log;
pub mod auth_token;
pub mod billing_event;
pub mod ca_stats;
pub mod certificate_info;
pub mod payment_info;
pub mod service_plan;
pub mod ssh_key_pair;
pub mod tunnel;
pub mod user;
pub mod user_service_plan;
pub mod user_usage;

// Re-export specific types to avoid naming conflicts
pub use api_stats::{
    ActiveModel as ApiStatsActiveModel, Column as ApiStatsColumn, Entity as ApiStats,
    Model as ApiStatsModel,
};
pub use audit_log::{
    ActiveModel as AuditLogActiveModel, Column as AuditLogColumn, Entity as AuditLog,
    Model as AuditLogModel,
};
pub use auth_token::{
    ActiveModel as AuthTokenActiveModel, Column as AuthTokenColumn, Entity as AuthToken,
    Model as AuthTokenModel,
};
pub use billing_event::{
    ActiveModel as BillingEventActiveModel, Column as BillingEventColumn, Entity as BillingEvent,
    Model as BillingEventModel,
};
pub use ca_stats::{
    ActiveModel as CaStatsActiveModel, Column as CaStatsColumn, Entity as CaStats,
    Model as CaStatsModel,
};
pub use certificate_info::{
    ActiveModel as CertificateInfoActiveModel, Column as CertificateInfoColumn,
    Entity as CertificateInfo, Model as CertificateInfoModel,
};
pub use payment_info::{
    ActiveModel as PaymentInfoActiveModel, Column as PaymentInfoColumn, Entity as PaymentInfo,
    Model as PaymentInfoModel,
};
pub use service_plan::{
    ActiveModel as ServicePlanActiveModel, Column as ServicePlanColumn, Entity as ServicePlan,
    Model as ServicePlanModel,
};
pub use ssh_key_pair::{
    ActiveModel as SshKeyPairActiveModel, Column as SshKeyPairColumn, Entity as SshKeyPair,
    Model as SshKeyPairModel,
};
pub use tunnel::{
    ActiveModel as TunnelActiveModel, Column as TunnelColumn, Entity as Tunnel,
    Model as TunnelModel,
};
pub use user::{
    ActiveModel as UserActiveModel, Column as UserColumn, Entity as User, Model as UserModel,
};
pub use user_service_plan::{
    ActiveModel as UserServicePlanActiveModel, Column as UserServicePlanColumn,
    Entity as UserServicePlan, Model as UserServicePlanModel,
};
pub use user_usage::{
    ActiveModel as UserUsageActiveModel, Column as UserUsageColumn, Entity as UserUsage,
    Model as UserUsageModel,
};
