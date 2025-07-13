# FleetingDNS Prototype 0.4 Implementation PRD: SSH Tunnel Infrastructure

## Executive Summary

This PRD defines the implementation of SSH tunnel infrastructure for FleetingDNS, enabling developers to create secure reverse tunnels for webhook testing and OAuth callback handling. The implementation builds on the existing EdgeHub architecture and integrates with the FleetingDNS authentication and certificate infrastructure.

## Background

FleetingDNS is a secure reverse tunneling service that provides ephemeral subdomains for development and testing. The system uses a comprehensive architecture involving:

- **edf-cli**: Command-line tool for tunnel management
- **edf-api**: Authentication and tunnel orchestration service  
- **edf-ca**: Certificate Authority for ephemeral certificates
- **EdgeHub**: SSH server and traffic routing component
- **edf-edge**: HTTP router and proxy layer

## Problem Statement

Developers need secure, enterprise-grade reverse tunnels to:
1. Test webhooks from external services in local development environments
2. Handle OAuth callbacks that require HTTPS and public domains
3. Bypass corporate firewalls (port 443 SSH-over-TLS)
4. Collaborate securely with team members and stakeholders

## Solution Overview

### Architecture Components

The SSH tunnel implementation integrates with the existing FleetingDNS infrastructure:

```
┌─────────────────┐    HTTPS/443     ┌─────────────────┐    SSH Tunnel    ┌─────────────────┐
│   Consumer      │◄────────────────►│    EdgeHub      │◄────────────────►│   Developer     │
│   Internet      │  myservice123.   │   Port 443      │    Port 443      │   Local App     │
│                 │  fleetingdns.run  │   Public GW     │   (Reverse)      │   Port 8080     │
└─────────────────┘                  └─────────────────┘                  └─────────────────┘
```

### Key Features

1. **TLS-Wrapped SSH**: SSH traffic wrapped in TLS on port 443 for firewall compatibility
2. **Ephemeral PKI**: Short-lived certificates (30 minutes) issued per session
3. **GitHub Authentication**: OAuth-based developer authentication
4. **Reverse Tunnel Architecture**: Outbound-only connections from developer machines
5. **Dynamic Subdomain Generation**: Automatic assignment of unique subdomains

## User Workflow

### Tunnel Creation Process

The developer interacts with FleetingDNS through the CLI/SDK, which handles all authentication and security automatically:

#### 1. Authentication & Certificate Issuance
```bash
# Developer initiates tunnel creation
edf forward 8080 --ttl 1800

# CLI handles GitHub OAuth authentication (if needed)
# API issues ephemeral certificate and SSH keys
# Returns tunnel configuration
```

**Sequence:**
1. **CLI Authentication**: `edf-cli` authenticates with `edf-api` via GitHub OAuth
2. **Certificate Request**: CLI submits CSR to `edf-ca` for ephemeral certificate
3. **Tunnel Configuration**: API returns:
   ```json
   {
     "fqdn": "myservice123456.fleetingdns.run",
     "slot": 12345,
     "tls_cert": "-----BEGIN CERTIFICATE-----...",
     "private_key": "-----BEGIN PRIVATE KEY-----...",
     "ssh_key": "-----BEGIN OPENSSH PRIVATE KEY-----...",
     "expires_at": "2025-01-15T15:30:00Z"
   }
   ```

#### 2. Secure Tunnel Establishment
```bash
# CLI automatically establishes TLS+SSH tunnel to EdgeHub
# No manual SSH commands required - all handled by edf-cli
```

**Technical Flow:**
1. **TLS Connection**: CLI connects to EdgeHub on port 443 using ephemeral certificate
2. **Mutual TLS**: EdgeHub verifies client certificate against `edf-ca`
3. **SSH Handshake**: SSH session established inside TLS tunnel using ephemeral SSH keys
4. **Reverse Tunnel**: SSH reverse port forwarding configured automatically
5. **DNS Registration**: Subdomain activated and routed to EdgeHub

#### 3. Service Exposure
```bash
# Tunnel is now active and ready for use
echo "Tunnel active: https://myservice123456.fleetingdns.run -> localhost:8080"
echo "Expires: 2025-01-15T15:30:00Z"
```

### Developer Usage Examples

#### Webhook Testing
```bash
# Start local development server
npm start  # Running on localhost:8080

# Create tunnel (in separate terminal)
edf forward 8080 --auth --ttl 3600
# Output: Tunnel: https://webhook-test-abc123.fleetingdns.run -> localhost:8080
# Output: Basic Auth: user=u7x82q, pass=dkT9!qH0

# Configure webhook provider (e.g., Stripe, GitHub)
# URL: https://webhook-test-abc123.fleetingdns.run/webhook
# The webhook will be securely routed to localhost:8080/webhook
```

#### OAuth Callback Testing
```bash
# Start OAuth-enabled app
python app.py  # Running on localhost:3000

# Create tunnel for OAuth callbacks
edf forward 3000 --no-auth --ttl 1800
# Output: Tunnel: https://oauth-callback-def456.fleetingdns.run -> localhost:3000

# Configure OAuth provider redirect URI:
# https://oauth-callback-def456.fleetingdns.run/auth/callback
# OAuth flow will work exactly as in production
```

#### Team Collaboration
```bash
# Share development environment with team
edf forward 8080 --auth --custom-subdomain myfeature-demo
# Output: Tunnel: https://myfeature-demo.fleetingdns.run -> localhost:8080
# Output: Share credentials: user=teamdemo, pass=secure123

# Team members can access via shared URL with credentials
# Perfect for demos, code reviews, or QA testing
```

### SDK Integration

For programmatic usage, FleetingDNS provides language-specific SDKs:

#### Python SDK
```python
from fleetingdns import Tunnel

# Context manager automatically handles lifecycle
with Tunnel(port=8080, ttl=1800, auth=True) as tunnel:
    print(f"Webhook URL: {tunnel.fqdn}")
    print(f"Auth: {tunnel.auth.username}:{tunnel.auth.password}")
    
    # Run tests that trigger webhooks
    run_webhook_tests(tunnel.fqdn)
    
# Tunnel automatically cleaned up on exit
```

#### JavaScript/TypeScript SDK
```typescript
import { Tunnel } from '@fleetingdns/sdk';

const tunnel = await Tunnel.create({
  port: 3000,
  ttl: 1800,
  auth: false
});

console.log(`OAuth callback URL: ${tunnel.fqdn}/auth/callback`);

// Configure OAuth provider with tunnel.fqdn
await setupOAuthProvider(tunnel.fqdn);

// Cleanup
await tunnel.destroy();
```

#### Go SDK
```go
import "github.com/fleetingdns/sdk-go"

client := fleetingdns.New()
tunnel, err := client.Create(fleetingdns.TunnelConfig{
    Port: 8080,
    TTL:  1800,
    Auth: true,
})

fmt.Printf("Tunnel URL: %s\n", tunnel.FQDN)
defer client.Delete(tunnel.ID)
```

## Security Features

### Authentication & Authorization
- **GitHub OAuth**: Developer identity verification
- **Ephemeral Certificates**: 30-minute TLS client certificates
- **Short-lived SSH Keys**: Session-specific SSH keypairs
- **Optional Basic Auth**: Per-tunnel HTTP authentication
- **HMAC Signature Validation**: Webhook signature verification

### Network Security
- **TLS-over-443**: SSH wrapped in TLS on port 443 (firewall-friendly)
- **Outbound-only**: No inbound ports required on developer machines
- **Certificate Pinning**: CLI validates EdgeHub certificate
- **Mutual TLS**: Both client and server authentication

### Data Protection
- **End-to-end Encryption**: TLS outer + SSH inner encryption layers
- **Memory-only Keys**: Private keys never written to disk
- **Automatic Cleanup**: Certificates and tunnels auto-expire
- **Audit Logging**: All tunnel creation and usage logged

## Implementation Tasks

### Completed Tasks
- [x] **T-26a**: SSH Server Implementation
  - Implemented `russh`-based SSH server in EdgeHub
  - Added support for reverse tunnels and port forwarding
  - Integrated with graceful shutdown framework
  - All SSH server tests passing (6/6)

- [x] **T-26b**: TCP Proxy Implementation  
  - Implemented bidirectional TCP proxy using tokio channels
  - Added reverse tunnel management and subdomain mapping
  - Resolved russh Channel trait limitations
  - Production-ready implementation with proper error handling

### Remaining Implementation Tasks

#### T-26c: API Integration (Priority: High)
**Objective**: Integrate SSH server with FleetingDNS API for authentication and certificate management

**Tasks**:
- [ ] Implement `edf-api` tunnel creation endpoint (`POST /v1/tunnels`)
- [ ] Add GitHub OAuth authentication flow
- [ ] Integrate with `edf-ca` for ephemeral certificate issuance
- [ ] Add tunnel metadata storage in Redis/etcd
- [ ] Implement tunnel expiry and cleanup logic

**Acceptance Criteria**:
- API issues ephemeral certificates with 30-minute TTL
- SSH server validates certificates against `edf-ca`
- Tunnel metadata stored and retrievable
- Automatic cleanup on expiry

#### T-26d: CLI Implementation (Priority: High)  
**Objective**: Implement `edf-cli` tunnel management commands

**Tasks**:
- [ ] Implement `edf forward` command with port forwarding
- [ ] Add GitHub OAuth authentication flow
- [ ] Implement TLS+SSH tunnel establishment
- [ ] Add tunnel status and monitoring commands
- [ ] Implement graceful tunnel shutdown

**Acceptance Criteria**:
- `edf forward 8080` creates working tunnel
- CLI handles authentication automatically
- Tunnel status visible and manageable
- Clean shutdown on interrupt/expiry

#### T-26e: DNS Integration (Priority: Medium)
**Objective**: Integrate tunnel subdomains with DNS system

**Tasks**:
- [ ] Implement dynamic subdomain generation
- [ ] Add DNS record creation/deletion in CoreDNS/etcd
- [ ] Implement subdomain-to-tunnel routing in `edf-edge`
- [ ] Add custom subdomain support (optional)

**Acceptance Criteria**:
- Subdomains automatically resolve to EdgeHub
- HTTP requests route to correct tunnels
- DNS records cleaned up on tunnel expiry

#### T-26f: Docker & CI Integration (Priority: Low)
**Objective**: Update deployment and CI for SSH tunnel support

**Tasks**:
- [ ] Update Docker Compose to expose port 443 for SSH
- [ ] Add SSH server certificates to container mounts
- [ ] Update CI with SSH tunnel integration tests
- [ ] Add metrics collection for tunnel usage

**Acceptance Criteria**:
- Docker Compose supports SSH tunnels
- CI tests cover end-to-end tunnel functionality
- Metrics available for monitoring

## Technical Specifications

### SSH Server Configuration
```rust
// EdgeHub SSH server configuration
pub struct SshServerConfig {
    pub bind_address: SocketAddr,     // 0.0.0.0:443
    pub host_key_path: PathBuf,       // SSH host key
    pub max_connections: usize,       // 1000
    pub connection_timeout: Duration, // 30 seconds
    pub tunnel_ttl: Duration,         // 30 minutes
}
```

### API Endpoints
```yaml
# Tunnel management API
POST /v1/tunnels:
  request:
    port: number
    ttl: number (seconds)
    auth: boolean
    custom_subdomain: string (optional)
  response:
    fqdn: string
    slot: number
    tls_cert: string
    private_key: string
    ssh_key: string
    expires_at: string (ISO 8601)

GET /v1/tunnels/{id}:
  response:
    status: string
    created_at: string
    expires_at: string
    bytes_transferred: number

DELETE /v1/tunnels/{id}:
  response:
    status: string
```

### Certificate Format
```yaml
# Ephemeral TLS certificate
Subject: CN=tunnel-client-{uuid}
Issuer: CN=FleetingDNS-CA
Valid: 30 minutes
Key Usage: Digital Signature, Key Encipherment
Extended Key Usage: Client Authentication
```

## Success Metrics

### Functional Metrics
- **Tunnel Creation Success Rate**: >99.5%
- **Authentication Success Rate**: >99.9%
- **Tunnel Uptime**: >99.9% during valid period
- **DNS Resolution Success**: >99.9%

### Performance Metrics
- **Tunnel Establishment Time**: <5 seconds
- **Certificate Issuance Time**: <1 second
- **Request Latency**: <100ms additional overhead
- **Throughput**: Support 1000+ concurrent tunnels

### Security Metrics
- **Certificate Expiry Compliance**: 100%
- **Authentication Bypass Attempts**: 0
- **Unauthorized Access**: 0
- **Key Material Exposure**: 0

## Risk Assessment

### Technical Risks
- **Certificate Management Complexity**: Mitigated by automated lifecycle
- **SSH Connection Stability**: Mitigated by keep-alive and reconnection logic
- **Port 443 Conflicts**: Mitigated by TLS SNI routing
- **Resource Exhaustion**: Mitigated by connection limits and TTLs

### Security Risks
- **Certificate Compromise**: Mitigated by 30-minute expiry
- **Tunnel Hijacking**: Mitigated by mutual TLS authentication
- **DDoS Attacks**: Mitigated by rate limiting and authentication
- **Data Interception**: Mitigated by end-to-end encryption

### Operational Risks
- **Complex Debugging**: Mitigated by comprehensive logging and metrics
- **User Experience**: Mitigated by CLI automation and clear error messages
- **Scalability**: Mitigated by stateless design and horizontal scaling

## Conclusion

The SSH tunnel infrastructure provides FleetingDNS with enterprise-grade reverse tunneling capabilities while maintaining security and ease of use. The integration with existing authentication and certificate infrastructure ensures a seamless developer experience while meeting corporate security requirements.

The implementation focuses on automation and security-by-default, abstracting away the complexity of SSH and TLS management from end users. This enables developers to focus on their applications while FleetingDNS handles the secure networking infrastructure. 