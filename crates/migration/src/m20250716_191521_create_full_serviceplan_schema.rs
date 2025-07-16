use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // USER table
        manager.create_table(
            Table::create()
                .table(User::Table)
                .if_not_exists()
                .col(pk_string(User::Id))
                .col(string(User::GithubId))
                .col(string(User::Username))
                .col(string(User::Email))
                .col(string(User::AvatarUrl))
                .col(timestamp(User::CreatedAt))
                .to_owned()
        ).await?;

        // SERVICE_PLAN table
        manager.create_table(
            Table::create()
                .table(ServicePlan::Table)
                .if_not_exists()
                .col(pk_string(ServicePlan::Id))
                .col(string(ServicePlan::Name))
                .col(integer(ServicePlan::ApiRateLimit))
                .col(integer(ServicePlan::TunnelCreationLimit))
                .col(integer(ServicePlan::DnsProvisioningLimit))
                .col(integer(ServicePlan::MaxConcurrentTunnels))
                .col(string(ServicePlan::FeaturesJson))
                .col(timestamp(ServicePlan::CreatedAt))
                .to_owned()
        ).await?;

        // PRICING table
        manager.create_table(
            Table::create()
                .table(Pricing::Table)
                .if_not_exists()
                .col(pk_string(Pricing::Id))
                .col(string(Pricing::ServicePlanId))
                .col(float(Pricing::Price))
                .col(string(Pricing::Currency))
                .col(string(Pricing::Region))
                .col(timestamp(Pricing::ValidFrom))
                .col(timestamp(Pricing::ValidTo))
                .col(string(Pricing::Description))
                .to_owned()
        ).await?;

        // USER_SERVICE_PLAN table
        manager.create_table(
            Table::create()
                .table(UserServicePlan::Table)
                .if_not_exists()
                .col(pk_string(UserServicePlan::Id))
                .col(string(UserServicePlan::UserId))
                .col(string(UserServicePlan::ServicePlanId))
                .col(timestamp(UserServicePlan::StartDate))
                .col(timestamp(UserServicePlan::EndDate))
                .col(boolean(UserServicePlan::IsActive))
                .to_owned()
        ).await?;

        // TUNNEL table
        manager.create_table(
            Table::create()
                .table(Tunnel::Table)
                .if_not_exists()
                .col(pk_string(Tunnel::Id))
                .col(string(Tunnel::UserId))
                .col(string(Tunnel::Subdomain))
                .col(string(Tunnel::Fqdn))
                .col(integer(Tunnel::LocalPort))
                .col(integer(Tunnel::Slot))
                .col(string(Tunnel::CertificateSerial))
                .col(string(Tunnel::SshKeyPairId))
                .col(timestamp(Tunnel::CreatedAt))
                .col(timestamp(Tunnel::ExpiresAt))
                .col(string(Tunnel::Status))
                .col(big_integer(Tunnel::BytesTransferred))
                .col(big_integer(Tunnel::RequestCount))
                .to_owned()
        ).await?;

        // SSH_KEY_PAIR table
        manager.create_table(
            Table::create()
                .table(SshKeyPair::Table)
                .if_not_exists()
                .col(pk_string(SshKeyPair::Id))
                .col(string(SshKeyPair::PrivateKey))
                .col(string(SshKeyPair::PublicKey))
                .col(string(SshKeyPair::Fingerprint))
                .to_owned()
        ).await?;

        // AUTH_TOKEN table
        manager.create_table(
            Table::create()
                .table(AuthToken::Table)
                .if_not_exists()
                .col(pk_string(AuthToken::Token))
                .col(string(AuthToken::TokenType))
                .col(timestamp(AuthToken::ExpiresAt))
                .col(string(AuthToken::UserId))
                .to_owned()
        ).await?;

        // CERTIFICATE_INFO table
        manager.create_table(
            Table::create()
                .table(CertificateInfo::Table)
                .if_not_exists()
                .col(pk_string(CertificateInfo::Serial))
                .col(string(CertificateInfo::Certificate))
                .col(string(CertificateInfo::PrivateKey))
                .col(string(CertificateInfo::Fingerprint))
                .col(timestamp(CertificateInfo::IssuedAt))
                .col(timestamp(CertificateInfo::ExpiresAt))
                .col(string(CertificateInfo::Subject))
                .to_owned()
        ).await?;

        // API_STATS table
        manager.create_table(
            Table::create()
                .table(ApiStats::Table)
                .if_not_exists()
                .col(pk_string(ApiStats::Id))
                .col(integer(ApiStats::ActiveTunnels))
                .col(integer(ApiStats::TunnelsCreatedToday))
                .col(big_integer(ApiStats::BytesTransferredToday))
                .col(integer(ApiStats::UptimeSeconds))
                .col(string(ApiStats::CaStatsId))
                .to_owned()
        ).await?;

        // CA_STATS table
        manager.create_table(
            Table::create()
                .table(CaStats::Table)
                .if_not_exists()
                .col(pk_string(CaStats::Id))
                .col(integer(CaStats::CertificatesIssued))
                .col(integer(CaStats::ActiveCertificates))
                .col(integer(CaStats::ExpiredCertificates))
                .col(float(CaStats::IssuanceRate))
                .to_owned()
        ).await?;

        // PAYMENT_INFO table
        manager.create_table(
            Table::create()
                .table(PaymentInfo::Table)
                .if_not_exists()
                .col(pk_string(PaymentInfo::Id))
                .col(string(PaymentInfo::UserId))
                .col(string(PaymentInfo::StripeCustomerId))
                .col(string(PaymentInfo::StripeSubscriptionId))
                .col(timestamp(PaymentInfo::LastPaymentDate))
                .col(timestamp(PaymentInfo::NextPaymentDate))
                .to_owned()
        ).await?;

        // USER_USAGE table
        manager.create_table(
            Table::create()
                .table(UserUsage::Table)
                .if_not_exists()
                .col(pk_string(UserUsage::Id))
                .col(string(UserUsage::UserId))
                .col(timestamp(UserUsage::PeriodStart))
                .col(integer(UserUsage::ApiCallsCount))
                .col(integer(UserUsage::TunnelsCreatedCount))
                .col(integer(UserUsage::DnsOperationsCount))
                .col(integer(UserUsage::ActiveTunnelsCount))
                .to_owned()
        ).await?;

        // AUDIT_LOG table
        manager.create_table(
            Table::create()
                .table(AuditLog::Table)
                .if_not_exists()
                .col(pk_string(AuditLog::Id))
                .col(string(AuditLog::UserId))
                .col(string(AuditLog::Action))
                .col(string(AuditLog::Resource))
                .col(timestamp(AuditLog::Timestamp))
                .col(string(AuditLog::DetailsJson))
                .to_owned()
        ).await?;

        // BILLING_EVENT table
        manager.create_table(
            Table::create()
                .table(BillingEvent::Table)
                .if_not_exists()
                .col(pk_string(BillingEvent::Id))
                .col(string(BillingEvent::UserId))
                .col(string(BillingEvent::ServicePlanId))
                .col(string(BillingEvent::EventType))
                .col(float(BillingEvent::Amount))
                .col(timestamp(BillingEvent::EventTime))
                .col(string(BillingEvent::DetailsJson))
                .to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(BillingEvent::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(AuditLog::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(UserUsage::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(PaymentInfo::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(CaStats::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(ApiStats::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(CertificateInfo::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(AuthToken::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(SshKeyPair::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Tunnel::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(UserServicePlan::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Pricing::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(ServicePlan::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(User::Table).to_owned()).await?;
        Ok(())
    }
}

// Table and column idents for all tables
#[derive(DeriveIden)]
enum User { Table, Id, GithubId, Username, Email, AvatarUrl, CreatedAt }
#[derive(DeriveIden)]
enum ServicePlan { Table, Id, Name, ApiRateLimit, TunnelCreationLimit, DnsProvisioningLimit, MaxConcurrentTunnels, FeaturesJson, CreatedAt }
#[derive(DeriveIden)]
enum Pricing { Table, Id, ServicePlanId, Price, Currency, Region, ValidFrom, ValidTo, Description }
#[derive(DeriveIden)]
enum UserServicePlan { Table, Id, UserId, ServicePlanId, StartDate, EndDate, IsActive }
#[derive(DeriveIden)]
enum Tunnel { Table, Id, UserId, Subdomain, Fqdn, LocalPort, Slot, CertificateSerial, SshKeyPairId, CreatedAt, ExpiresAt, Status, BytesTransferred, RequestCount }
#[derive(DeriveIden)]
enum SshKeyPair { Table, Id, PrivateKey, PublicKey, Fingerprint }
#[derive(DeriveIden)]
enum AuthToken { Table, Token, TokenType, ExpiresAt, UserId }
#[derive(DeriveIden)]
enum CertificateInfo { Table, Serial, Certificate, PrivateKey, Fingerprint, IssuedAt, ExpiresAt, Subject }
#[derive(DeriveIden)]
enum ApiStats { Table, Id, ActiveTunnels, TunnelsCreatedToday, BytesTransferredToday, UptimeSeconds, CaStatsId }
#[derive(DeriveIden)]
enum CaStats { Table, Id, CertificatesIssued, ActiveCertificates, ExpiredCertificates, IssuanceRate }
#[derive(DeriveIden)]
enum PaymentInfo { Table, Id, UserId, StripeCustomerId, StripeSubscriptionId, LastPaymentDate, NextPaymentDate }
#[derive(DeriveIden)]
enum UserUsage { Table, Id, UserId, PeriodStart, ApiCallsCount, TunnelsCreatedCount, DnsOperationsCount, ActiveTunnelsCount }
#[derive(DeriveIden)]
enum AuditLog { Table, Id, UserId, Action, Resource, Timestamp, DetailsJson }
#[derive(DeriveIden)]
enum BillingEvent { Table, Id, UserId, ServicePlanId, EventType, Amount, EventTime, DetailsJson }
