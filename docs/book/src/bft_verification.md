# BFT Verification Reference

This document is the canonical reference for CRCI's active Byzantine Fault Tolerance vectors.
Every claim below traces to actual code, test functions, and commit hashes in the repository.

> **Numbering note**: BFT-011 through BFT-029 are **permanently retired** due to contaminated
> provenance identified during the Session 84 audit. They must not be reused for new vectors.
> Active legitimate IDs are BFT-004, 008, 009, 030, 046, and 047.

---

## BFT-004: XOR Hash Collision Guard

**Protects against**: A Sybil attacker generating node IDs whose XOR hash collides with
an existing peer's hash in the K-bucket discovery table. If accepted, the attacker could
shadow a legitimate peer and intercept its traffic. The guard in `PeerTable::upsert()`
computes a SHA-256 hash of the candidate node ID and rejects insertion if the hash matches
any existing peer.

**Production code**:
- `crates/crci-core/src/discovery.rs:131` — XOR collision check in `PeerTable::upsert()`
- `crates/crci-core/src/discovery.rs:139` — Log warning on rejection

**Test**:
- `crates/crci-node/tests/bft_batch1_tests.rs:420` — `test_bft_xor_hash_collision_guard()`

**Commits**:
- `5780603` (Session 74): BFT batch 1 partial — introduced the test
- `a73065b` (Session 141): decoupled sig_verifications_count eviction

---

## BFT-008: Signature Verification Rate Limit

**Protects against**: A malicious peer flooding the network with messages that each require
an Ed25519 signature verification, exhausting CPU resources. The `process_inbox()` method in
`NodeRuntime` maintains a per-peer counter (`sig_verifications_count`) and drops messages from
any peer exceeding the threshold (5 verifications per round), incrementing `byzantine_events`.

**Production code**:
- `crates/crci-core/src/runtime.rs:384` — BFT-008 bounded signature verification check
- `crates/crci-core/src/runtime.rs:465-469` — Comment documenting that BFT-008 counters are
  intentionally NOT cleared during BFT-046 cache eviction

**Tests**:
- `crates/crci-node/tests/bft_batch1_tests.rs:75` — `test_bft_signature_verification_rate_limit()` (sends 10
  messages from one peer, asserts `byzantine_events >= 5`)
- `crates/crci-node/tests/bft_batch1_tests.rs:536` — `test_bft008_persists_across_bft046_eviction()` (BFT-047,
  confirms throttle count survives cache eviction)

**Commits**:
- `5780603` (Session 74): introduced the rate-limit test
- `a73065b` (Session 141): decoupled BFT-008 from BFT-046 eviction
- `9e84fae` (Session 147): corrected stale BFT-046 assertion

---

## BFT-009: Untrusted Node Escalation Guard

**Protects against**: Low-reputation (untrusted, unvouched) nodes artificially triggering
AEDA zone-level escalations. The `AedaEngine::process()` method checks the reporting node's
reputation against `MIN_TRUSTED_REP` (0.41) and silently drops escalation events from nodes
below this threshold.

**Production code**:
- `crates/crci-core/src/aeda.rs:143` — `return;` guard for reputation < 0.41
- `crates/crci-core/crates/crci-core/src/integration.rs:194` — sandbox weight for untrusted nodes in rescue processing

**Test**:
- `crates/crci-node/tests/bft_batch1_tests.rs:445` — `test_bft_untrusted_node_escalation_guard()` (sends 3
  events from nodes with rep 0.3, asserts no `ZoneEscalated` decision)

**Commits**:
- `5780603` (Session 74): BFT batch 1 partial — introduced the test and production guard
- `a73065b` (Session 141): decoupled sig_verifications_count eviction

---

## BFT-030: Trusted Node Passthrough Verification

**Protects against**: The complementary case to BFT-009 — verifies that legitimate
high-reputation nodes (rep ≥ 0.41) correctly DO trigger AEDA zone escalations when multiple
independent trusted reporters converge on the same severity signal. This is the positive
counterpart ensuring the guard doesn't over-block.

**Production code**:
- `crates/crci-core/src/aeda.rs:143` — same guard (trusted nodes pass through)
- `crates/crci-core/crates/crci-core/src/integration.rs:194` — full weight applied to trusted nodes

**Test**:
- `crates/crci-node/tests/bft_batch1_tests.rs:475` — `test_bft_trusted_node_passthrough_guard()` (sends 3
  events from nodes with rep 0.9, asserts `ZoneEscalated` decision IS produced)

**Commits**:
- `5780603` (Session 74): BFT batch 1 partial — introduced the test

---

## BFT-046: Seen Messages Cache Hard Ceiling

**Protects against**: Unbounded memory growth in the `seen_messages` HashSet used for message
deduplication. Without a ceiling, an attacker could flood the network with unique message IDs
until the node runs out of memory. When `seen_messages.len()` exceeds 5000, the runtime
clears the entire cache and re-inserts only the triggering message (plus a chain-head
announcement originated during `process_inbox()`).

**Production code**:
- `crates/crci-core/src/runtime.rs:464` — BFT-046 hard ceiling check and cache clear

**Test**:
- `crates/crci-node/tests/bft_batch1_tests.rs:505` — `test_bft_seen_messages_cache_ceiling()` (inserts 5001
  messages, asserts cache contains exactly 2 entries after eviction: the triggering message
  and a chain-head announcement)

**Commits**:
- `a73065b` (Session 141): introduced the decoupled eviction logic
- `9e84fae` (Session 147): corrected stale assertion from 1 to 2

---

## BFT-047: Signature Throttle Persistence Across Cache Eviction

**Protects against**: An attacker exploiting BFT-046 cache eviction to reset their BFT-008
signature verification counter. Prior to the Session 141 fix, clearing `seen_messages` also
cleared `sig_verifications_count`, allowing a peer to flood traffic until the 5000-entry
ceiling triggered, then resume signature-check abuse with a fresh counter. BFT-047 verifies
that `sig_verifications_count` is NOT cleared when `seen_messages` is evicted.

**Production code**:
- `crates/crci-core/src/runtime.rs:465-469` — explicit comment documenting the intentional non-clearing

**Test**:
- `crates/crci-node/tests/bft_batch1_tests.rs:536` — `test_bft008_persists_across_bft046_eviction()` (fills
  seen_messages to 5001, then sends 3 more messages from the same peer, asserts
  `sig_verifications_count` is 3, confirming the counter was not reset by the eviction)

**Commits**:
- `a73065b` (Session 141): introduced the decoupled logic
- `9b554b6` (Session 144): applied the BFT-047 label to the test

---

## TLA+ Formal Verification Parity

**Status**: VERIFIED WITH GAPS (2026-09-08, Session 146, commit `cc5e805`).

TLC model checker run on `MC.tla` (extending `CRCI.tla` + `CRCITypes.tla`) using `MC.cfg`
(4 nodes, 1 Byzantine, MaxSeq=0):

| Metric | Value |
|---|---|
| States generated | 649 |
| Distinct states | 81 |
| Safety invariants checked | 4 (NoForgedOrigin, ReplayNeverDeliveredTwice, ByzantineContainment, RescueNeverDropped) |
| Temporal properties checked | 1 (RescueLiveness) |
| Result | All passed, no errors |

**Documented gaps** (spec does not model):
- BFT-046 `seen_messages` cache eviction
- BFT-008 `sig_verifications_count` throttling
- Chain-head announcement origination
- Any post-Session-66 runtime features
- `GossipProgress` property is defined in `CRCI.cfg` but NOT checked in `MC.cfg`
  (the config actually used for the TLC run)

Source: `CURRENT_STATE.md`, Session 146 TLA+ parity entry.
