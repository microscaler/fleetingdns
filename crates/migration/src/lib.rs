// Canonical migration crate for FleetingDNS. All migration logic, tests, and binaries must reside here.
// Do not use a root-level migration crate. All workspace, CI, and Docker references must use crates/migration/.
//
// See ServicePlan PRD for policy details.
#[cfg(test)]
mod tests {
    use testcontainers::{clients, RunnableImage};
    use testcontainers_modules::postgres::Postgres;
    use sea_orm_migration::{MigratorTrait, SchemaManager, sea_orm::Database};
    use std::env;

    #[tokio::test]
    async fn test_migration_runs_on_postgres_18() {
        let docker = clients::Cli::default();
        let pg_image = RunnableImage::from(Postgres::default().with_version(18));
        let container = docker.run(pg_image);
        let port = container.get_host_port_ipv4(5432);
        let url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

        // Wait for DB to be ready
        let mut retries = 10;
        let db = loop {
            match Database::connect(&url).await {
                Ok(db) => break db,
                Err(_) if retries > 0 => {
                    retries -= 1;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect to Postgres: {e}"),
            }
        };

        // Run all migrations
        super::Migrator::up(&db, None).await.expect("Migration should succeed");

        // Verify all tables exist
        let schema = SchemaManager::new(&db);
        for table in [
            "user", "service_plan", "pricing", "user_service_plan", "tunnel", "ssh_key_pair", "auth_token", "certificate_info", "api_stats", "ca_stats", "payment_info", "user_usage", "audit_log", "billing_event"
        ] {
            assert!(schema.has_table(table).await.unwrap(), "Table {table} should exist");
        }
    }
} 

mod m20250716_191521_create_full_serviceplan_schema;