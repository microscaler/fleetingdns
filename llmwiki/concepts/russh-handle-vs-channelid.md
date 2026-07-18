---
title: russh::server::Handle vs ChannelId
kind: concept
status: active
tags: [ssh, russh, api-shape, anti-pattern]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
related:
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/forwarded-tcpip-channel.md
  - entities/edgehub.md
---

# russh::server::Handle vs ChannelId

A subtle russh API contract that is easy to get wrong, and was a major
contributor to the reverse-tunnel bug.

## The two types

- **`russh::ChannelId`** — opaque numeric channel identifier. Cheap to
  copy, but on its own it is **not** a sink: you cannot push bytes into
  the SSH channel given only a `ChannelId`. It only identifies the
  channel within a session.
- **`russh::server::Handle`** — `Clone + Send + 'static` async sink for
  a session. Obtained via `session.handle()` inside a `Handler` callback.
  Use it to:
  - open new channels back to the client
    (`channel_open_forwarded_tcpip`, `channel_open_session`, ...),
  - write to existing channels (`data(channel_id, payload)`),
  - close channels (`close(channel_id)`),
  - send disconnects.

## The bug pattern

`start_tunnel_port_listener` in `crates/edgehub/src/ssh_server.rs`
takes a bare `ChannelId`. Because it has no sink, the only thing it can
do is write a stub HTTP 200 reply directly to the inbound TCP socket. It
**cannot** forward bytes into the SSH channel and onward to the CLI.
This is the "wrong mental model" baked into the function signature.

The compiler accepts it; russh runtime accepts it; the function "works"
in unit tests that only check for a 200. The product just doesn't.

## The correct pattern

Anywhere a long-lived task needs to write into the SSH session from
outside the immediate `Handler` callback, **store
`Arc<russh::server::Handle>`**, not a `ChannelId`:

```rust
async fn tcpip_forward(
    &mut self, address: &str, port: &mut u32, session: &mut Session,
) -> Result<bool, Self::Error> {
    let handle: russh::server::Handle = session.handle();
    let listener = TcpListener::bind((address, *port as u16)).await?;
    tokio::spawn(async move {
        loop {
            let (mut tcp, peer) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let handle = handle.clone();
            tokio::spawn(async move {
                let channel = handle
                    .channel_open_forwarded_tcpip(
                        address.to_string(), *port,
                        peer.ip().to_string(), peer.port().into(),
                    )
                    .await?;
                let mut stream = channel.into_stream();
                tokio::io::copy_bidirectional(&mut tcp, &mut stream).await
            });
        }
    });
    Ok(true)
}
```

## Method placement (corollary)

The only place with access to the live SSH session — and therefore to
the handle — is the `Handler` impl on `SshSession`. The current code
defines `register_reverse_tunnel` on `SshServer`, which has no handle,
so the function is callable only from unit tests. **Put the method on
the object that has the state it needs.**
