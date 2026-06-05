# Contributing to CRCI

Welcome to the CRCI (Crisis Response Communication Infrastructure) project! This document outlines the guidelines and constraints for contributing to the codebase.

## What This Project Is

CRCI is a decentralized, Byzantine-fault-tolerant mesh communication system for use when internet and cellular infrastructure is unavailable. It is written in Rust, has no central server, and is fully peer-to-peer.

Target environments: disaster zones, warfare, infrastructure collapse.
Target devices: phones, tablets, older hardware.

---

## Hard Architecture Constraints

These are core technical constraints of the project. Please adhere to them when submitting pull requests.

### Runtime
- Tokio is the only async runtime. Never introduce a second runtime.
- No blocking calls on async threads. Use `spawn_blocking` if blocking I/O is unavoidable.
- No `std::thread::sleep` inside async contexts. Use `tokio::time::sleep`.

### Language / Dependencies
- Rust only. No FFI unless explicitly approved.
- Minimize external dependencies. Every new crate requires justification.
- Approved cryptographic dependencies: `ed25519-dalek v2`, `rand v0.8`, `sha2 v0.10`.
- Do not introduce alternative crypto crates or reimplement crypto primitives.

### Networking
- Target transports: TCP (async), BLE, WiFi Direct, LoRa/Meshtastic (layered).
- No central server. No DNS-based discovery. No hardcoded peer addresses.

### Serialization
- `serde` with `serde_json` or `bincode` only.
- All network messages must be serializable and deserializable without ambiguity.

### Security
- Every node has an Ed25519 keypair generated at identity creation.
- Every message must be signed by its originating node.
- Signature verification is mandatory before processing any inbound message.
- SHA3-256 Merkle-chained state history must remain intact. Do not break the chain.

---

## Module Boundaries

Each module owns its domain. Do not reach across module boundaries directly.

| Module | Owns | Must NOT |
|---|---|---|
| `identity` | Node keypair, NodeId, signing, verification | Touch network or state |
| `message` | Message types, serialization, signature fields | Generate keys or touch transport |
| `node` | Node struct, reputation, behavioral logic | Own network I/O |
| `network` | Transport, peer discovery, gossip propagation | Make reputation decisions |
| `state` | State history, Merkle chain, persistence | Own message types |
| `main` | Wiring only | Contain business logic |

If a feature requires crossing two modules, add a well-defined interface.

---

## Invariants That Must Always Hold

These are system-level guarantees. If any change risks breaking these, flag it in your pull request.

1. **No message is processed without signature verification.**
2. **Reputation scores are always in range [0.0, 1.0].** Clamp on write, never let them drift.
3. **Zone isolation is enforced.** Consensus penalties must never bleed across geographic zones.
4. **Low-confidence and unknown-visibility nodes are exempt from consensus penalties.**
5. **Rescue requests survive round resets.** They are persistent, not ephemeral.
6. **Dead device messages are adopted by nearby peers.** Message continuity is guaranteed.
7. **MCE (Mass Casualty Event) is declared only when 3+ rescue requests cluster in the same zone.**
8. **The Merkle chain is append-only.** Never rewrite history.
9. **Floating-point comparisons use thresholds, never exact equality.** (e.g., use `> 0.41` not `== 0.41`).

---

## Forbidden Patterns

Avoid these patterns in your contributions:

- `unwrap()` in production paths. Use `?`, `expect()` with a clear message, or proper error handling.
- `clone()` on large data structures inside hot loops.
- Hardcoded peer addresses or node IDs.
- Global mutable state outside of clearly owned modules.
- `unsafe` blocks without explicit justification and review.
- Synchronous blocking inside async functions.
- Catching all errors with `_` and silencing them.
- Magic numbers without named constants.

---

## Coding Standards

- All public functions must have doc comments.
- All error types should be explicit (`thiserror` or custom enums). No stringly-typed errors.
- Tests should cover at least: the happy path, boundary conditions, and known edge cases.
- Format your code with `cargo fmt` before every commit.
- Ensure `cargo clippy -- -D warnings` passes without errors.

---

## Design Principles (Ordered by Priority)

1. **Correctness over performance.** Get it right first.
2. **Simplicity over cleverness.** If it needs a long comment to explain, simplify it.
3. **Explicit over implicit.** Make state transitions, ownership, and failure modes visible.
4. **Resilience over optimization.** The system must degrade gracefully under adversarial conditions.
5. **No feature added that cannot be tested offline.** Everything must be simulatable.

## How to Build and Test

To build the project:
```bash
cargo build
```

To run all validations (highly recommended before submitting a PR):
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
