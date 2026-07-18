# FleetingDNS Technical Context

## Technology Stack

### Core Language & Runtime
- **Rust**: Memory-safe systems programming language
- **Edition**: 2021 (latest stable features)
- **Toolchain**: Stable channel with nightly for coverage
- **Async Runtime**: Tokio 1.46+ for async/await support

### Dependencies & Libraries

#### DNS & Networking
- **hickory-proto**: DNS protocol implementation
- **hickory-resolver**: DNS resolution capabilities
- **tokio**: Async runtime and networking
- **socket2**: Low-level socket operations

#### Security & TLS
- **rustls**: Pure Rust TLS implementation
- **tokio-rustls**: Async TLS integration
- **rustls-pemfile**: PEM file parsing
- **rcgen**: Certificate generation for testing

#### Data Storage & Caching
- **redis**: Redis client with async support
- **bb8**: Connection pooling for Redis
- **bb8-redis**: Redis-specific connection pool

#### Serialization & Configuration
- **serde**: Serialization/deserialization framework
- **serde_json**: JSON support
- **toml**: Configuration file format
- **clap**: Command-line argument parsing

#### Observability & Metrics
- **tracing**: Structured logging and instrumentation
- **tracing-subscriber**: Log formatting and output
- **metrics**: Metrics collection framework
- **metrics-exporter-prometheus**: Prometheus metrics export

#### Testing & Development
- **cargo-llvm-cov**: Code coverage analysis
- **tracing-test**: Testing with tracing support
- **mini-redis**: Redis server for testing

## Infrastructure Technologies

### Container & Orchestration
- **Docker**: Containerization platform
- **Kubernetes**: Container orchestration
- **Crossplane**: Infrastructure as Code
- **Flux**: GitOps continuous delivery

### Cloud & Providers
- **Google Cloud Platform**: Primary cloud provider
- **Upbound Providers**: Modern Crossplane providers
- **Crossplane Providers**: Legacy advanced services

### Monitoring & Observability
- **Prometheus**: Metrics collection and storage
- **Grafana**: Metrics visualization
- **Jaeger**: Distributed tracing
- **Loki**: Log aggregation

## Development Tools

### Build & Package Management
- **Cargo**: Rust package manager and build tool
- **Justfile**: Task runner for development commands
- **rust-toolchain.toml**: Toolchain specification

### Code Quality
- **rustfmt**: Code formatting
- **clippy**: Linting and code analysis
- **cargo-audit**: Security vulnerability scanning
- **cargo-deny**: Dependency management and licensing

### Testing Tools
- **cargo test**: Unit and integration testing
- **cargo-llvm-cov**: Code coverage analysis
- **act**: Local GitHub Actions testing
- **Docker Compose**: Integration test environment

### CI/CD
- **GitHub Actions**: Continuous integration
- **GitOps**: Deployment automation
- **Flux**: Kubernetes deployment
- **Crossplane**: Infrastructure provisioning

## Architecture Details

### Crate Structure
```
fleetingdns/
├── cmd/                    # Binary applications
│   ├── api-bin/           # Main API server
│   ├── dnsd-bin/          # DNS server binary
│   ├── edgehub-bin/       # Edge hub server
│   ├── fleetingdns-ctl/   # Control interface
│   └── */                 # Other service binaries
├── crates/                # Library crates
│   ├── common/            # Shared utilities
│   ├── dnsd/              # DNS server logic
│   ├── edgehub/           # Edge hub logic
│   ├── auth/              # Authentication
│   └── */                 # Other service crates
└── crates/bin/            # Legacy binaries
    ├── dnsd/              # Configurable DNS server
    └── slot-setter/       # Redis slot management
```

### Async Architecture
```rust
// Tokio-based async architecture
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Create services
    let dns_service = DnsService::new().await?;
    let edge_service = EdgeService::new().await?;
    
    // Spawn concurrent tasks
    let dns_handle = tokio::spawn(dns_service.run());
    let edge_handle = tokio::spawn(edge_service.run());
    
    // Wait for completion
    tokio::try_join!(dns_handle, edge_handle)?;
    Ok(())
}
```

### Error Handling Strategy
```rust
// Comprehensive error handling
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FleetingDnsError {
    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),
    
    #[error("Redis connection error")]
    Redis(#[from] redis::RedisError),
    
    #[error("TLS error")]
    Tls(#[from] rustls::Error),
    
    #[error("IO error")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, FleetingDnsError>;
```

## Configuration Management

### Environment Variables
```bash
# Core configuration
FLEETINGDNS_BIND_ADDR=0.0.0.0:6353
FLEETINGDNS_REDIS_URL=redis://localhost:6379
FLEETINGDNS_LOG_LEVEL=info

# Security configuration
FLEETINGDNS_TLS_CERT_PATH=/etc/certs/server.crt
FLEETINGDNS_TLS_KEY_PATH=/etc/certs/server.key

# Feature flags
FLEETINGDNS_ENABLE_DNSSEC=true
FLEETINGDNS_ENABLE_METRICS=true
```

### Configuration Files
```toml
# fleetingdns.toml
[server]
bind_addr = "0.0.0.0:6353"
worker_threads = 4

[redis]
url = "redis://localhost:6379"
pool_size = 10
timeout_ms = 5000

[tls]
cert_path = "/etc/certs/server.crt"
key_path = "/etc/certs/server.key"
```

## Performance Characteristics

### Benchmarks
- **DNS Resolution**: <10ms average latency
- **Throughput**: 10,000+ queries/second
- **Memory Usage**: <100MB baseline
- **Connection Handling**: 1,000+ concurrent connections

### Optimization Strategies
- **Connection Pooling**: bb8 for Redis connections
- **Async Processing**: Non-blocking I/O operations
- **Caching**: Redis-based DNS response caching
- **Resource Management**: Careful memory and connection management

## Security Implementation

### TLS Configuration
```rust
// TLS server configuration
let tls_config = ServerConfig::builder()
    .with_cipher_suites(&[TLS13_AES_256_GCM_SHA384])
    .with_kx_groups(&[&X25519])
    .with_protocol_versions(&[&TLS13])
    .with_no_client_auth()
    .with_single_cert(cert_chain, private_key)?;
```

### Authentication & Authorization
- **JWT Tokens**: Stateless authentication
- **RBAC**: Role-based access control
- **API Keys**: Service-to-service authentication
- **mTLS**: Mutual TLS for internal services

## Testing Strategy

### Test Categories
1. **Unit Tests**: Individual function testing
2. **Integration Tests**: Service interaction testing
3. **E2E Tests**: End-to-end workflow testing
4. **Performance Tests**: Load and stress testing

### Coverage Targets
- **Minimum**: 65% (TDD compliance)
- **Target**: 80% (excellence goal)
- **Critical Paths**: 95% (security, DNS resolution)

### Test Infrastructure
```rust
// Test setup pattern
#[tokio::test]
async fn test_dns_resolution() {
    let redis = start_test_redis().await;
    let dns_server = DnsServer::new(redis.url()).await.unwrap();
    
    let response = dns_server.resolve("example.com").await.unwrap();
    assert_eq!(response.status(), ResponseCode::NoError);
}
```

## Deployment Architecture

### Container Strategy
```dockerfile
# Multi-stage build for optimization
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/dnsd-bin /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/dnsd-bin"]
```

### Kubernetes Deployment
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: fleetingdns-dnsd
spec:
  replicas: 3
  selector:
    matchLabels:
      app: fleetingdns-dnsd
  template:
    metadata:
      labels:
        app: fleetingdns-dnsd
    spec:
      containers:
      - name: dnsd
        image: fleetingdns/dnsd:latest
        ports:
        - containerPort: 6353
        env:
        - name: FLEETINGDNS_REDIS_URL
          value: "redis://redis:6379"
```

## Future Technical Considerations

### Scalability Plans
- **Horizontal Scaling**: Multi-instance deployment
- **Geographic Distribution**: Multi-region deployment
- **Load Balancing**: Intelligent request routing
- **Auto-scaling**: Dynamic scaling based on load

### Technology Evolution
- **Rust Updates**: Stay current with Rust releases
- **Dependency Updates**: Regular dependency maintenance
- **Security Updates**: Proactive security patching
- **Performance Optimization**: Continuous performance improvement
