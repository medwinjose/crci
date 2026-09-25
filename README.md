# Crisis Response Communication Infrastructure (CRCI)

[![CI](https://github.com/medwinjose/crci/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/medwinjose/crci/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

CRCI is a distributed systems framework designed to maintain strict state consistency and network integrity across malicious or failing nodes.

## The Problem

In distributed systems, trust is often implicitly assumed between nodes on the network. When nodes become malicious or undergo Byzantine faults (e.g., sending conflicting information, forging messages, or dropping traffic), standard consensus protocols can break down, leading to divergent state or network partitions. Securing a network against these faults typically requires heavy cryptographic overhead and rigid topology assumptions, making it difficult to maintain performance under real-world WAN conditions.

## The Solution

CRCI introduces a peer-to-peer mesh protocol implemented in Rust that integrates local misbehaving-peer isolation directly into the gossip layer. Rather than attempting to reach globally synchronized Byzantine fault tolerance (BFT) or quorum consensus, CRCI enforces a purely local reputation-based peer eviction mechanism using a deterministic three-strike threshold. By binding authenticated Ed25519 identities to protocol-verifiable behavioral evidence (e.g., detecting explicit message replays or invalid signatures), CRCI allows honest nodes to independently isolate malicious relays without requiring voting rounds or stable membership.

## Architecture

![CRCI Architecture](docs/architecture/crci-architecture.png)

## Key Results

- **Test Suite**: Backed by 195 passed tests.
- **WAN Chaos Testing**: Runs on an automated 5-node Linux network-namespace testbed (`tc`/`netem`) validating partition tolerance and packet-jitter behavior under real kernel-level network emulation. This is a CI-scale subset of the 100-node target topology, chosen to fit CI runner resource limits.
- **Continuous Integration**: Green across all jobs (Ubuntu, Windows, and cross-compilation), including the `test_real_wan_chaos_netns` integration test. See the [Actions tab](https://github.com/medwinjose/crci/actions) for current build status.

### Byzantine Eviction Benchmarks (100 total runs)

| Condition | Strikes to Ban | Eviction Latency (Mean) | Min / Max Latency | Honest Survival Rate |
| :--- | :--- | :--- | :--- | :--- |
| **Baseline** | 3 | $< 1$ ms | $< 1$ ms | 50/50 |
| **Fixed 15 ms Delay** | 3 | 45.8 ms | 45 ms / 46 ms | 50/50 |

Data source: `crates/crci-node/benches/` test harness.

## Quick Start

CRCI has been reorganized into a Cargo workspace. To build and run the project:

```bash
# Clone the repository
git clone https://github.com/medwinjose/crci.git
cd crci

# Build the entire workspace
cargo build --workspace

# Run the test suite (195 tests)
cargo test --workspace

# Spin up a local simulated multi-node mesh
docker compose up --build

# Or run a single local node directly
cargo run --bin node
```

## Documentation & Papers

- [**CRCI Academic Preprint (`docs/preprint.md`)**](docs/preprint.md): The formal research preprint detailing the architecture and experimental validation.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.
