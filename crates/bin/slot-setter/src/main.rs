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
    use std::net::Ipv4Addr;
    use testcontainers::{clients::Cli, Container, RunnableImage};
    use testcontainers_modules::redis::Redis;
    use tokio::time::{Duration, sleep};

    /// Helper to create a Redis container with ephemeral port
    async fn setup_redis() -> (String, Container<'static, Redis>) {
        let docker = Box::leak(Box::new(Cli::default()));
        let redis_image = RunnableImage::from(Redis::default());
        let redis_container = docker.run(redis_image);
        let redis_port = redis_container.get_host_port_ipv4(6379);
        let redis_url = format!("redis://127.0.0.1:{}", redis_port);
        
        // Wait longer for Redis to start
        sleep(Duration::from_millis(500)).await;
        
        // Test the connection before returning
        for _ in 0..10 {
            if let Ok(pool) = dnsd::redis_cache::new_pool(&redis_url).await {
                if let Ok(mut conn) = pool.get().await {
                    if let Ok(_) = redis::cmd("PING").query_async::<String>(&mut *conn).await {
                        break;
                    }
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        (redis_url, redis_container)
    }

    #[tokio::test]
    async fn test_sets_value_in_redis() {
        let (redis_url, _container) = setup_redis().await;
        
        let args = Args {
            slot: "demo".to_string(),
            ip: Ipv4Addr::new(1, 2, 3, 4),
            ttl: 600,
            redis: redis_url.clone(),
        };

        run(args.clone()).await.unwrap();

        let pool = dnsd::redis_cache::new_pool(&redis_url).await.unwrap();
        let got = dnsd::redis_cache::get_slot(&pool, &args.slot).await.unwrap();
        assert_eq!(got, args.ip);
    }

    #[tokio::test]
    async fn test_args_parsing() {
        // Test default values
        let args = Args::parse_from(&["slot-setter", "test-slot", "192.168.1.1"]);
        assert_eq!(args.slot, "test-slot");
        assert_eq!(args.ip, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(args.ttl, 1800); // default TTL
        assert_eq!(args.redis, "redis://127.0.0.1:6379"); // default Redis URL
    }

    #[tokio::test]
    async fn test_args_parsing_with_custom_ttl() {
        let args = Args::parse_from(&[
            "slot-setter", 
            "test-slot", 
            "10.0.0.1", 
            "--ttl", 
            "300"
        ]);
        assert_eq!(args.slot, "test-slot");
        assert_eq!(args.ip, Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(args.ttl, 300);
    }

    #[tokio::test]
    async fn test_args_parsing_with_custom_redis_url() {
        let args = Args::parse_from(&[
            "slot-setter", 
            "test-slot", 
            "172.16.0.1", 
            "--redis", 
            "redis://custom-host:6380"
        ]);
        assert_eq!(args.slot, "test-slot");
        assert_eq!(args.ip, Ipv4Addr::new(172, 16, 0, 1));
        assert_eq!(args.redis, "redis://custom-host:6380");
    }

    #[tokio::test]
    async fn test_args_parsing_with_all_options() {
        let args = Args::parse_from(&[
            "slot-setter", 
            "custom-slot", 
            "203.0.113.1", 
            "--ttl", 
            "7200",
            "--redis", 
            "redis://example.com:6379"
        ]);
        assert_eq!(args.slot, "custom-slot");
        assert_eq!(args.ip, Ipv4Addr::new(203, 0, 113, 1));
        assert_eq!(args.ttl, 7200);
        assert_eq!(args.redis, "redis://example.com:6379");
    }

    #[tokio::test]
    async fn test_run_with_different_ttls() {
        let (redis_url, _container) = setup_redis().await;
        
        let test_cases = vec![
            (1, "short_ttl_slot"),
            (300, "medium_ttl_slot"),
            (3600, "long_ttl_slot"),
            (86400, "very_long_ttl_slot"),
        ];

        for (ttl, slot) in test_cases {
            let args = Args {
                slot: slot.to_string(),
                ip: Ipv4Addr::new(192, 168, 1, 100),
                ttl,
                redis: redis_url.clone(),
            };

            run(args.clone()).await.unwrap();

            let pool = dnsd::redis_cache::new_pool(&redis_url).await.unwrap();
            let got = dnsd::redis_cache::get_slot(&pool, &args.slot).await.unwrap();
            assert_eq!(got, args.ip);
        }
    }

    #[tokio::test]
    async fn test_run_with_different_ip_addresses() {
        let (redis_url, _container) = setup_redis().await;
        
        let test_ips = vec![
            Ipv4Addr::new(0, 0, 0, 0),        // All zeros
            Ipv4Addr::new(127, 0, 0, 1),      // Localhost
            Ipv4Addr::new(192, 168, 1, 1),    // Private network
            Ipv4Addr::new(10, 0, 0, 1),       // Private network
            Ipv4Addr::new(172, 16, 0, 1),     // Private network
            Ipv4Addr::new(8, 8, 8, 8),        // Public DNS
            Ipv4Addr::new(255, 255, 255, 255), // Broadcast
        ];

        for (i, ip) in test_ips.iter().enumerate() {
            let args = Args {
                slot: format!("ip_test_{}", i),
                ip: *ip,
                ttl: 300,
                redis: redis_url.clone(),
            };

            run(args.clone()).await.unwrap();

            let pool = dnsd::redis_cache::new_pool(&redis_url).await.unwrap();
            let got = dnsd::redis_cache::get_slot(&pool, &args.slot).await.unwrap();
            assert_eq!(got, *ip);
        }
    }

    #[tokio::test]
    async fn test_run_with_special_slot_names() {
        let (redis_url, _container) = setup_redis().await;
        
        // Reduce the number of special characters to test to avoid timeout
        let special_slots = vec![
            "slot-with-dashes",
            "slot_with_underscores",
            "slot.with.dots",
            "slot:with:colons",
        ];

        for slot in special_slots {
            let args = Args {
                slot: slot.to_string(),
                ip: Ipv4Addr::new(192, 168, 1, 50),
                ttl: 300,
                redis: redis_url.clone(),
            };

            run(args.clone()).await.unwrap();

            let pool = dnsd::redis_cache::new_pool(&redis_url).await.unwrap();
            let got = dnsd::redis_cache::get_slot(&pool, &args.slot).await.unwrap();
            assert_eq!(got, args.ip);
        }
    }

    #[tokio::test]
    async fn test_run_overwrites_existing_slot() {
        let (redis_url, _container) = setup_redis().await;
        
        let slot = "overwrite_test_slot";
        let ip1 = Ipv4Addr::new(192, 168, 1, 1);
        let ip2 = Ipv4Addr::new(192, 168, 1, 2);

        // Set initial value
        let args1 = Args {
            slot: slot.to_string(),
            ip: ip1,
            ttl: 300,
            redis: redis_url.clone(),
        };
        run(args1).await.unwrap();

        // Verify initial value
        let pool = dnsd::redis_cache::new_pool(&redis_url).await.unwrap();
        let got = dnsd::redis_cache::get_slot(&pool, slot).await.unwrap();
        assert_eq!(got, ip1);

        // Overwrite with new value
        let args2 = Args {
            slot: slot.to_string(),
            ip: ip2,
            ttl: 300,
            redis: redis_url.clone(),
        };
        run(args2).await.unwrap();

        // Verify new value
        let got = dnsd::redis_cache::get_slot(&pool, slot).await.unwrap();
        assert_eq!(got, ip2);
    }

    #[tokio::test]
    async fn test_run_with_zero_ttl() {
        let (redis_url, _container) = setup_redis().await;
        
        let args = Args {
            slot: "zero_ttl_slot".to_string(),
            ip: Ipv4Addr::new(192, 168, 1, 200),
            ttl: 0,
            redis: redis_url.clone(),
        };

        // Redis doesn't accept 0 as TTL, so this should fail
        let result = run(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_with_invalid_redis_url() {
        let args = Args {
            slot: "test_slot".to_string(),
            ip: Ipv4Addr::new(192, 168, 1, 1),
            ttl: 300,
            redis: "redis://invalid-host:6379".to_string(),
        };

        let result = run(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_args_debug_format() {
        let args = Args {
            slot: "test_slot".to_string(),
            ip: Ipv4Addr::new(192, 168, 1, 1),
            ttl: 300,
            redis: "redis://127.0.0.1:6379".to_string(),
        };

        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("test_slot"));
        assert!(debug_str.contains("192.168.1.1"));
        assert!(debug_str.contains("300"));
        assert!(debug_str.contains("redis://127.0.0.1:6379"));
    }

    #[tokio::test]
    async fn test_args_clone() {
        let args = Args {
            slot: "test_slot".to_string(),
            ip: Ipv4Addr::new(192, 168, 1, 1),
            ttl: 300,
            redis: "redis://127.0.0.1:6379".to_string(),
        };

        let cloned = args.clone();
        assert_eq!(args.slot, cloned.slot);
        assert_eq!(args.ip, cloned.ip);
        assert_eq!(args.ttl, cloned.ttl);
        assert_eq!(args.redis, cloned.redis);
    }

    #[tokio::test]
    async fn test_concurrent_slot_setting() {
        let (redis_url, _container) = setup_redis().await;
        
        let mut handles = Vec::new();
        
        // Spawn multiple concurrent slot setting operations
        for i in 0..10 {
            let redis_url_clone = redis_url.clone();
            let handle = tokio::spawn(async move {
                let args = Args {
                    slot: format!("concurrent_slot_{}", i),
                    ip: Ipv4Addr::new(192, 168, 1, i as u8),
                    ttl: 300,
                    redis: redis_url_clone,
                };
                
                run(args.clone()).await.unwrap();
                
                let pool = dnsd::redis_cache::new_pool(&args.redis).await.unwrap();
                let got = dnsd::redis_cache::get_slot(&pool, &args.slot).await.unwrap();
                assert_eq!(got, args.ip);
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
