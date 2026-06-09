# Changelog

All notable changes to CRCI are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] — 2026-06-09

### Core Protocol
- Byzantine fault-tolerant node eviction with p95 latency < 512ms (20-trial benchmark)
- Cryptographic node identity: Ed25519 keypairs, AES-GCM message encryption
- Peer registry with concurrent state management (DashMap)
- Byzantine agent simulator for adversarial injection testing

### Transport
- Async TCP transport layer (Tokio)
- Prometheus metrics endpoint
- Axum REST API with OpenAPI 3.1 specification
- WebSocket real-time event stream

### Cross-Platform
- Linux x86_64 (primary)
- Raspberry Pi ARMv7 cross-compilation pipeline
- Android library via UniFFI v0.28 Kotlin bindings + Jetpack Compose app

### Formal Methods
- TLA+ specification of BFT eviction protocol
- TLC model checker integration

### Observability & Tooling
- React/Vite/Tailwind/Recharts web dashboard
- Docker Compose 3-node mesh with chaos engineering suite
- 4-scenario adversarial test harness (link partition, node crash, latency, combined)
- mdBook documentation site with architecture, BFT, benchmark, chaos pages
- GitHub Actions CI (4/4 checks)

### Research
- arXiv paper draft: system design, formal properties, benchmark methodology
- 221 passing unit and integration tests, zero Clippy warnings
