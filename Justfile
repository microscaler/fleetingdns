# FleetingDNS Development Environment
# ===================================

# Show available commands
default:
    @echo "FleetingDNS Development Environment"
    @echo "=================================="
    @echo ""
    @echo "🚀 Quick Start:"
    @echo "  just dev              # Setup and start everything"
    @echo "  just up               # Start development environment"
    @echo "  just down             # Stop development environment"
    @echo ""
    @echo "📋 Available commands:"
    @just --list

# Setup and Environment Management
# ================================

# Setup Kind cluster and local registry
setup:
    @echo "🚀 Setting up FleetingDNS development environment..."
    python3 scripts/kind-setup.py

# Create Kind cluster only
kind-up:
    @echo "🚀 Creating vanilla Kind cluster with Kubernetes 1.32..."
    kind create cluster --image kindest/node:v1.32.0

# Delete Kind cluster
kind-down:
    @echo "🛑 Deleting Kind cluster..."
    kind delete cluster --name kind

# Setup complete Kind environment (cluster + registry + namespace)
setup-kind: kind-up
    @echo "🐳 Setting up local registry..."
    python3 scripts/kind-setup.py --registry-only
    @echo "📦 Creating namespace..."
    kubectl create namespace fleetingdns --dry-run=client -o yaml | kubectl apply -f -

# Start development environment with Tilt
up:
    @echo "🔧 Starting development environment..."
    tilt up

# Stop development environment
down:
    @echo "🛑 Stopping development environment..."
    tilt down

# Clean up everything (cluster, registry, images)
clean:
    @echo "🧹 Cleaning up development environment..."
    -tilt down
    -kind delete cluster --name kind
    -docker stop kind-registry && docker rm kind-registry
    -docker system prune -f

# Setup and start development environment
dev: setup up
    @echo "🎉 Development environment is ready!"
    @echo ""
    @just urls

# Reset everything and start fresh
reset: clean setup up
    @echo "🔄 Environment reset complete!"

# Development Commands
# ====================

# Show logs for all services
logs:
    tilt logs

# Show logs for specific service (e.g., just logs-service dnsd)
logs-service service:
    tilt logs {{service}}

# Show status of all services
status:
    @echo "🔍 Checking service status..."
    @echo ""
    @echo "Tilt Resources:"
    @tilt get resources 2>/dev/null || echo "Tilt not running"
    @echo ""
    @echo "Kubernetes Pods:"
    @kubectl get pods -n fleetingdns 2>/dev/null || echo "Cluster not accessible"
    @echo ""
    @echo "Services:"
    @kubectl get svc -n fleetingdns 2>/dev/null || echo "Cluster not accessible"

# Restart specific service (e.g., just restart dnsd)
restart service:
    @echo "🔄 Restarting {{service}}..."
    tilt trigger {{service}}

# Testing
# =======

# Run all tests
test:
    @echo "🧪 Running tests..."
    cargo test --workspace

# Run unit tests only
test-unit:
    @echo "🧪 Running unit tests..."
    cargo test --lib

# Run e2e tests (requires running cluster)
test-e2e:
    @echo "🧪 Running e2e tests..."
    cargo test --workspace --features e2e

# Run tests with coverage
test-coverage:
    @echo "🧪 Running tests with coverage..."
    cargo test --workspace
    farm coverage python

# Building
# ========

# Build all services
build:
    @echo "🔧 Building all services..."
    cargo build --release

# Build specific service (e.g., just build-service dnsd)
build-service service:
    @echo "🔧 Building {{service}}..."
    cargo build --release -p {{service}}-bin

# Development Helpers
# ===================

# Open shell in running pod (e.g., just shell dnsd)
shell service:
    @echo "🐚 Opening shell in {{service}}..."
    kubectl exec -it -n fleetingdns deployment/{{service}} -- /bin/sh

# Port forward to service (e.g., just port-forward redis)
port-forward service:
    @echo "🔌 Port forwarding to {{service}}..."
    kubectl port-forward -n fleetingdns svc/{{service}} 8080:8080

# Describe Kubernetes resource (e.g., just describe dnsd)
describe service:
    @echo "📋 Describing {{service}}..."
    kubectl describe -n fleetingdns deployment/{{service}}

# Observability
# =============

# Open Grafana dashboard
grafana:
    @echo "📊 Opening Grafana..."
    open http://localhost:3000

# Open Prometheus UI
prometheus:
    @echo "📈 Opening Prometheus..."
    open http://localhost:9090

# Open Tilt UI
tilt-ui:
    @echo "🎛️  Opening Tilt UI..."
    open http://localhost:10350

# Utility Commands
# ================

# Check if all required tools are installed
check-deps:
    @echo "🔍 Checking dependencies..."
    @echo -n "docker: "; docker version >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"
    @echo -n "kind: "; kind version >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"
    @echo -n "kubectl: "; kubectl version --client >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"
    @echo -n "tilt: "; tilt version >/dev/null 2>&1 && echo "✅ installed" || echo "❌ missing"

# Show cluster information
cluster-info:
    @echo "🎯 Cluster Information:"
    @echo "======================"
    @kubectl cluster-info --context kind-kind 2>/dev/null || echo "Cluster not accessible"
    @echo ""
    @echo "Nodes:"
    @kubectl get nodes 2>/dev/null || echo "Cluster not accessible"
    @echo ""
    @echo "Namespaces:"
    @kubectl get namespaces 2>/dev/null || echo "Cluster not accessible"

# Show local registry information
registry-info:
    @echo "🐳 Registry Information:"
    @echo "======================="
    @echo "Registry URL: http://localhost:5001"
    @echo ""
    @echo "Registry contents:"
    @curl -s http://localhost:5001/v2/_catalog 2>/dev/null | jq . || echo "Registry not accessible"

# CI/CD Integration
# =================

# Run tests suitable for CI environment
ci-test:
    @echo "🤖 Running CI tests..."
    cargo fmt -- --check
    cargo clippy --all -- -D warnings
    cargo test --workspace

# Documentation
# =============

# Generate and open documentation
docs:
    @echo "📚 Generating documentation..."
    cargo doc --open

# Serve documentation locally
docs-serve:
    @echo "📚 Serving documentation..."
    cargo doc --no-deps
    @echo "Documentation available at: file://$(pwd)/target/doc/fleetingdns/index.html"

# Show all service URLs
urls:
    @echo "🌐 Service URLs:"
    @echo "==============="
    @echo "DNS Server:     localhost:5354 (UDP)"
    @echo "EdgeHub:        localhost:2222 (TCP)"
    @echo "Backend API:    localhost:8080 (HTTP)"
    @echo "Grafana:        localhost:3000 (HTTP)"
    @echo "Prometheus:     localhost:9090 (HTTP)"
    @echo "Redis:          localhost:6379 (TCP)"
    @echo "PostgreSQL:     localhost:5432 (TCP)"
    @echo "Tilt UI:        localhost:10350 (HTTP)"
    @echo ""
    @echo "🔧 Management:"
    @echo "Registry:       localhost:5001 (HTTP)"

# Health checks
# =============

# Check health of all services
health:
    @echo "🏥 Checking service health..."
    @echo ""
    @echo "Cluster health:"
    @kubectl get nodes 2>/dev/null || echo "❌ Cluster not accessible"
    @echo ""
    @echo "Pod health:"
    @kubectl get pods -n fleetingdns 2>/dev/null || echo "❌ Pods not accessible"
    @echo ""
    @echo "Service health:"
    @for service in dnsd edgehub api redis postgres; do \
        echo -n "$service: "; \
        kubectl get pod -n fleetingdns -l app=$service -o jsonpath='{.items[0].status.phase}' 2>/dev/null || echo "Unknown"; \
    done

# Run nextest for faster test execution
nextest-test:
    cargo nextest run --workspace --all-targets --fail-fast --retries 1

alias nt := nextest-test
