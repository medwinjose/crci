# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Entries before v2.1.0 are reconstructed from the
git history; `git log` is the complete record.

## [Unreleased]

### Changed
- Documentation rewritten to describe the code as it is: two message paths (signed `NodeRuntime` vs.
  unsigned TCP), the real transport (custom TCP, no libp2p/Noise), the real TLA+ model scope, and every
  benchmark with its command and raw data. Known gaps are listed in `docs/threat-model.md`.
- `docs/openapi.json` now describes all 15 HTTP/WebSocket routes; `docs/openapi.yaml` removed.
- `docs/design-decisions.md` folded into ADR 004 and ADR 005.
- Evidence reorganised into dated folders with SHA-256 hashes (`docs/evidence/README.md`).
- `crci-node --listen` carries origin-signed `WireMessage` frames over TCP and runs every received message
  through `NodeRuntime` (signature check, replay window, strikes, bans). Each frame names the node that sent
  it separately from the message's author. The frame format is not compatible with earlier versions.
- `crci-node --send` and `--byzantine` send signed messages; `--byzantine` adds the `bad_signature`,
  `unsigned` and `unsigned_mce` modes.
- The 5-second severity-median check in `crci-node` only counts outliers; it no longer publishes
  placeholder divergence alerts.

### Fixed
- Messages of type `mce` are no longer accepted without a valid signature.
- Idle peers are no longer banned: inactivity decay stops at the ban threshold, and only a strike bans.
- Packets older than the replay window are dropped without a strike; the W = 64 sliding window is always
  on, and the sequence number is covered by the origin's signature.
- Signature checks are bounded per sender by the token bucket instead of a lifetime cap of five per
  claimed origin, which dropped every later message from an honest node.
- `crci_version()` reports the crate version instead of a hard-coded `0.1.0`.
- The TCP transport no longer panics on clock errors and logs through `log` instead of standard output.
- `/chain/head` on a TCP node reports the node's Merkle chain head instead of the latest message ID.

### Removed
- The mdBook (`docs/book/`) and its workflow, `docs/fault-model.md`, `docs/chaos/README.md`,
  `docs/dashboard.html`, `docs/architecture.mmd`, the unused `docker/` Prometheus/Grafana config,
  one-off scripts, and two benchmark files that were never compiled.
- Tracked build products (`libcrci_core.so`) and TLC scratch state.
- The legacy environment-variable-driven `node` binary and `scripts/run_proof.ps1`.
- The `paper_v2` build feature (the sliding window is always on).
- The unused `--config` and `--argon2-memory` command-line options.

## [2.1.0] - 2026-09-29

### Added
- `ablation_bench` (window size vs. reorder depth) and `recovery_bench` (paced attack) harnesses.
- `CITATION.cff` with the preprint DOI.

### Changed
- Version bumped to 2.1.0; documentation reframed around local misbehaving-peer isolation.
- Benchmark CSVs frozen; bench targets no longer run under `cargo test`.

### Fixed
- Seen-message cache eviction (BFT-046) that caused a test failure.
- Clippy warnings under `-D warnings`.

## [2.0.0] - 2026-09-15

### Changed
- Cargo workspace split into `crci-core` and `crci-node`.

### Added
- 5-node network-namespace WAN smoke test in CI.

## [0.1.0] - 2026-06-09

Initial tagged release: node runtime with reputation scoring, replay filter, TCP transport, HTTP API,
Android FFI bindings and the web dashboard.

[Unreleased]: https://github.com/medwinjose/crci/compare/v2.1.0...HEAD
[2.1.0]: https://github.com/medwinjose/crci/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/medwinjose/crci/compare/v0.1.0...v2.0.0
[0.1.0]: https://github.com/medwinjose/crci/releases/tag/v0.1.0
