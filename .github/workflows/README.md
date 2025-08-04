# FleetingDNS CI/CD Workflows

This directory contains the GitHub Actions workflows for FleetingDNS CI/CD pipeline.

## Workflows

### `fleetingdns-ci.yml` - Main CI Pipeline

The main consolidated CI workflow that runs all tests and validations:

#### Jobs:

1. **Rust Unit Tests & Code Quality** (`rust-tests`)
   - Format checking with `cargo fmt`
   - Linting with `cargo clippy`
   - Unit tests with `cargo test`
   - DoT (DNS-over-TLS) feature tests
   - Binary smoke tests

2. **Integration Tests** (`integration-tests`)
   - Testcontainers-based integration tests
   - Redis cache integration tests
   - Slot-setter tests
   - Docker container management

3. **DNS Integration Testing** (`dns-integration`)
   - Full Docker Compose stack testing
   - Custom DNS client testing
   - DNS integration script execution
   - Grafana and Prometheus health checks
   - Metrics validation
   - Test report artifact upload

4. **Docker Compose Smoke Tests** (`compose-smoke`)
   - Docker Compose CI overlay testing
   - Service health verification
   - Round-trip demo execution

#### Triggers:
- Push to `main` branch
- Pull requests
- Manual workflow dispatch
- Daily scheduled run at 2 AM UTC

#### Dependencies:
- Jobs 3 and 4 depend on Jobs 1 and 2 completing successfully
- Ensures unit tests pass before running integration tests

#### Artifacts:
- DNS test reports uploaded as artifacts
- JUnit XML and JSON test reports available for download

## Local Testing

To run the same tests locally:

```bash
# Unit tests
cargo test --workspace

# Integration tests with testcontainers
cargo test --workspace
env TESTCONTAINERS_RYUK_DISABLED=true RUST_TEST_THREADS=1

# DNS integration tests
./scripts/test_dns_ci.sh

# Custom DNS client tests
python3 scripts/dns_test_client.py test

# Docker Compose smoke tests
docker compose up -d --build
# ... run tests ...
docker compose down
```

## Environment Variables

### Testcontainers Configuration
- `TESTCONTAINERS_RYUK_DISABLED=true` - Disable Ryuk container for CI
- `TESTCONTAINERS_COMMAND_TIMEOUT=180` - Increase timeout for container operations
- `RUST_TEST_THREADS=1` - Run tests serially to avoid Docker conflicts
- `RUST_TEST_TIME_UNIT=180s` - Increase test timeout for container startup

### Docker Configuration
- `DOCKER_HOST=unix:///var/run/docker.sock` - Use local Docker socket
- `TESTCONTAINERS_LOG_LEVEL=OFF` - Reduce log noise in CI

## Troubleshooting

### Common Issues

1. **Docker Resource Conflicts**
   - Tests run serially (`RUST_TEST_THREADS=1`)
   - Container cleanup happens after each job
   - Use `docker container prune -f` for cleanup

2. **DNS Service Not Ready**
   - Wait loops with health checks
   - Service startup delays built in
   - Check service logs for startup issues

3. **Test Timeouts**
   - Increased timeouts for container operations
   - Graceful degradation for non-critical tests
   - Artifact upload even on failure

### Debugging

To debug workflow issues:

1. Check job dependencies and execution order
2. Review service logs in Docker Compose
3. Download test report artifacts
4. Run failing tests locally with same environment

## Workflow Optimization

The consolidated workflow provides:

- **Single Source of Truth**: All CI logic in one place
- **Efficient Resource Usage**: Parallel jobs where possible, dependencies where needed
- **Comprehensive Coverage**: Unit, integration, and end-to-end testing
- **Artifact Management**: Test reports and logs preserved
- **Graceful Degradation**: Non-blocking tests for non-critical components

## Migration from Old Workflows

This workflow consolidates the functionality from:
- `rust_ci.yml` - Rust unit tests and code quality
- `testcontainers.yml` - Integration tests with containers
- `compose-ci.yml` - Docker Compose smoke tests
- `dns-integration.yml` - DNS-specific integration testing

All functionality is now available in the single `fleetingdns-ci.yml` workflow. 