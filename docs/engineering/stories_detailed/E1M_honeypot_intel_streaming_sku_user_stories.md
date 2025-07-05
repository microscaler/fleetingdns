# 📗 **E1m – Honeypot Threat‑Intel Streaming SKU**  
*Epic → User‑story breakdown (v0.1)*

Commercial spin‑off of E1l: operate FleetingDNS DoT‑pin honeypots as a stand‑alone, paid security‑intel product.  Collect source IPs & handshake fingerprints, enrich with ML‑based attacker profiling, and stream real‑time block‑lists to customer firewalls (Cloudflare / Palo Alto / AWS WAF, etc.).

---

## Epic Goal
> “Monetise pin‑mismatch telemetry by offering a SaaS feed (‘FDNS Shield’) that delivers real‑time, ML‑scored bad‑actor IPs to enterprise firewalls, with SLA‑backed <60‑second propagation and evidence detail for SOC triage.”

---

## 🗂️ Story List
| ID         | Story                                                                                                                              | Outcome |
|------------|------------------------------------------------------------------------------------------------------------------------------------|---------|
| **E1m‑S1** | As *Sales*, list SKU tiers (Insight / Hunt / Enterprise) and entitlement flags in billing DB.                                      |         |
| **E1m‑S2** | As *Customer*, create honeypot domain in portal and start receiving **streamed block‑list** via HTTPS webhook.                     |         |
| **E1m‑S3** | As *Pipeline*, write honeypot events to **BigQuery + FeatureStore** and train LightGBM model that scores IP risk (0‑100).          |         |
| **E1m‑S4** | As *Threat‑Intel*, publish **gRPC bidirectional stream** `ThreatFeed` with signed JWT + protobuf messages.                         |         |
| **E1m‑S5** | As *Firewall‑admin*, install pre‑built **Cloudflare Workers / Palo Alto MineMeld** transformer that ingests feed and writes rules. |         |
| **E1m‑S6** | As *Finance*, meter **billable events** = (# IPs × score≥80 delivered) and push usage to Stripe.                                   |         |
| **E1m‑S7** | As *Compliance*, store raw honeypot data 30days, aggregated intel 365days, GDPR‑delete on request.                       |         |

---

## E1m‑S1 — SKU & Entitlement Flags
**Tasks**
1. Add table `intel_plans` (fields: max_feeds, latency_sla, ml_score_access).  
2. Stripe product IDs mapped to plans.  
3. Portal billing page shows upgrade path.

**Functional**
* Entitlement checked by Intel API middleware.  
* 403 if plan expired.

**Non‑Functional**
* Plan switch propagation <60s via Redis pub/sub.

---

## E1m‑S2 — Honeypot Domain + Webhook Flow
**Tasks**
1. Portal collects webhook URL & JWT secret.  
2. Verify webhook via `GET /health` handshake.  
3. Events pushed `POST /feed { ip, score, ja3, first_seen }`\, signed HMAC.

**Functional**
* Latency SLA <60s from first honeypot hit to webhook deliver.  
* Retry w/ exponential back‑off (x6).

**Non‑Functional**
* Webhook body ≤1KB.  
* Failure rate <0.1%.

---

## E1m‑S3 — ML Scoring Pipeline
**Tasks**
1. Dataflow job aggregates 7features (ja3, ASN entropy, country, hourly count…).  
2. Train LightGBM on 90‑day labels (malicious vs benign).  
3. VertexAI FeatureStore & model registry; deploy online prediction endpoint.

**Functional**
* Score≥90% recall @ 1% FPR in validation.  
* Model refreshed weekly.

**Non‑Functional**
* Prediction latency <20ms (online).  
* Training cost <€50/run.

---

## E1m‑S4 — gRPC ThreatFeed Stream
**Tasks**
1. Define proto `ThreatRecord { ip, score, tag, evidence_url }`.  
2. Implement bidirectional streaming service with `tokio-tonic`; client acks receipt.  
3. JWT auth (`aud=threatfeed`, sub=tenantId).

**Functional**
* Supports back‑pressure (client `ready` flag).  
* Historical replay on reconnect.

**Non‑Functional**
* Throughput 2000 rec/s.  
* TLS mutual‑auth cert pinned.

---

## E1m‑S5 — Firewall Integrations
**Tasks**
1. Cloudflare Worker script pulls gRPC → KV → IP Access Rules.  
2. Palo‑Alto: MineMeld prototype node pulling webhook feed.  
3. Terraform module examples.

**Functional**
* Rule hit‑rate validated in staging (<30s propagate).  
* Docs page per integration.

**Non‑Functional**
* Worker CPU <10ms per update.  
* Integration maintained under Apache‑2.

---

## E1m‑S6 — Metered Billing
**Tasks**
1. Counter `intel_events_delivered_total{plan}` in Redis.  
2. Nightly job posts `usage_record` to Stripe (`intell‑event‑ delivered`).  
3. Portal usage graph.

**Functional**
* Billing accuracy±1%.  
* Over‑usage throttle429.

**Non‑Functional**
* Job runtime <5min for 10M events.

---

## E1m‑S7 — Data Retention & GDPR
**Tasks**
1. BigQuery table partition day; raw events TTL30days (policy).  
2. Aggregated feature table TTL365days.  
3. GDPR delete job removes ip‑hash rows by request.

**Functional**
* Delete SLA 30days.  
* Audit log of deletion.

**Non‑Functional**
* Failure rate <0.01%.  
* Retention config IaC via Crossplane.

---

©2025FleetingDNS— Honeypot Threat‑Intel Streaming SKU stories

