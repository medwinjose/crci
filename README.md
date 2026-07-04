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
### See It Work (One-Command Live Mesh & Dashboard Demo)
To launch the full multi-node mesh (node-alpha, node-beta, node-byzantine) along with the containerized React live dashboard in a single command, run the following:

- **Linux / macOS**:
  ```bash
  ./scripts/demo.sh
  ```
- **Windows (PowerShell)**:
  ```powershell
  ./scripts/demo.ps1
  ```

This script will start the mesh, wait for healthiness, automatically open your default browser to `http://localhost:5173`, and inject normal telemetry followed by a Byzantine attacker. Watch as the Byzantine node gets penalized and evicted dynamically in real time. Press `Ctrl+C` in the terminal to tear down all containers cleanly.

### Run locally
```bash
cargo build --release
./target/release/crci-node --help
```

### Gossip CLI Quick Start (Two-Node Communication Demo)
To demonstrate real-time async communication and message origination via the CLI:

1. **Start Node A** (listener):
   ```bash
   ./target/release/crci-node --listen 127.0.0.1:7001 --node-id node-a
   ```
   *Expected Output:*
   ```text
   Node node-a listening on 127.0.0.1:7001
   ```

2. **Start Node B** and peer it with Node A:
   ```bash
   ./target/release/crci-node --listen 127.0.0.1:7002 --node-id node-b --peers 127.0.0.1:7001
   ```
   *Expected Output:*
   ```text
   Successfully dialed peer: 127.0.0.1:7001
   Node node-b listening on 127.0.0.1:7002
   ```

3. **Originate a message from A to B**:
   In a new terminal window, send an emergency rescue message from A targeting B's listen address:
   ```bash
   ./target/release/crci-node --send "trapped under debris" --to 127.0.0.1:7002 --severity rescue --node-id node-a
   ```
   *Expected Output (Sender):*
   ```text
   SUCCESS: Message originated and sent to 127.0.0.1:7002
   ```

   *Expected Output (Node B's Terminal):*
   ```text
   Node node-b received message from node-a: kind=Rescue, payload_bytes=20
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
