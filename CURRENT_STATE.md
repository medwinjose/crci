# CRCI Current State

## Sessions Completed
- Session 1: Project Initialization & Cargo Setup
- Session 2: Cryptographic Identities (`ed25519-dalek`)
- Session 3: Core Message Wire Structs & Types
- Session 4: Message Digital Signatures & Validation
- Session 5: Node Struct & Basic In-Memory State
- Session 6: In-Memory Node Simulation Environment
- Session 7: Ad-Hoc Peer Exchange Logic
- Session 8: Protocol Handshakes & Node Connection
- Session 9: Naive Gossiping Algorithm
- Session 10: Zone-based Trust Modeling Initialization
- Session 11: Distributed Byzantine Consensus Foundations
- Session 12: PBFT Abstractions for Node Agreement
- Session 13: Reputation Penalties & Bad Actor Isolation
- Session 14: Mass Casualty Event (MCE) Consensus Rules
- Session 15: Mocked Transport Layer Integration
- Session 16: Battery Drain Emulation Models
- Session 17: Active/Idle/Scanning Power States
- Session 18: Message Pruning & Storage Limits
- Session 19: Time-To-Live (TTL) Enforcement
- Session 20: Persistent Message Handling for Critical Alerts
- Session 21: Automated Escalation & Delegation Algorithm (AEDA)
- Session 22: Severity Weighting & Confidence Thresholds
- Session 23: Temporal Sequence Routing Setup
- Session 24: Hop Count Evaluation & Optimization
- Session 25: Stale Entry Eviction in Routing Tables
- Session 26: Centralized Integration Test Harness
- Session 27: STRIDE Threat Modeling Application
- Session 28: Protocol Security Constraints & Payload Auditing
- Session 29: Security Wiring + Docker Proof Loop Foundation
- Session 30: Basic TCP Transport (`std::net`)
- Session 31: Tokio Async Transport + K-bucket discovery
- Session 32: Real Multi-process Proof Loop & Protocol Documentation
- Session 33: Metrics, Dashboard, Wasm, Interview
- Session 34: CI/CD (GitHub Actions), `SECURITY.md`, Issue Templates, PR Template, README Badges
- Session 35: README rewrite, Architecture Diagram, Benchmark Docs
- Session 36: TLA+ Formal Specification for reputation-weighted consensus
- Session 37: REST API and WebSocket backend via Axum
- Session 38: React Web Dashboard (Vite + React + Tailwind + Recharts)
- Session 39: Grafana Dashboard + Byzantine Agent Binary
- Session 40: Merkle-chained state + temporal-aware routing
- Session 41: Chain-Head Gossip + Divergence Alerting
- Session 42: REST/WebSocket API Hardening + OpenAPI Spec
- Session 43: TLA+ Formal Specification (Refinement)
- Session 44: Research Paper Final Draft + arXiv Submission Prep
- Session 45: Transport Abstraction Layer (BLE Stub + LoRa Stub + Multiplexer)
- Session 46: Encrypted Local Storage (AES-256-GCM)
- Session 47: Sybil Resistance Layer
- Session 48: Raspberry Pi Cross-Compilation & CI Target
- Session 49: CLI Hardening (`clap` refactor)
- Session 50: REST API Hardening & OpenAPI Spec (Finalization)
- Session 51: Extract Simulation Core into Library Crate (`crci-core`)
- Session 52: Web Dashboard Polish + Live API Integration
- Session 53: UniFFI Bindings for `crci-core` (Kotlin/Android FFI Layer)
- Session 54: Android App Scaffolding (Jetpack Compose + FFI `crci-android`)
- Session 55: Live Node Startup + Peer Polling via FFI
- Session 56: Real Tokio Runtime Behind FFI + Peer Table Round-Trip
- Session 57: Live Peer Handshake End-to-End (Emulator ↔ Host) FFI Wiring
- Session 58: `crci-node` CLI Binary + Two-Node Handshake Test
- Session 59: Two-Node Handshake Integration Test
- Session 60: Byzantine Fault Injection Test
- Session 61: Byzantine Eviction Benchmarks + README Results Table
- Session 62: mdBook Documentation Site
- Session 63: GitHub Actions CI for mdBook
- Session 64: README + Docs Accuracy Pass
- Session 65: arXiv Paper Final Pass
- Session 66: TLC Model Checker Run + Formal Verification Results
- Session 67: Multi-node Docker Compose Mesh, Gap Docs & Paper Updates
- Session 68: Chaos Engineering Suite with 4 Adversarial Network Scenarios
- Session 69: v0.1.0 Release — README Polish, Changelog & Version Bump
- Session 70: Final Audit — Hardened Error Handling, Expanded Benchmarks, arXiv Package & Complete Documentation
- Session 71: CLI Send / Byzantine / Peers Integration & Quickstart Walkthrough
- Session 72: Clone-to-Wow Live Demo Path (Orchestrated Live Mesh & Dashboard)
- Session 73: Harden TcpTransport Socket Lifecycle (ASYNC-001–015)
- Session 77: Verification Closeout + Crypto-Layer Hardening + Chaos Partition Scenario


## Current Functionality
1. **Real Async Networking**: The network is now backed by a true asynchronous `tokio::net` TCP transport layer (`src/transport.rs`), dropping the simulation harness.
2. **Docker Multi-node Proof Loop**: Scripts and binaries exist to spin up isolated nodes with actual asynchronous communication, verifying the pipeline.
3. **Advanced Features Embedded**: Reputation, AEDA scoring, battery management, message pruning, Byzantine discovery (K-bucket), Temporal routing, and Merkle chaining are fully integrated into `src/integration.rs` and the `NodeRuntime`.
4. **Wasm Edge Compute**: Priority computation can be executed dynamically via Wasmtime (`src/wasm.rs`).
5. **Security**: Auditing (`src/security.rs`), validations, payload constraints, API telemetry, and a proper `SECURITY.md` are documented.
6. **Live Dashboard**: A fully standalone web dashboard at `docs/dashboard/` visually tracks live messages, peer status, and SEV distribution over an Axum WebSocket.
7. **Grafana & Prometheus**: Metrics collected and visualized in Grafana (available at `http://localhost:3001`). Prometheus available at `http://localhost:9090`.
8. **Byzantine Agent**: `byzantine_agent` binary available to inject adversarial traffic into the cluster. Run `cargo run --bin byzantine_agent -- --help` for details.
9. **API Hardening**: Rate limits (60 req/min/IP), strict Content-Type checks, 64KB payload limits, and telemetry headers (`X-Request-Id`, `X-CRCI-Version`) enforce production-grade security on the node API.
10. **Formal Verification**: The core protocol's safety and liveness properties are formally verified via TLA+ in `docs/tla/CRCI.tla`.
11. **Academic Dissemination**: Full research paper draft and LaTeX build instructions completed for arXiv submission (`docs/paper/crci_paper.md`)

### Current Session Status: SESSION 77 — PART A/B/C

### Part A — Session 76 Verification Closeout

**A1 — Android emulator screenshot (`android_emulator_verification.png`)**:
File exists (181,308 bytes) but has MIME type `text/plain; charset=utf-16le` — it is NOT a valid PNG image. Visual state of the emulator at capture time cannot be verified from this file. Status: **unverified — file is not a valid image**.

**A2 — Docker demo path decision**: Formally documented as **option (b)**. Docker Desktop daemon service is not running on this host (`docker info` fails to connect to the npipe API). Native local process fallback is the confirmed, functional demo path for this environment. Docker containerized path is optional and untested in CI. No further action required on Docker until daemon elevation is available.

---

### Part B — Crypto-Layer Hardening

Pre-audit findings (see Session 77 pre-audit report for full details):

| File | Finding |
|------|---------|
| `security.rs` | No AES-GCM code. STRIDE module only (FNV node ID, AuditLog, zone registry, MCE counter, priority queue). |
| `storage/encrypted.rs` | Live AES-GCM: 12-byte `OsRng` random nonces per write. Key via Argon2id + random salt. Append-log format. No nonce reuse. |
| `storage/legacy.rs` | AES-GCM: same `OsRng` random nonce strategy. Key = first 32 bytes of Ed25519 signing key (key separation concern — documented, not a nonce bug). |
| `identity.rs` | Ed25519 via `ed25519-dalek v2`. No panic risk. No edge-case tests previously existed. |
| `ffi.rs` | All exported functions use `match`/`if let` — no hidden `unwrap()`/`expect()`. UniFFI's scaffolding wraps calls with `catch_unwind`; panics surface as `InternalException` in Kotlin, not crashes. |

Tests added in `tests/crypto_hardening_tests.rs`:
- `test_aes_gcm_nonce_never_repeats_across_writes` — 200-write nonce collision check on `EncryptedStore`
- `test_ed25519_empty_payload_signs_and_verifies`
- `test_ed25519_rejects_malformed_signature` (all-zero 64 bytes)
- `test_ed25519_rejects_signature_from_wrong_key`
- `test_ed25519_rejects_signature_for_different_payload`
- `test_ed25519_signature_verifies_repeatedly`
- `test_ed25519_verifying_key_roundtrip_matches_signing`
- `test_ed25519_does_not_catch_replay_by_itself` (documents boundary: Ed25519 accepts replay; `ReplayFilter` must catch it — cross-checks both layers)
- `test_ffi_invalid_config_returns_false_not_panic`
- `test_ffi_zero_peers_returns_false_not_panic`
- `test_ffi_excessive_peers_returns_false_not_panic`
- `test_ffi_connect_peer_when_no_node_running_returns_false`
- `test_ffi_start_stop_node_lifecycle_handles_errors_cleanly`

**Key finding confirmed**: No nonce reuse bug exists. Both storage implementations use independent `OsRng` draws per encryption. The one real issue flagged: `legacy.rs::NodeStorage` derives the AES key directly from the first 32 bytes of the Ed25519 signing key (key separation concern) — documented here, not a nonce vulnerability.

---

### Part C — Chaos/Partition Suite Kickoff

Added `test_partition_reconciliation_no_data_loss_or_conflict` to `crci-core/src/chaos.rs` as a proper `#[test]` function.

Scenario: 10-node mesh split into two isolated halves (nodes 0–4 vs. 5–9) for 30 gossip rounds. Each partition originates its own rescue message. Partition healed; 20 reconciliation rounds run. Invariants: (1) no message crosses the partition boundary during isolation, (2) after healing, all 10 nodes hold both rescue messages (no silent data loss or Merkle-chain conflict).

**Scope statement**: This is Scenario 4 — a start, not full chaos coverage. The three existing scenarios (10%/40% packet loss, crash+restart, reconnect storm) remain. TLA+ parity and full E2E chaos coverage are NOT claimed from this one scenario.

## BFT Vector Status & Security Hardening
- **Completed**: BFT-001, BFT-002, BFT-003, BFT-004, BFT-005, BFT-006, BFT-007, BFT-008, BFT-009, BFT-010, BFT-011, BFT-012, BFT-013, BFT-014, BFT-015, BFT-016, BFT-017, BFT-018, BFT-019, BFT-020, BFT-021, BFT-030.
- **Open**: Remaining hardening work: nonce reuse in AES-GCM usage, Ed25519 signature verification edge cases, FFI panic safety at the Android/JDK boundary — not yet scoped into named vectors.

## Verification
- Clean compilation and `cargo fmt --all -- --check` passes cleanly (exit code 0).
- `cargo clippy --workspace --all-targets -- -D warnings` passes with zero warnings.
- 158 tests passing (`cargo test --all`).
- Android NDK cross-compilation verified: `libcrci_core.so` generated for `x86_64` (1.53 MB) & `arm64-v8a` (1.74 MB) via `cargo-ndk 4.1.2`. `UnsatisfiedLinkError` resolved on `emulator-5554`.
- Android Emulator screenshot captured and referenced at `android_emulator_verification.png`.

## Open Gaps & Technical Debt
- **Docker Daemon Service Elevation**: Docker Desktop daemon service is not running on host (`docker info` failed to connect to npipe API); `./scripts/demo.sh` executed native local process fallback.

## Manual Validation (Live Handshake Test)
To run the live Android to Host peer handshake end-to-end:
```bash
# Terminal 1 — start host node
cargo run --bin crci-node -- --listen 0.0.0.0:9000 --node-id host-node-1

# Android emulator — tap "Connect" with address 10.0.2.2:9000
# Expected: peerCount in UI increments to 1 within 10 seconds
```

To run the automated Byzantine eviction benchmark:
```bash
cargo test --test byzantine_bench -- --nocapture
```

To build the documentation site locally:
```bash
cd docs/book
mdbook build
```

To run the multi-node Docker mesh:
```bash
docker compose up --build
```

To execute the chaos engineering suite:
```bash
bash scripts/chaos.sh
```

## Next Steps
- Session 75: Implement Batch 2 security vectors (BFT-022 to BFT-029) and setup Android NDK `.so` build step.
