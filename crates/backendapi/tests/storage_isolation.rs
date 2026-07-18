//! Cross-tunnel isolation invariants at the storage layer, against a real
//! Redis (testcontainers). These back the multi-tunnel security guarantees:
//!
//! - A subdomain can belong to at most ONE active tunnel (the subdomain is
//!   the SNI routing key; a duplicate would route one tunnel's viewers into
//!   another tunnel).
//! - Concurrent slot allocation never hands the same port to two tunnels
//!   (SET NX EX in `allocate_port`; the old GET-then-SET pair was a TOCTOU
//!   race).

use std::collections::HashSet;
use std::sync::Arc;

use backendapi::{Tunnel, TunnelStorage};
use testcontainers::{ImageExt, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;

async fn redis_fixture() -> (testcontainers::ContainerAsync<Redis>, String) {
    let container = Redis::default()
        .with_tag("7.2-alpine")
        .start()
        .await
        .expect("start redis container");
    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("redis port");
    (container, format!("redis://localhost:{port}"))
}

fn tunnel(subdomain: &str, slot: u16) -> Tunnel {
    Tunnel::new(
        "user-1".to_string(),
        "testuser".to_string(),
        subdomain.to_string(),
        "fleetingdns.run",
        3000,
        slot,
        "cert-serial".to_string(),
        600,
    )
}

#[tokio::test]
async fn subdomain_uniqueness_is_enforced() {
    let (_container, url) = redis_fixture().await;
    let storage = TunnelStorage::new(&url).await.expect("storage");

    assert!(
        storage.is_subdomain_available("iso-a").await.unwrap(),
        "fresh subdomain must be available"
    );

    storage.store_tunnel(&tunnel("iso-a", 30001)).await.unwrap();

    assert!(
        !storage.is_subdomain_available("iso-a").await.unwrap(),
        "an active tunnel's subdomain must NOT be available (create_tunnel rejects it)"
    );
    assert!(
        storage.is_subdomain_available("iso-b").await.unwrap(),
        "other subdomains stay available"
    );
}

#[tokio::test]
async fn delete_tunnel_cleans_all_indexes() {
    let (_container, url) = redis_fixture().await;
    let storage = TunnelStorage::new(&url).await.expect("storage");

    let t = tunnel("iso-del", 30002);
    storage.store_tunnel(&t).await.unwrap();
    assert!(!storage.is_subdomain_available("iso-del").await.unwrap());

    // Regression: the user lookup is a JSON string, not a Redis set —
    // delete_tunnel used SREM on it and blew up with WRONGTYPE.
    let deleted = storage.delete_tunnel(&t.id).await.expect("delete_tunnel");
    assert!(deleted, "existing tunnel should report deleted");

    assert!(
        storage.is_subdomain_available("iso-del").await.unwrap(),
        "subdomain must be reusable after deletion"
    );
    let remaining = storage.list_user_tunnels("user-1").await.unwrap();
    assert!(
        remaining.iter().all(|x| x.id != t.id),
        "deleted tunnel must leave the user's tunnel list"
    );
}

#[tokio::test]
async fn concurrent_port_allocation_never_hands_out_duplicates() {
    let (_container, url) = redis_fixture().await;
    let storage = Arc::new(TunnelStorage::new(&url).await.expect("storage"));

    let mut tasks = Vec::new();
    for i in 0..32 {
        let storage = storage.clone();
        tasks.push(tokio::spawn(async move {
            storage
                .allocate_port(&format!("tunnel-{i}"), 600)
                .await
                .expect("allocate_port")
        }));
    }

    let mut seen = HashSet::new();
    for task in tasks {
        let port = task.await.unwrap();
        assert!(
            seen.insert(port),
            "port {port} was allocated to two tunnels concurrently"
        );
    }
}
