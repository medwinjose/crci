# CRCI — Current State

# Update this at the END of every session before committing.

# This is the single source of truth for where the project is right now.

# Last updated: End of Session 18


---

## Active Module Structure

```
src/
main.rs
identity.rs
message.rs
node.rs
network.rs
state.rs
replay.rs         — NEW: Lamport sequence, clock drift tolerance
bench.rs          — NEW: Benchmark harness (5 measurements)
crisis.rs         — Crisis scenarios (flood, earthquake, conflict, hazmat)
stress.rs         — Stress tests (Byzantine, Sybil, forgery, partition)
```

All modules introduced in Session 6. Module split is complete and stable.

---

## What Is Working and Stable

- Node identity with Ed25519 keypair generation (`identity.rs`)
- Message signing and signature verification (`identity.rs` + `message.rs`)
- SHA3-256 Merkle-chained state history (`state.rs`)
- Reputation engine with Byzantine detection (`node.rs`)
  - Floating-point threshold fix: use `> 0.41`, not `== 0.41`
- Gossip protocol with deduplication via seen-message tracking (`network.rs`)
- Structured signal input: severity, needs_help, can_help_others, location_confirmed
- Behavioral anomaly detection and consensus engine (`node.rs`)
- Zone-isolated consensus (penalties do not bleed across geographic zones)
- Confidence and visibility fields on signals
- One-tap panic button (auto-fills max emergency values)
- Persistent rescue request storage (survives round resets)
- Offline node detection with message adoption by peers
- MCE (Mass Casualty Event) declaration at 3+ rescue requests per zone
- Serde serialization across all message types
- rust-libp2p transport layer (Session 7 — integrated, verify compile status)

---

## Dependencies (Cargo.toml — Locked)

```toml
ed25519-dalek = "2"
rand = "0.8"
sha2 = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
rust-libp2p = "..."   # version locked in Session 7 — check Cargo.lock
```

Do not add dependencies without updating this file.

---

## Known Issues / Active Technical Debt

- [ ] Over-penalization bug: fixed in Session 3, monitor for regression in new penalty paths
- [ ] Stale observation carryover: fixed in Session 3, ensure zone isolation didn't reintroduce it
- [ ] `unwrap()` calls may still exist in early session code — audit needed before v1
- [ ] No persistent storage yet (planned: Session 10)
- [ ] Device-agnostic cross-platform support deferred — not addressed yet
- [ ] Broader security pass pending (Ed25519 landed but full security hardening not done)

---

## Recent Non-Obvious Decisions (Do Not Reverse Without Review)

- **Low-confidence / unknown-visibility nodes exempt from consensus penalties.**
  Reason: GPS ambiguity indoors causes false positives that punish honest reporters.
  This is intentional, not an oversight.

- **Floating-point threshold set to > 0.41 (not 0.4 or exact equality).**
  Reason: binary floating-point imprecision causes silent failures with exact comparisons.
  Apply this principle to any new threshold you introduce.

- **Rescue requests are persistent, not round-scoped.**
  Reason: a person in danger doesn't disappear when the gossip round resets.

- **Voice recognition explicitly rejected as an input method.**
  Reason: battery cost + noise unreliability in hostile environments.
  Do not reintroduce this without strong justification.

- **Peer proximity triangulation for GPS-denied location.**
  Reason: disaster environments often have GPS denial. Location is inferred from peer proximity.

---

## Session Roadmap

| Session | Status | Capability |
|---|---|---|
| 1–7 | Complete | Core (signatures, gossip, zones, MCE, persistence) |
| 8 | Complete | Mesh transport + distributed architecture |
| 10 | Complete | Encrypted local storage |
| 11–13 | Complete | Stress tests + adversarial hardening |
| 14–16 | Complete | Crisis scenarios + GPS + priority queue |
| 17 | Complete | Replay protection + Lamport sequence |
| 18 | Complete | Benchmark harness (5 measurements for paper) |
| **19** | **Next** | **Chaos engineering (packet loss, node crashes)** |

## Current Development Priority (Next Session)

**Session 19: Chaos Engineering**
Test system under 10% and 40% packet loss, simultaneous node restarts, reconnect storms.
One file: `src/chaos.rs` + call in `main()`.
---

## Current Development Priority (Next Session)

**Session 8+9: Mesh transport simulation + distributed node architecture**

Goal: Multiple real nodes communicating over simulated BLE/WiFi Direct/LoRa transports.
No more single-process simulation — actual distributed message passing between node instances.

Pre-session checklist:

- [ ] Confirm Session 7 (libp2p) compiles cleanly with no warnings
- [ ] Confirm all existing tests pass
- [ ] Run `rustfmt` on all files
- [ ] Commit current state with message format: `session-7: libp2p transport layer`

---

## Environment

- OS: Windows, VS Code
- Rust: 1.96.0
- Docker: 29.5.2
- Git: configured
- Repo: github.com/medwinjose/crci
- Local path: C:\Users\medwi\crci

---

## How to Update This File

At the end of every session:

1. Move the completed session row to "Complete" in the roadmap table.
2. Add any new known issues discovered.
3. Add any non-obvious decisions made this session to the decisions section.
4. Update "Current Development Priority" to the next session.
5. Update the dependency block if anything changed.
6. Commit this file alongside the session code.
