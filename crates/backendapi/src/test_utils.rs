#[cfg(test)]
pub mod postgres_test_container {
    use migration::Migrator;
    use sea_orm::{Database, DatabaseConnection};
    use sea_orm_migration::MigratorTrait;
    use std::time::Duration;
    use testcontainers::ImageExt;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    /// PostgreSQL test container configuration
    #[allow(dead_code)] // container/port/url kept alive for the container's lifetime
    pub struct PostgresTestContainer {
        pub container: testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
        pub port: u16,
        pub url: String,
        pub db: DatabaseConnection,
    }

    impl PostgresTestContainer {
        /// Start a new PostgreSQL test container and return the container info
        pub async fn new() -> Self {
            // Start Postgres container
            let container = Postgres::default()
                .with_tag("17.5-alpine")
                .with_env_var("POSTGRES_DB", "test")
                .with_env_var("POSTGRES_USER", "test")
                .with_env_var("POSTGRES_PASSWORD", "test");

            let container = container
                .start()
                .await
                .expect("Failed to start Postgres container");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get port");
            let url = format!("postgresql://test:test@localhost:{}", port);

            // Wait for container to be ready with retry logic
            let mut retries = 30;
            let db = loop {
                match Database::connect(&url).await {
                    Ok(db) => break db,
                    Err(_) if retries > 0 => {
                        retries -= 1;
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                    Err(e) => panic!("Failed to connect to Postgres after retries: {e:?}"),
                }
            };

            // Run migrations with retry logic
            let mut migration_retries = 10;
            loop {
                match Migrator::up(&db, None).await {
                    Ok(()) => break,
                    Err(_) if migration_retries > 0 => {
                        migration_retries -= 1;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    Err(e) => panic!("Failed to run migrations after retries: {e:?}"),
                }
            }

            Self {
                container,
                port,
                url,
                db,
            }
        }

        /// Get a reference to the database connection
        pub fn database(&self) -> &DatabaseConnection {
            &self.db
        }
    }
}
