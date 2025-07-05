# 🔒 **E6 – Paid Tier & Billing (Internal)**  
*Epic → User-story breakdown (v0.1)*  
**CONFIDENTIAL – keep in canvas only**

---

## Epic Goal
> “Implement Stripe‑based subscription management, plan enforcement, usage‑based add‑ons, and team/organization account hierarchy to monetise FleetingDNS without friction.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E6P-S1** | As a *new user*, self‑subscribe to **Free → Supporter (€10/mo)** via Stripe Checkout. |
| **E6P-S2** | As a *Team admin*, invite colleagues & share a pooled **Team plan** quota. |
| **E6P-S3** | As *Finance*, see **monthly invoice** in portal + email. |
| **E6P-S4** | As *DevOps*, API keys scoped to **team** not personal user. |
| **E6P-S5** | As *Org*, purchase **custom domain add‑on** billed annually. |
| **E6P-S6** | As *Billing backend*, reconcile **usage records** (bytes, minutes) nightly to Stripe metered‑billing. |

---

## E6P-S1 — Stripe Checkout Flow
**Tasks**
1. Create products & prices in Stripe dashboard.  
2. `/billing/checkout-session` API returns session URL.  
3. Stripe webhook `checkout.session.completed` → upgrade plan in Postgres.

**Functional**
* User redirected back to portal with `plan=supporter` active.  
* `plan_id` stored in users table.

**Non-Functional**
* Checkout success rate ≥ 98 %.  
* PII stored only in Stripe, not DB.

---

## E6P-S2 — Team Invitation
**Tasks**
1. Create `teams` table; `team_members`.  
2. Portal UI invite via email; token link.  
3. Quota enforcement counts by team_id.

**Functional**
* Team plan supports 10 concurrent tunnels across members.  
* Member leave revokes API keys.

**Non-Functional**
* Invite link expiry 48 h.  
* GDPR delete cascades.

---

## E6P-S3 — Invoice Delivery
**Tasks**
1. Enable Stripe Billing Portal.  
2. Webhook `invoice.finalized` → email send via Postmark.

**Functional**
* Email PDF attached.  
* Invoice shows usage add‑ons.

**Non-Functional**
* Email SLA 99.9 %.  
* Bounce rate < 1 %.

---

## E6P-S4 — Team-Scoped API Keys
**Tasks**
1. `api_keys` table adds `owner_type` (user/team).  
2. Portal UI pagination.

**Functional**
* Team admin can revoke member keys.  
* All SDK calls succeed with team tokens.

**Non-Functional**
* Key issue API < 50 ms.  
* Key prefix indicates scope.

---

## E6P-S5 — Custom Domain Add‑on
**Tasks**
1. Checkout price `€99/yr`.  
2. Portal collects CNAME target; API writes DNS cert via ACME.

**Functional**
* `mycorp.dev` resolves to tunnel.  
* Auto-renew cert every 60 d.

**Non-Functional**
* Cert issuance < 2 min.  
* Add‑on renewal reminder 30 d prior.

---

## E6P-S6 — Usage Reconciliation
**Tasks**
1. Nightly job reads Redis `usage:{slot}` batches.  
2. Creates Stripe `usage_record` per subscription item.  
3. Clears processed keys.

**Functional**
* All usage posted before invoice finalised.  
* Retry on 409 conflict.

**Non-Functional**
* Job runtime < 5 min (50 k records).  
* Idempotent rerun safe.

---

© 2025 FleetingDNS — Paid Tier stories (confidential)

