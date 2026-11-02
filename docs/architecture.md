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

## Message path

Every message is a `WireMessage` (JSON) carrying the origin's Ed25519 public key, a signature and a
sequence number, and every node runs the same checks in `NodeRuntime` (`crates/crci-core/src/runtime.rs`),
whether messages arrive in-process (simulator, tests, benchmarks, Android FFI) or over TCP
(`crci-node --listen`).

### Checks in `NodeRuntime::process_inbox`

`process_inbox()` reads `(sender_id, bytes)` pairs from a shared inbox (`transport::SharedInbox`) and
applies, in order:

1. **Admission** (`sybil.rs`, `SybilGuard::check`): reject banned senders; token bucket of 20 messages,
   refilled at 5 per second.
2. **Signature-check budget**: no separate counter. Every message that reaches the signature check has
   passed the sender's token bucket in step 1, so verifications are bounded per transport sender (20, then
   5 per second), and every invalid signature is a strike, so a sender of bad signatures is banned after
   three checks. The claimed origin is not used for budgeting, because it is not authenticated.
3. **Ed25519 signature check**: an invalid signature costs the *sender* (relaying neighbour) one strike.
   Every message type is checked, mass-casualty events (`mce`) included.
4. **Duplicate check**: the same message ID received again from the same sender is a strike; from a
   different sender it is ignored as normal gossip redundancy. The ID cache is cleared when it reaches
   5 000 entries.
5. **Key consistency**: the first public key seen for an origin ID is pinned; a message claiming that origin
   with another key is dropped. It is not a strike: the receiver cannot tell which key is genuine, so the
   relay that delivered it is not at fault (a strike here would let an adversary that announces an honest
   node's ID first get that node's honest relays banned).
6. **Replay filter** (`replay.rs`): a sliding window of W = 64 sequence numbers per origin. With `seq_max`
   the highest sequence accepted from that origin, a packet with `seq < seq_max − W` is stale and dropped
   without a strike; a late packet inside the window is accepted once; the same sequence number again from
   the same sender is a strike, from a different sender it is dropped without a strike. A message resent by
   the same sender after it has left the window is also dropped as stale (step 4 does not strike it). The
   sequence number is covered by the origin's signature, so a relay cannot move the window.

A strike is −0.15 reputation (start 0.5, ban below 0.1, so three strikes ban). Between strikes a score
recovers with elapsed time, 0.0005 per second (one strike forgiven in 300 s), whatever the peer sends,
and never above 0.5, so good behaviour cannot be banked for a later burst. While two or more strikes fall
within the last 300 s, recovery cannot raise the score above 0.18 (strike damping; it never lowers a
score). A banned peer recovers at the same rate and is readmitted at 0.25. Inactivity never changes a
score (`sybil.rs`). Any forgiving recovery leaves a residual bound: a peer that paces its violations at
one per 300 s or slower is never banned (measured in
[benchmarks.md](benchmarks.md#paced-attack-vs-reputation-recovery)).

The signature covers every `WireMessage` field except the signature itself (`WireMessage::signing_bytes`):
a domain tag (`CRCI-WIRE-MESSAGE-v1`), then each field in declaration order, variable-length fields with a
64-bit length prefix and the optional note with a presence byte. No two distinct messages encode to the
same bytes, so a relay cannot change any field, including the message type, note, flags or sequence
number, without the signature failing. Signer and verifier use the same function.

### Over TCP: the `crci-node` binary

`crates/crci-core/src/transport/tcp.rs`. Every message travels as a 4-byte big-endian length prefix
followed by JSON (handshake messages at most 4 KiB, frames at most 1 MiB, 5 s to read a value once it has started,
at most 100 concurrent connections; an authenticated connection may stay idle for 10 min). Each connection starts with a mutual Ed25519 challenge-response, using the same
key the node signs its messages with:

```
listener -> dialer    ServerHello { node_id, public_key, nonce_L }
dialer   -> listener  ClientAuth  { node_id, public_key, nonce_D, sig_D(transcript) }
listener -> dialer    ServerAuth  { sig_L(transcript) }
dialer   -> listener  TcpFrame    { counter, message, sig_D(session, counter, message) }  ...
```

The transcript is a domain tag followed by both node IDs, both public keys and both fresh 32-byte nonces,
length-prefixed; each side also signs its role, so one side's signature cannot be presented as the
other's. The listener drops the connection, before reading any frame, if the dialer's signature is invalid,
if its key is the listener's own, or if a field is malformed; the dialer likewise refuses a listener whose
signature does not verify. A signature from one connection is useless on another, because the listener's
nonce is fresh.

After the handshake the listener attributes every frame to the dialer's **authenticated identity**:
`peer_identity(node_id, key)` = the declared node ID, `#`, and the hex of the key the dialer proved it
holds. This identity, not a claimed ID and never the message's origin, is what `NodeRuntime` admits,
strikes and bans. A peer that declares an honest node's ID has a different key and so a different identity:
its strikes never reach the honest node. Each frame is signed by the dialer over a domain tag, the session
ID (SHA-256 of the transcript), the frame counter (0, 1, ... within the connection) and the whole message
(every signed field and the origin's signature). A party on the path can drop frames but cannot inject,
alter, reorder or replay them, so it cannot make the dialer appear to have sent an invalid or repeated
message. A frame with a wrong counter or signature closes the connection. Nothing is encrypted.

`message` is the origin-signed `WireMessage`; its author (`origin`) is kept separate from the connection's
identity, so a replay of an honest node's message is charged to whoever replayed it.

For each frame, `crci-node --listen` (`crates/crci-node/src/main.rs`, `run_node`):

1. checks the field rules from `validation.rs` (origin ID length, severity 1–5, sequence overflow, note at
   most 255 bytes) and drops a malformed message without a strike;
2. passes the message to `NodeRuntime`, which applies the checks above;
3. hands each message `NodeRuntime` accepted to `GossipPipeline` (`integration.rs`: AEDA, panic cooldown,
   duplicate/TTL, battery throttling, per-round rate limit). The pipeline only decides application-level
   handling and never strikes. Zone membership is not enforced on this path: every accepted origin is
   registered in the node's zone;
4. relays what `NodeRuntime` queued for other nodes over TCP, to every peer address given with
   `--peers` / `--dial` except the message's origin. The in-process router is switched off on a TCP node,
   because it picks next hops by node ID, which TCP cannot address. Every configured peer is a relay
   target from start-up, whether or not it is reachable yet. Each peer has one persistent authenticated
   connection and its own send queue (1 024 frames), so frames to a peer stay in order, a slow or
   unreachable peer delays only its own frames, and the receive loop never waits on the network. A
   connection the peer closed is detected before the next write and re-established with a new
   handshake; a frame that cannot be delivered is logged and dropped, and anti-entropy recovers it.

Every 5 s the binary advances the pipeline round and compares the severities reported by verified zone
members with their median (anchored at 3), treating deviations ≥ 2 as outliers unless 3f ≥ n. An outlier
is only logged and counted in `byzantine_events`; a validly signed report never changes reputation.
`/chain/head` reports the node's Merkle chain head. A node announces its chain head to its peers after an
inbox pass that delivered at least one message, if the head changed since its last announcement (a
snapshot is appended every 50 delivered messages); receiving an announcement never triggers one, so
neighbours do not echo announcements back and forth. Announcing only a changed head keeps the frames on
each link close to one per gossip message: every node relays every new message to all its neighbours, so
the per-neighbour token bucket (5 per second) bounds the network-wide sustained message rate.

### Re-synchronisation after a partition (anti-entropy)

`NodeRuntime` (`runtime.rs`) keeps the last W accepted messages of each origin (at most 1 024 origins).
To catch up, a node sends a neighbour a signed `sync_request`: a `WireMessage` whose `sync` digest lists,
per origin, the highest accepted sequence number and every sequence number held inside the window
(at most 256 origins), plus an optional `reply_to` address. The digest is covered by the requester's
signature. The neighbour answers only if the request came straight from its author (a relayed copy is
ignored, so a captured request cannot be used to flood its author), at most once per 5 s per requester
and about once per second in total (burst 4), with up to 16 of its recent messages that the digest lacks, oldest first, as ordinary signed messages.
Requests are never delivered, stored or forwarded.

The requester holds a 10 s sync session with the neighbour it asked. During the session a duplicate from
that neighbour of a message the digest did **not** list as held is dropped without a strike, once per
message and for at most 16 messages (one answer batch): it can be an answer that crossed with the same
message relayed as ordinary gossip. A duplicate of a message the digest listed as held, a second
duplicate, any duplicate beyond the first 16, and duplicates from any other peer follow the normal rules
and strike. The session is short and bounded so that periodic sync requests do not let a neighbour repeat
every message it relays once without a strike.

In-process, `MeshSimulator::heal(a, b)` restores a link and has both ends request. A TCP node requests
from every `--peers` / `--dial` address 0.5 s after start and every 30 s, and retries an unreachable peer
every 5 s, so a healed link is re-synchronised within about 5 s. The session is opened after the
listener's identity has been authenticated and before the request frame is sent. `reply_to` is the
node's listen address, so it must be an address the peers can reach: a node listening on `0.0.0.0` or
`::` prints a warning and does not request sync.

## `crci-core` modules

| Module | Role |
|---|---|
| `runtime.rs` | `NodeRuntime` and `WireMessage`: the message checks described above. |
| `sybil.rs` | `SybilGuard`: per-peer reputation, ban threshold, token bucket, proof-of-work check, subnet correlation. |
| `replay.rs` | `ReplayFilter`: per-origin sliding window (W = 64); `window_size = 0` gives strict-monotonic delivery. |
| `identity.rs` | Ed25519 key pair (`ed25519-dalek` 2) and signing. |
| `message.rs` | Application message and signal types. |
| `integration.rs` | `GossipPipeline`: application-level handling (AEDA, TTL, battery, panic cooldown) of messages the TCP node has accepted. |
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
| `crci-node` | Main CLI. `--listen` (+ `--peers`/`--dial`) runs a TCP node with the HTTP API on port 8080. `--send … --to …` sends one message. `--byzantine <mode> --to …` runs an adversary that sends signed frames (`replay`, `false_allclear`, `sybil`, `flood`, `partition_heal`) or frames that must be rejected (`bad_signature`, `unsigned`, `unsigned_mce`). With no arguments it runs a scripted in-process demonstration and serves the API. |
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
