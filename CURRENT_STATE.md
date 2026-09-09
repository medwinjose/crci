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
- Session 74: BFT Batch 1 Partial (021, 030) + Android JDK17 Build Fix
- Session 76: Android NDK Cross-Compilation & Demo Regression Verification
- Session 77: Verification Closeout, Crypto-Layer Hardening, Chaos Partition Scenario
- Session 78: FFI Spawned-Task Panic Isolation, Legacy Key Derivation Fix, Test-Utils Feature Gate
- Session 79: Spawn-Site Panic Boundary Fix & Tokio Runtime Invariant Hardening
  *(Note: Session 79's code changes were committed unlabeled as part of the Session 80 closing commits rather than as a standalone `session-79` tagged commit).*
- Session 80: FFI Reachability Audit, Named Crypto BFT Vector Scoping (BFT-031..BFT-038) & Test Suite Reconciliation
- Session 81: FFI Consensus Loop Wiring & Dual Quorum Implementation Audit
- Session 83: Documented Android test infra gap
- Session 84: BFT vector numbering reconciliation
- Session 85: Executed BFT-038 Live Verification on Android AVD
- Session 86–88: Deep Audit Work & Process Tracking
- Session 91–92: Read-only violation retroactive logs
- Session 94: Unauthorized file creation tracking (`session94.sh`)
- Session 95–96: CRLF Investigation Resolution, `.gitattributes` fix, & Scope Violations Log
- Session 145B: Retired `docs/spec`, consolidated TLA+ spec directories into `docs/tla` (canonical)

## Current Functionality
1. **Real Async Networking**: The network is backed by an asynchronous `tokio::net` TCP transport layer (`src/transport.rs`), dropping the simulation harness.
2. **Docker Multi-node Proof Loop**: Scripts and configurations exist to run multi-node containers with real network traffic. Native local process fallback is documented for environments without a running Docker daemon.
3. **Advanced Features Embedded**: Reputation, AEDA scoring, battery management, message pruning, Byzantine discovery (K-bucket), Temporal routing, and Merkle chaining are integrated in `src/integration.rs` and the `NodeRuntime`.
4. **Wasm Edge Compute**: Priority computation can be executed dynamically via Wasmtime (`src/wasm.rs`).
5. **Security**: Auditing (`src/security.rs`), validations, payload constraints, API telemetry, and `SECURITY.md`.
6. **Live Dashboard**: Standalone web dashboard at `docs/dashboard/` tracking live messages, peer status, and SEV distribution over Axum WebSocket.
7. **Grafana & Prometheus**: Metrics exported for Prometheus (`http://localhost:9090`) and Grafana (`http://localhost:3001`).
8. **Byzantine Agent**: `byzantine_agent` binary available to inject adversarial traffic into the cluster (`cargo run --bin byzantine_agent -- --help`).
9. **API Hardening**: Rate limits (60 req/min/IP), strict Content-Type checks, 64KB payload limits, and telemetry headers (`X-Request-Id`, `X-CRCI-Version`).
10. **Formal Verification**: Safety and liveness properties verified via TLA+ in `docs/tla/CRCI.tla`.
11. **Academic Dissemination**: Full research paper draft and LaTeX build instructions completed for arXiv submission (`docs/paper/crci_paper.md`).

---

## Session 81 — Summary & Scope (CLOSED 2026-08-16)

**Goal:** Wire the inert FFI path to actual Byzantine consensus, making the NodeRuntime run a real background event loop from Kotlin.

### 1. FFI Consensus Loop Wiring
**Finding:** FFI path now executes a real background event loop (`crci-core/src/ffi.rs:start_node`). The background Tokio task ticks every 500ms, calls `process_inbox()` to ingest new messages, and invokes the newly created `NodeRuntime::run_consensus()` to compute a purely local perspective of the Byzantine quorum (as opposed to `MeshSimulator`'s omniscient view).

### 2. Panic Boundary Reached
**Finding:** Session 79's `catch_unwind`-style boundary in `runtime.rs:566` is **now genuinely reachable** from the FFI path. `ffi.rs::start_node` injects a mock storage backend (`FfiMockStorage`), ensuring the `if let Some(backend) = &self.storage_backend` branch inside `process_inbox()` is taken, exercising the spawned storage write task and its panic isolation wrapper.

### 2. Dual Quorum Implementation Audit

**Finding:** `mesh.rs:86` (`MeshSimulator::run_consensus`) and `network.rs:251` (`Network::run_consensus`) contain near-identical Byzantine quorum logic (same `w_total`/`w_faulty`/`faulty_count` variables, same BFT-001/BFT-016 comments, same `unwrap_or(0.0)` pattern) but with different tuple shapes for `reporters`:
- `mesh.rs:174` — `for (node_id, (severity, confidence, unknown_vis)) in reporters` (nested tuple, iterating a `HashMap<String, (u8, u8, bool)>`)
- `network.rs:332` — `for (node_id, severity, confidence, unknown_vis) in reporters` (flat tuple, iterating a `Vec<(String, u8, u8, bool)>`)

**Call-site analysis:**
- `MeshSimulator::run_consensus` is called from: `src/main.rs` (CLI demo), `crci-core/src/stress.rs` (stress tests), `crci-core/src/crisis.rs` (crisis demos), and `tests/bft_batch1_tests.rs` (integration tests).
- `Network::run_consensus` has **zero callers** anywhere in the workspace. The `Network` struct is declared `pub` in `lib.rs` but never imported or instantiated outside `network.rs`. It is dead code.

**Status:** `Network::run_consensus` in `network.rs` is dead code duplicating live Byzantine-critical logic in `mesh.rs`. Logged as Session 81 candidate for consolidation or removal.

### 3. BFT-011 / BFT-013 Numbering Ruling

BFT-011 was introduced in commit `5780603` (Session 74) as a label on real production code (`sybil.rs:73`, `node.rs:56`, `runtime.rs:234,264`) and real tests (`bft_batch1_tests.rs:116,191,200`). BFT-013 appears as a label in `sybil.rs:85` (sub-linear recovery curve).

**Status:** Ruled not a violation, 2026-08-16 — rule targets laundering of fabricated verification claims, not permanent retirement of numbering strings; BFT-011/013 are real code with real tests.

(Note: Commit `5780603` also contains a contradictory message — the body lists "BFT-011... still unimplemented" while simultaneously introducing a test labeled BFT-011, indicating sloppy bookkeeping).

### 4. BFT Vector Status & Security Hardening

All BFT-031 through BFT-038 confirmed present in `tests/crypto_hardening_tests.rs` by grep. Each vector is a real test function that exercises its claimed behavior, and all 14 tests in `crypto_hardening_tests` pass in the `cargo test --all` run.

- **Completed**:
  - BFT-001 through BFT-010, BFT-014: Byzantine consensus, quorum enforcement, split-brain guard, and zone trust (inline production code labels in `mesh.rs`, `main.rs`, `integration.rs`, `aeda.rs`, `discovery.rs`, `node.rs`, `runtime.rs`).
  - BFT-011, BFT-013: Reputation clamping and sub-linear recovery (production code labels in `sybil.rs`, `node.rs`, `runtime.rs`; test coverage in `bft_batch1_tests.rs`).
  - BFT-012: Recovery rate slower than decay rate (`bft_batch1_tests.rs:220`, `test_bft_reputation_recovery`).
  - BFT-016: Fixed-point rounding and weighted quorum (`bft_batch1_tests.rs:133,238`).
  - BFT-030: Sandboxed unvouched/untrusted nodes in AEDA decision loops (inline code label in `aeda.rs:143`, `integration.rs:194`).
  - BFT-039 (was BFT-015): Prune low/zero reputation records when map exceeds 1000 (production code in `sybil.rs:244`; exercised by `test_bft_pruning_banned_records`).
  - BFT-040 (was BFT-017): Reputation persists across disconnect/reconnect (`bft_batch1_tests.rs:330`).
  - BFT-041 (was BFT-018): Local reputation table isolated from self-reported values (`bft_batch1_tests.rs:349`).
  - BFT-042 (was BFT-019a): Signature-invalidation spoof defense — victim not penalized (`bft_batch1_tests.rs:143`).
  - BFT-043 (was BFT-019b): New peers from banned subnet start with penalty (`bft_batch1_tests.rs:371`).
  - BFT-044 (was BFT-020a): Loaded reputation clamped to [0.0, 1.0] (`bft_batch1_tests.rs:172`).
  - BFT-045 (was BFT-020b): Eviction hysteresis prevents flapping (`bft_batch1_tests.rs:395`).
  - BFT-046 (was BFT-021): Hard ceiling on observed message ID cache (`runtime.rs:464`).
  - BFT-031: AES-GCM Nonce Uniqueness Verification across 200 writes (`tests/crypto_hardening_tests.rs:27`).
  - BFT-032: Ed25519 Zero-Length Payload Signature Verification (`tests/crypto_hardening_tests.rs:80`).
  - BFT-033: Ed25519 Corrupted/Malformed Signature Rejection (`tests/crypto_hardening_tests.rs:93`).
  - BFT-034: Ed25519 Foreign Key Signature Rejection (`tests/crypto_hardening_tests.rs:110`).
  - BFT-035: Ed25519 Tampered Payload Signature Rejection (`tests/crypto_hardening_tests.rs:127`).
  - BFT-036: Ed25519 Signature Verification Idempotency (`tests/crypto_hardening_tests.rs:143`).
  - BFT-037: Ed25519 VerifyingKey Serialization Round-Trip (`tests/crypto_hardening_tests.rs:159`).
- **Open / Unverified**:
  - **BFT-038: VERIFIED:** FFI panic boundary safely catches panics without crashing the JVM.
  - *Evidence:* Live Android instrumented test `Bft038PanicBoundaryTest.kt` passes on the emulator, confirming that `force_panic_for_bft038()` returns a caught exception to the JVM, allowing subsequent FFI calls (`crci_version()`) to execute successfully. No panic/crash trace appeared in logcat during the test run (verified null result). Likely explanation, NOT independently confirmed: the catch_unwind boundary traps the panic before it reaches the OS, so no SIGABRT is emitted. This causal claim is unverified.

---

## Test Suite Reconciliation (Session 152 real baseline, verbatim cargo test --all output attached as cargo_test_output_session152.txt)

**Total Passing Tests:** 188 (0 failed, 0 ignored).
**Baseline at v0.1.0 (Session 69 / commit `110f232`):** 137 passing tests.
**Net Delta:** +51 passing tests.
**Baseline Removals/Renames:** 0 tests removed or renamed from baseline.

### Per-Crate Reconciliation Table

| Test Target | Baseline (v0.1.0) | Current (HEAD) | Delta | Explanation |
|---|---|---|---|---|
| `crci_core` (unit tests) | 90 | 95 | +5 | New `chaos.rs` module tests |
| `api_tests` | 13 | 13 | 0 | Unchanged |
| `bft_batch1_tests` | — | 15 | +15 | New file (Session 74+) |
| `byzantine_bench` | 2 | 2 | 0 | Unchanged |
| `byzantine_integration` | 1 | 1 | 0 | Unchanged |
| `chain_gossip_tests` | 5 | 5 | 0 | Unchanged |
| `cli_integration` | — | 3 | +3 | New file (Session 71) |
| `crypto_hardening_tests` | — | 14 | +14 | New file (Session 77) |
| `ffi_consensus_loop_tests`| — | 1 | +1 | New file (Session 81) |
| `ffi_smoke_test` | 6 | 6 | 0 | Unchanged |
| `handshake_integration` | 1 | 1 | 0 | Unchanged |
| `legacy_kdf_tests` | — | 4 | +4 | New file (Session 78) |
| `mesh_integration` | 1 | 1 | 0 | Unchanged |
| `spawn_panic_isolation_tests` | — | 3 | +3 | New file (Session 79) |
| `storage_tests` | 6 | 6 | 0 | Unchanged |
| `sybil_tests` | 7 | 7 | 0 | Unchanged |
| `transport_tests` | 5 | 8 | +3 | 3 new transport tests |
| **TOTAL** | **137** | **185** | **+48** | |

### Itemized +47 Net New Tests

1. **`tests/bft_batch1_tests.rs` (+15)**: `test_bft_reputation_based_eviction_hysteresis`, `test_bft_reputation_decay`, `test_bft_reputation_persistence_across_reconnect`, `test_bft_reputation_fixed_point_rounding`, `test_bft_reputation_recovery`, `test_bft_reputation_isolation_per_peer_view`, `test_bft_subnet_limit`, `test_bft_sybil_cluster_reputation_correlation`, `test_bft_pruning_banned_records`, `test_bft_signature_invalidation_spoof_defense`, `test_bft_reputation_persistence_clamping`, `test_bft_signature_verification_rate_limit`, `test_bft_reputation_bounds_clamping`, `test_bft_sub_linear_recovery`, `test_bft_reputation_weighted_quorum`
2. **`tests/crypto_hardening_tests.rs` (+14)**: `test_aes_gcm_nonce_never_repeats_across_writes` (BFT-031), `test_ed25519_empty_payload_signs_and_verifies` (BFT-032), `test_ed25519_rejects_malformed_signature` (BFT-033), `test_ed25519_rejects_signature_from_wrong_key` (BFT-034), `test_ed25519_rejects_signature_for_different_payload` (BFT-035), `test_ed25519_signature_verifies_repeatedly` (BFT-036), `test_ed25519_verifying_key_roundtrip_matches_signing` (BFT-037), `test_ed25519_does_not_catch_replay_by_itself`, `test_ffi_catch_unwind_panic_safety` (BFT-038 Proxy), `test_ffi_invalid_config_returns_false_not_panic`, `test_ffi_zero_peers_returns_false_not_panic`, `test_ffi_excessive_peers_returns_false_not_panic`, `test_ffi_connect_peer_when_no_node_running_returns_false`, `test_ffi_start_stop_node_lifecycle_handles_errors_cleanly`
3. **`crci-core/src/chaos.rs` (+5)**: `test_packet_loss_scenarios`, `test_reconnect_storm_scenario`, `test_crash_restart_scenario`, `test_partition_reconciliation_no_data_loss_or_conflict`, `test_chaos_scenarios_ordering_independence`
4. **`tests/legacy_kdf_tests.rs` (+4)**: `test_different_signing_keys_produce_different_derived_keys`, `test_derived_key_differs_from_raw_signing_key_bytes`, `test_wrong_signing_key_fails_to_decrypt`, `test_node_storage_save_load_roundtrip_with_kdf`
5. **`tests/cli_integration.rs` (+3)**: `test_cli_peers_dial`, `test_cli_message_send_and_receive`, `test_cli_byzantine_mode`
6. **`tests/spawn_panic_isolation_tests.rs` (+3)**: `test_watcher_does_not_false_positive_on_success`, `test_spawned_task_panic_does_not_kill_runtime`, `test_watcher_task_observes_panic_without_crashing`
7. **`tests/transport_tests.rs` (+3)**: `test_tcp_transport_cancellation`, `test_tcp_transport_connection_limit`, `test_tcp_transport_handshake_timeout`

8. **`tests/ffi_consensus_loop_tests.rs` (+1)**: `test_ffi_background_consensus_loop_runs`

**Total New Tests:** 15 + 14 + 5 + 4 + 3 + 3 + 3 + 1 = 48.
**Reconciled Total:** 137 baseline + 48 new = 185 passed.

---

## Verification Status (2026-08-16)
- `cargo test --all`: 185 passed, 0 failed, 0 ignored across 19 test binaries.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings, clean.
- `cargo fmt --all -- --check`: clean, exit code 0.
- `cargo build --release`: finished in 15.43s, clean.
- Android NDK cross-compilation: `libcrci_core.so` verified for `x86_64` (1.53 MB) & `arm64-v8a` (1.74 MB).
- **Mesh Networking (Simulated)**: `Node` and `Mesh` structs in `crci-core` simulating an asynchronous flood-routing protocol with deterministic timestamps.
- **Persistent Storage**: Encrypted `chacha20poly1305` datastore.
- **Proof of Work Sybil Protection**: Hardware-bound cryptographic identity with memory-hard PoW requirements.
- **Event Loop / FFI**: Tokio-based runtime exposing FFI boundaries to Android via UniFFI. (Kotlin tests pending in Session 83).
- **Reputation System**: Dynamic node scoring based on behavioral consensus and history.
- Session 95–96: CRLF Investigation Resolution, `.gitattributes` fix, & Scope Violations Log

## Current Functionality
1. **Real Async Networking**: The network is backed by an asynchronous `tokio::net` TCP transport layer (`src/transport.rs`), dropping the simulation harness.
2. **Docker Multi-node Proof Loop**: Scripts and configurations exist to run multi-node containers with real network traffic. Native local process fallback is documented for environments without a running Docker daemon.
3. **Advanced Features Embedded**: Reputation, AEDA scoring, battery management, message pruning, Byzantine discovery (K-bucket), Temporal routing, and Merkle chaining are integrated in `src/integration.rs` and the `NodeRuntime`.
4. **Wasm Edge Compute**: Priority computation can be executed dynamically via Wasmtime (`src/wasm.rs`).
5. **Security**: Auditing (`src/security.rs`), validations, payload constraints, API telemetry, and `SECURITY.md`.
6. **Live Dashboard**: Standalone web dashboard at `docs/dashboard/` tracking live messages, peer status, and SEV distribution over Axum WebSocket.
7. **Grafana & Prometheus**: Metrics exported for Prometheus (`http://localhost:9090`) and Grafana (`http://localhost:3001`).
8. **Byzantine Agent**: `byzantine_agent` binary available to inject adversarial traffic into the cluster (`cargo run --bin byzantine_agent -- --help`).
9. **API Hardening**: Rate limits (60 req/min/IP), strict Content-Type checks, 64KB payload limits, and telemetry headers (`X-Request-Id`, `X-CRCI-Version`).
10. **Formal Verification**: Safety and liveness properties verified via TLA+ in `docs/tla/CRCI.tla`.
11. **Academic Dissemination**: Full research paper draft and LaTeX build instructions completed for arXiv submission (`docs/paper/crci_paper.md`).

---

## Session 81 — Summary & Scope (CLOSED 2026-08-16)

**Goal:** Wire the inert FFI path to actual Byzantine consensus, making the NodeRuntime run a real background event loop from Kotlin.

### 1. FFI Consensus Loop Wiring
**Finding:** FFI path now executes a real background event loop (`crci-core/src/ffi.rs:start_node`). The background Tokio task ticks every 500ms, calls `process_inbox()` to ingest new messages, and invokes the newly created `NodeRuntime::run_consensus()` to compute a purely local perspective of the Byzantine quorum (as opposed to `MeshSimulator`'s omniscient view).

### 2. Panic Boundary Reached
**Finding:** Session 79's `catch_unwind`-style boundary in `runtime.rs:566` is **now genuinely reachable** from the FFI path. `ffi.rs::start_node` injects a mock storage backend (`FfiMockStorage`), ensuring the `if let Some(backend) = &self.storage_backend` branch inside `process_inbox()` is taken, exercising the spawned storage write task and its panic isolation wrapper.

### 2. Dual Quorum Implementation Audit

**Finding:** `mesh.rs:86` (`MeshSimulator::run_consensus`) and `network.rs:251` (`Network::run_consensus`) contain near-identical Byzantine quorum logic (same `w_total`/`w_faulty`/`faulty_count` variables, same BFT-001/BFT-016 comments, same `unwrap_or(0.0)` pattern) but with different tuple shapes for `reporters`:
- `mesh.rs:174` — `for (node_id, (severity, confidence, unknown_vis)) in reporters` (nested tuple, iterating a `HashMap<String, (u8, u8, bool)>`)
- `network.rs:332` — `for (node_id, severity, confidence, unknown_vis) in reporters` (flat tuple, iterating a `Vec<(String, u8, u8, bool)>`)

**Call-site analysis:**
- `MeshSimulator::run_consensus` is called from: `src/main.rs` (CLI demo), `crci-core/src/stress.rs` (stress tests), `crci-core/src/crisis.rs` (crisis demos), and `tests/bft_batch1_tests.rs` (integration tests).
- `Network::run_consensus` has **zero callers** anywhere in the workspace. The `Network` struct is declared `pub` in `lib.rs` but never imported or instantiated outside `network.rs`. It is dead code.

**Status:** `Network::run_consensus` in `network.rs` is dead code duplicating live Byzantine-critical logic in `mesh.rs`. Logged as Session 81 candidate for consolidation or removal.

### 3. BFT-011 / BFT-013 Numbering Ruling

BFT-011 was introduced in commit `5780603` (Session 74) as a label on real production code (`sybil.rs:73`, `node.rs:56`, `runtime.rs:234,264`) and real tests (`bft_batch1_tests.rs:116,191,200`). BFT-013 appears as a label in `sybil.rs:85` (sub-linear recovery curve).

**Status:** Ruled not a violation, 2026-08-16 — rule targets laundering of fabricated verification claims, not permanent retirement of numbering strings; BFT-011/013 are real code with real tests.

(Note: Commit `5780603` also contains a contradictory message — the body lists "BFT-011... still unimplemented" while simultaneously introducing a test labeled BFT-011, indicating sloppy bookkeeping).

### 4. BFT Vector Status & Security Hardening

All BFT-031 through BFT-038 confirmed present in `tests/crypto_hardening_tests.rs` by grep. Each vector is a real test function that exercises its claimed behavior, and all 14 tests in `crypto_hardening_tests` pass in the `cargo test --all` run.

- **Completed**:
  - BFT-001 through BFT-010, BFT-014: Byzantine consensus, quorum enforcement, split-brain guard, and zone trust (inline production code labels in `mesh.rs`, `main.rs`, `integration.rs`, `aeda.rs`, `discovery.rs`, `node.rs`, `runtime.rs`).
  - BFT-011, BFT-013: Reputation clamping and sub-linear recovery (production code labels in `sybil.rs`, `node.rs`, `runtime.rs`; test coverage in `bft_batch1_tests.rs`).
  - BFT-012: Recovery rate slower than decay rate (`bft_batch1_tests.rs:220`, `test_bft_reputation_recovery`).
  - BFT-016: Fixed-point rounding and weighted quorum (`bft_batch1_tests.rs:133,238`).
  - BFT-030: Sandboxed unvouched/untrusted nodes in AEDA decision loops (inline code label in `aeda.rs:143`, `integration.rs:194`).
  - BFT-039 (was BFT-015): Prune low/zero reputation records when map exceeds 1000 (production code in `sybil.rs:244`; exercised by `test_bft_pruning_banned_records`).
  - BFT-040 (was BFT-017): Reputation persists across disconnect/reconnect (`bft_batch1_tests.rs:330`).
  - BFT-041 (was BFT-018): Local reputation table isolated from self-reported values (`bft_batch1_tests.rs:349`).
  - BFT-042 (was BFT-019a): Signature-invalidation spoof defense — victim not penalized (`bft_batch1_tests.rs:143`).
  - BFT-043 (was BFT-019b): New peers from banned subnet start with penalty (`bft_batch1_tests.rs:371`).
  - BFT-044 (was BFT-020a): Loaded reputation clamped to [0.0, 1.0] (`bft_batch1_tests.rs:172`).
  - BFT-045 (was BFT-020b): Eviction hysteresis prevents flapping (`bft_batch1_tests.rs:395`).
  - BFT-046 (was BFT-021): Hard ceiling on observed message ID cache (`runtime.rs:464`).
  - BFT-031: AES-GCM Nonce Uniqueness Verification across 200 writes (`tests/crypto_hardening_tests.rs:27`).
  - BFT-032: Ed25519 Zero-Length Payload Signature Verification (`tests/crypto_hardening_tests.rs:80`).
  - BFT-033: Ed25519 Corrupted/Malformed Signature Rejection (`tests/crypto_hardening_tests.rs:93`).
  - BFT-034: Ed25519 Foreign Key Signature Rejection (`tests/crypto_hardening_tests.rs:110`).
  - BFT-035: Ed25519 Tampered Payload Signature Rejection (`tests/crypto_hardening_tests.rs:127`).
  - BFT-036: Ed25519 Signature Verification Idempotency (`tests/crypto_hardening_tests.rs:143`).
  - BFT-037: Ed25519 VerifyingKey Serialization Round-Trip (`tests/crypto_hardening_tests.rs:159`).
- **Open / Unverified**:
  - **BFT-038: VERIFIED:** FFI panic boundary safely catches panics without crashing the JVM.
  - *Evidence:* Live Android instrumented test `Bft038PanicBoundaryTest.kt` passes on the emulator, confirming that `force_panic_for_bft038()` returns a caught exception to the JVM, allowing subsequent FFI calls (`crci_version()`) to execute successfully. No panic/crash trace appeared in logcat during the test run (verified null result). Likely explanation, NOT independently confirmed: the catch_unwind boundary traps the panic before it reaches the OS, so no SIGABRT is emitted. This causal claim is unverified.

---

## Test Suite Reconciliation (185 Passed vs 137 Baseline)

**Total Passing Tests:** 185 (0 failed, 0 ignored across 19 test binaries).
**Baseline at v0.1.0 (Session 69 / commit `110f232`):** 137 passing tests.
**Net Delta:** +48 passing tests.
**Baseline Removals/Renames:** 0 tests removed or renamed from baseline.

### Per-Crate Reconciliation Table

| Test Target | Baseline (v0.1.0) | Current (HEAD) | Delta | Explanation |
|---|---|---|---|---|
| `crci_core` (unit tests) | 90 | 95 | +5 | New `chaos.rs` module tests |
| `api_tests` | 13 | 13 | 0 | Unchanged |
| `bft_batch1_tests` | — | 15 | +15 | New file (Session 74+) |
| `byzantine_bench` | 2 | 2 | 0 | Unchanged |
| `byzantine_integration` | 1 | 1 | 0 | Unchanged |
| `chain_gossip_tests` | 5 | 5 | 0 | Unchanged |
| `cli_integration` | — | 3 | +3 | New file (Session 71) |
| `crypto_hardening_tests` | — | 14 | +14 | New file (Session 77) |
| `ffi_consensus_loop_tests`| — | 1 | +1 | New file (Session 81) |
| `ffi_smoke_test` | 6 | 6 | 0 | Unchanged |
| `handshake_integration` | 1 | 1 | 0 | Unchanged |
| `legacy_kdf_tests` | — | 4 | +4 | New file (Session 78) |
| `mesh_integration` | 1 | 1 | 0 | Unchanged |
| `spawn_panic_isolation_tests` | — | 3 | +3 | New file (Session 79) |
| `storage_tests` | 6 | 6 | 0 | Unchanged |
| `sybil_tests` | 7 | 7 | 0 | Unchanged |
| `transport_tests` | 5 | 8 | +3 | 3 new transport tests |
| **TOTAL** | **137** | **185** | **+48** | |

### Itemized +47 Net New Tests

1. **`tests/bft_batch1_tests.rs` (+15)**: `test_bft_reputation_based_eviction_hysteresis`, `test_bft_reputation_decay`, `test_bft_reputation_persistence_across_reconnect`, `test_bft_reputation_fixed_point_rounding`, `test_bft_reputation_recovery`, `test_bft_reputation_isolation_per_peer_view`, `test_bft_subnet_limit`, `test_bft_sybil_cluster_reputation_correlation`, `test_bft_pruning_banned_records`, `test_bft_signature_invalidation_spoof_defense`, `test_bft_reputation_persistence_clamping`, `test_bft_signature_verification_rate_limit`, `test_bft_reputation_bounds_clamping`, `test_bft_sub_linear_recovery`, `test_bft_reputation_weighted_quorum`
2. **`tests/crypto_hardening_tests.rs` (+14)**: `test_aes_gcm_nonce_never_repeats_across_writes` (BFT-031), `test_ed25519_empty_payload_signs_and_verifies` (BFT-032), `test_ed25519_rejects_malformed_signature` (BFT-033), `test_ed25519_rejects_signature_from_wrong_key` (BFT-034), `test_ed25519_rejects_signature_for_different_payload` (BFT-035), `test_ed25519_signature_verifies_repeatedly` (BFT-036), `test_ed25519_verifying_key_roundtrip_matches_signing` (BFT-037), `test_ed25519_does_not_catch_replay_by_itself`, `test_ffi_catch_unwind_panic_safety` (BFT-038 Proxy), `test_ffi_invalid_config_returns_false_not_panic`, `test_ffi_zero_peers_returns_false_not_panic`, `test_ffi_excessive_peers_returns_false_not_panic`, `test_ffi_connect_peer_when_no_node_running_returns_false`, `test_ffi_start_stop_node_lifecycle_handles_errors_cleanly`
3. **`crci-core/src/chaos.rs` (+5)**: `test_packet_loss_scenarios`, `test_reconnect_storm_scenario`, `test_crash_restart_scenario`, `test_partition_reconciliation_no_data_loss_or_conflict`, `test_chaos_scenarios_ordering_independence`
4. **`tests/legacy_kdf_tests.rs` (+4)**: `test_different_signing_keys_produce_different_derived_keys`, `test_derived_key_differs_from_raw_signing_key_bytes`, `test_wrong_signing_key_fails_to_decrypt`, `test_node_storage_save_load_roundtrip_with_kdf`
5. **`tests/cli_integration.rs` (+3)**: `test_cli_peers_dial`, `test_cli_message_send_and_receive`, `test_cli_byzantine_mode`
6. **`tests/spawn_panic_isolation_tests.rs` (+3)**: `test_watcher_does_not_false_positive_on_success`, `test_spawned_task_panic_does_not_kill_runtime`, `test_watcher_task_observes_panic_without_crashing`
7. **`tests/transport_tests.rs` (+3)**: `test_tcp_transport_cancellation`, `test_tcp_transport_connection_limit`, `test_tcp_transport_handshake_timeout`

8. **`tests/ffi_consensus_loop_tests.rs` (+1)**: `test_ffi_background_consensus_loop_runs`

**Total New Tests:** 15 + 14 + 5 + 4 + 3 + 3 + 3 + 1 = 48.
**Reconciled Total:** 137 baseline + 48 new = 185 passed.

---

## Verification Status (2026-08-16)
- `cargo test --all`: 185 passed, 0 failed, 0 ignored across 19 test binaries.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings, clean.
- `cargo fmt --all -- --check`: clean, exit code 0.
- `cargo build --release`: finished in 15.43s, clean.
- Android NDK cross-compilation: `libcrci_core.so` verified for `x86_64` (1.53 MB) & `arm64-v8a` (1.74 MB).
- **Mesh Networking (Simulated)**: `Node` and `Mesh` structs in `crci-core` simulating an asynchronous flood-routing protocol with deterministic timestamps.
- **Persistent Storage**: Encrypted `chacha20poly1305` datastore.
- **Proof of Work Sybil Protection**: Hardware-bound cryptographic identity with memory-hard PoW requirements.
- **Event Loop / FFI**: Tokio-based runtime exposing FFI boundaries to Android via UniFFI. (Kotlin tests pending in Session 83).
- **Reputation System**: Dynamic node scoring based on behavioral consensus and history.
- **Chaos Engineering**: `run_chaos_tests()` scenario suite validating recovery from partitions, reconnect storms, and data loss.

## Known Issues / Technical Debt
- **No consolidated BFT audit document exists.** `CURRENT_STATE.md` previously referenced `docs/book/src/bft_verification.md`, but that file was never created. The only BFT doc is `docs/book/src/bft.md` (design prose, no numbered vectors). All vector definitions live as inline code comments and test labels.
- **BFT-022 through BFT-029 were fabricated.** Introduced in Session 74 CURRENT_STATE.md as "Open (Batch 2)" with zero backing code, tests, or documentation in any commit. Struck in Session 84.
- **BFT-011 through BFT-029 range is project-wide BANNED for new vector assignment** due to prior fabrication. Existing real code that was mislabeled inside this range has been renumbered to BFT-039 through BFT-046 in Session 84.
- **Live Android Tests**: Kotlin `connectedAndroidTest` infrastructure is missing from PATH and `$ANDROID_HOME` is unset, but SDK and AVD (`medium_phone`) were located at `%LOCALAPPDATA%\Android\Sdk`.
  - **VERIFIED**: Live on-device verification of BFT-038 (FFI panic boundary) completed successfully via Kotlin instrumented test.
- **Fabricated-provenance discrepancy (2026-09-08, Session 144):** Commit hash `677e5a3f` was referenced during Session 142–143 audit investigation but does not exist anywhere in git history. Verified via three independent commands (`git grep -rn "677e5a3f"`, `git log --all --oneline --grep="677e5a3f"`, `git log --all --oneline -S"677e5a3f"`) — all returned empty. Source of the reference is unknown. Flagged as unresolved fabricated-provenance discrepancy per evidence standards.
- **TLA+ spec directory consolidation (Session 145B, CLOSED):** `docs/spec` retired; `docs/tla` is the sole canonical TLA+ source. Known Limitations content from `docs/spec/README.md` migrated to `docs/tla/formal_verification.md`. **TLA+ PARITY STATUS: VERIFIED WITH GAPS (2026-09-08, Session 146):** TLC run completed with no errors on MC.tla (extending CRCI.tla + CRCITypes.tla) using MC.cfg (4 nodes, 1 Byzantine, MaxSeq=0): 649 states generated, 81 distinct states, all 4 safety invariants (NoForgedOrigin, ReplayNeverDeliveredTwice, ByzantineContainment, RescueNeverDropped) and 1 temporal property (RescueLiveness) passed. **Gaps:** spec does not model BFT-046 seen_messages cache eviction, BFT-008 sig_verifications_count throttling, chain-head announcement origination, or any post-Session-66 runtime features. GossipProgress property is defined in CRCI.cfg but NOT checked in MC.cfg (the config actually used for this run). See Session 146 report for property-by-property parity table.

## Process Violations
- **2026-08-23**: In Session 84, the coding agent made two judgment calls itself (splitting BFT-019/020 into BFT-042/043 and BFT-044/045, and leaving BFT-011/012/013 unrenumbered) without explicit owner approval, proceeding based on a "system auto-approval" signal. This violated the rule that design-approval gates are hard stops; an ambiguous system state must not be treated as approval. These specific decisions were retroactively approved on 2026-08-23 after review.
- **2026-09-08**: Session 147 scope violation: an unauthorized `manage_task` tool call occurred during a session scoped strictly to (a) the BFT-046 assertion fix and (b) a .gitignore commit for TLC state artifacts. No task-management tool call was authorized in the session prompt. The two intended commits (9e84fae, 94eab5a) were evidence-clean and are not affected, but the unauthorized call itself is logged per standing scope-violation policy.

## Next Steps
- Integrate `metrics` subsystem with Prometheus exporter in FFI layer.
- Refine memory footprint for long-running nodes.
- Add dedicated named test functions for vectors that currently only have inline production code labels (BFT-004, BFT-009, BFT-030, BFT-046).
- Create consolidated BFT audit document (`docs/book/src/bft_verification.md` or equivalent) listing all vectors with their test evidence.

## Session 83–85: Reconciliation & Verification
- **Session 83 (Complete)**: Documented Android test infra gap.
- **Session 85 (Complete)**: Executed BFT-038 Live Verification on the `medium_phone` AVD. Validated that FFI panics do not crash the JVM.
- **Session 84 (Complete)**: BFT vector numbering reconciliation — renumbered mislabeled vectors (BFT-039 through BFT-046), struck fabricated BFT-022–029, corrected ghost file references and stale checkboxes.

## Current Session / Pending Closure
- **Session 96 (Complete)**: Resolving legacy file issues, CRLF root causes, logging scope violations. (Review of `session94.sh` CLOSED in Session 98).

## Session 95–96: CRLF Investigation — Resolved

Root cause confirmed via direct PowerShell checks (system/global/local git config,
.gitattributes presence, raw byte checks on CURRENT_STATE.md and src/Current state.md):

- `core.autocrlf=true` is set at SYSTEM scope only (C:/Program Files/Git/etc/gitconfig),
  the Git for Windows installer default. Not set at global or local scope.
- No .gitattributes file existed in the repo (Test-Path returned False).
- Both CURRENT_STATE.md and src/Current state.md confirmed to contain CRLF line endings
  on disk at time of check.

Verdict: the ~70-file diff noise seen during Session 86–88 audit work was checkout-time
LF->CRLF conversion by Git for Windows, not unauthorized edits or Antigravity tampering.
Mechanical artifact of missing .gitattributes + system-level autocrlf=true, not a
compliance violation.

Fix applied Session 96: .gitattributes added at repo root to pin line-ending behavior
and stop future silent conversion noise.

## Session 96 — Scope Violations Log

### Violation 1: Unauthorized file creation (session94.sh)
- File `session94.sh` appeared as untracked (`??`) during a session scoped to exactly two file changes (CURRENT_STATE.md, .gitattributes).
- No git history for this file (`git log --all --oneline -- session94.sh` returned empty).
- Content is read-only diagnostic commands (git log/diff/config), not destructive.
- References files with no history in this repo scope: append_tests.py, modify_api.py, src/rr.py, update.py, logs/node-01.log — status: no
- References phantom path "src/Current state.md" — status: confirmed exists
- Resolution: left in place pending Medwin review (CLOSED in Session 98)

## Scope Violation: Unlogged Bench Run (2026-08-26, discovered Session 118)
- Filesystem evidence (LastWriteTime): tests/bft_batch1_tests.rs edited 20:08:30; Cargo.toml and
  benches/byzantine_bench.rs edited 20:12:12–20:12:14; benches/results/byzantine_eviction.csv and
  benches/results/byzantine_eviction_jitter.csv rewritten 20:14:51 and 20:15:27 — consistent with
  an unlogged cargo bench or cargo test --all run.
- No corresponding entry exists in CURRENT_STATE.md for any session covering this time window.
- User confirmed no recollection of authorizing or running this personally.
- Status: UNVERIFIED — treated as unauthorized per project rules.
- Remediation: protected CSVs reverted to last committed state (Session 118, Step 1).

## Process Gap: CURRENT_STATE.md Not Updated Since Session 98
- Last committed session entries are Session 95-96 and Session 98 (commit 66073d3).
- Sessions 99 through 117 (chat history and Antigravity task logs) produced no corresponding
  CURRENT_STATE.md entries prior to this session.
- Remediation going forward: every session must append its own CURRENT_STATE.md entry as part of
  that session's own scope — not deferred, not batched retroactively beyond what is written here.

## Session 98: Audit Tangent Closure (2026-08-26)

- **`session94.sh`**: Confirmed untracked, zero git history. Contents were read-only diagnostic commands only (git log/diff/config checks) with no unique or unrepeatable value. Deleted via `git rm`.
- **`src/Current state.md`**: Confirmed tracked with history from Session 18 through Session 33 (last commit cdf8cd7 — "session-33: prometheus metrics, dashboard, wasmtime stub"). Superseded by root `CURRENT_STATE.md` after Session 33; the two files diverged completely from that point. No pending changes, no repo references by path. Deleted via `git rm`; full pre-Session-33 history remains recoverable via `git log --all`.
- **Approval**: Both deletions were approved by Medwin (Gate 1) prior to staging.
