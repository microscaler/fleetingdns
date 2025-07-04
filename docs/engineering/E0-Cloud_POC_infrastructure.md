# 📘 **E0 – Infrastructure & GCP Deployment (Designv0.1)**

> *Foundation epic for all runtime and Ops work.  Replaces earlier Hetzner notes; assumes primary cloud = GoogleCloud Platform.*

---

## 1 ▪ WHY GCP?

| Factor                          | Rationale                                                                                                         |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| **Global any‑cast L4 TCP LB**   | Single public IP close to every user; no DIY BGP / Keepalived.                                                    |
| **Generous free tier**          | CloudSQL `db‑f1‑micro`, GKE Autopilot free control plane, e2‑micro compute credits → perfect for MVP flush burn. |
| **Crossplane.io provider‑gcp**  | Provision clusters, node‑pools, CloudSQL, Redis **GitOps‑style** from YAML.                                      |
| **Rapid scale**                 | Autopilot auto‑adds nodes; can flip to committed‑use discounts or Spot pools as we grow.                          |
| **Managed Redis (MemoryStore)** | 0‑ops instead of self‑hosted Helm chart.                                                                          |

---

## 2 ▪ High‑level architecture graph

```mermaid
flowchart TD
    subgraph UserEdge["Google Global Network"]
        GLB[Global L4 TCP LB -anycast IP]
    end

    subgraph GKE_Workload["AutopilotGKE (europe‑west1)"]
        NPsmall[Node Pool-small e2_micro API / UI pods]
        NPlarge[Node Pool-large e2_standard_ Edge+Hub pods]
        EdgeHub[Edge+Hub StatefulSet]
        API[API + CA Deploy]
    end

    subgraph GKE_Infra["StandardGKE – infra e2‑micro"]
        Crossplane[Crossplane Controller+provider_gcp]
    end

    subgraph Payment["Payment Services"]
        StrBilling[(Stripe SaaS)]
    end

    subgraph Managed["ManagedGCP Services"]
        SQL[(Cloud SQL Postgres db_f1_micro)]
        Redis[(MemoryStore Redis basic_tier)]
    end

    subgraph Hetzner["Hetzner Self‑hosted Runner"]
        Runner[GitHub Actions Runner - dedicated Hezner CX11]
    end

    Github[GitHub private repo]

    %% data plane
    GLB --> EdgeHub
    EdgeHub --> Redis
    API --> SQL
    API --> Redis
    Github -.webhook tests.-> GLB
    StrBilling --> GLB

    %% CI / infra plane
    Github -- CI jobs --> Runner
    Runner --> GLB 
    %% end‑to‑end smoke tests
    Crossplane -.-> GKE_Workload
    Crossplane -.-> Managed
```

*Crossplane runs in a **dedicated StandardGKE infra cluster** (single e2‑micro node) to keep management isolated.  Compositions then provision the **Autopilot workload cluster**, node pools, CloudSQL, and Redis.  Self‑hosted runner lives outside GCP to avoid paid minutes.*\* compositions declare **GKE cluster**, **node pools**, **redis‑instance**, **sql‑instance**; FluxCD applies them on each commit.\*

---

## 3 ▪ Resource specification (Crossplane XR snippets)

```yaml
apiVersion: container.gcp.crossplane.io/v1beta2
kind: GKECluster
metadata: {name: edf-dev}
spec:
  location: europe-west1
  autopilot: true
---
apiVersion: container.gcp.crossplane.io/v1alpha1
kind: NodePool
metadata: {name: edge-large}
spec:
  clusterRef: {name: edf-dev}
  config:
    machineType: e2-standard-4
    spot: true
  autoscaling:
    minNodeCount: 0
    maxNodeCount: 2
```

(Full compositions in `/infra/` directory.)

---

## 4 ▪ MVP OPEX (August2025€)

| Service                          | SKU / Node            | Qty / hrs | Unit€    | Est.€/mo          | Notes                            |
| -------------------------------- | --------------------- | --------- | --------- | ------------------ | -------------------------------- |
| **Workload cluster (Autopilot)** | control plane         | free      | —         | **0.00**           | Free tier.                       |
| Autopilot compute\*              | 2 × e2‑micro          | 730h     | included  | **0.00**           | Within free 744 vCPU‑sec credit. |
| Spot pool (e2‑standard‑4)        | avg10h              | 0.0104/h  | **0.10**  | Load‑test only.    |                                  |
| **L4 TCP LB**                    | 1 rule                | 744h     | 0.0065/h  | **4.70**           | Minor data cost extra.           |
| CloudSQL Postgres               | db‑f1‑micro           | 744h     | free      | **0.00**           | 10 GB disk.                      |
| MemoryStore Redis                | basic1 GB            | 744h     | 0.0267/h  | **19.80**          | Smallest tier.                   |
| Cloud Logging & Metrics          | 5 GB                  | 0.50/GB   | **2.50**  | Sampled.           |                                  |
| Cloud NAT egress                 | 1 GB                  | 0.11/GB   | **0.11**  | Webhook responses. |                                  |
| **Infra cluster (Standard)**     | control‑plane fee     | 744h     | 0.092 €/h | **68.45**          | €0.10/hr ≈ \$72/mo.              |
| Infra node                       | e2‑micro pre‑emptible | 744h     | 0.004 €/h | **2.98**           | Runs Crossplane <200 m CPU.      |
| **Hetzner CI Runner**            | CX11 dedicated        | 720h     | 49.00/mo  | **49.00**          | Unlimited private‑repo minutes.  |
| **Estimated monthly OPEX**       | —                     | —         | —         | **≈€147.64**      | ≈€4.9 / day.                    |

*Autopilot bills per‑pod; e2‑micro pods fit within the free quota.*\*\* |  |  |  | **≈€27.21** | <€1/day. |

\*Autopilot charges vCPU/Memory per‑pod; e2‑micro pods fit free quota.

---

## 5 ▪ Roll‑out phases

1. **Week1**— Crossplane bootstrap (`kubectl crossplane install`).
2. **Week2**— Deploy API + Edge Hub; internal test with GitHub Action.
3. **Week3**— Hook up CloudSQL & Redis secrets via `Workload Identity`.
4. **Week4**— Enable global LB, managed cert, DNS A → LB IP.
5. **Week5**— Stress‑test 50k tunnels/day; observe MemStore CPU.
6. **Week6**— Turn on Spot pool autoscaling; pre‑prod cut‑over.

---

## 6 ▪ Risks & Mitigations

| Risk                                             | Mitigation                                           |
| ------------------------------------------------ | ---------------------------------------------------- |
| Redis basic tier caps at 1GB RAM / \~15k conns | Scale to standard‑tier when concurrent tunnels >5k. |
| GKE free‑tier Pod quotas exhausted in heavy CI   | NodePool autoscaling to Spot restores headroom.      |
| Global LB cold‑start 90s new back‑end health    | Pre‑warm by rolling update strategy `maxSurge=1`.    |

---

## 7 ▪ Multi‑Region NEG Topology (future phase)

```mermaid
flowchart TD
  subgraph GlobalLB["Global TCP LB (anycast)"]
  end

  subgraph EU["GKEeurope‑west1"]
    EdgeEU[EdgeHub pods]
  end
  subgraph US["GKEus‑central1"]
    EdgeUS[EdgeHub pods]
  end
  subgraph APAC["GKEasia‑southeast1"]
    EdgeAP[EdgeHub pods]
  end
  GlobalLB --> EdgeEU & EdgeUS & EdgeAP
  classDef edge fill:#79c,stroke:#333,color:#fff;
  class EdgeEU,EdgeUS,EdgeAP edge;

  subgraph ManagedGcp["Shared GCP services"]
    RedisG[(MemoryStore
replication)]
    SQLG[(Cloud SQL – read replica)]
  end
  EdgeEU --> RedisG & SQLG
  EdgeUS --> RedisG & SQLG
  EdgeAP --> RedisG & SQLG

  subgraph Hetzner["Self‑hosted CI Runner"]
    RunnerH[Runner]
  end
  RunnerH --> GlobalLB
```

*Each region registers its NEG back‑end to the global LB. `cluster_id` bits (E1f) ensure the receiving Edge can redirect to correct region if cross‑hit.*

---

©2025 Ephemeral DNS Forwarder — **Infrastructure Epic (GCP)**\*\*
