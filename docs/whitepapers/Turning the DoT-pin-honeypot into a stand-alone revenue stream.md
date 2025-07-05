### Turning the **DoT-pin-honeypot** into a stand-alone revenue stream

|                       | **fleetingDNS honeypot SKU**                                                                     | **Closest market analogue**                                                                  | Competitive edge                                                                            |
| --------------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| **Sensor type**       | *Out-of-band* DoT endpoint that only triggers when TLS-interception is attempted (pin mismatch). | Thinkst Canary DNS tokens – unique sub-zones that fire on resolution ([blog.thinkst.com][1]) | Pin-mismatch gives **zero noise** (legit resolvers never connect) → very high fidelity IOC. |
| **Deployment effort** | One CNAME (`_honeypot.<corp>.net → hpin.fdns.run`) – no host to patch.                           | Host-based or subnet-wide canaries need VM/container or AD creds ([blog.thinkst.com][2])     | Lower friction → easier to sell to SaaS teams, SMBs.                                        |
| **Visibility**        | Captures attackers **before** tunnel exists (namespace discovery phase).                         | Classic web honeypots only see traffic once endpoint reachable.                              | Earlier signal → higher perceived value to IR/SOC.                                          |
| **Threat-intel feed** | JA3 + ASN + GeoIP stream via MISP/STIX.                                                          | Spamhaus / GreyNoise paid feeds ([hunt.io][3])                                               | Fresh, proprietary data set specific to DNS/DoT MITM.                                       |

---

## 1 ▪ Product concept

| Tier                 | Monthly € | Included honeypots              | Retention | Add-ons                         |
| -------------------- | --------- | ------------------------------- | --------- | ------------------------------- |
| **Free SOC-trial**   | 0         | 1 domain • 2 events/day         | 7 days    | —                               |
| **Insight**          | 49        | 5 domains • 100 events/day      | 90 days   | Extra domains €5 ea.            |
| **Hunt**             | 299       | 25 domains • 5 000 events/day   | 180 days  | GreyNoise enrichment, Slack app |
| **Enterprise Intel** | Custom    | 100+ domains, private STIX feed | 365 days  | Private API, 24×7 response      |

> *Attach as a separate SKU: users don’t need tunnels, only a FleetingDNS account.*

---

## 2 ▪ Technical delta from E1l

1. **Multi-tenant Cloud Pub/Sub → BigQuery** pipeline already sketched in E1l;
   add **API endpoint** `/v1/honeypot/events` (JWT-scoped) for the portal.
2. Public **docs & SDK snippets** (Go / Python) for real-time SIEM ingestion.
3. Billing hook: count “actionable events” (≥ severity 4) and bill per 10 k events.

---

## 3 ▪ Revenue model snapshot (year 1 Europe focus)

| Metric                       | Conservative                | Aggressive            |
| ---------------------------- | --------------------------- | --------------------- |
| Paying orgs                  | 250 (mostly “Insight” tier) | 800                   |
| ARPU                         | €55                         | €68                   |
| ARR                          | **€165 k**                  | **€654 k**            |
| COGS (GCP + enrichment APIs) | \~€1 000 / mo → €12 k       | \~€2 800 / mo → €34 k |
| Gross margin                 | **> 90 %**                  | **> 94 %**            |

*Traffic cost is trivial because every honeypot connection is a **failed TLS handshake** (few kB).*

---

## 4 ▪ Go-to-market

* **Foot-in-door** upsell: every tunnels customer gets one free honeypot domain → prove value.
* Target security-minded SaaS (FinTech, HealthTech) and MSPs needing external attack-surface monitoring.
* Quarterly “DNS Tunnel Threat Report” using aggregated anonymised data → PR & lead-gen.

---

## 5 ▪ Backlog extension *(add to board as Epic E1m – Honeypot SKU)*

| Story                                                                         | Goal |
|-------------------------------------------------------------------------------|------|
| **E1m-S1** – Self-serve “Create honeypot domain” wizard (no tunnel required). |      |
| **E1m-S2** – Event API + webhook to Splunk / QRadar.                          |      |
| **E1m-S3** – Stripe product & metered event quota.                            |      |
| **E1m-S4** – Threat-score engine (GreyNoise & AbuseIPDB enrichment).          |      |
| **E1m-S5** – Quarterly PDF report generator (automations tool).               |      |
| **E1m-S6** – Marketing site landing page + pricing table update.              |      |

*(Can draft full user stories on request.)*

---

### Bottom line

*There is **no mainstream, pin-mismatch–specific honeypot SaaS** today—closest analogues are Thinkst Canary DNS tokens and generic SOC threat-intel feeds.*
By turning the E1l honeypot into a paid add-on (or stand-alone product) FleetingDNS can open a **high-margin, security-analytics revenue stream** that complements the core tunnel service without heavy additional infra.

[1]: https://blog.thinkst.com/2023/08/default-behaviour-sticks-and-so-do-examples.html?utm_source=chatgpt.com "Default behaviour sticks (And so do examples) - Thinkst's Blog"
[2]: https://blog.thinkst.com/2022/09/sensitive-command-token-so-much-offense.html?utm_source=chatgpt.com "Sensitive Command Token – So much offense in my defense"
[3]: https://hunt.io/glossary/best-threat-intelligence-feeds?utm_source=chatgpt.com "Top 5 Best Threat Intelligence Feeds (Updated 2025) - Hunt.io"
