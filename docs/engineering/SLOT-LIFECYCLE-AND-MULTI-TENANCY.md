# Slot lifecycle and multi-tenancy

**Status: current as of 2026-07-29.** Derived from code review of
`backendapi/storage.rs` (`allocate_port`, `release_port`), `edgehub/ssh_server.rs`
(`tcpip_forward`, `Drop for SshSession`), `edgehub-bin/main.rs` (router), and
`common/redis/tunnel.rs`.

This document exists because the intended slot design was never written down and
is only partly implemented. Reading the code alone, it is easy to conclude the
current behaviour *is* the design.

---

## 1. Intended design

A slot is a TCP port on the hub that an external request is spliced into. The
intent was:

1. **The system allocates a port dynamically as part of the CLI handshake** —
   the developer's client connects, and the port is chosen at that moment.
2. **Redis holds a record of every connection and every port in use.**
3. **Standing up a connection is validated by marking port state**, moving
   through something like `available → reserved (waiting for connection) →
   connected → released`, so the platform can always distinguish a port that is
   merely claimed from one that is actually carrying traffic.

The point of (3) is that reservation and use are different things, and only a
state machine can tell them apart.

## 2. What is actually implemented

| Intent | Reality |
|---|---|
| Port chosen at CLI handshake | Port is chosen **earlier**, by the API at `POST /v1/tunnels` (`allocate_port`), returned as `slot`, and the CLI then asks the hub to bind that exact port. The hub never chooses. |
| Redis records every port in use | Partly. `port:{n}` → `tunnel_id`, claimed atomically with `SET NX EX` (the TOCTOU race of a prior `GET`-then-`SET` was already fixed and is documented in the code). |
| Port state machine | **Not implemented.** `port:{n}` is binary: present (claimed) or absent (free). There is no "waiting for connection" vs "connected" vs "draining". |
| Ports released when finished | **`release_port` exists but is never called.** Claims disappear only when their TTL (`certificate_ttl`, 30 min default, 2 h max) expires. |
| Liveness of a slot | Added 2026-07-29: the hub publishes `tunnel:live:{tunnel_id}` while it holds a bound listener (15 s refresh, 45 s TTL). This is effectively the *connected* state of the intended machine — but it is keyed by tunnel id, not port, and is not part of a formal lifecycle. |

Ownership was tightened on 2026-07-29: `tcpip_forward` now requires the
authenticated session to own the slot's tunnel record (see §4.1).

## 3. Consequences at scale

These matter specifically for "hundreds of tunnels from different developers".

**3.1 Abandoned reservations hold inventory.** A developer who calls
`POST /v1/tunnels` and never connects (crash, `Ctrl-C`, lost network) holds
`port:{n}` for the full certificate TTL. Nothing distinguishes that port from a
busy one. A short "waiting for connection" timeout is exactly what the intended
state machine provides and what its absence costs.

**3.2 Explicit deletes do not return inventory.** `delete_tunnel` removes the
tunnel record but never calls `release_port`, so a deleted tunnel's port stays
claimed until TTL.

**3.3 Slot reuse is a cross-tenant hazard, and the naive fix makes it worse.**
It is tempting to "fix" 3.2 by calling `release_port` on delete. Do not do this
unconditionally. The hub keeps its listener until the SSH session drops, which is
not the same moment the API deletes the record. Releasing the claim early lets
the API hand the same port to a different developer, whose record then says
`slot = n` while the hub's listener on `n` still belongs to the previous tenant —
the router would splice the new tenant's inbound traffic into the old tenant's
SSH channel. Returning a port to the pool is only safe once the hub has
confirmed the listener is gone, which again requires the state machine.

**3.4 Reserved-vs-live is not diagnosable.** Answering "how many slots are
actually carrying traffic?" currently means joining `port:*` against
`tunnel:live:*` by hand.

**3.5 Per-user quotas are not enforced on the create path.**
`enforce_concurrent_tunnel_quota` and friends exist in `quota_enforcement.rs` but
are `#[allow(dead_code)]` and unreferenced, so one developer can allocate
tunnels until the pool is exhausted. Port space itself is ample (55,535), but
inventory is not the binding constraint — see 3.6.

**3.6 The hub cannot scale horizontally.** Slots are bound on `127.0.0.1` inside
the hub pod and the router in that same pod dials `127.0.0.1:slot`. With more
than one replica, an inbound request can land on a pod that does not hold the
tunnel's listener and will simply fail. The chart pins `edgehub.replicas: 1`,
which encodes this. Hundreds of tunnels therefore all terminate in one process:
file descriptors, CPU and two tasks per slot (accept loop + liveness heartbeat)
are the real ceiling, and there are no per-slot or global connection caps yet
(TDP-15, open).

## 4. Multi-tenant isolation

**4.1 Slot ownership (fixed 2026-07-29).** `tcpip_forward` previously accepted
any slot that had *a* tunnel record. Since TDP-13 authenticates the session
against an API-issued key, that check was far too weak: an authenticated
developer could scan for allocated slots and bind another developer's, after
which the router would splice that subdomain's traffic into the attacker's SSH
channel — interception and impersonation. The hub now requires the authenticated
session id to equal the slot's tunnel id, and `e2e_slot_allocation_gate` asserts
the denial.

Note the test that should have caught this authenticated as username `"gate"`
while the record id was `"gate-test-1"` — it passed only because the check it was
meant to exercise did not exist. Tests in this area must authenticate as
`tunnel-{tunnel_id}`, the way the CLI does.

**4.2 Subdomain uniqueness** is enforced at create time
(`is_subdomain_available`), which is what keeps SNI routing deterministic.

**4.3 Traffic is not metered per tunnel**, deliberately — see `common::billing`
and D-8 in the E2/E3 story doc.

## 5. Target state machine

Recommended shape, smallest first:

1. **`reserved` with a short TTL** on `POST /v1/tunnels` (a connect deadline,
   not the certificate lifetime), so abandoned reservations return quickly.
2. **`connected`** written by the hub when it binds the listener — generalise the
   existing `tunnel:live:{id}` heartbeat to a per-slot state value.
3. **`draining` / hub-confirmed release**: the API marks intent to release; the
   port returns to the pool only after the hub confirms teardown (or its
   liveness lapses). This closes 3.3.
4. **Enforce the per-user concurrent-tunnel quota** on the create path (3.5).
5. **Per-slot and global connection caps** (TDP-15) so one tunnel cannot starve
   the shared hub.
6. **Multi-replica routing** (3.6) is a separate architectural decision: either
   route inbound traffic to the pod holding the slot (hub identity in the tunnel
   record), or move the splice off pod-local loopback.

Items 1–4 are incremental. Item 6 changes the deployment model and should be its
own design.
