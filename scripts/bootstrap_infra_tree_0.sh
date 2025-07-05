#!/usr/bin/env bash
# bootstrap_infra_tree.sh
# Run from repo root: `bash scripts/bootstrap_infra_tree.sh`

set -euo pipefail

BASE="infra"

declare -A files=(
  # providers
  ["providers/provider-gcp.yaml"]="# Crossplane provider-gcp install CRD\n# Should: install v0.35 provider package via XPKG"
  ["providers/provider-hcloud.yaml"]="# Crossplane provider-hcloud install CRD\n# Should: install Hetzner provider for optional runner"
  ["providers/providerconfig-gcp.yaml"]="# ProviderConfig binding to Workload Identity\n# Should: reference SA flux-deployer for all GCP resources"
  ["providers/providerconfig-hcloud.yaml"]="# ProviderConfig with HETZNER_API_TOKEN secret\n# Should: enable Crossplane to create CX11 server"

  # gcp-core
  ["gcp-core/vpc-infra.yaml"]="# VPC for infra project\n# Should: auto subnets, private nodes"
  ["gcp-core/vpc-workload.yaml"]="# VPC for workload project (Autopilot uses it)"
  ["gcp-core/network-peering.yaml"]="# VPC peering between infra and workload projects"
  ["gcp-core/budget-alert.yaml"]="# Budget alert at €200 with 50/90% thresholds"
  ["gcp-core/global-address-glb.yaml"]="# Reserved global anycast IP for L4 TCP LB"
  ["gcp-core/dns-zone-edf.yaml"]="# Cloud DNS managed zone for *.fleetingdns.run"
  ["gcp-core/dns-record-wildcard.yaml"]="# Wildcard A record to anycast IP"
  ["gcp-core/artifact-registry.yaml"]="# Artifact Registry repo for containers"
  ["gcp-core/memstore-redis.yaml"]="# MemoryStore Redis 1-GB instance"
  ["gcp-core/cloudsql-instance.yaml"]="# Cloud SQL Postgres db-f1-micro instance"
  ["gcp-core/cloudsql-database.yaml"]="# Initial Postgres database 'edf_meta'"
  ["gcp-core/cloudsql-user.yaml"]="# Postgres user 'edf_app' with IAM auth"
  ["gcp-core/forwarding-rule-l4tcp.yaml"]="# External TCP LB rule (80/443)"
  ["gcp-core/forwarding-rule-wg.yaml"]="# UDP LB rule 51820 for WireGuard"

  # clusters
  ["clusters/infra-cluster.yaml"]="# Standard GKE infra cluster XR"
  ["clusters/infra-nodepool-default.yaml"]="# Default nodepool (e2-micro)"
  ["clusters/workload-cluster-autopilot.yaml"]="# Autopilot workload cluster XR"
  ["clusters/workload-nodepool-edge-spot.yaml"]="# Spot nodepool (e2-standard-4) for Edge + runners"
  ["clusters/workload-nodepool-api.yaml"]="# Small nodepool for API pods"

  # iam-oidc
  ["iam-oidc/wi-pool-github.yaml"]="# Workload Identity Pool trusting GitHub OIDC"
  ["iam-oidc/wi-provider-github.yaml"]="# OIDC provider inside the pool"
  ["iam-oidc/sa-flux-deployer.yaml"]="# Service Account used by Flux infra"
  ["iam-oidc/sa-flux-binding.yaml"]="# IAM binding allowing GitHub principalSet to impersonate SA"
  ["iam-oidc/sa-arc-controller.yaml"]="# SA for actions-runner-controller pods"
  ["iam-oidc/sa-arc-binding.yaml"]="# IAM binding for ARC SA"

  # runner-hcloud (legacy / optional)
  ["runner-hcloud/ssh-key-runner.yaml"]="# Hetzner SSH key resource (optional legacy)"
  ["runner-hcloud/server-ci-runner.yaml"]="# CX11 runner server resource (optional)"

  # observability
  ["observability/otel-collector.yaml"]="# OpenTelemetry Collector Deployment + Service"
  ["observability/dashboard-edge-latency.yaml"]="# Cloud Monitoring dashboard JSON"
  ["observability/alertpolicy-budget50.yaml"]="# Alert policy 50% budget usage"

  # bootstrap
  ["bootstrap/bootstrap-vm.yaml"]="# One-shot ComputeInstance (spot) running k3s+Flux; DeletionPolicy: Delete"
)

echo "Creating infra directory tree…"
for path in "${!files[@]}"; do
  dir="$BASE/$(dirname "$path")"
  mkdir -p "$dir"
  file="$BASE/$path"
  if [[ ! -f "$file" ]]; then
    echo -e "${files[$path]}\n" > "$file"
    echo "  • $file"
  fi
done

# root kustomization stub
root_kustom="$BASE/kustomization.yaml"
if [[ ! -f "$root_kustom" ]]; then
cat <<'YAML' > "$root_kustom"
# Root kustomization aggregating all sub-dirs for infra cluster
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

# simple README placeholder
readme="$BASE/README.md"
if [[ ! -f "$readme" ]]; then
  cat <<'MD' > "$readme"
# Infra Directory

This tree is managed by Flux in the **infra cluster**.

* Every YAML file starts with a comment block describing the expected content.
* Apply order is governed by `kustomization.yaml`.
* Secret manifests should live **outside** the repo or be sealed via KSOPS.
MD
  echo "  • $readme"
fi

echo "✅ Infra scaffolding complete."
