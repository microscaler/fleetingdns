# FleetingDNS Tiltfile
# Development environment for FleetingDNS using Kind cluster with proper manifest structure

# Load Tilt extensions
load('ext://helm_resource', 'helm_resource', 'helm_repo')
load('ext://restart_process', 'docker_build_with_restart')

# Configuration
config.define_string("registry", args=False, usage="Docker registry to use")
config.define_bool("debug", args=False, usage="Enable debug mode")

cfg = config.parse()
registry = cfg.get("registry", "localhost:5001")
debug_mode = cfg.get("debug", False)

# Ensure Kind cluster is running
print("🚀 Starting FleetingDNS development environment...")

# Deploy using Kustomize overlays
print("📦 Deploying with Kustomize overlays...")

# Use local cluster overlay for Kind development
k8s_yaml(kustomize('k8s-tilt/clusters/workload/alocal'))

# Build and deploy FleetingDNS services
print("🔧 Building and deploying FleetingDNS services...")

# Common build configuration
rust_build_args = [
    '--target-dir', '/tmp/target',  # Use shared target dir for faster builds
]

if debug_mode:
    rust_build_args.extend(['--features', 'debug'])

# DNS Server (dnsd)
docker_build(
    'fleetingdns/dnsd:dev',
    '.',
    dockerfile='docker/Dockerfile.dnsd',
    only=[
        'Cargo.toml',
        'Cargo.lock',
        'rust-toolchain.toml',
        'cmd/dnsd-bin/',
        'crates/dnsd/',
        'crates/common/',
        'crates/dnsd_backend/',
    ],
    live_update=[
        sync('cmd/dnsd-bin/src', '/usr/src/app/cmd/dnsd-bin/src'),
        sync('crates/dnsd/src', '/usr/src/app/crates/dnsd/src'),
        sync('crates/common/src', '/usr/src/app/crates/common/src'),
        sync('crates/dnsd_backend/src', '/usr/src/app/crates/dnsd_backend/src'),
        run('cd /usr/src/app && cargo build --release -p dnsd-bin', trigger=['cmd/dnsd-bin/src', 'crates/dnsd/src', 'crates/common/src']),
        run('cp /usr/src/app/target/release/dnsd-bin /app/', trigger=['cmd/dnsd-bin/src', 'crates/dnsd/src']),
    ],
)

# EdgeHub
docker_build(
    'fleetingdns/edgehub:dev',
    '.',
    dockerfile='docker/Dockerfile.edgehub',
    only=[
        'Cargo.toml',
        'Cargo.lock',
        'rust-toolchain.toml',
        'cmd/edgehub-bin/',
        'crates/edgehub/',
        'crates/common/',
    ],
    live_update=[
        sync('cmd/edgehub-bin/src', '/usr/src/app/cmd/edgehub-bin/src'),
        sync('crates/edgehub/src', '/usr/src/app/crates/edgehub/src'),
        sync('crates/common/src', '/usr/src/app/crates/common/src'),
        run('cd /usr/src/app && cargo build --release -p edgehub-bin', trigger=['cmd/edgehub-bin/src', 'crates/edgehub/src', 'crates/common/src']),
        run('cp /usr/src/app/target/release/edgehub-bin /app/', trigger=['cmd/edgehub-bin/src', 'crates/edgehub/src']),
    ],
)

# Backend API
docker_build(
    'fleetingdns/api:dev',
    '.',
    dockerfile='docker/Dockerfile.api',
    only=[
        'Cargo.toml',
        'Cargo.lock',
        'rust-toolchain.toml',
        'cmd/api-bin/',
        'crates/backendapi/',
        'crates/auth/',
        'crates/common/',
    ],
    live_update=[
        sync('cmd/api-bin/src', '/usr/src/app/cmd/api-bin/src'),
        sync('crates/backendapi/src', '/usr/src/app/crates/backendapi/src'),
        sync('crates/auth/src', '/usr/src/app/crates/auth/src'),
        sync('crates/common/src', '/usr/src/app/crates/common/src'),
        run('cd /usr/src/app && cargo build --release -p api-bin', trigger=['cmd/api-bin/src', 'crates/backendapi/src', 'crates/auth/src', 'crates/common/src']),
        run('cp /usr/src/app/target/release/api-bin /app/', trigger=['cmd/api-bin/src', 'crates/backendapi/src']),
    ],
)

# Configure resource dependencies and port forwards
k8s_resource('dnsd', resource_deps=['redis', 'otel-collector'])
k8s_resource('edgehub', port_forwards='2222:2222', resource_deps=['redis', 'dnsd', 'otel-collector'])
k8s_resource('api', port_forwards='8080:8080', resource_deps=['postgres', 'redis', 'otel-collector'])

# Infrastructure resources
k8s_resource('redis', port_forwards='6379:6379')
k8s_resource('postgres', port_forwards='5432:5432')

# Observability resources
k8s_resource('otel-collector', port_forwards=['4317:4317', '4318:4318'])
k8s_resource('prometheus', port_forwards='9090:9090', resource_deps=['otel-collector'])
k8s_resource('loki', port_forwards='3100:3100')
k8s_resource('mimir', resource_deps=['prometheus'])
k8s_resource('grafana', port_forwards='3000:3000', resource_deps=['prometheus', 'loki', 'mimir'])

# Group resources for better organization
k8s_resource(
    new_name='fleetingdns-core',
    objects=['dnsd', 'edgehub', 'api'],
    resource_deps=['infrastructure', 'observability']
)

k8s_resource(
    new_name='infrastructure',
    objects=['redis', 'postgres'],
)

k8s_resource(
    new_name='observability',
    objects=['otel-collector', 'prometheus', 'loki', 'mimir', 'grafana'],
    resource_deps=['infrastructure']
)

# Development helpers
print("🎯 Development environment ready!")
print("")
print("🌐 Access points:")
print("  • EdgeHub: localhost:2222 (TCP)")
print("  • Backend API: localhost:8080 (HTTP)")
print("  • Grafana: localhost:3000 (HTTP)")
print("  • Prometheus: localhost:9090 (HTTP)")
print("  • Loki: localhost:3100 (HTTP)")
print("  • Redis: localhost:6379 (TCP)")
print("  • PostgreSQL: localhost:5432 (TCP)")
print("  • OTEL Collector: localhost:4317 (gRPC), localhost:4318 (HTTP)")
print("")
print("🔧 Useful commands:")
print("  • tilt up - Start development environment")
print("  • tilt down - Stop development environment")
print("  • tilt logs <service> - View service logs")
print("  • kubectl get pods -n fleetingdns - Check pod status")
print("")

# Enable experimental features for better development experience
experimental_analytics_report({
    'tilt.analytics.enabled': False  # Disable analytics for privacy
})

# Set up file watching for faster rebuilds
update_settings(max_parallel_updates=3, k8s_upsert_timeout_secs=60) 