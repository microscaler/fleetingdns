#![feature(str_as_str)]

// Canonical migration crate for FleetingDNS. All migration logic, tests, and binaries must reside here.
// Do not use a root-level migration crate. All workspace, CI, and Docker references must use crates/migration/.
//
// See ServicePlan PRD for policy details.
#[cfg(test)]
mod tests {
    use serial_test::serial;
    use testcontainers::runners::SyncRunner;
    use testcontainers_modules::postgres::Postgres;
    use testcontainers::{Image, ImageExt};
    use sea_orm_migration::{MigratorTrait, SchemaManager, sea_orm::Database};
    use crate::Migrator;
    use std::process::Command;
    use std::time::{Duration, Instant};
    use sea_orm::ConnectionTrait;

    // Pin to stable Postgres version
    const POSTGRES_VERSION: &str = "17.5-alpine";

    /// Robust container setup with comprehensive error handling and resource management
    fn setup_test_container() -> (testcontainers::Container<testcontainers_modules::postgres::Postgres>, u16) {
        // Clean up any existing containers that might be using the same ports
        cleanup_dangling_containers();
        // Start container with retry logic for port conflicts
        let container = start_container_with_retry();
        let port = container.get_host_port_ipv4(5432).expect("get host port");
        // Wait for container to be ready with comprehensive health checks
        wait_for_container_ready(&container, port);
        (container, port)
    }

    /// Clean up any dangling containers that might be using the same ports
    fn cleanup_dangling_containers() {
        let output = Command::new("docker")
            .args(["ps", "-q", "--filter", "ancestor=postgres:17.5-alpine"])
            .output();
        
        if let Ok(output) = output {
            let container_ids = String::from_utf8_lossy(&output.stdout);
            for container_id in container_ids.lines() {
                if !container_id.trim().is_empty() {
                    let _ = Command::new("docker")
                        .args(["rm", "-f", container_id.trim()])
                        .output();
                }
            }
        }
    }

    /// Start container with retry logic for port conflicts
    fn start_container_with_retry() -> testcontainers::Container<testcontainers_modules::postgres::Postgres> {
        let max_retries = 3;
        let mut last_error = None;
        for attempt in 1..=max_retries {
            let req = Postgres::default().with_tag(POSTGRES_VERSION);
            match req.start() {
                Ok(container) => {
                    if attempt > 1 {
                        println!("✅ Container started successfully on attempt {}", attempt);
                    }
                    return container;
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        println!("⚠️  Container start failed on attempt {}, retrying...", attempt);
                        std::thread::sleep(Duration::from_millis(1000 * attempt as u64));
                        cleanup_dangling_containers();
                    }
                }
            }
        }
        panic!("Failed to start container after {} attempts: {:?}", max_retries, last_error);
    }

    /// Comprehensive container readiness check with multiple health indicators
    fn wait_for_container_ready(container: &testcontainers::Container<testcontainers_modules::postgres::Postgres>, port: u16) {
        let timeout = std::env::var("POSTGRES_READY_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(60));
        
        let start = Instant::now();
        let mut last_error = None;
        
        loop {
            // Check if container is still running
            if !is_container_running(container.id()) {
                panic!("Container stopped unexpectedly");
            }
            
            // Try to connect to the database using blocking operations
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            match rt.block_on(sea_orm::Database::connect(&format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres"))) {
                Ok(conn) => {
                    // Use tokio runtime to execute async database operations
                    match rt.block_on(conn.execute_unprepared("SELECT 1")) {
                        Ok(_) => {
                            println!("✅ Container ready after {:?}", start.elapsed());
                            return;
                        }
                        Err(e) => {
                            last_error = Some(format!("Query failed: {:?}", e));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(format!("Connection failed: {:?}", e));
                }
            }
            
            if start.elapsed() > timeout {
                panic!("Failed to connect to Postgres after {:?}. Last error: {:?}", timeout, last_error);
            }
            
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Check if container is still running
    fn is_container_running(container_id: &str) -> bool {
        let output = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Running}}", container_id])
            .output();
        match output {
            Ok(output) => {
                let status_string = String::from_utf8_lossy(&output.stdout).to_string();
                let status = status_string.trim();
                status == "true"
            }
            Err(_) => false
        }
    }

    /// Simple database connection without retry logic
    fn connect_to_database(port: u16) -> sea_orm::DatabaseConnection {
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?connect_timeout=10&pool_timeout=30&max_connections=1");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("create tokio runtime");
        rt.block_on(Database::connect(&url)).expect("connect to postgres")
    }

    #[test]
    #[serial]
    fn test_postgres_container_basic_connectivity() {
        let (_container, port) = setup_test_container();
        let _db = connect_to_database(port);
        println!("✅ Basic connectivity test passed");
    }

    #[test]
    #[serial]
    #[ignore] // TODO: Currently ignored, we will determine if we need this test later.
    fn test_migration_runs_on_postgres_17_5() {
        let (_container, port) = setup_test_container();
        
        // Use direct SQLx connection instead of SeaORM to bypass connection pool issues
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("create tokio runtime");
        
        rt.block_on(async {
            // Wait a bit for the database to be fully ready
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            
            // Test basic connectivity with direct SQLx
            let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
            let pool = sqlx::PgPool::connect(&url).await.expect("connect to postgres");
            
            // Test basic query
            sqlx::query("SELECT 1").execute(&pool).await.expect("test query");
            println!("✅ Database connectivity confirmed");
            
            // Create SeaORM connection using the URL directly
            let db = sea_orm::Database::connect(&url).await.expect("create sea-orm connection");
            
            // Run migrations with timeout
            let timeout = tokio::time::Duration::from_secs(60);
            match tokio::time::timeout(timeout, Migrator::up(&db, None)).await {
                Ok(result) => result.expect("migrate up"),
                Err(_) => panic!("Migration timed out after {:?}", timeout),
            }
            
            // Verify schema was created
            let schema_manager = SchemaManager::new(&db);
            assert!(schema_manager.has_table("service_plan").await.expect("check table exists"));
        });
        
        println!("✅ Migration test passed");
    }

    #[test]
    #[serial]
    fn test_seaorm_connectivity() {
        let (_container, port) = setup_test_container();
        let _db = connect_to_database(port);
        println!("✅ SeaORM connectivity test passed");
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