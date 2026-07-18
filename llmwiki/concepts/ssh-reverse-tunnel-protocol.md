---
title: SSH reverse-tunnel protocol (`tcpip-forward` vs `direct-tcpip`)
kind: concept
status: active
tags: [ssh, russh, rfc4254, reverse-tunnel, root-cause]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
  - sources/tasks-connectivity.md
  - sources/e2-tunnel-design.md
related:
  - entities/edgehub.md
  - entities/edf-cli.md
  - concepts/forwarded-tcpip-channel.md
  - concepts/russh-handle-vs-channelid.md
---

# SSH reverse-tunnel protocol (`tcpip-forward` vs `direct-tcpip`)

The category-error at the heart of the FleetingDNS reverse-tunnel
incident. RFC 4254 defines two distinct port-forwarding primitives.
**FleetingDNS needs the second; today it implements the first.**

| Purpose | Primitive | Initiator | Data direction |
|---|---|---|---|
| Local forwarding (`ssh -L`) | `direct-tcpip` channel | client opens channel | client → server → target TCP host |
| **Remote forwarding** (`ssh -R`) | `tcpip-forward` global request + `forwarded-tcpip` channel | client sends global request; **server** opens the channel on each inbound TCP connection | external → server:port → client → localhost |

The PRD is explicit (cf.
[prd-ephemeral-dns-forwarder-v1.1](../sources/prd-ephemeral-dns-forwarder-v1.1.md)):

> CLI spawns russh client — opens an SSH **reverse-port** `0.0.0.0:hub_slot` on **hub**.

## How the bug currently expresses itself

- `cmd/edf-cli/src/ssh_client.rs:210` calls
  `handle.channel_open_direct_tcpip("127.0.0.1", allocated_port,
  "127.0.0.1", 0)` — semantically: *"server, please dial outbound to
  `127.0.0.1:<allocated_port>` and splice it to this channel."*
- `crates/edgehub/src/ssh_server.rs:1075` accepts the channel, files
  metadata in `active_tunnels`, and spawns a 30-second sleep loop —
  **never** binds a `TcpListener`, **never** dials anywhere.

External callers therefore reach an EdgeHub HTTPS router that, on a
match, returns a hard-coded `"Tunnel {id} is active for {sni}"` body
instead of forwarding bytes anywhere.

## The correct shape

### Client (`cmd/edf-cli/src/ssh_client.rs`)

```rust
handle.tcpip_forward("0.0.0.0", allocated_port).await?;

// And implement on the russh client Handler:
async fn server_channel_open_forwarded_tcpip(
    &mut self,
    channel: Channel<Msg>,
    connected_address: &str, connected_port: u32,
    originator_address: &str, originator_port: u32,
    session: &mut Session,
) -> Result<(), Self::Error> {
    let (mut ssh_stream, _) = channel.into_stream().split();
    let mut local = TcpStream::connect(("127.0.0.1", local_port)).await?;
    tokio::io::copy_bidirectional(&mut ssh_stream, &mut local).await?;
    Ok(())
}
```

### Server (`crates/edgehub/src/ssh_server.rs`)

Implement `tcpip_forward` on `impl Handler for SshSession`:

1. `let listener = TcpListener::bind(("0.0.0.0", *port)).await?;` (port=0
   → pick one and update `*port` so the response carries the chosen
   port).
2. Clone `session.handle()` into the listener task — see
   [russh-handle-vs-channelid](./russh-handle-vs-channelid.md).
3. For each accepted TCP connection:
   `handle.channel_open_forwarded_tcpip(connected_address, connected_port,
   originator_address, originator_port).await?` to open a
   [`forwarded-tcpip`](./forwarded-tcpip-channel.md) channel back to the
   CLI. Then `tokio::spawn(copy_bidirectional(accepted_tcp,
   channel.into_stream()))`.
4. Track listener + channel-set in `SshServerState::reverse_proxy_state`
   so `cancel_tcpip_forward` and session-close can clean up.
5. `ReverseProxyState::register_tunnel_route(subdomain, port)` writes the
   Redis routing entry consumed by the edge HTTPS router.

## Acceptance test (R9)

`crates/edgehub/tests/e2e_reverse_tunnel_http.rs`: start a real
`SshServer` on `127.0.0.1:0`, start a fake echo HTTP server on
`127.0.0.1:0`, run the CLI's `TunnelClient` to register + forward, then
`curl` the server-side `allocated_port` and assert the echoed body is
received. **This is the test that would have caught the bug in PR #52.**

## Don't (anti-patterns)

- Don't accept `direct-tcpip` and call it "reverse port forwarding". It
  isn't.
- Don't write a TCP listener with only a `ChannelId` in scope.
- Don't store the allocated port client-side from UUID bytes (cf.
  [redis-slot-allocation](./redis-slot-allocation.md)).
