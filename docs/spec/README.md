# CRCI TLA+ Formal Specification

This directory contains the formal TLA+ specification for CRCI's reputation-weighted consensus protocol, developed in Session 36.

## Scope of the Model

**What this specification models:**
- **Nodes and Reputation:** The network is modeled as a finite set of honest and Byzantine nodes, each possessing a tracked reputation score.
- **Quorum Logic:** A dynamic consensus threshold where voting power is proportional to a node's reputation weight.
- **Byzantine Behavior:** Malicious nodes are non-deterministically allowed to inject messages with false severity levels (e.g., oscillating between 1 and 5).
- **Reputation Penalties & Isolation:** The systemic response to Byzantine behavior, applying fixed reputation decrements to malicious nodes until they fall below the isolation threshold and are functionally exiled from quorum calculations.

**What is intentionally omitted:**
This specification strictly models the *mathematical correctness of the consensus engine*. It intentionally abstracts away lower-level implementation details, including:
- **Transport and Networking:** Asynchronous TCP, K-bucket routing, and packet propagation delays.
- **Cryptography:** Ed25519 signature generation and verification.
- **Gossip Layer:** The actual pipeline, TTL storage, rate limiting, and replay caches.

## How to Run the Model Checker

To mathematically verify the safety and liveness invariants across all possible state permutations, run the TLC model checker locally using the provided configuration file:

```bash
tlc crci_consensus.tla -config MC.cfg
```

## Properties Asserted

The model checker continuously evaluates the system against two critical formal properties:

1. **Safety Property (`SafetyInvariant`)**: Asserts that a false-severity message injected by a Byzantine actor can *never* achieve a valid network quorum, provided that the total reputation weight of the Byzantine actors remains strictly below the 1/3 theoretical threshold.
2. **Liveness Property (`LivenessProperty`)**: Asserts that under normal operational conditions (with a sufficient number of active honest nodes), a valid message originated by an honest node will *always eventually* successfully reach quorum.

## Known Limitations

- **Finite Model Approximation**: The model is configured for a micro-network (3–5 nodes) to ensure the state-space exploration completes in reasonable time. 
- **Rational Approximation**: TLA+ does not natively support floating-point arithmetic. Reputation scores (normally `0.0` to `1.0` floats) are modeled as scaled integers (`0` to `100`).
- **Simplified Byzantine Threat**: The Byzantine actor is currently only modeled injecting severity anomalies. More complex attacks (e.g., eclipse attacks or eclipse routing) are abstracted out.

## Next Steps

With the mathematical foundation verified, the system proceeds to broader external integration.
**[Session 37: REST API + WebSocket Backend](../CURRENT_STATE.md)** — We will extend CRCI with a RESTful interface (`src/api.rs` via Axum/Warp) to expose `/status`, `/peers`, and `/messages` endpoints, alongside a real-time WebSocket broadcast channel for the upcoming dashboard.
