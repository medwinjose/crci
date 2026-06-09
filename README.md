# CRCI — Crisis Response Communication Infrastructure

> Byzantine fault-tolerant mesh networking in Rust. Designed for
> resilience when up to ⌊(n-1)/3⌋ nodes are actively adversarial.

## What This Is
CRCI is a distributed systems research project demonstrating a secure, decentralized mesh networking stack for zero-infrastructure environments. It employs a reputation-weighted Byzantine fault-tolerant gossip protocol, utilizing strict cryptographic node identity and an autonomous emergency decision engine to prioritize critical telemetry. For a comprehensive overview of the theoretical models and evaluation methodology, please refer to our [arXiv paper preprint](docs/paper/crci_paper.md).

## Key Properties
- Byzantine fault tolerance (PBFT-inspired eviction, p95 < 512ms)
- Cryptographic node identity (Ed25519 + AES-GCM)
- Formal specification (TLA+ with TLC model checker)
- Cross-platform: Linux, Raspberry Pi (ARM), Android (via UniFFI FFI)
- Observability: Prometheus metrics, REST/WebSocket API, React dashboard

## Quick Start
### Run locally
```bash
cargo build --release
./target/release/crci-node --help
```

### Run the mesh (Docker)
```bash
docker compose up --build
```

### Run chaos engineering suite
```bash
bash scripts/chaos.sh
```

## Architecture
The CRCI networking stack fundamentally isolates physical transmission complexity from localized consensus mechanics. It is structured into three distinct layers:
1. **Transport Abstraction**: An extensible interface (currently supporting async TCP) handling low-level byte transmission and connection state.
2. **BFT Consensus & Routing**: Evaluates peer reputation, verifies cryptographic signatures, evicts malicious actors, and intelligently routes telemetry based on severity, K-bucket peer discovery, and TTL metrics.
3. **Application & Verification**: The `NodeRuntime` that applies business logic, stores Merkle-chained histories, and interacts with edge clients through FFI or REST/WebSocket.

For a deeper dive, visit the [Architecture Documentation](docs/book/src/architecture.md).

## Benchmarks
Real numbers from the 20-trial Byzantine loopback benchmark. Under active injection, the localized reputation engine strictly penalizes and evicts adversarial peers.
- p95 eviction latency: **< 512ms**
- Honest peer connection survivability: **20 / 20 trials**

For more granular results and methodology, see the [Benchmark Results](docs/book/src/benchmarks.md).

## Documentation
- [Architecture](docs/book/) — full mdBook site
- [TLA+ Specification](docs/tla/)
- [Chaos Engineering Report](docs/chaos/README.md)
- [Paper (preprint)](docs/paper/crci_paper.md)

## Status
v0.1.0 — research prototype. See CHANGELOG.md.

## License
MIT
