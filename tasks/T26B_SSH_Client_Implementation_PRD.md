# PRD: SSH Client Implementation for T-26b Dynamic Reverse Proxy

> **⚠️ STATUS (2026-07-17): SUPERSEDED.** The "dynamic reverse proxy" architecture this PRD
> targets was dead-on-arrival (allocated ports with no listeners; see postmortem and story
> D-4) and is being deleted (TDP-10). The SSH client now exists and uses
> `tcpip-forward`/`forwarded-tcpip` (TDP-1). Still-open items from this PRD carried forward
> as stories: subdomain collision → TDP-14/backlog, port-allocation coordination → TDP-3
> (slot gate) resolved it, issued-key auth → TDP-12/TDP-13. Authoritative:
> `docs/engineering/stories_detailed/E2_E3_tunnel_data_plane_user_stories_v0.3.md`.

## Document Overview

**Version**: 1.0  
**Date**: August 16, 2025  
**Status**: Superseded (was: Draft)  
**Owner**: Engineering Team  
**Related Documents**: 
- [Production Readiness PRD](../tasks/production_readiness_prd.md)
- [T-26b Epic Story](../docs/engineering/Epic_highlevel/E2-Tunnel_Server_&_CLI_(Design_v0.2).md)
- [EdgeHub SSH Server Implementation](../crates/edgehub/src/ssh_server.rs)

---

## Executive Summary

This PRD addresses the critical gap in T-26b implementation: the CLI can create tunnels via the API and establish TCP connections to EdgeHub, but lacks the SSH client functionality needed to establish proper reverse tunnels. Without this, the core dynamic reverse proxy feature remains non-operational.

**Current Status**: CLI infrastructure complete, TCP connection established, but protocol mismatch prevents tunnel establishment.

**Goal**: Implement production-ready SSH client that can establish authenticated SSH connections to EdgeHub and create functional reverse tunnels.

---

## Problem Statement

**Current Status**: CLI infrastructure complete, TCP connection established, but protocol mismatch prevents tunnel establishment.

**Root Cause**: The CLI is attempting to send plain TCP messages to EdgeHub's port 8443, which expects TLS-wrapped SSH connections. This creates a fundamental protocol mismatch that prevents tunnel establishment.

**Impact**: T-26b (Dynamic Reverse Proxy) cannot be completed, blocking end-to-end tunnel functionality for FleetingDNS.

## Critical Implementation Failures

**⚠️ MULTI-USER FUNCTIONALITY IS BROKEN** - The following implementation failures prevent the multi-user concept from working:

### 1. **Subdomain Collision System Failure**
- **Problem**: CLI hardcodes subdomain to `"test"` when none provided
- **Location**: `cmd/edf-cli/src/tunnel.rs:54` - `subdomain.unwrap_or_else(|| "test".to_string())`
- **Impact**: Multiple users will always get the same subdomain, causing conflicts
- **Required Fix**: Implement proper subdomain generation with collision detection

### 2. **Redis Subdomain Mapping Inconsistency**
- **Problem**: Two different subdomain lookup systems exist and are not synchronized
- **Location 1**: `crates/backendapi/src/storage.rs:145` - Uses `subdomain:{subdomain}` key format
- **Location 2**: `crates/common/src/redis/tunnel.rs:141` - Scans `tunnel_lookup:*` keys
- **Impact**: EdgeHub cannot find tunnels created by API, breaking the routing system
- **Required Fix**: Unify subdomain storage and lookup across all components

### 3. **Port Allocation Conflict**
- **Problem**: API allocates ports (10000-65535) but EdgeHub allocates different ports (30000-60000)
- **Location 1**: `crates/backendapi/src/storage.rs:300` - API port range
- **Location 2**: `crates/edgehub/src/ssh_server.rs:45` - EdgeHub port range
- **Impact**: Ports allocated by API are never used by EdgeHub, breaking tunnel routing
- **Required Fix**: Coordinate port allocation between API and EdgeHub

### 4. **Tunnel Registration Disconnect**
- **Problem**: EdgeHub's `register_reverse_tunnel` allocates ports but doesn't update Redis subdomain mapping
- **Location**: `crates/edgehub/src/ssh_server.rs:613` - Only updates local state
- **Impact**: HTTPS router cannot route requests to tunnels, returning 404 for all subdomains
- **Required Fix**: EdgeHub must update Redis subdomain mapping when registering tunnels

### 5. **HTTPS Router Integration Failure**
- **Problem**: HTTPS router looks up tunnels in Redis but EdgeHub stores them in local memory
- **Location**: `cmd/edgehub-bin/src/main.rs:74` - Calls `get_tunnel_by_subdomain` in Redis
- **Impact**: No HTTP requests can reach tunnels, making the system unusable
- **Required Fix**: HTTPS router must use EdgeHub's tunnel routing system

### 6. **Subdomain Generation Logic Missing**
- **Problem**: API has `generate_random_subdomain()` function but CLI doesn't use it
- **Location**: `crates/backendapi/src/handlers/tunnels.rs:760` - Function exists but unused
- **Impact**: CLI creates predictable subdomains, causing conflicts in multi-user scenarios
- **Required Fix**: CLI must use API's subdomain generation and availability checking

## Solution Overview

Implement a proper SSH client using the `russh` crate that can:
1. **Establish SSH connections** to EdgeHub's SSH server (port 8443)
2. **Authenticate** using key-based authentication
3. **Create reverse port forwarding** channels for tunnel data
4. **Integrate** with the existing dynamic port allocation system

**⚠️ CRITICAL**: Before implementing SSH client, the above multi-user failures must be addressed to prevent a broken system.

## Implementation Strategy Options

**🎯 RECOMMENDED APPROACH: Option A (Single User First)**

### **Option A: Single User End-to-End Validation First**
**Goal**: Get one tunnel working completely before scaling to multi-user

**Rationale**: The current system is fundamentally broken and we need to prove the core tunnel concept works before building complex multi-user infrastructure around it.

**Pros**:
- ✅ **Faster validation** of core concepts
- ✅ **Simpler debugging** - fewer moving parts
- ✅ **Prove the architecture** works fundamentally
- ✅ **Lower risk** of complex integration issues
- ✅ **Faster feedback loop** for development
- ✅ **Learn real requirements** before building infrastructure

**Cons**:
- ❌ **Technical debt** - will need to refactor for multi-user later
- ❌ **Architecture assumptions** might be wrong for multi-user
- ❌ **Redis integration** still needs to be built (even for single user)

**Implementation Path**:
1. **Week 1**: Fix minimal single-user tunnel (bypass Redis, hardcode subdomain)
2. **Week 2**: Get SSH client working and establish first tunnel
3. **Week 3**: Validate end-to-end data flow works
4. **Week 4**: Add Redis integration and subdomain management
5. **Week 5**: Scale to multi-user with proven architecture

### **Option B: Multi-User Infrastructure First**
**Goal**: Fix all infrastructure issues before implementing SSH client

**Pros**:
- ✅ **Correct architecture** from the start
- ✅ **No technical debt** - built for scale from day one
- ✅ **Redis integration** properly designed
- ✅ **Port allocation** coordinated across all components
- ✅ **Production-ready** infrastructure

**Cons**:
- ❌ **Longer development time** before first working tunnel
- ❌ **More complex debugging** - multiple systems to coordinate
- ❌ **Higher risk** of integration issues
- ❌ **Delayed validation** of core tunnel functionality
- ❌ **Building infrastructure** around unproven concepts

**Implementation Path**:
1. **Week 1**: Fix all multi-user infrastructure failures
2. **Week 2**: Implement SSH client foundation
3. **Week 3**: Add reverse port forwarding
4. **Week 4**: Tunnel management and lifecycle
5. **Week 5**: Integration testing and optimization

## **RECOMMENDED IMPLEMENTATION PLAN**

**Phase 0: Single User Tunnel Validation (Week 1)**
- [ ] **Bypass Redis Complexity**: Use hardcoded subdomain "test" temporarily
- [ ] **Fix Port Coordination**: Align API and EdgeHub port allocation for single tunnel
- [ ] **Implement Basic SSH Client**: Establish SSH connection to EdgeHub
- [ ] **Create First Working Tunnel**: Prove reverse port forwarding works
- [ ] **Validate Data Flow**: End-to-end HTTP request → tunnel → local service

**Phase 1: SSH Client Foundation (Week 2)**
- [ ] **Enhance SSH Client**: Add authentication and channel management
- [ ] **Tunnel Lifecycle**: Start, monitor, and stop tunnel functionality
- [ ] **Error Handling**: Robust connection management and recovery
- [ ] **Performance Testing**: Validate tunnel establishment time and throughput

**Phase 2: Redis Integration (Week 3)**
- [ ] **Unify Subdomain Storage**: Single Redis schema for subdomain mapping
- [ ] **Port Allocation Coordination**: Centralized port management
- [ ] **Tunnel Registration**: EdgeHub updates Redis when registering tunnels
- [ ] **HTTPS Router Integration**: Connect router to EdgeHub's tunnel system

**Phase 3: Multi-User Scaling (Week 4)**
- [ ] **Subdomain Generation**: Implement collision detection and availability checking
- [ ] **User Isolation**: Ensure tunnels are properly isolated per user
- [ ] **Concurrent Tunnel Support**: Handle multiple active tunnels simultaneously
- [ ] **Resource Management**: Proper cleanup and port deallocation

**Phase 4: Production Readiness (Week 5)**
- [ ] **End-to-End Testing**: Multi-user scenarios with Docker Compose
- [ ] **Performance Optimization**: Memory usage, CPU, and network latency
- [ ] **Security Hardening**: Authentication, encryption, and access control
- [ ] **Documentation**: User guides and operational procedures

## Architecture Understanding

**EdgeHub Multi-Server Architecture**:
- **TLS Server**: `0.0.0.0:8443` - Handles TLS-wrapped connections
- **HTTPS Router**: `0.0.0.0:443` - SNI-based HTTP routing for tunnel access
- **SSH Server**: `0.0.0.0:8443` - SSH-over-TLS for tunnel establishment

**Multi-Tenant Design**:
- **Dynamic Port Allocation**: Each tunnel gets a unique port (30000-60000 range)
- **SNI-based Routing**: Unique subdomains route to allocated tunnel ports
- **Independent Tunnels**: Multiple users can have active tunnels simultaneously
- **No Port Conflicts**: Each tunnel operates on its own allocated port

**Example Multi-Tenant Setup**:
```
User A: tunnel-abc123.fleetingdns.run → Port 30123
User B: tunnel-def456.fleetingdns.run → Port 30456  
User C: tunnel-ghi789.fleetingdns.run → Port 30789
```

## User Stories & Acceptance Criteria

### Primary User Story
As a developer, I want to establish a secure SSH tunnel through EdgeHub so that I can expose my local service to the internet with a unique subdomain.

### Acceptance Criteria
1. **SSH Connection**: CLI successfully establishes SSH connection to EdgeHub port 8443
2. **Authentication**: SSH key-based authentication works with EdgeHub
3. **Tunnel Creation**: Reverse port forwarding channel is established
4. **Port Allocation**: EdgeHub allocates unique tunnel port (30000-60000)
5. **Subdomain Assignment**: Unique subdomain is assigned to the tunnel
6. **Data Forwarding**: HTTP requests to subdomain route to local service
7. **Tunnel Lifecycle**: Tunnel can be started, monitored, and stopped

## Technical Requirements

### Functional Requirements
- **SSH Protocol Support**: Full SSH 2.0 protocol implementation
- **Key-based Authentication**: Support for Ed25519/RSA private keys
- **Reverse Port Forwarding**: SSH channel type for tunnel data
- **Dynamic Port Management**: Integration with EdgeHub's port allocation
- **Subdomain Registration**: Automatic subdomain assignment and DNS updates
- **Connection Monitoring**: Keep-alive and health checking

### Non-Functional Requirements
- **Performance**: <2 second tunnel establishment
- **Security**: TLS-wrapped SSH with certificate validation
- **Reliability**: Automatic reconnection on failures
- **Scalability**: Support for multiple concurrent tunnels

## Implementation Design

### High-Level Architecture
```
CLI (SSH Client) → EdgeHub (SSH Server) → Dynamic Port Allocation → HTTP Router
     ↓                    ↓                        ↓                    ↓
Local Service ← SSH Channel ← Reverse Port Forward ← Tunnel Port ← SNI Routing
```

### Authentication Flow
1. CLI loads private key from `~/.ssh/id_ed25519`
2. Establishes SSH connection to EdgeHub:8443
3. Sends public key for authentication
4. EdgeHub validates key against Redis user database
5. Authentication successful, session established

### Reverse Tunnel Flow
1. CLI requests reverse port forwarding for local port
2. EdgeHub allocates unique tunnel port (30000-60000)
3. Creates SSH channel for data forwarding
4. Registers subdomain → tunnel port mapping
5. Updates DNS records for subdomain
6. Establishes bidirectional data flow

### Data Forwarding
```rust
// SSH channel data forwarding
async fn forward_tunnel_data(
    ssh_channel: Channel<Msg>,
    local_stream: TcpStream,
) -> Result<()> {
    let (mut ssh_read, mut ssh_write) = ssh_channel.split();
    let (mut local_read, mut local_write) = local_stream.split();
    
    // Bidirectional data copying
    let (_, _) = tokio::join!(
        tokio::io::copy(&mut local_read, &mut ssh_write),
        tokio::io::copy(&mut ssh_read, &mut local_write)
    );
    
    Ok(())
}
```

## Implementation Phases

### Phase 0: Single User Tunnel Validation (CRITICAL - Week 1)
- [x] **Bypass Redis Complexity**: Use hardcoded subdomain "test" temporarily
- [x] **Fix Port Coordination**: Align API and EdgeHub port allocation for single tunnel
- [x] **Implement Basic SSH Client**: Establish SSH connection to EdgeHub
- [ ] **Create First Working Tunnel**: Prove reverse port forwarding works
- [ ] **Validate Data Flow**: End-to-end HTTP request → tunnel → local service

**Phase 0 Progress**: 
- ✅ **Basic connectivity verified** - CLI successfully connects to EdgeHub port 8443
- ✅ **Architecture validated** - EdgeHub is running and expecting SSH connections
- 🔄 **Next**: Implement proper SSH handshake to replace plain TCP connection
- 🔄 **Next**: Test SSH authentication and tunnel establishment

### Phase 1: SSH Client Foundation (Week 2)
- [ ] **Enhance SSH Client**: Add authentication and channel management
- [ ] **Tunnel Lifecycle**: Start, monitor, and stop tunnel functionality
- [ ] **Error Handling**: Robust connection management and recovery
- [ ] **Performance Testing**: Validate tunnel establishment time and throughput

### Phase 2: Redis Integration (Week 3)
- [ ] **Unify Subdomain Storage**: Single Redis schema for subdomain mapping
- [ ] **Port Allocation Coordination**: Centralized port management
- [ ] **Tunnel Registration**: EdgeHub updates Redis when registering tunnels
- [ ] **HTTPS Router Integration**: Connect router to EdgeHub's tunnel system

### Phase 3: Multi-User Scaling (Week 4)
- [ ] **Subdomain Generation**: Implement collision detection and availability checking
- [ ] **User Isolation**: Ensure tunnels are properly isolated per user
- [ ] **Concurrent Tunnel Support**: Handle multiple active tunnels simultaneously
- [ ] **Resource Management**: Proper cleanup and port deallocation

### Phase 4: Production Readiness (Week 5)
- [ ] **End-to-End Testing**: Multi-user scenarios with Docker Compose
- [ ] **Performance Optimization**: Memory usage, CPU, and network latency
- [ ] **Security Hardening**: Authentication, encryption, and access control
- [ ] **Documentation**: User guides and operational procedures

## Technical Implementation Details

### Dependencies
```toml
[dependencies]
russh = "0.40"
russh-keys = "0.40"
tokio-rustls = "0.26"
rustls = "0.23"
rustls-pemfile = "1"
```

### Key Files
- `cmd/edf-cli/src/ssh_client.rs` - SSH client implementation
- `cmd/edf-cli/src/tunnel.rs` - Tunnel management integration
- `crates/edgehub/src/ssh_server.rs` - SSH server integration

### Error Handling
- SSH connection failures
- Authentication errors
- Port allocation conflicts
- Network timeouts
- Channel establishment failures

### Logging & Monitoring
- SSH connection events
- Authentication attempts
- Tunnel lifecycle events
- Performance metrics
- Error tracking

## Testing Strategy

### Unit Tests
- SSH client functionality
- Key management
- Channel operations
- Error handling

### Integration Tests
- SSH connection to EdgeHub
- Authentication flow
- Port allocation
- Subdomain registration

### Performance Tests
- Tunnel establishment time
- Data throughput
- Concurrent tunnel limits
- Memory usage

### Security Tests
- Key validation
- Authentication bypass attempts
- Certificate pinning
- Brute force protection

## Risk Assessment & Mitigation

### High Risk
- **SSH Protocol Complexity**: Mitigation: Use mature `russh` crate
- **Performance Impact**: Mitigation: Optimize data forwarding algorithms
- **Multi-User Infrastructure Failures**: Mitigation: Fix all identified issues before SSH implementation

### Medium Risk
- **Key Management**: Mitigation: Secure key storage and validation
- **Network Failures**: Mitigation: Robust reconnection logic

### Low Risk
- **Port Conflicts**: Mitigation: Dynamic port allocation system
- **Subdomain Collisions**: Mitigation: Unique generation algorithms

## Success Metrics

### Functional Metrics
- SSH connection success rate >99%
- Tunnel establishment time <2 seconds
- Data forwarding reliability >99.9%
- **Multi-user support**: 10+ concurrent tunnels without conflicts

### Performance Metrics
- Memory usage <50MB per tunnel
- CPU usage <5% per tunnel
- Network latency <100ms

### Quality Metrics
- Test coverage >80%
- Zero security vulnerabilities
- Documentation completeness

## Dependencies & Prerequisites

### Existing Dependencies
- EdgeHub SSH server implementation
- Redis tunnel metadata storage
- Dynamic port allocation system
- SNI-based HTTP routing

### External Dependencies
- `russh` crate for SSH client
- `russh-keys` for key management
- `tokio-rustls` for TLS integration

### Critical Prerequisites
- **MUST FIX**: All multi-user infrastructure failures identified above
- **MUST VERIFY**: Multi-user scenarios work before SSH implementation
- **MUST TEST**: End-to-end tunnel routing with multiple concurrent users

## Conclusion

This SSH client implementation will complete T-26b by providing the missing client-side SSH functionality needed to establish secure tunnels through EdgeHub. The multi-tenant architecture with dynamic port allocation ensures scalability while maintaining security and performance.

**🎯 RECOMMENDED APPROACH**: **Single User Validation First, Then Multi-User Scaling**

**Rationale**: The current system has fundamental architectural issues that need to be resolved at the core level. By starting with a single working tunnel, we can:
1. **Prove the concept works** before building complex infrastructure
2. **Learn real requirements** through actual implementation
3. **Validate the architecture** before scaling to multi-user
4. **Reduce risk** by testing core functionality first

**⚠️ CRITICAL WARNING**: The current system has multiple implementation failures that prevent multi-user functionality. However, we will address these incrementally, starting with a working single-user tunnel and then scaling up.

**Next Steps**: 
1. **IMMEDIATE**: Implement single-user tunnel validation (Phase 0)
2. **VALIDATE**: Prove end-to-end tunnel functionality works
3. **ENHANCE**: Add Redis integration and proper infrastructure (Phase 2)
4. **SCALE**: Extend to multi-user with proven architecture (Phase 3)
5. **PRODUCTION**: Optimize and harden for production use (Phase 4)
