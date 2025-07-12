# FleetingDNS Kind + Tilt Development Environment

This document describes the new Kubernetes-based development environment for FleetingDNS using Kind (Kubernetes in Docker) and Tilt for local development.

## 🎯 Why Kind + Tilt?

**Benefits over Docker Compose:**
- **Production Parity**: Kubernetes environment matches production deployment
- **Better Resource Management**: Proper CPU/memory limits and requests
- **Service Discovery**: Native Kubernetes service discovery and networking
- **Observability**: Built-in metrics, logging, and tracing integration
- **Live Reload**: Tilt provides fast incremental builds and hot reloading
- **Dependency Management**: Explicit service dependencies and health checks
- **Scalability Testing**: Easy to test scaling scenarios locally

## 📋 Prerequisites

### Required Tools

1. **Docker** (v20.10+)
   ```bash
   # macOS
   brew install docker
   # Or download from https://docs.docker.com/get-docker/
   ```

2. **Kind** (v0.20+)
   ```bash
   # macOS
   brew install kind
   # Linux
   curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.20.0/kind-linux-amd64
   chmod +x ./kind && sudo mv ./kind /usr/local/bin/kind
   ```

3. **kubectl** (v1.28+)
   ```bash
   # macOS
   brew install kubectl
   # Linux
   curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
   chmod +x kubectl && sudo mv kubectl /usr/local/bin/
   ```

4. **Tilt** (v0.33+)
   ```bash
   # macOS
   brew install tilt
   # Linux
   curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash
   ```

### System Requirements

- **Memory**: 8GB+ RAM (16GB recommended)
- **CPU**: 4+ cores
- **Disk**: 10GB+ free space
- **Ports**: 2222, 3000, 4317, 4318, 5353, 6379, 8080, 9090

## 🚀 Quick Start

### 1. Setup Kind Cluster

Run the automated setup script:

```bash
python3 scripts/kind-setup.py
```

This script will:
- ✅ Check all dependencies are installed
- 🐳 Start a local Docker registry on `localhost:5001`
- 🎯 Create a Kind cluster named `fleetingdns-dev`
- 🔗 Connect the registry to the cluster
- 🔍 Verify everything is working

### 2. Start Development Environment

```bash
# Start all services
tilt up

# Open Tilt UI (optional)
open http://localhost:10350
```

### 3. Access Services

Once Tilt shows all services as "Ready":

| Service | URL | Description |
|---------|-----|-------------|
| **DNS Server** | `localhost:5353` (UDP) | DNS queries and tunnel registration |
| **EdgeHub** | `localhost:2222` (TCP) | Tunnel server for client connections |
| **Backend API** | `localhost:8080` (HTTP) | REST API for management |
| **Grafana** | `localhost:3000` (HTTP) | Observability dashboard |
| **Prometheus** | `localhost:9090` (HTTP) | Metrics collection |
| **Redis** | `localhost:6379` (TCP) | Cache and session storage |
| **PostgreSQL** | `localhost:5432` (TCP) | Primary database |

## 📁 Project Structure

```
fleetingdns/
├── kind-config.yaml              # Kind cluster configuration
├── Tiltfile                      # Tilt development configuration
├── k8s/                         # Kubernetes manifests
│   ├── namespace.yaml           # FleetingDNS namespace
│   ├── configmaps/              # Configuration files
│   │   └── observability-configs.yaml
│   ├── infrastructure/          # Infrastructure services
│   │   ├── redis.yaml
│   │   ├── postgres.yaml
│   │   └── observability.yaml
│   └── services/                # FleetingDNS services
│       ├── dnsd.yaml
│       ├── edgehub.yaml
│       ├── api.yaml
│       └── other-services.yaml
├── scripts/
│   └── kind-setup.py           # Automated setup script
└── docker/                     # Dockerfiles (unchanged)
    ├── Dockerfile.dnsd
    ├── Dockerfile.edgehub
    └── ...
```

## 🔧 Development Workflow

### Daily Development

```bash
# Start development environment
tilt up

# View logs for specific service
tilt logs dnsd

# Restart a service
tilt trigger dnsd

# Stop everything
tilt down
```

### Code Changes

Tilt automatically detects changes and rebuilds/redeploys:

1. **Fast Path**: Source code changes trigger incremental builds
2. **Dependency Changes**: `Cargo.toml` changes trigger full rebuilds
3. **Config Changes**: Kubernetes manifest changes trigger redeployments

### Debugging

```bash
# Check pod status
kubectl get pods -n fleetingdns

# Get detailed pod info
kubectl describe pod <pod-name> -n fleetingdns

# Access pod shell
kubectl exec -it <pod-name> -n fleetingdns -- /bin/sh

# View pod logs
kubectl logs <pod-name> -n fleetingdns -f

# Port forward for debugging
kubectl port-forward -n fleetingdns svc/dnsd 8080:8080
```

## 🔄 Migration from Docker Compose

### Key Differences

| Aspect | Docker Compose | Kind + Tilt |
|--------|----------------|-------------|
| **Networking** | Bridge network | Kubernetes ClusterIP + NodePort |
| **Service Discovery** | Container names | Kubernetes DNS (e.g., `redis.fleetingdns.svc.cluster.local`) |
| **Configuration** | Environment variables | ConfigMaps + Environment variables |
| **Storage** | Named volumes | PersistentVolumeClaims |
| **Health Checks** | `healthcheck` | `livenessProbe` + `readinessProbe` |
| **Dependencies** | `depends_on` | `resource_deps` in Tilt |

### Environment Variables

Services now use Kubernetes-style service discovery:

```yaml
# Old (Docker Compose)
REDIS_URL: redis://redis:6379

# New (Kubernetes)
REDIS_URL: redis://redis.fleetingdns.svc.cluster.local:6379
# Or simplified (within same namespace)
REDIS_URL: redis://redis:6379
```

### Port Access

External access now uses NodePort services:

```yaml
# Old (Docker Compose)
ports: ["5353:53/udp"]

# New (Kubernetes)
# ClusterIP for internal communication
# NodePort for external access (30053 -> 53)
```

## 📊 Observability

### Built-in Monitoring

The environment includes a complete observability stack:

- **Metrics**: Prometheus scrapes all services
- **Logs**: Loki collects structured logs
- **Traces**: OpenTelemetry collector receives traces
- **Dashboards**: Grafana visualizes everything

### Accessing Dashboards

1. **Grafana**: http://localhost:3000
   - Username: `admin`
   - Password: `admin`

2. **Prometheus**: http://localhost:9090
   - Query metrics directly
   - View targets and service discovery

### Adding Metrics

Services expose metrics on port 8080 (or 8081 for some services):

```rust
// In your service code
use prometheus::{Counter, register_counter};

let requests_total = register_counter!(
    "requests_total",
    "Total number of requests"
)?;
```

## 🧪 Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run e2e tests (uses the running cluster)
cargo test --features e2e e2e_tunnel_complete_flow
```

### E2E Test Integration

E2E tests now use the Kind cluster directly:

- Tests connect to services via NodePort
- Use graceful shutdown (no more `pkill`!)
- Proper cleanup and resource management

## 🔧 Customization

### Resource Limits

Adjust resource limits in Kubernetes manifests:

```yaml
resources:
  requests:
    memory: "64Mi"
    cpu: "50m"
  limits:
    memory: "256Mi"
    cpu: "200m"
```

### Adding New Services

1. Create Dockerfile in `docker/`
2. Add Kubernetes manifests in `k8s/services/`
3. Update `Tiltfile` with build configuration
4. Add service dependencies

### Development vs Production

Use different configurations for development:

```python
# In Tiltfile
if config.tilt_subcommand == 'up':
    # Development-specific settings
    k8s_yaml('k8s/dev-overrides.yaml')
```

## 🧹 Cleanup

### Stop Development Environment

```bash
# Stop Tilt (keeps cluster running)
tilt down

# Delete Kind cluster
kind delete cluster --name fleetingdns-dev

# Remove local registry
docker stop kind-registry && docker rm kind-registry
```

### Reset Everything

```bash
# Complete cleanup
kind delete cluster --name fleetingdns-dev
docker stop kind-registry && docker rm kind-registry
docker system prune -f
```

## 🔍 Troubleshooting

### Common Issues

1. **Port Conflicts**
   ```bash
   # Check what's using a port
   lsof -i :5353
   
   # Kill process using port
   kill -9 $(lsof -t -i:5353)
   ```

2. **Docker Registry Issues**
   ```bash
   # Restart registry
   docker restart kind-registry
   
   # Check registry contents
   curl http://localhost:5001/v2/_catalog
   ```

3. **Cluster Not Accessible**
   ```bash
   # Check cluster status
   kind get clusters
   
   # Get cluster info
   kubectl cluster-info --context kind-fleetingdns-dev
   ```

4. **Build Failures**
   ```bash
   # Force rebuild
   tilt trigger <service-name>
   
   # Check build logs
   tilt logs <service-name>
   ```

### Performance Tuning

1. **Increase Docker Resources**
   - Docker Desktop: Settings → Resources
   - Minimum: 8GB RAM, 4 CPUs

2. **Optimize Rust Builds**
   ```bash
   # Use faster linker (macOS)
   export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
   
   # Use shared target directory
   export CARGO_TARGET_DIR=/tmp/cargo-target
   ```

3. **Tilt Optimization**
   ```python
   # In Tiltfile
   update_settings(max_parallel_updates=3)
   ```

## 📚 Additional Resources

- [Kind Documentation](https://kind.sigs.k8s.io/)
- [Tilt Documentation](https://docs.tilt.dev/)
- [Kubernetes Documentation](https://kubernetes.io/docs/)
- [FleetingDNS Architecture](./docs/engineering/)

## 🆘 Support

If you encounter issues:

1. Check this troubleshooting guide
2. Review Tilt logs: `tilt logs`
3. Check Kubernetes events: `kubectl get events -n fleetingdns`
4. Ask for help in the team chat with:
   - Error messages
   - Output of `tilt status`
   - Output of `kubectl get pods -n fleetingdns` 