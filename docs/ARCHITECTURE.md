# CRCI Architecture

CRCI is organized into a modular Cargo workspace with clearly defined subsystem boundaries.

## Repository Layout

- **`crates/crci-core/`**: The core library crate containing all business logic, protocol rules, cryptography, routing, Sybil resistance, state management, and the `rust-libp2p` networking layer. This module has no knowledge of external APIs or CLIs.
- **`crates/crci-node/`**: The primary executable wrapper that handles argument parsing (`clap`), initializing the Tokio asynchronous runtime, and standing up the REST/WebSocket API endpoints. It depends on `crci-core`.
- **`crci-android/`**: Contains the Kotlin Android application which consumes the core Rust logic via Foreign Function Interface (FFI).
- **`bindings/`**: Contains the UniFFI setup that automatically generates Kotlin bindings from the `crci-core` Rust API, allowing Android to interact natively with the peer-to-peer network.
- **`web/`**: The React-based Web Dashboard that connects to the `crci-node` REST API to provide a live view of network telemetry, node reputation, and partition status.
- **`docker/`**: Contains Dockerfiles and `docker-compose.yml` for standing up the containerized 3-node mesh and web dashboard.
- **`docs/`**: Contains project documentation, paper drafts, benchmark results, and formal TLA+ specifications.
- **`scripts/`**: Development and testing scripts.
- **`.cargo/`**: Cargo toolchain and linker configuration for cross-compilation targets.
- **`.github/`**: CI workflow definitions (GitHub Actions) and issue/PR templates.
- `target/` is the standard Cargo build artifact directory and is gitignored.

## Subsystem Interactions

1. **Client to API Layer**: The `web` dashboard and external CLI tooling communicate with the `crci-node` over standard REST JSON endpoints.
2. **API Layer to Core**: The `crci-node` binary translates REST commands into async Rust function calls into the `crci-core` library.
3. **Core to FFI**: The `crci-android` app bypasses the REST API entirely. Instead, it statically links against a compiled `.so` of `crci-core` using the scaffolding in `bindings/`, calling core Rust methods directly from Kotlin.
4. **Core to Network**: Inside `crci-core`, the `rust-libp2p` stack handles all inter-node communication, gossiping validated payloads and enforcing Byzantine reputation rules before surfacing data to the state machine.
