# DNS Service (dnsd)

The DNS service provides authoritative DNS resolution for FleetingDNS ephemeral subdomains. It supports both UDP and DNS-over-TLS (DoT) protocols.

## Features

- **UDP DNS Server**: Standard DNS queries over UDP port 53
- **DNS-over-TLS (DoT)**: Encrypted DNS queries over TLS port 853
- **Redis Integration**: Dynamic subdomain resolution from Redis
- **DNSSEC Support**: Optional DNSSEC signing for responses
- **DDoS Protection**: Rate limiting and connection protection
- **Performance Metrics**: Comprehensive monitoring and observability

## Metrics

The DNS service exports the following metrics for monitoring and observability:

### DNS Query Counter

**Metric Name**: `dns_queries_total`

**Description**: Total number of DNS queries processed by the service.

**Labels**:
- `protocol`: The DNS protocol used (`udp` or `dot`)

**Example**:
```
dns_queries_total{protocol="udp"} 1234
dns_queries_total{protocol="dot"} 567
```

**Usage**: This metric helps track DNS query volume and protocol distribution. It's useful for:
- Monitoring DNS service load
- Understanding protocol usage patterns
- Capacity planning
- Alerting on unusual query volumes

### Implementation

The metrics are automatically incremented for each incoming DNS query:

- **UDP Protocol**: Metrics are incremented in `crates/dnsd/src/lib.rs` when a UDP packet is received
- **DoT Protocol**: Metrics are incremented in `crates/dnsd/src/lib.rs` when a TLS connection processes a DNS query

The metrics use the `metrics` crate and are compatible with Prometheus and other monitoring systems.

## Configuration

The DNS service can be configured via environment variables:

```bash
# DNS server configuration
DNS_BIND_ADDR=0.0.0.0          # Bind address for DNS server
DNS_PORT=6353                   # Port for DNS server
DNS_ENABLE_DNSSEC=true         # Enable DNSSEC signing
DNS_ENABLE_DDOS_PROTECTION=true # Enable DDoS protection
DNS_CACHE_TTL=300              # Cache TTL in seconds
DNS_MAX_CACHE_SIZE=5000        # Maximum cache size

# Redis configuration
REDIS_URL=redis://localhost:6379
REDIS_POOL_SIZE=10
REDIS_TIMEOUT_SECS=5

# Metrics configuration
METRICS_ENABLED=true
METRICS_PORT=9090
```

## Testing

Integration tests are available to verify metrics functionality:

```bash
# Run metrics integration tests
RUN_REDIS_TESTS=1 cargo test -p dnsd --test metrics_integration

# Run all DNS tests
cargo test -p dnsd
```

## Architecture

The DNS service consists of several components:

- **Main Server Loop**: Handles UDP connections and spawns DoT connections
- **DNS Handler**: Processes DNS queries and builds responses
- **Redis Cache**: Stores ephemeral subdomain mappings
- **DNSSEC Signer**: Signs responses when enabled
- **Metrics Collection**: Tracks query volumes and performance

## Monitoring

The DNS service exports metrics that can be scraped by Prometheus or other monitoring systems. Key metrics to monitor:

- `dns_queries_total`: Query volume by protocol
- Response times and error rates
- Cache hit ratios
- DNSSEC signing performance

## Related Documentation

- [FleetingDNS Architecture](../README.md)
- [Redis Integration](src/redis_cache.rs)
- [DNSSEC Implementation](src/sign.rs)
- [Performance Configuration](src/dns_handler.rs) 