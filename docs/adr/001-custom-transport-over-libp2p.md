# ADR 001: Custom TCP Transport over rust-libp2p

## Date: 2026-06-05
## Status: Accepted

## Context
CRCI requires a reliable transport layer to gossip state and coordinate the network. Early in development, we considered integrating `rust-libp2p` as it is the industry standard for peer-to-peer networking, providing built-in protocols for mDNS discovery, Kademlia routing, multiplexing, and NAT traversal.

However, incorporating `rust-libp2p` introduces significant complexity. It adds approximately 50MB to the compiled binary size and drastically increases compile times. Furthermore, its event-driven macro-heavy architecture forces application logic to conform tightly to the libp2p swarm behavior, which creates a steep learning curve and obscures the underlying networking principles during the prototyping phase.

## Decision
We decided to implement a minimal, custom TCP-based async transport using `tokio::net` instead of adopting `rust-libp2p`.

The custom transport simply frames payloads using a 4-byte length prefix and relies on raw TCP streams for node-to-node connectivity, completely decoupling the GossipPipeline logic from the networking shim.

## Tradeoffs
**Lost:** 
- Free mDNS local network discovery.
- Kademlia distributed hash table (DHT) routing.
- Automatic NAT traversal (STUN/TURN) and relaying.
- Noise protocol encryption out-of-the-box.

**Gained:**
- Extreme simplicity: The entire transport layer is ~100 lines of code.
- Lean binaries: Fast compilation and minimal footprint, ideal for constrained edge devices.
- Direct control: We have explicit, transparent control over framing, connection management, and error handling without fighting framework abstractions.

## Future Path
This is a calculated prototyping decision. As the system matures toward production physical-layer deployment (e.g., across true LoRa links and public Internet bridges), the custom TCP transport will likely hit scaling and firewall limitations. Because the transport layer is cleanly isolated behind a shim that interfaces with `GossipPipeline::process()`, migrating to `rust-libp2p` in the future is fully documented as a planned upgrade path.
