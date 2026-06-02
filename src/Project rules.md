# CRCI — Project Rules
# Read this before every session. Every AI system and every developer must follow these.
# Last updated: Session 7 complete

---

## What This Project Is

CRCI (Crisis Response Communication Infrastructure) is a decentralized, Byzantine-fault-tolerant
mesh communication system for use when internet and cellular infrastructure is unavailable.
It is written in Rust. It has no central server. It is fully peer-to-peer.

Target environments: disaster zones, warfare, infrastructure collapse.
Target devices: phones, tablets, older hardware (device-agnostic, deferred to later session).

---

## Hard Architecture Constraints

These are non-negotiable. Do not violate them. Do not propose alternatives without explicit approval.

### Runtime
- Tokio is the only async runtime. Never introduce a second runtime.
- No blocking calls on async threads. Use `spawn_blocking` if blocking I/O is unavoidable.
- No `std::thread::sleep` inside async contexts. Use `tokio::time::sleep`.

### Language / Dependencies
- Rust only. No FFI unless explicitly approved.
- Minimize external dependencies. Every new crate requires justification.
- Approved cryptographic dependencies: `ed25519-dalek v2`, `rand v0.8`, `sha2 v0.10`.
- Do not introduce alternative crypto crates. Do not reimplement crypto primitives.

### Networking
- Target transports: BLE, WiFi Direct, LoRa/Meshtastic (layered).
- Planned transport library: `rust-libp2p`. Introduced in Session 7.
- No central server. No DNS-based discovery. No hardcoded peer addresses.

### Serialization
- `serde` with `serde_json` or `bincode` only.
- All network messages must be serializable and deserializable without ambiguity.

### Security
- Every node has an Ed25519 keypair generated at identity creation.
- Every message must be signed by its originating node.
- Signature verification is mandatory before processing any inbound message.
- Do not skip signature checks for "testing convenience." Use test keypairs instead.
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

If a feature requires crossing two modules, add a well-defined interface, not a shortcut.

---

## Invariants That Must Always Hold

These are system-level guarantees. If any change risks breaking these, stop and flag it.

1. **No message is processed without signature verification.**
2. **Reputation scores are always in range [0.0, 1.0].** Clamp on write, never let them drift.
3. **Zone isolation is enforced.** Consensus penalties must never bleed across geographic zones.
4. **Low-confidence and unknown-visibility nodes are exempt from consensus penalties.**
   Reason: indoor/outdoor GPS ambiguity causes false positives. This is a deliberate design decision.
5. **Rescue requests survive round resets.** They are persistent, not ephemeral.
6. **Dead device messages are adopted by nearby peers.** Message continuity is guaranteed.
7. **MCE (Mass Casualty Event) is declared only when 3+ rescue requests cluster in the same zone.**
8. **The Merkle chain is append-only.** Never rewrite history.
9. **Floating-point comparisons use thresholds, never exact equality.**
   Established in Session 1: use `> 0.41` not `== 0.41`. Apply this principle everywhere.

---

## Forbidden Patterns

Do not use these. If you see them in existing code, flag them as technical debt.

- `unwrap()` in production paths. Use `?`, `expect()` with a message, or proper error handling.
- `clone()` on large data structures inside hot loops.
- Hardcoded peer addresses or node IDs.
- Global mutable state outside of clearly owned modules.
- `unsafe` blocks without explicit justification and review.
- Synchronous blocking inside async functions.
- Catching all errors with `_` and silencing them.
- Magic numbers without named constants.

---

## Coding Standards

- All public functions have doc comments.
- All error types are explicit (`thiserror` or custom enums). No stringly-typed errors.
- Tests cover at least: happy path, boundary conditions, known edge cases from past bugs.
- No `todo!()` macros committed to main without a corresponding tracked TODO.
- Format with `rustfmt` before every commit. No exceptions.

---

## Design Principles (Ordered by Priority)

1. **Correctness over performance.** Get it right first.
2. **Simplicity over cleverness.** If it needs a long comment to explain, simplify it.
3. **Explicit over implicit.** Make state transitions, ownership, and failure modes visible.
4. **Resilience over optimization.** The system must degrade gracefully under adversarial conditions.
5. **No feature added that cannot be tested offline.** Everything must be simulatable.

---

## What Requires Human Approval Before Implementation

- Any change to the gossip propagation algorithm.
- Any change to reputation score calculation or penalty logic.
- Any change to zone boundary logic.
- Any new cryptographic operation or key management decision.
- Any change to the Merkle chain structure.
- Any new external crate.
- Any change to module boundaries.

---

## AI Usage Rules (For Any Model Reading This)

- Read this file and CURRENT_STATE.md before proposing any code or design.
- Do not invent new architectural patterns not established in this document.
- Do not remove existing safety checks to make tests pass.
- If a constraint here conflicts with what the user is asking for, flag the conflict explicitly.
- When in doubt, implement less and ask.