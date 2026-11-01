# Architecture

This page describes the code as it is today. Where the code differs from the design in the preprint, the
difference is stated and marked as planned work.

Diagram: [`architecture/crci-architecture.html`](architecture/crci-architecture.html)
(PNG export: `architecture/crci-architecture.png`).

## Repository layout

| Path | Contents |
|---|---|
| `crates/crci-core/` | Library crate: protocol logic, reputation, replay filter, cryptography, storage, transports, the Axum HTTP API, UniFFI exports, and in-process simulation/demo code. Built as `lib` and `cdylib`. |
| `crates/crci-node/` | Binary crate: the `crci-node` CLI, helper binaries, integration tests and benchmark harnesses. |
| `crci-android/` | Jetpack Compose demo app that calls `crci-core` through UniFFI-generated Kotlin bindings. |
| `bindings/kotlin/generate.sh` | Regenerates those Kotlin bindings. |
| `web/dashboard/` | React + Vite dashboard that reads a node's HTTP API and WebSocket feeds. |
| `docs/` | Documentation, ADRs, TLA+ specification, OpenAPI description and run evidence. |
| `scripts/` | Demo, cross-build and chaos helper scripts (see `scripts/README.md`). |
| `Dockerfile`, `docker-compose.yml` | Container build and a 3-container demo mesh plus dashboard. |
| `.github/workflows/` | CI (`ci.yml`) and tag-triggered release builds (`release.yml`). |

## Two message paths

The most important fact about the current code: the reputation and isolation mechanism and the TCP
network path are **separate code paths**.

### 1. Signed path: `NodeRuntime` (simulator, tests, benchmarks)

`crates/crci-core/src/runtime.rs`. Messages are `WireMessage` values (JSON) carrying the origin's Ed25519
public key, a signature and a sequence number. `NodeRuntime::process_inbox()` reads
`(sender_id, bytes)` pairs from a shared in-memory inbox (`transport::SharedInbox`) and applies, in order:

1. **Admission** (`sybil.rs`, `SybilGuard::check`): reject banned senders; token bucket of 20 messages,
   refilled at 5 per second.
2. **Signature-check budget**: at most 5 signature verifications per origin, counted over the node's
   lifetime and never reset. After that every message from that origin is dropped.
3. **Ed25519 signature check**: an invalid signature costs the *sender* (relaying neighbour) one strike.
4. **Duplicate check**: the same message ID received again from the same sender is a strike; from a
   different sender it is ignored as normal gossip redundancy. The ID cache is cleared when it reaches
   5 000 entries.
5. **Key consistency**: a known origin arriving with a different public key is a strike for the sender.
6. **Replay filter** (`replay.rs`): a sliding window of W = 64 sequence numbers per origin. With `seq_max`
   the highest sequence accepted from that origin, a packet with `seq < seq_max − W` is stale and dropped
   without a strike; a late packet inside the window is accepted once; the same sequence number again from
   the same sender is a strike, from a different sender it is dropped without a strike. A message resent by
   the same sender after it has left the window is also dropped as stale (step 4 does not strike it). The
   sequence number is covered by the origin's signature, so a relay cannot move the window.

A strike is −0.15 reputation (start 0.5, ban below 0.1, so three strikes ban). Reputation also drifts:
small recovery while a peer is active, and decay of 0.05/s once a peer has been silent for more than 2 s.
Idle decay stops at the ban threshold and never bans; only a strike can.

Messages of type `mce` (mass-casualty event) skip steps 2, 3 and 5: they are accepted without a signature.

### 2. Network path: `TcpTransport` + `GossipPipeline` (the `crci-node` binary)

`crates/crci-core/src/transport/tcp.rs` and `integration.rs`. Over TCP, nodes exchange
`PipelineMessage` values (JSON, 4-byte big-endian length prefix, at most 1 MiB, 5 s read timeouts, at most
100 concurrent connections). A `PipelineMessage` has **no signature and no public key**. The sender
identity is the self-declared `origin_node` field. `GossipPipeline::process` applies zone-registration,
node-ID, sequence-overflow, severity, payload-size, rate-limit, panic-cooldown, duplicate/TTL and
battery rules; failures are dropped or throttled without any penalty. It does **not** run `SybilGuard`,
the replay filter, signature checks or the three-strike rule.

Every 5 s the binary also compares the severities reported by verified zone members with their median
(anchored at 3) and treats deviations ≥ 2 as faulty, unless 3f ≥ n. A "penalty" there only increments
the `byzantine_events` counter and publishes a placeholder divergence alert (head hashes all `0`/all `f`);
it does not affect reputation or connectivity. The `/chain/head` value on this path is the latest message
ID padded to 32 bytes, not a hash-chain head.

There is no handshake on connect, and TCP traffic is not encrypted.

Joining the two paths (signed messages, reputation and strikes on TCP) and authenticating the transport
peer are the next planned milestones.

## `crci-core` modules

| Module | Role |
|---|---|
| `runtime.rs` | `NodeRuntime` and `WireMessage`: the signed message path described above. |
| `sybil.rs` | `SybilGuard`: per-peer reputation, ban threshold, token bucket, proof-of-work check, subnet correlation. |
| `replay.rs` | `ReplayFilter`: per-origin sliding window (W = 64); `window_size = 0` gives strict-monotonic delivery. |
| `identity.rs` | Ed25519 key pair (`ed25519-dalek` 2) and signing. |
| `message.rs` | Application message and signal types. |
| `integration.rs` | `PipelineMessage` and `GossipPipeline`: the TCP path's validation pipeline. |
| `validation.rs` | Input checks (severity range, payload size, node-ID format, sequence overflow) and rate limiting. |
| `battery.rs` | Battery-tier gossip throttling; rescue messages are always forwarded. |
| `ttl.rs` | Round-based message TTL and storage pruning with tombstones. |
| `merkle.rs` | Per-node SHA-256 hash chain of state snapshots and divergence alerts. |
| `aeda.rs` | AEDA: rule-based zone escalation and misinformation tagging (demonstration only). |
| `node.rs`, `mesh.rs` | In-process mesh simulator with zone-level, reputation-weighted severity aggregation. |
| `discovery.rs` | In-memory peer discovery simulation (beacons, K-bucket table). No real I/O. |
| `routing.rs` | Priority scoring for outbound scheduling. |
| `security.rs` | Node-ID derivation, payload audit, audit log types. |
| `storage/` | `EncryptedStore`: AES-256-GCM with an Argon2id-derived key; `legacy.rs` is the older format. |
| `transport/` | `Transport` trait; `tcp.rs` (real), `sim.rs` (in-memory), `multiplexer.rs`; `ble.rs` and `lora.rs` are stubs that return `NotAvailable`. |
| `api.rs`, `api_types.rs` | Axum HTTP + WebSocket API (see `docs/openapi.json`). |
| `metrics.rs` | Counters exported in Prometheus text format at `/metrics`. |
| `ffi.rs` | UniFFI exports used by the Android app. |
| `wasm.rs` | Interface for sandboxed edge modules; no WebAssembly engine is linked (in-process stand-in only). |
| `bench.rs`, `bench_extended.rs`, `stress.rs`, `chaos.rs`, `crisis.rs`, `byzantine_behavior.rs` | Simulation, demo and adversary code used by the CLI demo mode and the Byzantine agent. Compiled into the library. |

`crci-core` currently depends on `axum`, `tower-http`, `clap` and `tokio` (full), so these are part of
the Android `.so` as well.

## Binaries (`crates/crci-node`)

| Binary | Purpose |
|---|---|
| `crci-node` | Main CLI. `--listen` (+ `--peers`/`--dial`) runs a TCP node with the HTTP API on port 8080. `--send … --to …` sends one message. `--byzantine <mode> --to …` runs an adversary (`replay`, `false_allclear`, `sybil`, `flood`, `partition_heal`). With no arguments it runs a scripted in-process demonstration and serves the API. |
| `node` | Older environment-variable-driven node (`NODE_ID`, `LISTEN_PORT`, `PEERS`, …); used by `scripts/run_proof.ps1`. |
| `byzantine_agent`, `adv_client` | Stand-alone adversary clients. |
| `uniffi-bindgen` (in `crci-core`) | Generates Kotlin bindings. |

## Android and web

- **Android:** `crci-android` loads `libcrci_core.so` (built with `cargo ndk`, not committed) through the
  bindings in `app/src/main/kotlin/dev/crci/android/ffi/CrciCore.kt`. The app calls the FFI surface
  directly; it does not use the HTTP API.
- **Dashboard:** `web/dashboard` reads `/api/v1/peers`, `/chain/head`, `/health` and the
  `/ws/divergences` WebSocket of one node (default `http://localhost:8080`).

## Build features

| Feature | Effect |
|---|---|
| `test-utils` | Test-only helpers for `EncryptedStore`; enabled for tests through a dev-dependency. |
