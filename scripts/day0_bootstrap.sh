#!/opt/homebrew/Cellar/bash/5.2.37/bin/bash
# day0_bootstrap.sh  —  create org scaffolding for FleetingDNS
set -euo pipefail

############### USER VARS ########################################
export ORG_ID="131547940867"                # gcloud organizations list
export BILLING_ACCT="0197D9-AD23AA-0EFFBF"  # gcloud beta billing accounts list
export region="us-central1"

export INFRA_PROJ_ID="fleetingdns-infra-aa"          # globally unique
export INFRA_PROJ_NAME="FleetingDNS Infra-aa"  # globally unique

export WORKLOAD_PROJ_ID="fleetingdns-workload-aa"    # globally unique
export WORKLOAD_PROJ_NAME="FleetingDNS Workload-aa"  # globally unique
##################################################################

#echo "==> Creating projects…"
#gcloud projects create "$INFRA_PROJ_ID"   --name="$INFRA_PROJ_NAME"   --organization="$ORG_ID"
#gcloud projects create "$WORKLOAD_PROJ_ID" --name="$WORKLOAD_PROJ_NAME" --organization="$ORG_ID"

echo "==> Linking billing…"
gcloud beta billing projects link "$INFRA_PROJ_ID"   --billing-account="$BILLING_ACCT"
gcloud beta billing projects link "$WORKLOAD_PROJ_ID" --billing-account="$BILLING_ACCT"

APIS="container.googleapis.com compute.googleapis.com \
      iam.googleapis.com iamcredentials.googleapis.com \
      dns.googleapis.com serviceusage.googleapis.com \
      cloudresourcemanager.googleapis.com redis.googleapis.com \
      sqladmin.googleapis.com artifactregistry.googleapis.com \
      cloudbuild.googleapis.com cloudkms.googleapis.com"

echo "==> Enabling APIs (infra)…"
gcloud services enable $APIS --project="$INFRA_PROJ_ID"
echo "==> Enabling APIs (workload)…"
gcloud services enable $APIS --project="$WORKLOAD_PROJ_ID"

echo "==> Creating VPCs with non-overlapping CIDRs…"
gcloud compute networks create infra-vpc --project="$INFRA_PROJ_ID" --subnet-mode=custom
gcloud compute networks subnets create infra-subnet \
    --network=infra-vpc --region="$region" --range="10.172.16.0/20" --project="$INFRA_PROJ_ID"

gcloud compute networks create workload-vpc --project="$WORKLOAD_PROJ_ID" --subnet-mode=custom
gcloud compute networks subnets create workload-subnet \
    --network=workload-vpc --region="$region" --range="10.172.32.0/20" --project="$WORKLOAD_PROJ_ID"

echo "==> VPC peering between projects…"
gcloud compute networks peerings create infra-to-workload \
    --network=infra-vpc --peer-project="$WORKLOAD_PROJ_ID" \
    --peer-network=workload-vpc --auto-create-routes \
    --project="$INFRA_PROJ_ID"

gcloud compute networks peerings create workload-to-infra \
    --network=workload-vpc --peer-project="$INFRA_PROJ_ID" \
    --peer-network=infra-vpc --auto-create-routes \
    --project="$WORKLOAD_PROJ_ID"

echo "==> Infra cluster (Standard, single e2-micro)…"
gcloud container clusters create infra-cluster \
    --zone="${region}-b" --project="$INFRA_PROJ_ID" \
    --machine-type=e2-micro \
    --num-nodes=1 --enable-ip-alias \
    --enable-private-nodes --release-channel=stable

echo "==> Workload cluster (Autopilot)…"
gcloud container clusters create-auto workload-cluster \
    --region="$region" --project="$WORKLOAD_PROJ_ID" \
    --enable-private-nodes

echo "==> Workload Identity for infra Flux SA…"
gcloud iam service-accounts create flux-deployer \
    --project="$INFRA_PROJ_ID" \
    --description="Flux CD deployer into workload cluster"

gcloud projects add-iam-policy-binding "$WORKLOAD_PROJ_ID" \
    --member="serviceAccount:flux-deployer@$INFRA_PROJ_ID.iam.gserviceaccount.com" \
    --role=roles/container.developer

echo "==> All set.\n• Infra project:     $INFRA_PROJ_ID\n• Workload project:  $WORKLOAD_PROJ_ID"
echo "Next steps:\n1.  bootstrap_flux.sh against infra-cluster\n2.  Commit Crossplane provider configs\n3.  Let Flux provision Cloud SQL, Redis, runner-controller, etc."
