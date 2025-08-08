//! SeaORM entities for FleetingDNS database models

pub mod user;
pub mod service_plan;
pub mod user_service_plan;
pub mod auth_token;
pub mod tunnel;
pub mod ssh_key_pair;
pub mod certificate_info;
pub mod api_stats;
pub mod ca_stats;
pub mod payment_info;
pub mod user_usage;
pub mod audit_log;
pub mod billing_event;

// Re-export specific types to avoid naming conflicts
pub use user::{Entity as User, Model as UserModel, ActiveModel as UserActiveModel, Column as UserColumn};
pub use service_plan::{Entity as ServicePlan, Model as ServicePlanModel, ActiveModel as ServicePlanActiveModel, Column as ServicePlanColumn};
pub use user_service_plan::{Entity as UserServicePlan, Model as UserServicePlanModel, ActiveModel as UserServicePlanActiveModel, Column as UserServicePlanColumn};
pub use auth_token::{Entity as AuthToken, Model as AuthTokenModel, ActiveModel as AuthTokenActiveModel, Column as AuthTokenColumn};
pub use tunnel::{Entity as Tunnel, Model as TunnelModel, ActiveModel as TunnelActiveModel, Column as TunnelColumn};
pub use ssh_key_pair::{Entity as SshKeyPair, Model as SshKeyPairModel, ActiveModel as SshKeyPairActiveModel, Column as SshKeyPairColumn};
pub use certificate_info::{Entity as CertificateInfo, Model as CertificateInfoModel, ActiveModel as CertificateInfoActiveModel, Column as CertificateInfoColumn};
pub use api_stats::{Entity as ApiStats, Model as ApiStatsModel, ActiveModel as ApiStatsActiveModel, Column as ApiStatsColumn};
pub use ca_stats::{Entity as CaStats, Model as CaStatsModel, ActiveModel as CaStatsActiveModel, Column as CaStatsColumn};
pub use payment_info::{Entity as PaymentInfo, Model as PaymentInfoModel, ActiveModel as PaymentInfoActiveModel, Column as PaymentInfoColumn};
pub use user_usage::{Entity as UserUsage, Model as UserUsageModel, ActiveModel as UserUsageActiveModel, Column as UserUsageColumn};
pub use audit_log::{Entity as AuditLog, Model as AuditLogModel, ActiveModel as AuditLogActiveModel, Column as AuditLogColumn};
pub use billing_event::{Entity as BillingEvent, Model as BillingEventModel, ActiveModel as BillingEventActiveModel, Column as BillingEventColumn}; 