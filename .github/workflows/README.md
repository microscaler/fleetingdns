# GitHub Actions Workflows

This directory contains the CI/CD workflows for FleetingDNS.

## Workflows

### 1. `rust_ci.yml` - Main Rust CI Pipeline

**Triggers:** Push to main, Pull Requests

**Purpose:** Fast feedback loop for code quality and basic functionality.

**Jobs:**
- **build**: Runs formatting, linting, unit tests, and basic smoke tests
  - Code formatting check (`cargo fmt`)
  - Linting with Clippy (`cargo clippy`)
  - Unit tests (libraries and binaries)
  - DoT (DNS-over-TLS) feature tests
  - Basic dnsd smoke test

**Duration:** ~5-10 minutes

### 2. `testcontainers.yml` - Integration Tests

**Triggers:** Push to main, Pull Requests, Manual dispatch, Nightly schedule (2 AM UTC)

**Purpose:** Comprehensive integration testing with real Redis containers.

**Features:**
- Uses testcontainers-rs for Redis integration testing
- Ephemeral port allocation to prevent conflicts
- Proper Docker container lifecycle management
- Comprehensive Redis cache and slot-setter testing

**Environment Variables:**
- `TESTCONTAINERS_RYUK_DISABLED=true` - Disables Ryuk container cleanup (not needed in CI)
- `TESTCONTAINERS_COMMAND_TIMEOUT=180` - 3-minute timeout for container operations
- `RUST_TEST_THREADS=1` - Serial test execution to avoid Docker conflicts
- `RUST_TEST_TIME_UNIT=180s` - Extended test timeouts for container startup

**Duration:** ~15-20 minutes

### 3. `compose-ci.yml` - Full Stack Integration

**Triggers:** Pull Requests, Manual dispatch

**Purpose:** End-to-end testing with complete Docker Compose stack.

**Features:**
- Full service stack with DNS, Grafana, observability
- Health checks for all services
- Round-trip demo testing

## Testcontainers Configuration

The testcontainers workflow is specifically configured for CI environments:

### Docker Setup
- Uses `docker/setup-buildx-action@v3` for reliable Docker environment
- Pre-pulls Redis 7 Alpine image to reduce test startup time
- Verifies Docker availability before running tests

### Resource Management
- Serial test execution (`RUST_TEST_THREADS=1`) prevents resource conflicts
- Extended timeouts account for container startup in CI environments
- Automatic cleanup of Docker containers and system pruning

### Test Organization
- Full workspace test run for comprehensive coverage
- Specific Redis cache tests (`cargo test -p dnsd redis_cache::tests`)
- Specific slot-setter tests (`cargo test -p slot-setter`)

## Local Testing

To test the workflows locally using [act](https://github.com/nektos/act):

```bash
# Test the main Rust CI workflow
act pull_request -W .github/workflows/rust_ci.yml

# Test the testcontainers workflow (requires Docker)
act pull_request -W .github/workflows/testcontainers.yml

# List available jobs
act pull_request --list
```

## Caching Strategy

Both workflows use GitHub Actions caching to speed up builds:

- **Cargo registry cache**: `~/.cargo/registry`
- **Cargo git index cache**: `~/.cargo/git`
- **Build target cache**: `target/`

Cache keys are based on `Cargo.lock` hash for optimal invalidation.

## Environment Requirements

### Rust CI
- Ubuntu latest runner
- Rust nightly toolchain
- Basic system tools (kdig for DNS testing)

### Testcontainers
- Ubuntu latest runner with Docker support
- Docker Buildx for advanced Docker features
- Sufficient resources for multiple Redis containers
- Network access for Docker image pulls

## Troubleshooting

### Common Issues

1. **Testcontainer timeouts**: Increase `TESTCONTAINERS_COMMAND_TIMEOUT` if containers take longer to start
2. **Docker resource conflicts**: Ensure `RUST_TEST_THREADS=1` is set for serial execution
3. **Image pull failures**: Check network connectivity and Docker Hub availability
4. **Container cleanup issues**: The workflow includes robust cleanup steps that run even on failure

### Debug Options

Enable debug logging by adding to the testcontainers workflow:

```yaml
env:
  RUST_LOG: testcontainers=debug
  TESTCONTAINERS_LOG_LEVEL: DEBUG
```

## Security Considerations

- All workflows run in isolated GitHub-hosted runners
- Docker containers are ephemeral and cleaned up after each run
- No persistent data or secrets are stored in containers
- Ryuk container cleanup is disabled in CI for better resource management 