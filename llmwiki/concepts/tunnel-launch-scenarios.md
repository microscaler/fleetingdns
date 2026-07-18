---
title: Tunnel launch scenarios (desktop CLI, FAR VM, k8s)
kind: concept
status: current
sources: [owner direction 2026-07-06]
---

# Tunnel launch scenarios

Owner direction (2026-07-06): there are two scenarios in which a tunnel
is launched today, and one anticipated:

1. **Developer desktop via `edf-cli`.** A human developer runs the CLI
   on their own machine. The CLI calls the control API (create tunnel →
   API-issued slot + capability subdomain), opens SSH to the edge, and
   prints the capability URL. Humans view the URL in a browser, so this
   audience is the reason production needs a **publicly trusted**
   wildcard cert (browsers must not warn). `TeardownPolicy::ViewerIdle`
   exists for this audience's portal-tab usage.
2. **Inside a FAR VM (automation).** far-tunneld / agents stand tunnels
   up headlessly — the original Tilt-on-ms02 use case. This is why
   `TeardownPolicy::TtlOnly` is the default (deterministic for agents
   cycling Playwright browsers between iterations) and why the hub must
   interop with plain OpenSSH clients (127.0.0.1 slot-bind fix). Gap:
   machine credentials — a VM has no human for a GitHub OAuth dance, so
   tunnel creation needs a service token / pre-provisioned key.
3. **Future: k8s dev deployments.** The CLI (or a small sidecar) in a
   pod with secret-mounted credentials. Same protocol; nothing in the
   current design blocks it.

## Design invariant

All scenarios share ONE tunnel protocol. The only legitimate
per-scenario differences are (a) how the client authenticates to the
control API and (b) who needs to trust the edge cert. Keep the hub and
edge router client-agnostic: any scenario-specific branching in
`edgehub` is a design smell.

## Related

- [`capability-url-access-model.md`](./capability-url-access-model.md)
  — the viewer-side access model shared by all scenarios.
- [`redis-slot-allocation.md`](./redis-slot-allocation.md) — the API
  issues slots regardless of who the client is.
