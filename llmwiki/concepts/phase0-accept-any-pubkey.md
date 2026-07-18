---
title: Phase-0 "accept any pubkey" anti-pattern
kind: concept
status: active
tags: [auth, ssh, russh, anti-pattern, security]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
related:
  - entities/edf-cli.md
  - entities/edgehub.md
  - entities/edf-ca.md
---

# Phase-0 "accept any pubkey" anti-pattern

The current EdgeHub SSH server's `auth_publickey` callback returns
`Auth::Accept` for any client public key (Phase-0 dev shortcut). The CLI
in turn generates a fresh Ed25519 keypair on every `establish_tunnel`
call and presents that. The two things together hide a real bug:
**`SshKeyManager` is provisioned but never used.**

## Why this is dangerous (besides being insecure)

- The CA-signed key path (`SshKeyManager::get_or_request_key_pair`) is
  never exercised end-to-end, so any defect in it goes unnoticed until
  Phase-0 ends — at which point the fix is far away from the surface.
- "It worked yesterday" stops meaning anything: every CLI invocation
  uses a different key, so any auth-correlated bug is non-reproducible.
- Production hardening becomes a flag-flip in a hot path that has never
  been exercised under load.

## Fix (R8 of the postmortem)

In `cmd/edf-cli/src/ssh_client.rs::establish_tunnel`:

1. Acquire the keypair via `SshKeyManager::get_or_request_key_pair`.
2. Thread the resulting `KeyPair` through `TunnelClient::establish_tunnel`.
3. Remove the throwaway `russh::keys::PrivateKey::random(rng,
   Algorithm::Ed25519)` call.

On the server (defer until R8 lands):

1. Replace blanket `Auth::Accept` with a `Auth::PublicKey { ... }` check
   that validates the presented key against
   [`edf-ca`](../entities/edf-ca.md)'s issued cert chain and the Redis
   `tunnel.client_pubkey_fingerprint`.
2. Gate the blanket-accept behaviour behind a single env flag (e.g.
   `EDGEHUB_ACCEPT_ANY_KEY=1`), default off, never used in CI assertions.

## Detection in the wild

If you see logs like:

```
auth_publickey accepted (key fingerprint = SHA256:...)
```

with a different fingerprint on every invocation from the same CLI host,
Phase-0 is still in effect. The fix has not landed.
