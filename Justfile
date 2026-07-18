# FleetingDNS Development Environment
# ===================================
#
# Default: shared-k8s on ms02 (systemd tilt-fleetingdns.service, UI :10654).
# Legacy Kind: TILT_K8S_CLUSTER=kind on ms02, or `just up-kind` from Mac.
#
# From Mac: `just up` SSHes to ms02 with UI forward to localhost:10654.

MS02_HOST        := env_var_or_default("MS02_HOST", "ms02")
MS02_SSH_USER    := env_var_or_default("MS02_SSH_USER", "casibbald")
MS02_REPO_PATH   := env_var_or_default("MS02_REPO_PATH", "/home/casibbald/Workspace/microscaler/fleetingdns")
shared_k8s_root  := "../shared-k8s-cluster"
shared_k8s_kubeconfig := shared_k8s_root + "/kubeconfig/shared-k8s.yaml"
KIND_CONTEXT     := env_var_or_default("KIND_CONTEXT", "kind-kind")
TILT_K8S_CLUSTER := env_var_or_default("TILT_K8S_CLUSTER", "shared-k8s")
# 10654: clear of the systemd tilt fleet on ms02 (10348-10353, 10450 taken)
TILT_UI_PORT     := env_var_or_default("TILT_UI_PORT", "10654")
KUBECONFIG_PATH  := env_var_or_default("KUBECONFIG_PATH", justfile_directory() + "/.kube/fleetingdns.kubeconfig")

# kubectl-mode recipes inherit this so KUBECONFIG stays repo-scoped
# and never clobbers the developer's personal ~/.kube/config.
export KUBECONFIG := KUBECONFIG_PATH

# Hostname of the machine running `just` — used to decide whether to SSH
# to ms02 or run tilt directly (same Justfile works in both contexts).
HOST := `hostname -s 2>/dev/null || hostname`

# Show available commands
default:
    @echo "FleetingDNS Development Environment (Tilt on {{MS02_HOST}})"
    @echo "=========================================================="
    @echo ""
    @echo "🚀 Quick Start:"
    @echo "  just up               # ssh/tilt up on ms02 (UI on localhost:{{TILT_UI_PORT}})"
    @echo "  just down             # tilt down on ms02 + close ssh session"
    @echo "  just remote-status    # kind/kubectl/tilt status on ms02"
    @echo ""
    @echo "🔗 Optional kubectl-from-Mac:"
    @echo "  just kubeconfig-sync      # Pull ms02 kubeconfig"
    @echo "  just kubectl-tunnel-up    # Open apiserver+registry SSH forwards"
    @echo "  just kubectl-tunnel-down  # Close those forwards"
    @echo ""
    @echo "📋 All commands:"
    @just --list

# Enable the versioned git hooks (.githooks/pre-commit runs fmt + clippy + tests).
# Run once per clone.
setup-hooks:
    git config core.hooksPath .githooks
    @echo "✓ git hooks enabled (.githooks). Pre-commit runs fmt + clippy + fast tests."

# -----------------------------------------------------------------------
# Primary workflow: Tilt runs on ms02
# -----------------------------------------------------------------------

# NOTE: there is no sync step. The repo lives only on ms02; the Mac sees
# it over NFS, so edits made on the Mac are already on ms02's disk.

# Start Tilt. On ms02: shared-k8s via systemd (default) or Kind foreground.
up:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{HOST}}" = "{{MS02_HOST}}" ]; then
        if [ "{{TILT_K8S_CLUSTER}}" = "kind" ] || [ "{{TILT_K8S_CLUSTER}}" = "kind-kind" ]; then
            echo "🔧 Kind tilt up (UI :{{TILT_UI_PORT}})"
            tilt up --context {{KIND_CONTEXT}} --host 0.0.0.0 --port {{TILT_UI_PORT}}
        else
            echo "🚀 shared-k8s via systemd (tilt-fleetingdns.service :{{TILT_UI_PORT}})"
            export KUBECONFIG="$(realpath {{shared_k8s_kubeconfig}} 2>/dev/null || echo "${HOME}/Workspace/microscaler/shared-k8s-cluster/kubeconfig/shared-k8s.yaml")"
            (cd "{{shared_k8s_root}}" && just check-ready) || exit 1
            if ! kubectl get svc -n data minio >/dev/null 2>&1; then
                (cd "{{shared_k8s_root}}" && just systemd-tilt-up) || true
            fi
            (cd "{{shared_k8s_root}}" && just registry-configure-host) 2>/dev/null || true
            systemctl --user start tilt-fleetingdns.service
            echo "Tilt UI: http://0.0.0.0:{{TILT_UI_PORT}}/"
        fi
    else
        echo "🚀 tilt up on {{MS02_HOST}} (UI forwarded to http://localhost:{{TILT_UI_PORT}})"
        python3 hack/kubeconfig_sync.py remote-tilt-up
    fi

# Legacy Kind-only start on ms02
up-kind:
    TILT_K8S_CLUSTER=kind just up

# Stop Tilt. From the Mac this `tilt down`s on ms02 and closes the SSH
# session. On ms02 it just runs `tilt down` locally.
down:
    @if [ "{{HOST}}" = "{{MS02_HOST}}" ]; then \
        echo "🛑 tilt down (local)" ; \
        tilt down --context {{KIND_CONTEXT}} || true ; \
    else \
        echo "🛑 tilt down on {{MS02_HOST}}" ; \
        python3 hack/kubeconfig_sync.py remote-tilt-down ; \
    fi

# Tear everything down (tilt + optional kubectl tunnel).
clean: down kubectl-tunnel-down
    @echo "🧹 session cleaned up."

# End-to-end: tilt up + open UI.
dev: up
    @echo "🎉 Tilt is running on {{MS02_HOST}} — UI: http://localhost:{{TILT_UI_PORT}}"
    @just urls

# Reset (does NOT delete the remote kind cluster).
reset: clean up
    @echo "🔄 environment reset."

# Run an arbitrary command on ms02 inside the repo.
#   just remote-exec "kubectl get pods -A"
remote-exec cmd:
    python3 hack/kubeconfig_sync.py remote-exec {{cmd}}

# Report kind/kubectl/tilt status on ms02.
remote-status:
    python3 hack/kubeconfig_sync.py remote-status

# -----------------------------------------------------------------------
# Optional: run kubectl from the Mac (apiserver + registry SSH forwards)
# -----------------------------------------------------------------------

# Fetch ms02's kubeconfig into {{KUBECONFIG_PATH}}.
kubeconfig-sync:
    python3 hack/kubeconfig_sync.py fetch

# Open SSH forwards for apiserver (:38839) and kind-registry (:5001).
kubectl-tunnel-up:
    python3 hack/kubeconfig_sync.py tunnel-up

# Close those SSH forwards.
kubectl-tunnel-down:
    python3 hack/kubeconfig_sync.py tunnel-down

# Health of tilt session + optional kubectl tunnel.
cluster-status:
    python3 hack/kubeconfig_sync.py status

# -----------------------------------------------------------------------
# Day-to-day helpers
# -----------------------------------------------------------------------

# Show logs from Tilt (runs on ms02).
logs:
    just remote-exec "tilt logs --follow || tilt logs"

# Logs for a specific tilt resource.
logs-service service:
    just remote-exec "tilt logs {{service}}"

# Status summary of pods/services (runs on ms02).
status:
    just remote-status

# Restart a specific tilt resource.
restart service:
    just remote-exec "tilt trigger {{service}}"

# -----------------------------------------------------------------------
# Testing / build (local to whatever host you run `just` on)
# -----------------------------------------------------------------------

test:
    @echo "🧪 Running tests..."
    cargo test --workspace

test-unit:
    @echo "🧪 Running unit tests..."
    cargo test --lib

test-e2e:
    @echo "🧪 Running e2e tests..."
    cargo test --workspace --features e2e

test-coverage:
    @echo "🧪 Running tests with coverage..."
    cargo test --workspace
    farm coverage python

# Reverse-tunnel reproduction test (debug mode) — see
# docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md.
test-reverse-tunnel-repro:
    @echo "🧪 Running reverse-tunnel reproduction harness..."
    DEBUG_RUN_ID=post-build cargo test -p edgehub --test debug_reverse_tunnel -- --nocapture

build:
    @echo "🔧 Building all services..."
    cargo build --release

build-service service:
    @echo "🔧 Building {{service}}..."
    cargo build --release -p {{service}}-bin

# -----------------------------------------------------------------------
# kubectl helpers (only useful if kubectl-tunnel-up has been run)
# -----------------------------------------------------------------------

shell service:
    @echo "🐚 Opening shell in {{service}}..."
    kubectl --context {{KIND_CONTEXT}} exec -it -n fleetingdns deployment/{{service}} -- /bin/sh

port-forward service:
    @echo "🔌 Port forwarding to {{service}}..."
    kubectl --context {{KIND_CONTEXT}} port-forward -n fleetingdns svc/{{service}} 8880:8880

describe service:
    @echo "📋 Describing {{service}}..."
    kubectl --context {{KIND_CONTEXT}} describe -n fleetingdns deployment/{{service}}

# -----------------------------------------------------------------------
# Observability (served via ms02 NodePorts on kind's host-port bindings)
# -----------------------------------------------------------------------

grafana:
    @echo "📊 Opening Grafana..."
    open http://{{MS02_HOST}}:3000

prometheus:
    @echo "📈 Opening Prometheus..."
    open http://{{MS02_HOST}}:9090

tilt-ui:
    @echo "🎛️  Opening Tilt UI..."
    open http://localhost:{{TILT_UI_PORT}}

# -----------------------------------------------------------------------
# Utilities
# -----------------------------------------------------------------------

check-deps:
    @echo "🔍 Checking dependencies..."
    @echo -n "ssh:     "; ssh -V >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"
    @echo -n "python3: "; python3 --version >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"
    @echo -n "kubectl: "; kubectl version --client >/dev/null 2>&1 && echo "✅ installed (optional)" || echo "(missing — optional)"
    @echo -n "tilt:    "; tilt version >/dev/null 2>&1 && echo "✅ installed (optional)" || echo "(missing — runs remotely on {{MS02_HOST}})"

cluster-info:
    just remote-exec "kubectl cluster-info --context {{KIND_CONTEXT}}; echo; echo Nodes:; kubectl --context {{KIND_CONTEXT}} get nodes; echo; echo Namespaces:; kubectl --context {{KIND_CONTEXT}} get namespaces"

registry-info:
    @echo "🐳 kind-registry on {{MS02_HOST}} (reachable from ms02 as kind-registry:5000):"
    just remote-exec "curl -s http://localhost:5001/v2/_catalog | (jq . 2>/dev/null || cat)"

ci-test:
    @echo "🤖 Running CI tests..."
    cargo fmt -- --check
    cargo clippy --all -- -D warnings
    cargo test --workspace

docs:
    @echo "📚 Generating documentation..."
    cargo doc --open

docs-serve:
    @echo "📚 Serving documentation..."
    cargo doc --no-deps
    @echo "Documentation available at: file://$(pwd)/target/doc/fleetingdns/index.html"

urls:
    @echo "🌐 Service URLs (ms02 NodePorts via hack/kind/kind-config.yaml):"
    @echo "======================================================"
    @echo "Tilt UI:                    http://localhost:{{TILT_UI_PORT}}   (ssh -L to {{MS02_HOST}})"
    @echo "Backend API:                http://{{MS02_HOST}}:8880"
    @echo "Grafana:                    http://{{MS02_HOST}}:3000"
    @echo "Prometheus:                 http://{{MS02_HOST}}:9090"
    @echo "Loki:                       http://{{MS02_HOST}}:3100"
    @echo "Redis:                      {{MS02_HOST}}:6379"
    @echo "PostgreSQL:                 {{MS02_HOST}}:5433"
    @echo ""
    @echo "🔧 Optional (only if `just kubectl-tunnel-up` is running):"
    @echo "kind registry (tunneled):   http://localhost:5001"
    @echo "kube-apiserver (tunneled):  https://127.0.0.1:38839"

health:
    just remote-exec "kubectl --context {{KIND_CONTEXT}} get nodes; echo; kubectl --context {{KIND_CONTEXT}} get pods -n fleetingdns || echo 'namespace not yet created'"

nextest-test:
    cargo nextest run --workspace --all-targets --fail-fast --retries 1 --exclude migration

alias nt := nextest-test

# -----------------------------------------------------------------------
# Legacy laptop-local Kind recipes (DEPRECATED — no standalone clusters)
# -----------------------------------------------------------------------
# DEPRECATED: Standalone clusters are no longer supported.
# All development uses the shared Kind cluster on ms02 via `just up`.
# The recipes below are kept only for offline reference — they are NOT
# for production use and will be removed in a future version.
#
# To develop: `just up` (ssh to ms02, runs Tilt there on shared cluster)
# To stop:    `just down` (tilt down on ms02)

local-kind-up:
    @echo "❌ DEPRECATED: Standalone Kind clusters are no longer supported."
    @echo "   All development uses the shared Kind cluster on ms02."
    @echo "   Use: just up  (ssh to ms02, starts Tilt on shared cluster)"
    @exit 1

local-kind-down:
    @echo "❌ DEPRECATED: Standalone Kind clusters are no longer supported."
    @echo "   Use: just down  (stops Tilt on ms02)"
    @exit 1

local-setup-kind: local-kind-up
    @echo "❌ DEPRECATED: Use just up instead."
    @exit 1

local-clean:
    @echo "❌ DEPRECATED: Standalone Kind clusters are no longer supported."
    @echo "   Use: just clean  (stops Tilt on ms02, closes tunnels)"
    @exit 1
