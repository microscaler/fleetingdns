//! FleetingDNS database migration runner.
//!
//! Usage: `migration [up|down|status|fresh]` (default: `up`).
//!
//! Reads `DATABASE_URL` from the environment. The initial connection is retried
//! so this can run as a Kubernetes pre-install/pre-upgrade hook Job before
//! Postgres has finished starting, and in CI against an ephemeral database.

use std::time::Duration;

use migration::Migrator;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

/// Connect to `url`, retrying transient failures (e.g. Postgres still starting).
async fn connect_with_retry(url: &str, attempts: u32) -> Result<DatabaseConnection, DbErr> {
    let mut last_err: Option<DbErr> = None;
    for attempt in 1..=attempts {
        match Database::connect(url).await {
            Ok(db) => return Ok(db),
            Err(e) => {
                eprintln!("db connect attempt {attempt}/{attempts} failed: {e}");
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| DbErr::Custom("no connection attempts made".to_owned())))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "up".to_owned());
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL environment variable must be set")?;

    let db = connect_with_retry(&url, 30).await?;

    match cmd.as_str() {
        "up" => Migrator::up(&db, None).await?,
        "down" => Migrator::down(&db, None).await?,
        "status" => Migrator::status(&db).await?,
        "fresh" => Migrator::fresh(&db).await?,
        other => {
            return Err(format!("unknown command '{other}'; use up|down|status|fresh").into());
        }
    }

    println!("migration '{cmd}' completed successfully");
    Ok(())
}
