use std::net::SocketAddr;

use tracing::info;

use common::init_tracing;
#[cfg(feature = "dot")]
use common::tls;
use dnsd::Config;

#[tokio::main]
async fn main() -> common::AppResult<()> {
    init_tracing();
    let addr: SocketAddr = "0.0.0.0:5353".parse().unwrap();
    info!(addr=%addr, "dnsd-bin starting server");
    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"])?;
    dnsd::serve(Config {
        addr,
        #[cfg(feature = "dot")]
        dot_addr: SocketAddr::new(addr.ip(), 853),
        #[cfg(feature = "dot")]
        tls_config,
    })
    .await
}
