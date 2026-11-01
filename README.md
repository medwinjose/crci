# CRCI: Crisis Response Communication Infrastructure

[![CI](https://github.com/medwinjose/crci/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/medwinjose/crci/actions/workflows/ci.yml) [![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0) [![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23033990.svg)](https://doi.org/10.5281/zenodo.23033990)

CRCI is a research prototype, written in Rust, of a peer-to-peer gossip protocol for crisis communication
when central infrastructure is down. Each node keeps a **local** reputation score for its neighbours and
stops accepting traffic from a neighbour after repeated, locally verifiable misbehaviour, such as replayed
messages or invalid Ed25519 signatures. There is no voting and no global consensus: every node decides on
its own.

## Status

This is a prototype under active research, not deployable software. In particular:

- The reputation and isolation mechanism runs in `NodeRuntime`, which today is exercised by the in-process
  simulator, the tests and the benchmarks. The networked `crci-node` binary exchanges unsigned messages over
  plain TCP and does not apply that mechanism yet.
- TCP connections are neither authenticated nor encrypted.
- BLE and LoRa transports are interface stubs.

See [docs/architecture.md](docs/architecture.md) and [docs/threat-model.md](docs/threat-model.md) for
details and the list of known gaps.

## Architecture

![CRCI architecture](docs/architecture/crci-architecture.png)

Interactive version: [docs/architecture/crci-architecture.html](docs/architecture/crci-architecture.html).

## Results

Recorded measurements, each with its harness, command and raw data, are in
[docs/benchmarks.md](docs/benchmarks.md). Headline (in-process harness, 50 deterministic trials per
condition): an adversary replaying a message is banned after exactly 3 strikes, and an honest peer
sending one valid message is never banned.

## Quick start

Requires a stable Rust toolchain.

```bash
git clone https://github.com/medwinjose/crci.git
cd crci
cargo build --workspace
cargo test --workspace
```

Run a node with the HTTP API on port 8080, then send it a message from a second terminal:

```bash
cargo run -p crci-node --bin crci-node -- --listen 127.0.0.1:7001 --node-id node-a
cargo run -p crci-node --bin crci-node -- --send "hello" --to 127.0.0.1:7001 --node-id node-b
```

Running `crci-node` with no arguments runs a scripted in-process demonstration instead.

Reproduce the paper benchmarks:

```bash
cargo bench -p crci-node --bench byzantine_bench
cargo bench -p crci-node --bench ablation_bench --features paper_v2
cargo bench -p crci-node --bench recovery_bench --features paper_v2
```

A Docker Compose demo (two honest nodes, one adversary, the web dashboard) is defined in
`docker-compose.yml`; see [scripts/README.md](scripts/README.md).

## Repository layout

| Path | Contents |
|---|---|
| `crates/crci-core` | Protocol library (also built as the Android shared library) |
| `crates/crci-node` | CLI binary, integration tests, benchmark harnesses |
| `crci-android` | Android demo app using the library through UniFFI |
| `web/dashboard` | React dashboard for one node's HTTP API |
| `docs` | Documentation, ADRs, TLA+ model, recorded evidence ([index](docs/README.md)) |
| `scripts` | Demo and build helper scripts |

## Paper and citation

- Preprint: [doi.org/10.5281/zenodo.23032042](https://doi.org/10.5281/zenodo.23032042)
- Software archive (all versions): [doi.org/10.5281/zenodo.23033990](https://doi.org/10.5281/zenodo.23033990)

Citation metadata is in [CITATION.cff](CITATION.cff).

## License

Apache License 2.0. See [LICENSE](LICENSE).
