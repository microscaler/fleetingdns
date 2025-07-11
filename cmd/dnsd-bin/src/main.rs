use std::net::SocketAddr;

use tracing::info;

use common::init_tracing;
#[cfg(feature = "dot")]
use common::tls;
use dnsd::{Config, redis_cache};

#[tokio::main]
async fn main() -> common::AppResult<()> {
    init_tracing();
    let addr: SocketAddr = "0.0.0.0:5353".parse().unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let pool = redis_cache::new_pool(&redis_url)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    info!(addr=%addr, "dnsd-bin starting server");
    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"])?;
    dnsd::serve(Config {
        addr,
        redis_pool: pool,
        #[cfg(feature = "dot")]
        dot_addr: SocketAddr::new(addr.ip(), 853),
        #[cfg(feature = "dot")]
        tls_config,
    })
    .await
}
