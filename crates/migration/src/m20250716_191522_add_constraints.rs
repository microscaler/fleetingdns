use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add unique constraint on service_plan.name
        manager
            .create_index(
                Index::create()
                    .name("idx_service_plan_name_unique")
                    .table(ServicePlan::Table)
                    .col(ServicePlan::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on user.github_id
        manager
            .create_index(
                Index::create()
                    .name("idx_user_github_id_unique")
                    .table(User::Table)
                    .col(User::GithubId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on user.email
        manager
            .create_index(
                Index::create()
                    .name("idx_user_email_unique")
                    .table(User::Table)
                    .col(User::Email)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on tunnel.subdomain
        manager
            .create_index(
                Index::create()
                    .name("idx_tunnel_subdomain_unique")
                    .table(Tunnel::Table)
                    .col(Tunnel::Subdomain)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on tunnel.fqdn
        manager
            .create_index(
                Index::create()
                    .name("idx_tunnel_fqdn_unique")
                    .table(Tunnel::Table)
                    .col(Tunnel::Fqdn)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on ssh_key_pair.fingerprint
        manager
            .create_index(
                Index::create()
                    .name("idx_ssh_key_pair_fingerprint_unique")
                    .table(SshKeyPair::Table)
                    .col(SshKeyPair::Fingerprint)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on certificate_info.fingerprint
        manager
            .create_index(
                Index::create()
                    .name("idx_certificate_info_fingerprint_unique")
                    .table(CertificateInfo::Table)
                    .col(CertificateInfo::Fingerprint)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add foreign key constraints
        // pricing.service_plan_id -> service_plan.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_pricing_service_plan_id")
                    .from(Pricing::Table, Pricing::ServicePlanId)
                    .to(ServicePlan::Table, ServicePlan::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // user_service_plan.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_service_plan_user_id")
                    .from(UserServicePlan::Table, UserServicePlan::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // user_service_plan.service_plan_id -> service_plan.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_service_plan_service_plan_id")
                    .from(UserServicePlan::Table, UserServicePlan::ServicePlanId)
                    .to(ServicePlan::Table, ServicePlan::Id)
                    .on_delete(ForeignKeyAction::Restrict)
                    .to_owned(),
            )
            .await?;

        // tunnel.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tunnel_user_id")
                    .from(Tunnel::Table, Tunnel::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // tunnel.ssh_key_pair_id -> ssh_key_pair.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tunnel_ssh_key_pair_id")
                    .from(Tunnel::Table, Tunnel::SshKeyPairId)
                    .to(SshKeyPair::Table, SshKeyPair::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        // tunnel.certificate_serial -> certificate_info.serial
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tunnel_certificate_serial")
                    .from(Tunnel::Table, Tunnel::CertificateSerial)
                    .to(CertificateInfo::Table, CertificateInfo::Serial)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        // auth_token.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_auth_token_user_id")
                    .from(AuthToken::Table, AuthToken::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // payment_info.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_payment_info_user_id")
                    .from(PaymentInfo::Table, PaymentInfo::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // user_usage.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_usage_user_id")
                    .from(UserUsage::Table, UserUsage::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // audit_log.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_audit_log_user_id")
                    .from(AuditLog::Table, AuditLog::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // billing_event.user_id -> user.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_billing_event_user_id")
                    .from(BillingEvent::Table, BillingEvent::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // billing_event.service_plan_id -> service_plan.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_billing_event_service_plan_id")
                    .from(BillingEvent::Table, BillingEvent::ServicePlanId)
                    .to(ServicePlan::Table, ServicePlan::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // api_stats.ca_stats_id -> ca_stats.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_api_stats_ca_stats_id")
                    .from(ApiStats::Table, ApiStats::CaStatsId)
                    .to(CaStats::Table, CaStats::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop foreign keys first
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_api_stats_ca_stats_id")
                    .table(ApiStats::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_billing_event_service_plan_id")
                    .table(BillingEvent::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_billing_event_user_id")
                    .table(BillingEvent::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_audit_log_user_id")
                    .table(AuditLog::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_user_usage_user_id")
                    .table(UserUsage::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_payment_info_user_id")
                    .table(PaymentInfo::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_auth_token_user_id")
                    .table(AuthToken::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_tunnel_certificate_serial")
                    .table(Tunnel::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_tunnel_ssh_key_pair_id")
                    .table(Tunnel::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_tunnel_user_id")
                    .table(Tunnel::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_user_service_plan_service_plan_id")
                    .table(UserServicePlan::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_user_service_plan_user_id")
                    .table(UserServicePlan::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_pricing_service_plan_id")
                    .table(Pricing::Table)
                    .to_owned(),
            )
            .await?;

        // Drop indexes
        manager
            .drop_index(
                Index::drop()
                    .name("idx_certificate_info_fingerprint_unique")
                    .table(CertificateInfo::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_ssh_key_pair_fingerprint_unique")
                    .table(SshKeyPair::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_tunnel_fqdn_unique")
                    .table(Tunnel::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_tunnel_subdomain_unique")
                    .table(Tunnel::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_email_unique")
                    .table(User::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_github_id_unique")
                    .table(User::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_service_plan_name_unique")
                    .table(ServicePlan::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

// Table and column idents for all tables
#[derive(DeriveIden)]
#[allow(dead_code)]
enum User {
    Table,
    Id,
    GithubId,
    Username,
    Email,
    AvatarUrl,
    CreatedAt,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum ServicePlan {
    Table,
    Id,
    Name,
    ApiRateLimit,
    TunnelCreationLimit,
    DnsProvisioningLimit,
    MaxConcurrentTunnels,
    FeaturesJson,
    CreatedAt,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum Pricing {
    Table,
    Id,
    ServicePlanId,
    Price,
    Currency,
    Region,
    ValidFrom,
    ValidTo,
    Description,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum UserServicePlan {
    Table,
    Id,
    UserId,
    ServicePlanId,
    StartDate,
    EndDate,
    IsActive,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum Tunnel {
    Table,
    Id,
    UserId,
    Subdomain,
    Fqdn,
    LocalPort,
    Slot,
    CertificateSerial,
    SshKeyPairId,
    CreatedAt,
    ExpiresAt,
    Status,
    BytesTransferred,
    RequestCount,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum SshKeyPair {
    Table,
    Id,
    PrivateKey,
    PublicKey,
    Fingerprint,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum AuthToken {
    Table,
    Token,
    TokenType,
    ExpiresAt,
    UserId,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum CertificateInfo {
    Table,
    Serial,
    Certificate,
    PrivateKey,
    Fingerprint,
    IssuedAt,
    ExpiresAt,
    Subject,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum ApiStats {
    Table,
    Id,
    ActiveTunnels,
    TunnelsCreatedToday,
    BytesTransferredToday,
    UptimeSeconds,
    CaStatsId,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum CaStats {
    Table,
    Id,
    CertificatesIssued,
    ActiveCertificates,
    ExpiredCertificates,
    IssuanceRate,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum PaymentInfo {
    Table,
    Id,
    UserId,
    StripeCustomerId,
    StripeSubscriptionId,
    LastPaymentDate,
    NextPaymentDate,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum UserUsage {
    Table,
    Id,
    UserId,
    PeriodStart,
    ApiCallsCount,
    TunnelsCreatedCount,
    DnsOperationsCount,
    ActiveTunnelsCount,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum AuditLog {
    Table,
    Id,
    UserId,
    Action,
    Resource,
    Timestamp,
    DetailsJson,
}
#[derive(DeriveIden)]
#[allow(dead_code)]
enum BillingEvent {
    Table,
    Id,
    UserId,
    ServicePlanId,
    EventType,
    Amount,
    EventTime,
    DetailsJson,
}
