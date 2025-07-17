// Canonical migration crate for FleetingDNS. All migration logic, tests, and binaries must reside here.
// Do not use a root-level migration crate. All workspace, CI, and Docker references must use crates/migration/.
//
// See ServicePlan PRD for policy details.
use async_trait::async_trait;
#[cfg(test)]
mod tests {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;
    use sea_orm_migration::{MigratorTrait, SchemaManager, sea_orm::Database};
    use std::env;
    use crate::Migrator; // Ensure Migrator is public and imported

    #[tokio::test]
    async fn test_migration_runs_on_postgres_18() {
        let container = Postgres::default().start().await.expect("Failed to start Postgres");
        let port = container.get_host_port_ipv4(5432).await.unwrap();
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
        Migrator::up(&db, None).await.expect("Migration should succeed");

        // Verify all tables exist
        let schema = SchemaManager::new(&db);
        for table in [
            "user", "service_plan", "pricing", "user_service_plan", "tunnel", "ssh_key_pair", "auth_token", "certificate_info", "api_stats", "ca_stats", "payment_info", "user_usage", "audit_log", "billing_event"
        ] {
            assert!(schema.has_table(table).await.unwrap(), "Table {table} should exist");
        }
    }

    #[tokio::test]
    async fn test_postgres_container_basic_connectivity() {
        use tokio_postgres::{NoTls, Client, Connection};
        let container = Postgres::default().start().await.expect("Failed to start Postgres");
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("host=127.0.0.1 port={} user=postgres password=postgres dbname=postgres", port);

        // Wait for DB to be ready
        let mut retries = 10;
        let (client, connection) = loop {
            match tokio_postgres::connect(&url, NoTls).await {
                Ok((client, connection)) => break (client, connection),
                Err(_) if retries > 0 => {
                    retries -= 1;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(e) => panic!("Failed to connect to Postgres: {e}"),
            }
        };
        // Spawn the connection task
        tokio::spawn(connection);

        // Run a basic SQL test: create table, insert, select
        client.execute("CREATE TABLE test_table (id SERIAL PRIMARY KEY, name TEXT)", &[]).await.expect("create table");
        client.execute("INSERT INTO test_table (name) VALUES ($1)", &[&"hello"]).await.expect("insert");
        let row = client.query_one("SELECT name FROM test_table WHERE id = 1", &[]).await.expect("select");
        let name: &str = row.get(0);
        assert_eq!(name, "hello");
    }

    #[tokio::test]
    async fn test_seaorm_connectivity() {
        use sea_orm::{Database, DatabaseConnection, Statement};
        use sea_orm::ConnectionTrait;
        
        let container = Postgres::default().start().await.expect("Failed to start Postgres");
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

        // Wait for DB to be ready
        let mut retries = 10;
        let db = loop {
            match Database::connect(&url).await {
                Ok(db) => break db,
                Err(_) if retries > 0 => {
                    retries -= 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => panic!("Failed to connect to Postgres: {}", e),
            }
        };

        // Test basic SeaORM operations
        let result = db.execute(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1 as test_value",
            vec![]
        )).await;
        assert!(result.is_ok(), "SeaORM should be able to execute basic SQL");

        // Test creating a table
        let create_result = db.execute(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "CREATE TABLE IF NOT EXISTS test_seaorm (id SERIAL PRIMARY KEY, name VARCHAR(50))",
            vec![]
        )).await;
        assert!(create_result.is_ok(), "SeaORM should be able to create tables");

        // Test inserting data
        let insert_result = db.execute(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "INSERT INTO test_seaorm (name) VALUES ($1)",
            vec!["test_name".into()]
        )).await;
        assert!(insert_result.is_ok(), "SeaORM should be able to insert data");

        // Test selecting data
        let select_result = db.query_one(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT name FROM test_seaorm WHERE name = $1",
            vec!["test_name".into()]
        )).await;
        assert!(select_result.is_ok(), "SeaORM should be able to query data");
        
        let row = select_result.unwrap();
        assert!(row.is_some(), "Should find the inserted row");
        
        let name: String = row.unwrap().try_get("", "name").unwrap();
        assert_eq!(name, "test_name", "Should retrieve the correct value");

        println!("✅ SeaORM connectivity test passed - all operations working");
    }
} 

mod m20250716_191521_create_full_serviceplan_schema;
mod m20250716_191522_add_constraints;

pub struct Migrator;

#[async_trait::async_trait]
impl sea_orm_migration::MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![
            Box::new(m20250716_191521_create_full_serviceplan_schema::Migration),
            Box::new(m20250716_191522_add_constraints::Migration),
        ]
    }
}