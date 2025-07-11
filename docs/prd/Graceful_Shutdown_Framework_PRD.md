# FleetingDNS Graceful Shutdown Framework - Product Requirements Document

**Version**: 1.0  
**Date**: 2025-07-11  
**Status**: Draft  
**Owner**: Infrastructure Team  

---

## 1. Executive Summary

### Problem Statement
FleetingDNS daemon binaries currently lack proper graceful shutdown mechanisms, relying on crude `pkill` operations that pose significant risks:
- **Resource Leaks**: Redis connections, TLS sessions, and file descriptors not properly closed
- **Data Loss**: In-flight requests terminated abruptly without completion
- **Process Management Chaos**: No way to distinguish between daemon instances
- **Testing Fragility**: E2E tests using `pkill` are unreliable and dangerous
- **Production Risk**: No proper signal handling for container orchestration

### Solution Overview
Implement a unified graceful shutdown framework with:
- Unix socket control interface for precise daemon management
- Configurable socket addresses per component
- Signal handling (SIGTERM, SIGINT, SIGUSR1)
- Resource cleanup and connection draining
- Support for local deployment via plist (macOS) and systemd (Linux) files
- Control CLI for operational management

### Success Metrics
- **Zero** `pkill` usage in codebase
- **100%** of daemons support graceful shutdown
- **< 5 seconds** average shutdown time
- **Zero** resource leaks during shutdown
- **100%** in-flight request completion during graceful shutdown

---

## 2. Component Inventory

### 2.1 Daemon Binaries Requiring Graceful Shutdown

| Component | Binary | Current State | Priority | Connection Types | Complexity |
|-----------|--------|---------------|----------|------------------|------------|
| **DNS Server** | `dnsd-bin` | Infinite UDP loop, no shutdown | P0 | UDP socket, TCP listener (DoT), Redis pool, spawned tasks | Medium |
| **Edge Hub** | `edgehub-bin` | Infinite TLS accept loop, no shutdown | P0 | TCP listener, TLS streams, Redis pool, spawned tasks | Medium |
| **Backend API** | `api-bin` | Stub implementation | P1 | HTTP server, PostgreSQL pool, Redis pool, Stripe webhooks | High |
| **Intake Collector** | `intake_collector-bin` | Stub implementation | P1 | gRPC server, Pub/Sub client, background workers | High |
| **ML Scorer** | `ml_scorer-bin` | Stub implementation | P1 | gRPC server, HTTP server, model inference queues | High |
| **Feed gRPC** | `feed_grpc-bin` | Stub implementation | P1 | gRPC bidirectional streams, JWT validation, mTLS | High |
| **Feed Webhook** | `feed_webhook-bin` | Stub implementation | P1 | HTTP client, webhook queues, HMAC signing, retry logic | High |

### 2.2 Utility Binaries (No Shutdown Required)
- `slot-setter` - Short-lived CLI tool, exits naturally

### 2.3 Connection Cleanup Requirements by Service

#### 2.3.1 DNS Server (`dnsd-bin`) - **IMPLEMENTED**
**Current Connections:**
- UDP socket bound to configurable address (default 5353)
- TCP listener for DNS-over-TLS on port 853 (when `dot` feature enabled)
- Redis connection pool (bb8::Pool<RedisConnectionManager>)
- Spawned tokio tasks for DoT connection handling

**Shutdown Requirements:**
- **UDP Socket**: Close gracefully, allow pending packets to complete
- **DoT TCP Listener**: Stop accepting new connections, drain existing TLS streams
- **TLS Streams**: Call `tls.shutdown().await` for each active connection
- **Redis Pool**: Call `pool.close().await` to terminate all connections
- **Spawned Tasks**: Cancel DoT handler tasks, wait for completion with timeout

#### 2.3.2 Edge Hub (`edgehub-bin`) - **IMPLEMENTED**  
**Current Connections:**
- TCP listener bound to configurable address (default 2222)
- TLS acceptor for incoming tunnel connections
- Redis connection pool for slot state management
- Spawned tokio tasks for each tunnel connection

**Shutdown Requirements:**
- **TCP Listener**: Stop accepting new connections
- **TLS Connections**: Gracefully shutdown each active tunnel with `tls.shutdown().await`
- **Redis Operations**: Complete pending slot cleanup (`del_slot()` calls)
- **Redis Pool**: Close connection pool
- **Connection Draining**: Wait for active tunnels to complete (with timeout)

#### 2.3.3 Backend API (`api-bin`) - **PLANNED**
**Planned Connections:**
- HTTP server (Axum) bound to configurable port
- PostgreSQL connection pool (sqlx::PgPool)
- Redis connection pool for caching and sessions
- Stripe webhook HTTP client connections
- Background task workers for billing and cleanup

**Shutdown Requirements:**
- **HTTP Server**: Graceful shutdown with request draining (`with_graceful_shutdown()`)
- **PostgreSQL Pool**: Close all database connections (`pool.close().await`)
- **Redis Pool**: Terminate cache connections
- **HTTP Client**: Close Stripe webhook client connection pool
- **Background Tasks**: Cancel billing workers, complete pending transactions

#### 2.3.4 Intake Collector (`intake_collector-bin`) - **PLANNED**
**Planned Connections:**
- gRPC server for honeypot telemetry ingestion
- Google Cloud Pub/Sub client connections
- Background worker tasks for data processing
- Message queues for telemetry buffering

**Shutdown Requirements:**
- **gRPC Server**: Graceful shutdown with stream completion
- **Pub/Sub Client**: Flush pending messages, close publisher/subscriber
- **Message Queues**: Drain buffered telemetry data
- **Background Workers**: Cancel processing tasks, complete in-flight operations

#### 2.3.5 ML Scorer (`ml_scorer-bin`) - **PLANNED**
**Planned Connections:**
- gRPC server for scoring requests
- HTTP server for health checks and metrics
- Model inference queues and worker threads
- Background model reloading tasks

**Shutdown Requirements:**
- **gRPC Server**: Complete active scoring requests
- **HTTP Server**: Graceful shutdown
- **Inference Queues**: Drain pending scoring requests
- **Model Workers**: Wait for active inferences to complete
- **Background Tasks**: Cancel model reload operations

#### 2.3.6 Feed gRPC (`feed_grpc-bin`) - **PLANNED**
**Planned Connections:**
- gRPC bidirectional streaming server
- JWT validation service connections
- mTLS client certificate handling
- Client subscription management
- Redis connection for entitlements cache

**Shutdown Requirements:**
- **gRPC Streams**: Complete active bidirectional streams
- **Client Subscriptions**: Notify clients of service shutdown
- **mTLS Connections**: Graceful TLS termination
- **Redis Pool**: Close entitlements cache connections
- **JWT Service**: Cleanup validation state

#### 2.3.7 Feed Webhook (`feed_webhook-bin`) - **PLANNED**
**Planned Connections:**
- HTTP client connection pool for webhook delivery
- Webhook delivery queues with retry logic
- HMAC signing service state
- Background retry workers
- Redis connection for delivery tracking

**Shutdown Requirements:**
- **HTTP Client Pool**: Close webhook delivery connections
- **Delivery Queues**: Flush pending webhooks (with timeout)
- **Retry Workers**: Cancel retry attempts, log failed deliveries
- **Redis Pool**: Close delivery tracking connections
- **HMAC State**: Cleanup signing keys and state

---

## 3. Technical Requirements

### 3.1 Core Framework (`crates/common/src/shutdown.rs`)

#### 3.1.1 GracefulShutdown Manager
```rust
pub struct GracefulShutdown {
    // Configuration
    control_socket_path: PathBuf,
    shutdown_timeout: Duration,
    drain_timeout: Duration,
    
    // Internal state
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
    tasks: Vec<JoinHandle<()>>,
    state: Arc<Mutex<ShutdownState>>,
}

pub enum ShutdownSignal {
    Graceful,      // Allow in-flight requests to complete
    Immediate,     // Stop accepting new requests, finish current
    Force,         // Terminate immediately
}

pub enum ShutdownState {
    Running,
    Draining,      // No new requests, finishing current
    Stopping,      // Cleaning up resources
    Stopped,
}
```

#### 3.1.2 Signal Handling
- **SIGTERM**: Graceful shutdown (default 30s timeout)
- **SIGINT**: Graceful shutdown (Ctrl+C)
- **SIGUSR1**: Immediate shutdown (5s timeout)
- **SIGUSR2**: Force shutdown (1s timeout)

#### 3.1.3 Unix Socket Control Interface
```rust
pub enum ControlCommand {
    Shutdown { signal: ShutdownSignal },
    Status,
    Ping,
    Reload,  // For future configuration reloading
}

pub struct ControlResponse {
    status: String,
    uptime: Duration,
    active_connections: u64,
    shutdown_state: ShutdownState,
}
```

### 3.2 Configurable Socket Addresses

#### 3.2.1 Configuration Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    /// Unix socket path for control interface
    pub control_socket: PathBuf,
    
    /// Maximum time to wait for graceful shutdown
    pub shutdown_timeout: Duration,
    
    /// Maximum time to drain existing connections
    pub drain_timeout: Duration,
    
    /// Enable/disable specific signal handlers
    pub signal_handlers: SignalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    pub sigterm: bool,
    pub sigint: bool,
    pub sigusr1: bool,
    pub sigusr2: bool,
}
```

#### 3.2.2 POSIX-Compliant Socket Paths

**Linux (systemd/production):**
```rust
pub const LINUX_SOCKET_PATHS: &[(&str, &str)] = &[
    ("dnsd-bin", "/run/fleetingdns/dnsd.sock"),
    ("edgehub-bin", "/run/fleetingdns/edgehub.sock"),
    ("api-bin", "/run/fleetingdns/api.sock"),
    ("intake_collector-bin", "/run/fleetingdns/intake-collector.sock"),
    ("ml_scorer-bin", "/run/fleetingdns/ml-scorer.sock"),
    ("feed_grpc-bin", "/run/fleetingdns/feed-grpc.sock"),
    ("feed_webhook-bin", "/run/fleetingdns/feed-webhook.sock"),
];
```

**macOS (launchd/development):**
```rust
pub const MACOS_SOCKET_PATHS: &[(&str, &str)] = &[
    ("dnsd-bin", "/var/run/fleetingdns/dnsd.sock"),
    ("edgehub-bin", "/var/run/fleetingdns/edgehub.sock"),
    ("api-bin", "/var/run/fleetingdns/api.sock"),
    ("intake_collector-bin", "/var/run/fleetingdns/intake-collector.sock"),
    ("ml_scorer-bin", "/var/run/fleetingdns/ml-scorer.sock"),
    ("feed_grpc-bin", "/var/run/fleetingdns/feed-grpc.sock"),
    ("feed_webhook-bin", "/var/run/fleetingdns/feed-webhook.sock"),
];
```

**User Mode (non-root):**
```rust
pub const USER_SOCKET_PATHS: &[(&str, &str)] = &[
    // Linux: $XDG_RUNTIME_DIR/fleetingdns/ (typically /run/user/{uid}/fleetingdns/)
    // macOS: ~/Library/Application Support/fleetingdns/
    ("dnsd-bin", "{runtime_dir}/fleetingdns/dnsd.sock"),
    ("edgehub-bin", "{runtime_dir}/fleetingdns/edgehub.sock"),
    ("api-bin", "{runtime_dir}/fleetingdns/api.sock"),
    ("intake_collector-bin", "{runtime_dir}/fleetingdns/intake-collector.sock"),
    ("ml_scorer-bin", "{runtime_dir}/fleetingdns/ml-scorer.sock"),
    ("feed_grpc-bin", "{runtime_dir}/fleetingdns/feed-grpc.sock"),
    ("feed_webhook-bin", "{runtime_dir}/fleetingdns/feed-webhook.sock"),
];
```

**Socket Path Resolution Logic:**
```rust
fn get_socket_path(component: &str) -> PathBuf {
    // 1. Check command line arg: --control-socket
    // 2. Check environment: FLEETINGDNS_CONTROL_SOCKET
    // 3. Check config file: ~/.fleetingdns/config.toml
    // 4. Use OS-appropriate default:
    
    if running_as_root() {
        if cfg!(target_os = "linux") {
            format!("/run/fleetingdns/{}.sock", component)
        } else if cfg!(target_os = "macos") {
            format!("/var/run/fleetingdns/{}.sock", component)
        }
    } else {
        // User mode - use XDG_RUNTIME_DIR on Linux, ~/Library/Application Support on macOS
        let runtime_dir = get_user_runtime_dir();
        format!("{}/fleetingdns/{}.sock", runtime_dir, component)
    }
}
```

**POSIX Socket Location Rationale:**
- **`/run/`** (Linux) - Standard systemd runtime directory, automatically cleaned on boot
- **`/var/run/`** (macOS) - Traditional Unix runtime directory for system services  
- **`$XDG_RUNTIME_DIR`** (Linux user) - Per-user runtime directory with proper permissions
- **`~/Library/Application Support/`** (macOS user) - Standard macOS application data location
- **Never `/tmp/`** - Shared, world-writable, security risk, not appropriate for service sockets

#### 3.2.3 Configuration Sources (Priority Order)
1. **Command Line Arguments**: `--control-socket /custom/path.sock`
2. **Environment Variables**: `FLEETINGDNS_CONTROL_SOCKET`
3. **Configuration File**: `~/.fleetingdns/config.toml`
4. **OS-Appropriate Defaults**:
   - **Linux (root)**: `/run/fleetingdns/{component}.sock`
   - **macOS (root)**: `/var/run/fleetingdns/{component}.sock`
   - **Linux (user)**: `$XDG_RUNTIME_DIR/fleetingdns/{component}.sock`
   - **macOS (user)**: `~/Library/Application Support/fleetingdns/{component}.sock`

### 3.3 Per-Component Integration Requirements

#### 3.3.1 DNS Server (`dnsd-bin`)
```rust
// Required changes to cmd/dnsd-bin/src/main.rs
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:5353")]
    addr: SocketAddr,
    
    #[arg(long)]
    control_socket: Option<PathBuf>,
    
    #[arg(long, default_value = "30")]
    shutdown_timeout: u64,
}

async fn serve_with_shutdown(cfg: Config, shutdown: GracefulShutdown) -> AppResult<()> {
    let socket = UdpSocket::bind(cfg.addr).await?;
    let mut shutdown_rx = shutdown.subscribe();
    
    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                // Handle DNS request
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal, stopping DNS server");
                break;
            }
        }
    }
    
    // Cleanup: close socket, flush metrics, etc.
    Ok(())
}
```

#### 3.3.2 Edge Hub (`edgehub-bin`)
```rust
async fn serve_with_shutdown(cfg: Config, shutdown: GracefulShutdown) -> AppResult<()> {
    let listener = TcpListener::bind(cfg.addr).await?;
    let mut shutdown_rx = shutdown.subscribe();
    
    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer) = result?;
                // Spawn connection handler with shutdown signal
                spawn_connection_handler(stream, peer, shutdown.subscribe());
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal, stopping EdgeHub");
                break;
            }
        }
    }
    
    // Wait for active connections to drain
    shutdown.wait_for_connections_to_drain().await?;
    Ok(())
}
```

---

## 4. Local Deployment Support

### 4.1 macOS plist Files

#### 4.1.1 DNS Server (`deploy/macos/com.fleetingdns.dnsd.plist`)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.fleetingdns.dnsd</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/dnsd-bin</string>
        <string>--addr</string>
        <string>127.0.0.1:5353</string>
        <string>--control-socket</string>
        <string>/var/run/fleetingdns/dnsd.sock</string>
    </array>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    
    <key>StandardOutPath</key>
    <string>/usr/local/var/log/fleetingdns/dnsd.log</string>
    
    <key>StandardErrorPath</key>
    <string>/usr/local/var/log/fleetingdns/dnsd.error.log</string>
    
    <key>EnvironmentVariables</key>
    <dict>
        <key>REDIS_URL</key>
        <string>redis://127.0.0.1:6379</string>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    
    <key>SoftResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>4096</integer>
    </dict>
</dict>
</plist>
```

#### 4.1.2 Edge Hub (`deploy/macos/com.fleetingdns.edgehub.plist`)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.fleetingdns.edgehub</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/edgehub-bin</string>
        <string>--addr</string>
        <string>0.0.0.0:2222</string>
        <string>--redis</string>
        <string>redis://127.0.0.1:6379</string>
        <string>--control-socket</string>
        <string>/var/run/fleetingdns/edgehub.sock</string>
    </array>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    
    <key>StandardOutPath</key>
    <string>/usr/local/var/log/fleetingdns/edgehub.log</string>
    
    <key>StandardErrorPath</key>
    <string>/usr/local/var/log/fleetingdns/edgehub.error.log</string>
    
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    
    <key>SoftResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>8192</integer>
    </dict>
</dict>
</plist>
```

### 4.2 Linux systemd Files

#### 4.2.1 DNS Server (`deploy/systemd/fleetingdns-dnsd.service`)
```ini
[Unit]
Description=FleetingDNS DNS Server
Documentation=https://github.com/microscaler/fleetingdns
After=network.target redis.service
Wants=redis.service

[Service]
Type=notify
ExecStart=/usr/local/bin/dnsd-bin \
    --addr 127.0.0.1:5353 \
    --control-socket /run/fleetingdns/dnsd.sock
ExecReload=/bin/kill -USR1 $MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=30
Restart=always
RestartSec=5

# Security
User=fleetingdns
Group=fleetingdns
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true

# Resource limits
LimitNOFILE=4096
LimitNPROC=1024

# Environment
Environment=REDIS_URL=redis://127.0.0.1:6379
Environment=RUST_LOG=info

# Runtime directory
RuntimeDirectory=fleetingdns
RuntimeDirectoryMode=0755

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=fleetingdns-dnsd

[Install]
WantedBy=multi-user.target
```

#### 4.2.2 Edge Hub (`deploy/systemd/fleetingdns-edgehub.service`)
```ini
[Unit]
Description=FleetingDNS Edge Hub
Documentation=https://github.com/microscaler/fleetingdns
After=network.target redis.service fleetingdns-dnsd.service
Wants=redis.service
Requires=fleetingdns-dnsd.service

[Service]
Type=notify
ExecStart=/usr/local/bin/edgehub-bin \
    --addr 0.0.0.0:2222 \
    --redis redis://127.0.0.1:6379 \
    --control-socket /run/fleetingdns/edgehub.sock
ExecReload=/bin/kill -USR1 $MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=60
Restart=always
RestartSec=5

# Security
User=fleetingdns
Group=fleetingdns
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true

# Resource limits
LimitNOFILE=8192
LimitNPROC=2048

# Environment
Environment=RUST_LOG=info

# Runtime directory
RuntimeDirectory=fleetingdns
RuntimeDirectoryMode=0755

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=fleetingdns-edgehub

[Install]
WantedBy=multi-user.target
```

---

## 5. Control CLI Tool

### 5.1 CLI Design (`cmd/fleetingdns-ctl`)

#### 5.1.1 Command Structure
```bash
# Shutdown commands
fleetingdns-ctl shutdown dnsd --graceful
fleetingdns-ctl shutdown edgehub --immediate
fleetingdns-ctl shutdown all --force

# Status commands
fleetingdns-ctl status dnsd
fleetingdns-ctl status --all
fleetingdns-ctl ps  # List all running FleetingDNS processes

# Service management (with plist/systemd integration)
fleetingdns-ctl start dnsd
fleetingdns-ctl stop edgehub
fleetingdns-ctl restart api
fleetingdns-ctl reload feed-grpc

# Health checks
fleetingdns-ctl ping dnsd
fleetingdns-ctl health --all
```

#### 5.1.2 Implementation (`cmd/fleetingdns-ctl/src/main.rs`)
```rust
#[derive(Parser)]
#[command(name = "fleetingdns-ctl")]
#[command(about = "FleetingDNS daemon control utility")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(long, global = true)]
    socket_dir: Option<PathBuf>,
    
    #[arg(long, global = true)]
    timeout: Option<u64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Shutdown daemon(s)
    Shutdown {
        daemon: Option<String>,
        #[arg(long)]
        graceful: bool,
        #[arg(long)]
        immediate: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        all: bool,
    },
    
    /// Show daemon status
    Status {
        daemon: Option<String>,
        #[arg(long)]
        all: bool,
    },
    
    /// List running processes
    Ps,
    
    /// Start daemon (via systemd/launchd)
    Start { daemon: String },
    
    /// Stop daemon (via systemd/launchd)
    Stop { daemon: String },
    
    /// Restart daemon (via systemd/launchd)
    Restart { daemon: String },
    
    /// Ping daemon
    Ping { daemon: String },
}
```

---

## 6. Implementation Phases

### Phase 1: Core Framework (Week 1)
- [ ] Implement `crates/common/src/shutdown.rs`
- [ ] Add signal handling and Unix socket control
- [ ] Create configuration schema and defaults
- [ ] Unit tests for shutdown framework

### Phase 2: Primary Daemons (Week 2)
- [ ] Integrate graceful shutdown into `dnsd-bin`
- [ ] Integrate graceful shutdown into `edgehub-bin`
- [ ] Update e2e tests to use graceful shutdown
- [ ] Create basic control CLI

### Phase 3: Service Files (Week 3)
- [ ] Create macOS plist files for all daemons
- [ ] Create Linux systemd files for all daemons
- [ ] Add installation scripts
- [ ] Test local deployment scenarios

### Phase 4: Remaining Daemons (Week 4)
- [ ] Implement actual functionality for stub daemons
- [ ] Integrate graceful shutdown into all remaining daemons
- [ ] Complete control CLI with all features
- [ ] End-to-end testing and documentation

---

## 7. Testing Strategy

### 7.1 Unit Tests
- Signal handling behavior
- Unix socket communication
- Configuration parsing
- Shutdown state transitions

### 7.2 Integration Tests
- Multi-daemon shutdown scenarios
- Resource cleanup verification
- Control CLI functionality
- Service file validation

### 7.3 E2E Tests
- Replace all `pkill` usage with graceful shutdown
- Test graceful shutdown during active connections
- Verify zero resource leaks
- Test timeout scenarios

### 7.4 Load Tests
- Graceful shutdown under high connection load
- Performance impact of shutdown framework
- Memory usage during shutdown process

---

## 8. Success Criteria

### 8.1 Functional Requirements
- ✅ All daemon binaries support graceful shutdown
- ✅ Zero `pkill` usage in codebase
- ✅ Configurable socket addresses per component
- ✅ Working plist and systemd files
- ✅ Control CLI with full functionality

### 8.2 Performance Requirements
- ✅ Shutdown time < 5 seconds for graceful shutdown
- ✅ Shutdown time < 1 second for force shutdown
- ✅ Zero resource leaks during shutdown
- ✅ 100% in-flight request completion during graceful shutdown

### 8.3 Operational Requirements
- ✅ Clear logging during shutdown process
- ✅ Metrics for shutdown success/failure
- ✅ Health checks for shutdown readiness
- ✅ Documentation for operators

---

## 9. Risk Assessment

### 9.1 High Risk
- **Breaking existing functionality** during integration
- **Race conditions** during shutdown process
- **Resource deadlocks** in complex shutdown scenarios

### 9.2 Medium Risk
- **Platform differences** between macOS and Linux service management
- **Performance impact** of shutdown framework overhead
- **Configuration complexity** for operators

### 9.3 Mitigation Strategies
- Comprehensive testing at each phase
- Feature flags for gradual rollout
- Fallback to existing behavior if framework fails
- Clear documentation and examples

---

## 10. Dependencies

### 10.1 External Dependencies
- `tokio` - Async runtime and signal handling
- `clap` - CLI argument parsing
- `serde` - Configuration serialization
- `tracing` - Logging during shutdown

### 10.2 Internal Dependencies
- `crates/common` - Shared shutdown framework
- All daemon binaries - Integration points
- E2E test framework - Updated test patterns

---

## 11. Conclusion

This graceful shutdown framework is **critical infrastructure** that transforms FleetingDNS from a development prototype into a production-ready system. The unified approach ensures consistency across all components while providing the operational controls necessary for reliable deployment and management.

The inclusion of plist and systemd files enables seamless local deployment, bridging the gap between development and containerized production environments. The control CLI provides operators with precise tools for managing daemon lifecycles without resorting to crude process management techniques.

**Next Steps**: Begin Phase 1 implementation with the core shutdown framework, focusing on signal handling and Unix socket communication as the foundation for all subsequent daemon integrations. 