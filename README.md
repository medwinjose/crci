# Crisis Response Communication Infrastructure (CRCI)

[![CI](https://img.shields.io/github/actions/workflow/status/medwinjose/crci/ci.yml?branch=main)](https://github.com/medwinjose/crci/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

CRCI is a distributed systems framework designed to maintain strict state consistency and network integrity across malicious or failing nodes.

## The Problem

In distributed systems, trust is often implicitly assumed between nodes on the network. When nodes become malicious or undergo Byzantine faults (e.g., sending conflicting information, forging messages, or dropping traffic), standard consensus protocols can break down, leading to divergent state or network partitions. Securing a network against these faults typically requires heavy cryptographic overhead and rigid topology assumptions, making it difficult to maintain performance under real-world WAN conditions.

## The Solution

CRCI introduces a resilient, cryptographically hardened peer-to-peer architecture built on `rust-libp2p`. It enforces strict Byzantine fault tolerance (BFT) via multi-layered validation, isolating malicious actors by tracking reputation and dynamically evicting nodes that violate consensus rules. By treating all inbound data as untrusted and requiring cryptographic proof for state transitions, CRCI ensures that the network converges to a single correct state even when a subset of nodes actively attempt to sabotage it. 

## Architecture

```mermaid
%%{init: {'theme': 'base', 'themeVariables': { 'primaryColor': '#ffffff', 'primaryBorderColor': '#333333', 'lineColor': '#666666', 'textColor': '#000000'}}}%%
graph TD
    Client["Client App / WebDashboard"] -->|"REST/JSON"| APILayer["API Layer — Actix Web(REST/JSON)"]
    APILayer --> CRCI_Node["CRCI Core Node"]
    
    subgraph CRCI Node
        CRCI_Node --> Consensus["Consensus Engine (BFT)"]
        CRCI_Node --> Mempool["Transaction Mempool"]
        CRCI_Node --> StateMachine["State Machine"]
        CRCI_Node --> Network["P2P Network Layer — libp2p"]
        CRCI_Node --> Crypto["Cryptographic Verification"]
        CRCI_Node --> Storage["Persistent Storage —RocksDB"]
        
        Consensus -->|"validates"| Mempool
        Consensus -->|"commits"| StateMachine
        Mempool -->|"broadcasts/receives"| Network
        StateMachine -->|"reads/writes"| Storage
        Crypto -->|"signs/verifies"| Network
        Crypto -->|"validates blocks"| Consensus
    end
    
    Network <-->|"gossipsub / Kademlia DHT"| Peer1["Peer Node 1"]
    Network <-->|"TCP / Noise Protocol"| Peer2["Peer Node 2"]
    Network <-->|"QUIC"| Peer3["Peer Node 3"]
```

## Key Results

- **Test Suite**: Backed by 195 passed tests.
- **WAN Chaos Testing**: Successfully converges under real-world WAN conditions (packet loss, high latency). The real network-namespace-based chaos test (`chaos_wan_real_tests.rs`) proves the system's resilience by running a 5-node topology using Linux network namespaces with `tc`/`netem` for traffic shaping. Note: This represents a deliberate 5-node downscale from the original 100-node target to accommodate CI runner resource limits without overstating the current verified scale.
- **Continuous Integration**: Green across all jobs (Ubuntu, Windows, and cross-compilation), including the `test_real_wan_chaos_netns` integration test. See the [Actions tab](https://github.com/medwinjose/crci/actions) for current build status.

## Quick Start

CRCI has been reorganized into a Cargo workspace. To build and run the project:

```bash
# Build the entire workspace
cargo build --workspace

# Run the test suite
cargo test --workspace

# Run a local CRCI node
cargo run --bin node
```

## Documentation & Papers

- [**CRCI Academic Paper (`docs/paper.md`)**](docs/paper.md): The formal research paper and architectural overview.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.
