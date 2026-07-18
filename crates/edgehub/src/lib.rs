//! EdgeHub — the FleetingDNS tunnel hub.
//!
//! One live data plane (stories TDP-1/TDP-2, `docs/engineering/stories_detailed/
//! E2_E3_tunnel_data_plane_user_stories_v0.3.md`):
//!
//! ```text
//! external client → edgehub-bin HTTPS router (SNI → Redis → 127.0.0.1:slot)
//!                → ssh_server tcpip_forward slot listener
//!                → forwarded-tcpip channel → CLI → developer's local service
//! ```
//!
//! Historical note (TDP-10): earlier revisions carried three additional,
//! never-live forwarding implementations (`tls_router`, `proxy::TcpProxy`,
//! and the T-26b "dynamic reverse proxy" port map). They were deleted; do
//! not reintroduce a forwarding path that is not exercised by the e2e
//! reverse-tunnel tests.

pub mod certificate_manager;
pub mod ssh_server;

pub use certificate_manager::*;
pub use ssh_server::*;
