# FleetingDNS Tiltfile
# ---------------------
# Runs on ms02 against shared-k8s (default) or legacy Kind (TILT_K8S_CLUSTER=kind).
# systemd: tilt-fleetingdns.service (port 10654) — installed from shared-k8s-cluster.
#
# Shared platform (postgres, redis, otel, observability) lives in shared-k8s-cluster
# namespace `data` / `observability`. FleetingDNS deploys only its core services.

# Load Tilt extensions
load('ext://helm_resource', 'helm_resource', 'helm_repo')
load('ext://restart_process', 'docker_build_with_restart')

# Configuration
config.define_string("registry", args=False, usage="Docker registry to use")
config.define_bool("debug", args=False, usage="Enable debug mode")

cfg = config.parse()

# Shared platform cluster: shared-k8s (default) or legacy Kind.
_SHARED_K8S_KCFG = os.path.abspath('../shared-k8s-cluster/kubeconfig/shared-k8s.yaml')
_SHARED_K8S_REGISTRY = '10.177.76.220:5000'
_k8s_mode = os.environ.get('TILT_K8S_CLUSTER', '').strip().lower()
if _k8s_mode in ('kind', 'kind-kind'):
    _use_shared_k8s = False
elif _k8s_mode in ('shared-k8s', 'k3s'):
    _use_shared_k8s = True
else:
    _use_shared_k8s = os.path.exists(_SHARED_K8S_KCFG)

if _use_shared_k8s and os.path.exists(_SHARED_K8S_KCFG):
    allow_k8s_contexts(['shared-k8s'])
    os.putenv('KUBECONFIG', _SHARED_K8S_KCFG)
    registry = cfg.get("registry", _SHARED_K8S_REGISTRY)
    default_registry(registry)
    print("🚀 FleetingDNS on shared-k8s (registry %s)" % registry)
else:
    allow_k8s_contexts('kind-kind')
    registry = cfg.get("registry", "localhost:5001")
    default_registry(registry)
    print("🚀 FleetingDNS on Kind (registry %s)" % registry)

debug_mode = cfg.get("debug", False)

# Deploy using Kustomize overlays
print("📦 Deploying with Kustomize overlays...")

# Use local cluster overlay for Kind development
k8s_yaml(kustomize('k8s-tilt/clusters/workload/alocal'))

# Shared infrastructure comes from shared-k8s-cluster (or legacy shared-kind-cluster):
#   • redis           → redis.data.svc.cluster.local:6379
#   • postgres        → postgres.data.svc.cluster.local:5432
#   • otel-collector  → otel-collector.observability.svc.cluster.local:4317
# FleetingDNS deploys ONLY its core services; the alocal overlay patches
# their env to point at the shared services.
#
# One-shot job: create the `fdns` database in the shared postgres if it
# doesn't exist yet (CREATE DATABASE has no IF NOT EXISTS, hence \\gexec).
# The Job runs in the `data` namespace, alongside postgres and its credentials.
# secretKeyRef is namespace-local, so a Job in `fleetingdns` can NOT read a
# Secret in `data` — this matches the platform convention already used by
# hauliage-db-init / sesame-idam-db-init / rerp-accounting-db-init.
#
# The admin password is read on the fly from the shared `postgres-credentials`
# Secret (key `postgres-password`) — never hardcoded, and deliberately NOT
# optional: if the Secret or key is missing the pod must fail loudly rather
# than start with an empty password and report a confusing auth failure.
# See docs/engineering/SECURITY-DEPENDENCIES-AND-SECRETS.md
k8s_yaml(blob("""
apiVersion: batch/v1
kind: Job
metadata:
  name: fdns-db-init
  namespace: data
spec:
  backoffLimit: 6
  template:
    spec:
      restartPolicy: OnFailure
      containers:
        - name: createdb
          image: postgres:16-alpine
          env:
            # Primary endpoint in the `data` namespace. (Note: older jobs in
            # this cluster reference a `postgres-ha-pgpool` host — that
            # topology no longer exists; there is no pgpool service.)
            - {name: PGHOST, value: postgres.data.svc.cluster.local}
            # Superuser that `postgres-password` belongs to.
            - {name: PGUSER, value: postgres}
            - name: PGPASSWORD
              valueFrom:
                secretKeyRef:
                  name: postgres-credentials
                  key: postgres-password
          command:
            - /bin/sh
            - -c
            - |
              echo "SELECT 'CREATE DATABASE fdns' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'fdns')\\\\gexec" | psql -v ON_ERROR_STOP=1
"""))

# Build and deploy FleetingDNS services
print("🔧 Building and deploying FleetingDNS services...")

# Common build configuration
rust_build_args = [
    '--target-dir', '/tmp/target',  # Use shared target dir for faster builds
]

if debug_mode:
    rust_build_args.extend(['--features', 'debug'])

# The cargo workspace declares members crates/*, cmd/* and
# tests/integration, so every build context must contain ALL of them —
# a narrower `only` list makes `cargo build` fail with "failed to load
# manifest for workspace member".
rust_workspace_context = [
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    'nextest.toml',
    '.nextest.toml',
    'crates/',
    'cmd/',
    'tests/',
]

# NOTE: no live_update on the Rust services. The runtime images are
# debian-slim running as non-root with no cargo toolchain and no
# /usr/src/app, so syncing source + `cargo build` inside the running
# container can never work (tar: Permission denied → UpdateFailed →
# stale binary keeps serving). Full docker rebuilds are deterministic.

# DNS Server (dnsd)
docker_build(
    'fleetingdns/dnsd:dev',
    '.',
    dockerfile='docker/Dockerfile.dnsd',
    only=rust_workspace_context,
)

# EdgeHub
docker_build(
    'fleetingdns/edgehub:dev',
    '.',
    dockerfile='docker/Dockerfile.edgehub',
    only=rust_workspace_context,
)

# Backend API
docker_build(
    'fleetingdns/api:dev',
    '.',
    dockerfile='docker/Dockerfile.api',
    only=rust_workspace_context,
)

# Configure resource dependencies and port forwards.
# redis/postgres/otel-collector are NOT Tilt resources here — they belong
# to the shared-kind-cluster stack (data + observability namespaces).
# Host-side ports use a 1xxxx prefix where the natural port is already
# held by another Tilt environment on ms02 (see header comment).
k8s_resource('fdns-db-init', labels=['infra'])
k8s_resource('dnsd', labels=['core'])
k8s_resource('edgehub', port_forwards='2222:2222', resource_deps=['dnsd'], labels=['core'])
# api serves on 8880 (8080 is chronically contested on shared dev hosts).
k8s_resource('api', port_forwards='8880:8880', resource_deps=['fdns-db-init'], labels=['core'])

# ---------------------------------------------------------------------------
# Developer / CI checks (local_resource)
# ---------------------------------------------------------------------------
# Host-side cargo tasks (build, lint, test) runnable straight from Tilt and the
# Tilt MCP — no separate shell needed. All are MANUAL and do NOT run at
# `tilt up` (auto_init=False); trigger them on demand. A dedicated local target
# dir keeps host builds on fast local disk (off NFS) and separate from the
# in-container docker target at /tmp/target.
_cargo_env = 'CARGO_TERM_COLOR=never CARGO_TARGET_DIR=/tmp/fleetingdns-host-target'
_rust_deps = ['Cargo.toml', 'Cargo.lock', 'crates', 'cmd', 'tests']

def cargo_check(name, cmd, deps=_rust_deps):
    local_resource(
        name,
        '%s %s' % (_cargo_env, cmd),
        deps=deps,
        trigger_mode=TRIGGER_MODE_MANUAL,
        auto_init=False,
        # Run independently of the (slow) docker image builds — otherwise a
        # check sits `pending` behind a multi-minute in-container Rust build.
        # Concurrent cargo invocations sharing the target dir are safe: cargo
        # takes its own file lock and simply waits for it.
        allow_parallel=True,
        labels=['checks'],
    )

# Whole-workspace gates (mirror CI).
cargo_check('cargo-fmt',    'cargo fmt --all -- --check')
cargo_check('cargo-clippy', 'cargo clippy --workspace --all-targets --all-features -- -D warnings')
cargo_check('cargo-build',  'cargo build --workspace --all-features')
cargo_check('cargo-test',   'cargo test --workspace --all-features')

# Focused, fast feedback loop for crates under active work.
cargo_check('cargo-clippy-cert', 'cargo clippy -p edf-ca -p edgehub --all-targets -- -D warnings')
cargo_check('cargo-test-cert',   'cargo test -p edf-ca -p edgehub')

# ---------------------------------------------------------------------------
# Ops: self-management
# ---------------------------------------------------------------------------
# Restart this Tilt instance from inside Tilt (e.g. to pick up systemd unit or
# environment changes; plain Tiltfile edits are hot-reloaded automatically).
#
# `--no-block` is essential: systemd is about to SIGTERM the very process
# running this command, so we must return immediately and let systemd carry out
# the restart asynchronously. Tilt drops its connection briefly and comes back
# on the same port (10654). The UI will show this resource's run as interrupted
# — that is expected, not a failure.
local_resource(
    'tilt-restart-self',
    'systemctl --user restart --no-block tilt-fleetingdns.service',
    trigger_mode=TRIGGER_MODE_MANUAL,
    auto_init=False,
    labels=['ops'],
)

# Development helpers
print("🎯 Development environment ready!")
print("")
print("🌐 Access points (host = ms02; 1xxxx ports avoid other tilt stacks):")
print("  • EdgeHub: localhost:2222 (TCP)")
print("  • Backend API: localhost:8880 (HTTP)")
print("")
print("🧩 Shared services: data/postgres, data/redis, observability/* (shared-k8s platform Tilt :10349)")
print("")
print("🔧 Useful commands:")
print("  • tilt up - Start development environment")
print("  • tilt down - Stop development environment")
print("  • tilt logs <service> - View service logs")
print("  • kubectl get pods -n fleetingdns - Check pod status")
print("")

# Disable analytics for privacy
analytics_settings(False)

# Set up file watching for faster rebuilds
update_settings(max_parallel_updates=3, k8s_upsert_timeout_secs=60) 