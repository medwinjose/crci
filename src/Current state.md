# CRCI — Current State

# Update this at the END of every session before committing.

# This is the single source of truth for where the project is right now.

# Last updated: End of Session 30

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
replay.rs         — Lamport sequence, clock drift tolerance
bench.rs          — Benchmark harness (5 measurements)
bench_extended.rs — NEW: Extended benchmark harness (Session 28)
chaos.rs          — Chaos engineering scenarios
crisis.rs         — Crisis scenarios (flood, earthquake, conflict, hazmat)
stress.rs         — Stress tests (Byzantine, Sybil, forgery, partition)
validation.rs     — NEW: Input validation and rate limiting
aeda.rs           — NEW: Autonomous emergency decision architecture
battery.rs        — NEW: Battery-aware gossip throttling
ttl.rs            — NEW: Message TTL enforcement and storage pruning
integration.rs    — NEW: Gossip pipeline integrating all subsystems (Session 24)
security.rs       — NEW: STRIDE security hardening and safety features (Session 25)
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
- Chaos engineering scenarios: packet loss, node crashes, reconnect storms (Session 19)
- Input validation, rate limiting, and panic cooldowns (Session 21)
- Autonomous Emergency Decision Architecture (AEDA) (Session 22)
- Battery-aware gossip throttling and dynamic drain rates (Session 23)
- Message TTL enforcement, bounded storage pruning, and tombstones (Session 23)
- Integration layer wiring validation, AEDA, battery, and TTL (Session 24)
- STRIDE security hardening: opaque Node IDs, payload audits, signed audit log, zone membership vouching, reputation-weighted MCE (Session 25)
- Safety features: GOODBYE signal, priority message queue, rescue resolution (Session 25)
- Sequence overflow protection actively preventing long-term replay attacks (Session 26)
- Safe mutex and serialization execution — zero unwrap panics in production logic (Session 26)
- Node discovery protocol: beaconing, peer exchange, and zone bootstrap (Session 27)
- Extended quantitative benchmarks (TTL pruning, rate limit, discovery convergence, AEDA latency, queue throughput, pipeline throughput) (Session 28)
- Debug audit pass: upgraded TTL tombstones to O(1) bounds-checked storage, removed orphaned code, silenced dead code warnings (Session 28)
- Zone Registry pipeline integration: rejects unverified normal messages while allowing emergencies (Session 29)
- Thread-safe validation Rate Limiter via Arc<Mutex> wrapper (Session 29)
- Docker Compose foundation for 40-node proof loop (Session 29)
- TCP Gossip Transport layer shim (`src/transport.rs`) (Session 30)
- Standalone Node Entrypoint binary (`src/bin/node.rs`) exposing Prometheus metrics on port 9090 and running peer severity consensus (Session 30)
- Fixed Benchmark 6 TTL pruning metrics to naturally expire messages using configurable storage capacity (Session 30)


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
- [x] `unwrap()` calls may still exist in early session code (Audit completed, unwraps removed from production paths in Session 26)
- [ ] No persistent storage yet (planned: Session 10)
- [ ] Device-agnostic cross-platform support deferred — not addressed yet
- [x] Broader security pass pending (Completed Session 25 - STRIDE hardening)
- [x] Performance and bounds-check audit (Completed Session 28 - TTL storage O(1) fix)

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

- **Reputation-weighted MCE threshold.**
  Reason: Coordinated low-rep nodes could spam rescues to trigger false MCEs. Nodes with rep < 0.5 now count as 0.

- **Zone claim vouching.**
  Reason: To prevent zone spoofing, nodes claiming a zone must be vouched for by at least one existing verified member of that zone, unless they are the bootstrap node.

- **Peer exchange limit set to 8 nodes.**
  Reason: LoRa packet size constraints prevent sharing arbitrarily large peer tables.

- **Tombstone storage uses bounded VecDeque + HashSet.**
  Reason: O(N) array shifts caused a performance loophole during massive TTL expirations. This provides O(1) lookups and O(1) bounded evictions.

---

## Session Roadmap

| Session | Status   | Capability                                         |
| ------- | -------- | -------------------------------------------------- |
| 1–7     | Complete | Core (signatures, gossip, zones, MCE, persistence) |
| 8       | Complete | Mesh transport + distributed architecture          |
| 10      | Complete | Encrypted local storage                            |
| 11–13   | Complete | Stress tests + adversarial hardening               |
| 14–16   | Complete | Crisis scenarios + GPS + priority queue            |
| 17      | Complete | Replay protection + Lamport sequence               |
| 18      | Complete | Benchmark harness (5 measurements for paper)       |
| 19      | Complete | Chaos engineering (packet loss, node crashes)      |
| 21      | Complete | Input validation + rate limiting                   |
| 22      | Complete | Autonomous Emergency Decisions (AEDA)              |
| 23      | Complete | Battery-aware throttling & TTL enforcement         |
| 24      | Complete | Integration pipeline + bug fixes                   |
| 25      | Complete | STRIDE security hardening + safety features        |
| 26      | Complete | Debug audit, logic fixes, & unwrap cleanup         |
| 27      | Complete | Node discovery protocol (beacon + exchange)        |
| 28      | Complete | Extended benchmarks for quantitative paper claims  |
| 29      | Complete | Zone registry wired + Docker Proof Loop foundation |
| 30      | Complete | TCP transport, Docker entrypoint, and Proof Loop   |
| **31**  | **Next** | **Tokio Async Transport Migration**                |

## Current Development Priority (Next Session)

**Session 31: Tokio Async Transport Migration**
Migrate the minimal std::net TCP networking transport to full Tokio async networking to improve concurrency and remove the OS thread-per-connection model.

Pre-session checklist:

- [ ] Confirm all existing tests pass
- [ ] Run `rustfmt` on all files
- [ ] Commit current state with message format: `session-31: next steps`

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
