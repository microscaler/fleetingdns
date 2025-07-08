use std::net::Ipv4Addr;

use clap::Parser;
use tracing::info;

use common::{AppResult, init_tracing};
use dnsd::redis_cache;

/// Command line arguments for the slot-setter utility.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Slot name to store in Redis.
    slot: String,
    /// IPv4 address associated with the slot.
    ip: Ipv4Addr,
    /// Time-to-live for the key in seconds.
    #[arg(long, default_value_t = 1800)]
    ttl: u64,
    /// Redis connection URL.
    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    redis: String,
}

/// Execute the slot insertion logic.
#[tracing::instrument]
async fn run(args: Args) -> AppResult<()> {
    init_tracing();
    let pool = redis_cache::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    redis_cache::set_slot(&pool, &args.slot, args.ip, args.ttl)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    info!(slot=%args.slot, ip=%args.ip, ttl=args.ttl, "slot set");
    Ok(())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::process::Command;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn sets_value_in_redis() {
        let port = 6381u16;
        let mut child = Command::new("redis-server")
            .arg("--port")
            .arg(port.to_string())
            .arg("--bind")
            .arg("127.0.0.1")
            .arg("--save")
            .arg("")
            .arg("--appendonly")
            .arg("no")
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("start redis");

        sleep(Duration::from_millis(500)).await;

        let args = Args {
            slot: "demo".to_string(),
            ip: Ipv4Addr::new(1, 2, 3, 4),
            ttl: 600,
            redis: format!("redis://127.0.0.1:{port}"),
        };

        run(args.clone()).await.unwrap();

        let pool = redis_cache::new_pool(&args.redis).await.unwrap();
        let got = redis_cache::get_slot(&pool, &args.slot).await.unwrap();
        assert_eq!(got, args.ip);

        child.kill().await.ok();
    }
}
