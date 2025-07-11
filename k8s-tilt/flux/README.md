# FleetingDNS Flux GitOps Configuration

This directory contains the Flux v2.6.2 GitOps configuration for FleetingDNS with enterprise-grade multi-cluster support.

## 📁 Directory Structure

```
flux/
├── README.md                          # This documentation
├── kustomization.yaml                 # Main Flux installation + sync
├── versions/
│   └── v2_6_2/
│       └── kustomization.yaml         # Flux v2.6.2 installation
├── sync/
│   ├── kustomization.yaml             # Kustomize config for sync resources
│   ├── gitrepository.yaml             # GitRepository resources (placeholder)
│   └── fluxcd-kustomizations.yaml     # Flux Kustomization resources (placeholder)
└── patches/
    └── examples/
        ├── alocal-patches.yaml         # Example: Local development patches
        └── eu-west-1-prod-patches.yaml # Example: EU production patches
```

## 🚀 Quick Start

### ⚠️ CRITICAL: Flux Installation Rule

**Flux MUST NEVER be installed directly!** 

Flux is automatically installed as the **first component** in **production/staging** cluster kustomizations:

```yaml
# Production/Staging clusters (GitOps managed)
resources:
  # Flux kustomization, this is the only location where flux must be installed into a cluster.
  - flux/  # Local flux directory with cluster-specific patches
  - ../../../../../base/
```

```yaml
# Local development cluster (Tilt managed)
resources:
  # NOTE: alocal uses Tilt for deployment, NOT Flux!
  # Flux is only installed in production/staging clusters via GitOps
  - ../../../base/
```

### 1. Deploy Production/Staging Cluster (includes Flux)

```bash
# Deploy production cluster (Flux + workloads) - CORRECT WAY
kubectl apply -k k8s-tilt/clusters/workload/eu-west-1/staging/auto-pilot/

# Verify Flux is running
kubectl get pods -n flux-system
```

### 2. Deploy Local Development Cluster (uses Tilt)

```bash
# Local development uses Tilt, not kubectl apply
tilt up

# Verify services are running
kubectl get pods -n fleetingdns
```

### 2. Create Cluster-Specific Patches

Copy and customize the example patches for your cluster:

```bash
# For local development
cp k8s-tilt/flux/patches/examples/alocal-patches.yaml \
   k8s-tilt/clusters/workload/alocal/flux-patches.yaml

# For production
cp k8s-tilt/flux/patches/examples/eu-west-1-prod-patches.yaml \
   k8s-tilt/clusters/workload/eu-west-1/prod/auto-pilot/flux-patches.yaml
```

### 3. Apply Cluster-Specific Configuration

```bash
# Create a cluster-specific overlay
cat > k8s-tilt/clusters/workload/alocal/flux-overlay.yaml << EOF
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

resources:
  - ../../../flux/

patchesStrategicMerge:
  - flux-patches.yaml
EOF

# Apply to cluster
kubectl apply -k k8s-tilt/clusters/workload/alocal/flux-overlay/
```

## 🔧 Configuration Details

### GitRepository Resources

Two GitRepository resources are created with placeholder values:

1. **fleetingdns**: Main application repository
   - **Default Branch**: `main`
   - **Interval**: `1m0s` (fast for development)
   - **Timeout**: `60s`

2. **fleetingdns-infra**: Infrastructure/Crossplane repository
   - **Default Branch**: `main`
   - **Interval**: `5m0s` (slower for infrastructure)
   - **Timeout**: `60s`

### Kustomization Resources

Three Flux Kustomization resources manage different aspects:

1. **fleetingdns-base**: Base components (namespace, common resources)
   - **Path**: `./k8s-tilt/base`
   - **Interval**: `5m0s`
   - **No dependencies**

2. **fleetingdns-infra**: Infrastructure resources (Crossplane)
   - **Path**: `./k8s-tilt/crossplane`
   - **Interval**: `10m0s`
   - **No dependencies** (infrastructure first)

3. **fleetingdns-workload**: Application workloads
   - **Path**: `./k8s-tilt/clusters/workload/CLUSTER_PATH`
   - **Interval**: `10m0s`
   - **Depends on**: `fleetingdns-infra`

## 🎯 Cluster-Specific Customization

### Required Patches Per Cluster

Each cluster MUST patch the following placeholder values:

#### GitRepository Patches
```yaml
- target:
    kind: GitRepository
    name: fleetingdns
  patch: |-
    - op: replace
      path: /spec/ref/branch
      value: "main"  # or "staging", "dev"
    - op: replace
      path: /spec/secretRef/name
      value: "flux-system-cluster-specific"
```

#### Workload Kustomization Patches
```yaml
- target:
    kind: Kustomization
    name: fleetingdns-workload
  patch: |-
    - op: replace
      path: /spec/path
      value: "./k8s-tilt/clusters/workload/eu-west-1/prod/auto-pilot"
    - op: replace
      path: /spec/postBuild/substitute/cluster_name
      value: "fleetingdns-eu-west-1-prod"
    - op: replace
      path: /spec/postBuild/substitute/region
      value: "eu-west-1"
    - op: replace
      path: /spec/postBuild/substitute/environment
      value: "production"
```

#### Infrastructure Kustomization Patches
```yaml
- target:
    kind: Kustomization
    name: fleetingdns-infra
  patch: |-
    - op: replace
      path: /spec/postBuild/substitute/project_id
      value: "fleetingdns-prod-eu-west-1"
```

### Environment-Specific Settings

| Environment | Branch | Interval | Suspend Infra |
|-------------|--------|----------|---------------|
| **Local Dev** | `dev` | `30s` | `true` |
| **Staging** | `staging` | `2m0s` | `false` |
| **Production** | `main` | `5m0s` | `false` |

## 🔐 Security Configuration

### Multi-Tenancy
Flux is configured with `--watch-all-namespaces=false` for security:
- Only watches `flux-system` namespace
- Prevents cross-namespace access
- Suitable for multi-tenant clusters

### Secret Management
Each cluster should have its own GitHub access secret:
```bash
kubectl create secret generic flux-system-eu-west-1 \
  --from-literal=username=git \
  --from-literal=password=$GITHUB_TOKEN \
  -n flux-system
```

## 📊 Monitoring and Observability

### Check Flux Status
```bash
# Check all Flux resources
flux get all

# Check specific GitRepository
flux get source git fleetingdns

# Check specific Kustomization
flux get kustomization fleetingdns-workload

# View logs
flux logs --level=error
```

### Troubleshooting
```bash
# Force reconciliation
flux reconcile source git fleetingdns
flux reconcile kustomization fleetingdns-workload

# Suspend/resume
flux suspend kustomization fleetingdns-infra
flux resume kustomization fleetingdns-infra
```

## 🌍 Multi-Cluster Deployment Strategy

1. **Bootstrap Phase**: Install Flux with base configuration
2. **Customization Phase**: Apply cluster-specific patches
3. **Sync Phase**: Flux automatically deploys workloads
4. **Monitoring Phase**: Monitor via Flux CLI and Grafana

This setup enables **GitOps-native multi-cluster management** with environment-specific customization while maintaining a single source of truth.

## 🔄 Integration with Enterprise Structure

The Flux configuration integrates seamlessly with the enterprise Kubernetes layout:

- **Workload Clusters**: `k8s-tilt/clusters/workload/{region}/{env}/auto-pilot/`
- **Infrastructure Clusters**: `k8s-tilt/clusters/infra/{region}/{env}/`
- **Crossplane Resources**: `k8s-tilt/crossplane/*`
- **Base Components**: `k8s-tilt/base/` and `k8s-tilt/components/`

This enables **declarative infrastructure management** across all regions and environments. 