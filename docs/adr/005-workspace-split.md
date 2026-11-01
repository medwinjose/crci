# ADR 005: Cargo Workspace Split (`crci-core` / `crci-node`)

- **Date:** 2026-09-12
- **Status:** Accepted, partially realised

## Context

The code started as one crate holding the protocol, the CLI, the HTTP API, demos and tests. The Android
app links the protocol as a shared library and should not carry desktop-only dependencies.

## Decision

Split into a Cargo workspace:

- `crates/crci-core`: library (`lib` + `cdylib`) with the protocol logic and the UniFFI exports.
- `crates/crci-node`: the CLI binary, helper binaries, integration tests and benchmark harnesses.

## Consequences

- Tests and benchmarks live in `crci-node` and depend on `crci-core` only through its public API.
- **Not yet achieved:** `crci-core` still contains the Axum HTTP API (`api.rs`) and the simulation and
  demo modules, and depends on `axum`, `tower-http`, `clap` and `tokio` (full). These are therefore still
  compiled into the Android library. Moving them into `crci-node` (or behind a feature) remains to be done.
