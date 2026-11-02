# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Entries before v2.1.0 are reconstructed from the
git history; `git log` is the complete record.

## [Unreleased]

### Added
- Emulation testbed (`scripts/testbed/`): real `crci-node` processes in Linux network namespaces with
  netem delay, random geometric and random 4-regular topologies, a replaying relay, per-run analysis and
  fidelity checks.
- `crci-node` experiment options: `--emit-count`, `--emit-interval-ms`, `--emit-delay-ms` (periodic
  signed messages), `--replay-after-ms`, `--replay-copies` (fault injection) and
  `--experiment-epoch-unix-ms` (a shared time origin for those schedules). Origination, acceptance
  and bans are logged with a Unix-millisecond timestamp. `--sender-refill-per-sec` sets the per-neighbour
  budget refill for experiments.

### Changed
- The per-neighbour budget refills at 20 messages per second instead of 5, and only messages the
  receiver has not seen spend it. Chosen from a testbed sweep (refill 5/10/20/50 per second, offered load
  4–32 messages/s): 20 is the lowest rate with no honest frame dropped at 16 messages/s. Before, the
  network as a whole could sustain about 5 messages per second.
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
- Honest neighbours are no longer banned for late sync answers under packet loss. A sync answer is tagged
  with the request it answers (covered by the TCP frame signature, removed on receipt), and only a tagged
  answer to a request this node sent to that neighbour, for a message the request did not list as held, is
  excused, once, within 120 s. Before, a request delayed by retransmissions was answered after the
  requester's 10 s session had expired, and the answer's repeats of already-relayed messages were struck
  as replays (measured at 10–20 % loss); and before that, a 30 s session renewed by the 30 s sync loop
  let a neighbour repeat every message it relayed once without a strike. Untagged repeats are no longer
  excused at all.
- A repeat of a message from the same sender is judged before the sender's rate limit, so it is struck
  even when the sender is over budget. Before, a sender that kept its budget empty had its replays
  dropped by the rate limit without a strike.
- A copy of a message already received from another neighbour is dropped before the rate limit and the
  signature check. Before, every such copy spent a token and a signature check, so honest gossip
  redundancy alone exhausted the per-neighbour budget.
- `crci-node` no longer delays start-up while probing its peers: the probes run concurrently with a 2 s
  timeout. Before, each unreachable peer was probed in turn (about 21 s each on Windows) before the node
  processed any traffic.
- A node announces its chain head only when it changed. Before, it re-announced an unchanged head to every
  peer after every delivered message, doubling the frames on every link.
- Message and identity signatures are checked with strict Ed25519 verification: a small-order public key
  with a fixed signature previously verified for every message.
- A node listening on `0.0.0.0` or `::` warns that anti-entropy is off instead of sending sync requests
  whose answers could not reach it.
- Messages of type `mce` are no longer accepted without a valid signature.
- Idle peers are no longer banned: inactivity decay stops at the ban threshold, and only a strike bans.
- Packets older than the replay window are dropped without a strike; the W = 64 sliding window is always
  on, and the sequence number is covered by the origin's signature.
- Signature checks are bounded per sender by the token bucket instead of a lifetime cap of five per
  claimed origin, which dropped every later message from an honest node.
- The origin's signature covers every `WireMessage` field (note, message type, confidence, flags,
  visibility and sequence number included) in a length-prefixed, domain-separated encoding. Before, it
  covered only the ID, origin, severity and sequence number joined with `:`, so a relay could change the
  other fields, and IDs containing `:` were ambiguous. Signatures made by earlier versions do not verify.
- TCP connections start with a mutual Ed25519 challenge-response, and every frame is signed by the dialer
  for the session and numbered. Strikes and bans attach to the dialer's authenticated identity (declared
  node ID plus proved key) instead of a self-declared sender ID, so a peer can no longer get another node
  struck by claiming its ID, and frames cannot be injected, altered or replayed on the path.
  `TcpTransport::bind` takes the node's signing key. The frame format is not compatible with earlier
  versions.
- A message whose origin is pinned to another public key is dropped without a strike on the relay. The
  receiver cannot tell which key is genuine, and the strike let an adversary that announced an honest
  node's ID first get that node's honest relays banned.
- Property-based tests (proptest) of the replay window: with reordering depth at most W, every honest
  message is accepted once and no relay loses reputation; at any depth, no honest relay is struck.
- Reputation recovery depends on elapsed time only (0.0005 per second, one strike forgiven in 300 s)
  and never raises a score above the initial 0.5; two strikes within 300 s cap recovery at 0.18. Before,
  recovery grew with the peer's own traffic, so an adversary sending valid messages between replays was
  never banned, and good behaviour could be banked for a burst of up to six replays. Idle decay is
  removed: inactivity no longer changes a score. Banned peers recover at the same rate and are readmitted
  at 0.25.
- `recovery_bench` rewritten: exact duplicate replays on a simulated clock, sweeping the replay period.
- Anti-entropy after a partition: a node asks a neighbour for what it missed with a signed digest
  (`sync_request`), and the neighbour answers with up to 16 recent messages the digest lacks, at most
  once per 5 s per requester and about once per second in total. Answers
  that cross with ordinary gossip are not struck; unsolicited duplicates still are. `crci-node` requests
  from its `--peers` at start-up, every 30 s, and within 5 s of an unreachable peer coming back.
  `MeshSimulator` gains `disconnect` and `heal`. `TcpTransport::dial` opens an authenticated connection
  and reports the listener's identity.
- A node no longer announces its chain head after a pass that only received announcements; two flooding
  neighbours used to echo announcements until their rate limits dropped real messages.
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
