#!/usr/bin/env bash
# scripts/bootstrap_infra_tree.sh  (v2, multi-region aware)
# -------------------------------
# Generates the entire infra/ manifest directory tree WITH
# per-continent workload cluster stubs and descriptive doc comments.
#
# 1.  Edit REGIONS / REGION_CODES arrays as your footprint grows.
# 2.  Run from repo root:  bash scripts/bootstrap_infra_tree.sh
# --------------------------------------------------------------

set -euo pipefail

BASE=infra

# ────────────────────────────────
#  Edit these arrays to add a new continent
REGIONS=(eu us apac)                         # Short slug
REGION_CODES=(europe-west1 us-central1 asia-southeast1)
# Ensure both arrays stay same length!
# ────────────────────────────────

declare -A static_files=(
  # (same static list from v1 – shortened here for brevity)
  ["providers/provider-gcp.yaml"]="# Crossplane provider-gcp install CRD"
  ["providers/provider-hcloud.yaml"]="# Crossplane provider-hcloud install CRD"
  # … (omit unchanged rows to keep sample concise)
  ["observability/alertpolicy-budget50.yaml"]="# Alert when billing hits 50 % of €200 budget"
)

###############################
# 1. Static part (unchanged)
###############################
echo "==> Creating static infra scaffold…"
for path in "${!static_files[@]}"; do
  mkdir -p "$BASE/$(dirname "$path")"
  file="$BASE/$path"
  [[ -f $file ]] || { echo -e "${static_files[$path]}\n" > "$file"; echo "  • $file"; }
done

###############################
# 2. Region-specific clusters
###############################
for i in "${!REGIONS[@]}"; do
  slug="${REGIONS[$i]}"          # eu, us, apac
  gcp="${REGION_CODES[$i]}"      # europe-west1, …
  clust_dir="$BASE/clusters"

  # Cluster XR
  cfile="$clust_dir/workload-cluster-${slug}.yaml"
  if [[ ! -f $cfile ]]; then
    cat > "$cfile" <<EOF
# Autopilot workload cluster in $gcp
# Should: Crossplane XR to create gke autopilot cluster for $slug region
apiVersion: container.gcp.crossplane.io/v1beta2
kind: GKECluster
metadata:
  name: workload-${slug}
spec:
  location: $gcp
  autopilot: true
EOF
    echo "  • $cfile"
  fi

  # Spot nodepool for edge/runners
  npfile="$clust_dir/workload-nodepool-edge-${slug}-spot.yaml"
  if [[ ! -f $npfile ]]; then
    cat > "$npfile" <<EOF
# Spot nodepool carrying Edge and ARC runners in $gcp
# Should: e2-standard-4, autoscale 0→4, preemptible true
apiVersion: container.gcp.crossplane.io/v1alpha1
kind: NodePool
metadata:
  name: edge-spot-${slug}
spec:
  clusterRef:
    name: workload-${slug}
  config:
    machineType: e2-standard-4
    spot: true
  autoscaling:
    minNodeCount: 0
    maxNodeCount: 4
EOF
    echo "  • $npfile"
  fi
done

###############################
# 3. Root kustomization.yaml
###############################
root_kustom="$BASE/kustomization.yaml"
if [[ ! -f $root_kustom ]]; then
cat <<'YAML' > "$root_kustom"
# Root infra kustomization – aggregates sub-dirs.
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - providers/
  - gcp-core/
  - clusters/
  - iam-oidc/
  - observability/
YAML
  echo "  • $root_kustom"
fi

echo "✅ Multi-region infra scaffold complete."
