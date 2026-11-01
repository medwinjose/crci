# CRCI — Crisis Response Communication Infrastructure

[![CI](https://github.com/medwinjose/crci/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/medwinjose/crci/actions/workflows/ci.yml) [![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0) [![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/) [![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23033990.svg)](https://doi.org/10.5281/zenodo.23033990)

**A peer-to-peer mesh protocol, written in Rust, in which every node detects and isolates misbehaving
neighbours on its own — no central server, no voting rounds, no global consensus.**

---

## The Problem

When a disaster strikes, cellular towers and internet links are often the first things to fail, at the
moment coordination matters most. Ad-hoc mesh networks of phones and radios can restore reachability,
but an open mesh has no gatekeeper. Any participant can replay old alerts to trigger false alarms,
forward tampered messages, or flood its neighbours.

The classical answer, Byzantine fault-tolerant consensus such as PBFT, assumes a stable, known membership
and exchanges O(n²) messages per agreement round. Neither assumption holds on intermittently connected,
low-bandwidth links (LoRa runs at a few kbit/s), where nodes join, leave and partition constantly.

## The Solution

CRCI moves misbehaviour isolation **into the gossip layer itself** and makes it **purely local**:

- **Every message is signed.** Each node has an Ed25519 identity; messages carry the author's public key,
  a signature and a sequence number.
- **Each node keeps its own reputation score for each neighbour.** Only violations a receiver can verify
  by itself count against a neighbour: replaying a message it already sent, forwarding an invalid
  signature, or presenting a second public key for a known author.
- **Three strikes isolate.** A neighbour starts at 0.5 and loses 0.15 per strike; below 0.1 it is banned
  locally. The same message arriving through a *different* neighbour is ordinary gossip redundancy and is
  never penalised.
- **Reordering is tolerated.** A sliding-window replay filter (W = 64) accepts late packets inside the
  window; packets older than the window are dropped without a strike, since an honest neighbour can
  deliver old messages after a partition.
- **No coordination.** No voting, no membership protocol, no global state: dissemination stays O(n) per
  message, and every node keeps working while partitioned.

| Design decision | Instead of | Why | Record |
|---|---|---|---|
| Local reputation and isolation | PBFT-style consensus | No membership requirement; O(n) instead of O(n²) traffic | [ADR 002](docs/adr/002-reputation-over-pbft.md) |
| Custom transport behind a `Transport` trait | A general-purpose P2P framework | Small, auditable networking layer; other media (BLE, LoRa) plug in behind the same trait | [ADR 001](docs/adr/001-custom-tcp-transport.md) |
| Hash-chained local state | CRDTs | Ordered, tamper-evident record of what a node has seen | [ADR 003](docs/adr/003-merkle-over-crdt.md) |

## Architecture

![CRCI architecture](docs/architecture/crci-architecture.png)

Interactive version: [docs/architecture/crci-architecture.html](docs/architecture/crci-architecture.html) ·
written description: [docs/architecture.md](docs/architecture.md).

Every incoming message, in-process or over TCP, passes through the same local pipeline in `NodeRuntime`:

```
admission (ban list, token bucket) → Ed25519 verification → duplicate check per neighbour
  → author-key pinning → replay filter → accept
                 └──── each verifiable violation: −0.15 reputation for the sending neighbour ────┘
```

## Results

From the accompanying preprint ([doi.org/10.5281/zenodo.23032042](https://doi.org/10.5281/zenodo.23032042)),
measured with the in-process benchmark harness, 50 deterministic trials per condition:

| Condition | Strikes to isolation | Isolation time (mean) | Honest peer isolated |
|---|---|---|---|
| Replay attack, no delay | 3 | < 1 ms | 0 / 50 |
| Replay attack, 15 ms between replays | 3 | 45.8 ms | 0 / 50 |

- **Reordering:** late or stale packets never count against a neighbour; packets more than W behind
  are dropped instead of delivered. Measuring delivery under reordering on the network testbed is planned.
- **Known limit:** an adversary that paces its replays and sends enough valid traffic in between recovers
  reputation faster than it loses it. This evasion was measured on an earlier build and is disclosed; it
  is the focus of ongoing work on strike-damped recovery.

Every number above links back to a raw data file and the exact command in
[docs/benchmarks.md](docs/benchmarks.md).

## Engineering

- **Rust workspace:** `crci-core` (protocol library, also built as a shared library) and `crci-node`
  (CLI node, integration tests, benchmark harnesses).
- **Security:** Ed25519 signatures (`ed25519-dalek`), AES-256-GCM encrypted local storage with an
  Argon2id-derived key, per-neighbour rate limiting.
- **Interfaces:** HTTP + WebSocket API ([OpenAPI](docs/openapi.json)), Prometheus metrics, a React
  dashboard, and an Android app that calls the Rust core directly through UniFFI-generated Kotlin bindings.
- **Verification:** unit and integration test suite, a TLA+ model of message dissemination
  ([docs/formal-verification.md](docs/formal-verification.md)), and a Linux network-namespace test with
  `tc netem` delay and loss in CI.
- **CI:** formatting, Clippy with warnings denied, the full test suite on Ubuntu and Windows,
  cross-compilation for ARMv7 and AArch64 (Raspberry Pi), and a documentation check.

## Quick start

Requires a stable Rust toolchain.

```bash
git clone https://github.com/medwinjose/crci.git
cd crci
cargo build --workspace
cargo test --workspace
```

Run a node (HTTP API on port 8080), then send it a message from a second terminal:

```bash
cargo run -p crci-node --bin crci-node -- --listen 127.0.0.1:7001 --node-id node-a
cargo run -p crci-node --bin crci-node -- --send "hello" --to 127.0.0.1:7001 --node-id node-b
```

Reproduce the benchmarks:

```bash
cargo bench -p crci-node --bench byzantine_bench
cargo bench -p crci-node --bench ablation_bench
cargo bench -p crci-node --bench recovery_bench
```

A Docker Compose demo (two honest nodes, one adversarial node, and the dashboard) is in
`docker-compose.yml`; see [scripts/README.md](scripts/README.md).

## Research status and roadmap

CRCI is an active research project. The isolation pipeline runs in the node runtime used by both the
networked TCP node and the test suite; the benchmarks so far drive it in-process. Current work, in order:

1. Signatures over every message field, and an authenticated peer handshake so that strikes on TCP bind to
   a verified neighbour.
2. Strike-damped recovery to close the paced-attack evasion.
3. Anti-entropy re-synchronisation after network partitions.
4. A 20–50 node network-emulation testbed (loss, delay, reordering, link flapping) and comparison against
   baselines.
5. Safety and liveness invariants of the final protocol in TLA+.

Out of scope: large-scale Sybil attacks, colluding adversaries, and physical LoRa/BLE radios (the BLE and
LoRa transports are interface stubs). Known gaps are tracked in [docs/threat-model.md](docs/threat-model.md).

## Repository layout

| Path | Contents |
|---|---|
| `crates/crci-core` | Protocol library: runtime, reputation, replay filter, cryptography, storage, transports, API |
| `crates/crci-node` | CLI node, integration tests, benchmark harnesses |
| `crci-android` | Android app using the Rust core through UniFFI |
| `web/dashboard` | React dashboard for a node's HTTP API |
| `docs` | Architecture, threat model, benchmarks, ADRs, TLA+ model, raw evidence ([index](docs/README.md)) |
| `scripts` | Demo, build and documentation-check scripts |

## Paper and citation

**CRCI: Local Reputation-Based Fault Isolation for Ad-Hoc Gossip Communication** — Medwin Jose,
Vellore Institute of Technology, Bhopal.

- Preprint: [doi.org/10.5281/zenodo.23032042](https://doi.org/10.5281/zenodo.23032042)
- Software archive (all versions): [doi.org/10.5281/zenodo.23033990](https://doi.org/10.5281/zenodo.23033990)

Citation metadata is in [CITATION.cff](CITATION.cff).

## License

Apache License 2.0. See [LICENSE](LICENSE).
