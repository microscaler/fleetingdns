# 📗 **E1l – DoT Pinning Honeypot & Threat‑Intel**  
*Sub-Epic → User-story breakdown (v0.1)*

Deploys a decoy DoT endpoint advertising **bogus SPKI pins** to lure on‑path TLS‑stripping attackers or mis‑configured corporate middleboxes.  Captured connections feed an abuse–IP intelligence pipeline that automatically blocks, rate‑limits, or flags accounts exhibiting suspicious behaviour.

---

## Epic Goal
> “Detect and respond to Man‑In‑The‑Middle attempts against FleetingDNS DoT by operating a pin‑mismatch honeypot, analysing fingerprints, enriching with Geo/IP data, and shipping actionable threat intel to EdgeHub fire‑wall tables within 10minutes.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1l‑S1** | As a *Security engineer*, stand up a **honeypot DoT endpoint** (`badpin.fleetingdns.run`) presenting a valid cert but **unsigned SPKI**. |
| **E1l‑S2** | As *Threat‑Intel*, log **client hello fingerprints & SNI** of any connection that proceeds despite pin mismatch. |
| **E1l‑S3** | As *Abuse automation*, push offending **source IP / ASN** to Redis `abuse:set` and propagate to EdgeHub eBPF block‑list. |
| **E1l‑S4** | As *Ops*, enrich events with **MaxMind GeoIP + GreyNoise** reputation and display in security dashboard. |
| **E1l‑S5** | As *Compliance*, export anonymised intel feed to **MISP/STIX** for community sharing every 24h. |
| **E1l‑S6** | As *SRE*, alert if honeypot hits >100/min (possible widespread MITM) or if known customer CIDR appears. |

---

## E1l‑S1 — Honeypot Endpoint Deployment
**Tasks**
1. Allocate DNS `badpin.fleetingdns.run` → LB same anycast IP but different **TLS cert key pair**.  
2. Serve DoT on TCP853 with **rustls**; intentionally omit pin from `/v1/dot/pins`.  
3. Run in isolated k8s namespace `honeypot-dot` (no internal services).

**Functional**
* Legit clients with embedded pins will reject connection (verify in integration test).  
* Only mis‑configured or malicious middleboxes proceed and send queries. |

**Non‑Functional**
* Service CPU <50m.  
* No route back into production pods (NetworkPolicy default‑deny). |

---

## E1l‑S2 — Fingerprint Collection
**Tasks**
1. Capture TLS ClientHello via **rustls callback**; extract JA3 hash, SNI, ALPN.  
2. Log JSON to **Cloud Pub/Sub** topic `dot‑honeypot`.  
3. Strip query payload; collect only metadata. |

**Functional**
* Event JSON: `{ts, ip, ja3, sni, country, asn}`.  
* Pseudonymise IP (hash last /24 for GDPR). |

**Non‑Functional**
* Event size ≤200B.  
* Throughput sustain 1k events/s without loss. |

---

## E1l‑S3 — Abuse Propagation Pipeline
**Tasks**
1. Dataflow job aggregates events: count per IP per hour.  
2. Threshold: >10 hits/hr → add to Redis `abuse:set ttl 7d`.  
3. EdgeHub eBPF program reads set every 5min → drops packets. |

**Functional**
* Block active within 10min of detection.  
* Redis set capped 100k entries (LRU). |

**Non‑Functional**
* False‑block rate <0.01%.  
* Expiry auto‑removes IP after 7days. |

---

## E1l‑S4 — Threat Enrichment Dashboard
**Tasks**
1. CloudFunction enrich event with **MaxMind ASN, Geo, GreyNoise “noise” flag**.  
2. Push to BigQuery `honeypot_events`.  
3. Grafana JSON datasource panel & map visual. |

**Functional**
* Dashboard shows top ASNs, countries, time‑series.  
* Supports drill‑down to raw event. |

**Non‑Functional**
* Enrichment latency <2s.  
* Dashboard loads <5s for 1million rows. |

---

## E1l‑S5 — MISP/STIX Export
**Tasks**
1. Nightly Cloud Run job queries events where score >0.8.  
2. Build STIX `ipv4‑addr` objects; bundle; POST to `https://misp.fdns.net/events/add`.  
3. Publish changelog in security RSS feed. |

**Functional**
* Export contains only de‑identified IP / ASN / JA3.  
* 200 OK response logged. |

**Non‑Functional**
* Job runtime <3min.  
* InfoSec admin approves first export (manual). |

---

## E1l‑S6 — Alerting & Safety Nets
**Tasks**
1. Prometheus rule `rate(honeypot_hits_total[5m]) > 100`.  
2. PagerDuty “Possible MITM epidemic”.  
3. If source CIDR matches known customer, open internal Jira ticket rather than block. |

**Functional**
* Alert fires within 2min of surge.  
* Customer ticket contains instructions to check TLS‑intercept appliances. |

**Non‑Functional**
* False positive <1 / quarter.  
* Blocklist excludes customer CIDR allow‑list. |

---

©2025FleetingDNS — DoT Pinning Honeypot & Threat‑Intel stories

