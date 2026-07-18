# Security follow-up: dependency advisories & secret management

**Status as of 2026-07-17.** Tracks the remaining `cargo audit` advisories and the
GitGuardian secret findings on PR #70, with concrete next steps. Companion to the
tunnel data-plane work in `stories_detailed/E2_E3_tunnel_data_plane_user_stories_v0.3.md`.

---

## 1. Dependency advisories (`cargo audit`)

Progress this session: **27 → 5 vulnerabilities** (+ ~10 unmaintained/dev warnings).

### Cleared
- **russh 0.40 → 0.60.3** (RUSTSEC-2026-0154 7.5 high + russh-cryptovec 0.60.3,
  RUSTSEC-2026-0153) — full API migration, verified end-to-end (see commit
  "migrate russh 0.40 -> 0.60.3"). This was the highest-severity shipping advisory.
- **acme2 removed** — dead dep that pinned rustls 0.21 / reqwest 0.11, clearing
  rustls-webpki 0.101.7 ×3 (RUSTSEC-2026-0098/99/0104).
- Patch-level: aws-lc-sys (×4), postgres-protocol (×2), quinn-proto (×2),
  rustls-webpki 0.103, tracing-subscriber 0.3.20, bytes, slab, time, rkyv.

### Remaining — SHIPS in product binaries
**None.** hickory-proto 0.24 → 0.26.1 (RUSTSEC-2026-0119) was completed — see the
"migrate hickory-proto/resolver" commit. Every remaining advisory is dev/test-only.

### Remaining — DEV/TEST ONLY, do not ship (4)
Confirmed via `cargo tree -i <crate> -e no-dev` (all return "nothing to print"
for the product binaries):
| Advisory | Crate | Source | Fix status |
|----------|-------|--------|-----------|
| RUSTSEC-2023-0071 | rsa (Marvin timing) | testcontainers → sqlx/postgres | **No fix available** upstream; not linked into product |
| RUSTSEC-2025-0111 | tokio-tar | testcontainers | **No fix available**; bumping testcontainers 0.25 trades it for `astral-tokio-tar` ×4 (net worse), so not done |
| RUSTSEC-2025-0055 | tracing-subscriber 0.2 | mini-redis (dev) | fixed by replacing mini-redis (see §3) |
| unmaintained | structopt / atty / ansi_term | mini-redis (dev) | fixed by replacing mini-redis (see §3) |

---

## 2. hickory-proto 0.24 → 0.26 — deferral notes

**Why deferred:** 0.26 is a substantial DNS-API rewrite with high correctness stakes
(a wrong response code / id silently breaks resolution). It should be done in a
focused session with fast local iteration, not rushed on the remote build loop.
The exact breaking changes, already scoped, so the next attempt starts informed:

1. **`Message` construction** — no default `Message::new()`. Use:
   - `Message::response(id, op_code)` for answers
   - `Message::error_msg(id, op_code, response_code)` for error responses
   - `Message::new(id, message_type, op_code)` full form
2. **Header accessors removed** — `message.header()` is gone. `Message` now `Deref`s
   to `Metadata`; read `message.id`, `message.message_type`, `message.op_code`,
   `message.response_code` (fields via Deref). Setters (`set_id`, `set_message_type`,
   `set_response_code`) are gone — set via the constructor or `message.metadata.*`.
3. **`queries()` accessor changed** — confirm the new read accessor for the query
   list (the old `.queries()` method was removed; `add_query`/`add_queries` remain
   for writing).
4. **DNSSEC module moved** — `hickory_proto::rr::dnssec::…` → `hickory_proto::dnssec::…`
   (`sign.rs` import: `dnssec::{Algorithm, rdata::{DNSSECRData, RRSIG}}`).
5. **Cargo feature** — `features = ["dnssec", "dnssec-ring"]` → `["dnssec-ring"]`
   (the standalone `dnssec` feature was removed).
6. Bump in lockstep: `hickory-proto` (dnsd, tests/integration) **and**
   `hickory-resolver` (edgehub dev-dep) to 0.26.
7. **Gate:** dnsd unit tests + `dig`/`dot` integration tests must stay green;
   verify A/AAAA answers and RRSIG signing byte-for-byte.

Affected files: `crates/dnsd/src/dns_handler.rs` (Message build path, ~15 call
sites), `crates/dnsd/src/sign.rs` (DNSSEC), `crates/dnsd/tests/*`.

---

## 3. Dev-only advisory cleanup (optional, no production impact)

The dev-only advisories all trace to two test dependencies:
- **mini-redis** (dnsd, edgehub dev-deps) → tracing-subscriber 0.2, structopt,
  ansi_term, atty. Replace the in-process `mini_redis::server` test helper with a
  testcontainers Redis (already used elsewhere) or a maintained embedded redis.
- **testcontainers 0.24** → tokio-tar (no fix) and, if bumped to 0.25,
  astral-tokio-tar ×4. Best left until upstream testcontainers drops the tar dep.

Recommendation: once the above are unavoidable, add a documented `deny.toml` /
`.cargo/audit.toml` ignore list so CI audit stays green and only flags NEW or
SHIPPING advisories, with a comment per ID explaining why it's accepted.

---

## 4. Secret management (GitGuardian PR #70)

**Findings:** two "Generic Password" hits — `Tiltfile` (`PGPASSWORD: postgres`) and
`tests/integration/test_harness.rs` (already deleted; only in history at commit
6e40c2b). Both are **dev-default local credentials**, not production secrets.

**Full inventory of hardcoded dev credentials** (all local/dev defaults):
- `Tiltfile` — `PGPASSWORD: postgres` (db-init Job)
- `k8s-tilt/components/postgresql/helmrelease.yaml` — `postgresPassword: "postgres"`,
  `password: "fleetingdns"`
- `k8s-tilt/fleetingdns/api/deployment.yaml` — `DATABASE_URL: postgresql://fdns:fdns@…`
- `docker-compose.yml`, `docker/docker-compose.ci.yml`, `KIND-TILT-SETUP.md`,
  `docker/grafana/grafana.ini` — assorted dev defaults
- Test crates (config defaults, test_utils, migration) — test-only fixtures

**Implemented (2026-07-18): SOPS + profiles, mirroring `hauliage`.**
`deployment-configuration/profiles/dev/fleetingdns/core/` now holds `runtime/`
(namespace `fleetingdns`) and `bootstrap/` (namespace `data`, with the DB-init
Job moved out of the Tiltfile). Each has plaintext `application.properties`
(ConfigMap) and a SOPS-age-encrypted `application.secrets.env` (Secret), with
`disableNameSuffixHash: true` so `secretKeyRef` names stay stable. Repo-root
`.sops.yaml` carries the creation rules against the shared-k8s Flux age
recipient. Encrypt/edit only on ms02 with
`SOPS_AGE_KEY_FILE=~/.config/sops/age/flux-shared-gitops`. Verified: encrypt →
decrypt round-trip returns the value, and `kustomize build` renders the
ConfigMap/Secret/Job (Flux decrypts at apply time).

**Remaining (follow-on):** migrate the legacy `k8s-tilt/` manifests and
`docker-compose*.yml` literals to reference the generated Secrets; add a
pre-commit gitleaks/ggshield hook; add `staging`/`prod` profiles when needed.

**Original proposed direction (for reference):**
1. Introduce a `deployment-configuration/` (profiles) layout mirroring the `rerp`
   project's conventions — one directory per environment (local / ci / staging /
   prod), each with a plaintext `values.yaml` for non-secret config and a
   SOPS-encrypted `secrets.enc.yaml` for credentials.
2. Add `.sops.yaml` with the creation rules (age recipients per environment; agree
   whether keys live in the repo's `keys/` dir, in the cluster, or in a KMS).
3. Replace literal passwords in Tiltfile/k8s manifests with `valueFrom.secretKeyRef`
   pointing at a k8s Secret materialised from the decrypted SOPS file (Tilt can run
   `sops -d` at eval time; Flux has a SOPS decryption provider for GitOps).
4. History: the flagged commits contain only dev-default passwords, so rotation is
   moot, but if policy requires, scrub with `git filter-repo` on a coordinated push.
5. Add a pre-commit secret-scan hook (gitleaks/ggshield) so this is caught locally.

**Open questions for the team (blockers for implementation):**
- Which `rerp` conventions to mirror exactly (dir layout, file naming, age vs PGP)?
- Where do the age/PGP private keys live (repo, cluster secret, cloud KMS)?
- Is Flux the decryption point for deployed manifests, or is it Tilt-only for now?

Until the SOPS pipeline lands, the immediate low-risk step is to replace the
`Tiltfile` literal with a `secretKeyRef` (or `os.getenv` at eval time) so no
password string is committed.
