# ADR 001: Custom TCP Transport over rust-libp2p

- **Date:** 2026-06-05
- **Status:** Accepted

## Context

CRCI needs a transport to move messages between nodes. `rust-libp2p` was considered: it provides
discovery (mDNS, Kademlia), stream multiplexing, NAT traversal and Noise-encrypted, authenticated
connections. It also brings a large dependency tree, longer compile times, and a swarm/behaviour
programming model that the rest of the code would have to follow.

## Decision

Use a small custom transport on `tokio::net::TcpStream` behind a `Transport` trait
(`crates/crci-core/src/transport/`). Frames are a 4-byte big-endian length prefix followed by a JSON
payload (at most 1 MiB).

## Consequences

**Lost (must be built or accepted as missing):**
- Peer discovery and DHT routing.
- NAT traversal and relaying.
- Authenticated and encrypted connections. The custom transport has neither today: the sender identity is
  a self-declared `sender` field in each frame (an authenticated handshake is planned).

**Gained:**
- A transport small enough to read in full (`transport/tcp.rs`, about 440 lines including connection
  limits and timeouts).
- Direct control over framing, limits and error handling.
- Other media can implement the same trait (`ble.rs` and `lora.rs` are stubs today).

Moving to `rust-libp2p` later remains possible because callers only depend on the `Transport` trait.
