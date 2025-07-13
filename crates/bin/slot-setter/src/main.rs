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
    // Note: Testcontainer tests temporarily disabled due to API compatibility issues
    // These will be re-enabled after updating testcontainers dependencies
    
    #[test]
    fn test_args_debug_format() {
        let args = Args {
            slot: "test-slot".to_string(),
            ip: std::net::Ipv4Addr::new(192, 168, 1, 1),
            ttl: 300,
            redis: "redis://localhost:6379".to_string(),
        };
        
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("test-slot"));
        assert!(debug_str.contains("192.168.1.1"));
        assert!(debug_str.contains("300"));
    }

    #[test]
    fn test_args_clone() {
        let args = Args {
            slot: "test-slot".to_string(),
            ip: std::net::Ipv4Addr::new(192, 168, 1, 1),
            ttl: 300,
            redis: "redis://localhost:6379".to_string(),
        };
        
        let cloned_args = args.clone();
        assert_eq!(args.slot, cloned_args.slot);
        assert_eq!(args.ip, cloned_args.ip);
        assert_eq!(args.ttl, cloned_args.ttl);
        assert_eq!(args.redis, cloned_args.redis);
    }
}
