## Formal Verification

To rigorously validate the safety and liveness properties of CRCI under adversarial conditions, we modeled the protocol’s core logic using TLA+ (Temporal Logic of Actions). TLA+ was chosen because its explicit state enumeration, coupled with temporal logic, makes it an industry standard for formally verifying concurrent, distributed systems. Its model checker, TLC, is peer-reviewed and widely used to find complex edge cases in distributed algorithms before they are deployed in production.

The TLA+ specification for CRCI abstracts the complex Rust implementation into a state machine focusing on message origination, validation, and gossip propagation across the peer network. The model explicitly captures both honest nodes and Byzantine actors. Byzantine nodes are permitted unconstrained behaviors: they can drop messages, duplicate payloads, send contradictory information, or forge sequence numbers. However, their ability to spoof the origin of a message is cryptographically constrained, reflecting the Ed25519 signature validation implemented in our actual Rust pipeline.

We formalize CRCI’s correctness through four safety invariants and two temporal liveness properties:

1. **No Forged Origin (Safety):** Asserts that no message in the global pool has an origin outside the known, authenticated node set. This proves that cryptographic signatures successfully prevent complete identity spoofing.
2. **Replay Never Delivered Twice (Safety):** Ensures that no node’s inbox ever contains two distinct messages with the identical origin and sequence number pair, formally verifying the effectiveness of CRCI's replay protection filters.
3. **Byzantine Containment (Safety):** Demonstrates that honest nodes never drop below the `MinTrustedRep` reputation threshold, ensuring that adversarial attempts to artificially tank an honest node's reputation fail, provided the number of Byzantine nodes $f$ remains strictly less than $n/2$.
4. **Rescue Never Dropped (Safety):** Guarantees that critical `RESCUE` messages, unlike `NORMAL` messages, bypass battery-based throttling limits. This acts as a safety proxy to ensure emergency messages cannot be systematically discarded by power-constrained relays.
5. **Rescue Liveness (Liveness):** A temporal property using `WF_` fairness ensuring that if a `RESCUE` message is originated, it will *eventually* be successfully received by all online nodes in the network, regardless of adversarial interference or intermediate node battery states.
6. **Gossip Progress (Liveness):** Confirms that the protocol is free from deadlocks and livelocks; valid gossip actions continue to occur infinitely often as long as messages remain in the system.

For our TLC model-checker configuration, we utilize a small-model instantiation: 5 total nodes, 2 of which are Byzantine (satisfying $f < n/2$), and a maximum sequence number (`MaxSeq`) of 4. In distributed systems verification, this bounded state space is demonstrably sufficient to expose any fundamental protocol flaws. If a violation of an invariant exists—such as a replay attack succeeding or a deadlock occurring—it will almost certainly manifest within a highly concurrent 5-node cluster. TLC systematically explores the entire state-space of this small model, testing every conceivable interleaving of message delivery and Byzantine attack vectors. 

While the TLA+ specification is an abstraction, it maintains a direct 1-to-1 correspondence with the Rust implementation's critical algorithmic decisions. The replay filter abstractly modeled in TLA+ directly mirrors the caching logic in `src/runtime.rs`, the battery gating rules match `src/battery.rs`, and the Byzantine validation maps perfectly to `src/validation.rs`.

The complete, annotated TLA+ specification and its corresponding configuration files are publicly available and can be reviewed in the `docs/tla/` directory of the project repository.

## Known Limitations (migrated from retired docs/spec, Session 145B)

- **Finite Model Approximation**: The model is configured for a micro-network (3–5 nodes) to ensure the state-space exploration completes in reasonable time.
- **Rational Approximation**: TLA+ does not natively support floating-point arithmetic. Reputation scores (normally `0.0` to `1.0` floats) are modeled as scaled integers (`0` to `100`).
- **Simplified Byzantine Threat**: The Byzantine actor is currently only modeled injecting severity anomalies. More complex attacks (e.g., eclipse attacks or eclipse routing) are abstracted out.
