# Threat Model

What CRCI defends against today, where the defence lives, and what is not defended. "Signed path" and
"TCP path" refer to the two message paths described in [architecture.md](architecture.md#two-message-paths).

## Goal and scope

CRCI aims to let each node **locally** isolate a neighbour that commits observable protocol violations,
without voting rounds or global agreement. It is not a consensus protocol and gives no agreement,
ordering or safety guarantee across nodes.

**Adversary considered:** a peer that commits acts of *commission* that a receiver can check by itself:
replaying messages it already sent, forwarding messages with invalid signatures, or presenting a second
public key for a known origin. The adversary cannot forge Ed25519 signatures of keys it does not hold.

**Not considered:** Sybil attacks at scale (many identities), collusion, majority-Byzantine networks,
semantically false but correctly signed reports, radio jamming, side channels, compromised devices.

## Defences that exist

| Threat | Defence | Where | Path |
|---|---|---|---|
| Forged message content | Ed25519 signature on every `WireMessage`; invalid → dropped, sender struck | `runtime.rs` | Signed only |
| Impersonating a known origin | Public key pinned on first sight; a different key → dropped, sender struck | `runtime.rs` | Signed only |
| Direct replay | Same message ID from the same sender → strike; from a different sender → ignored | `runtime.rs` | Signed only |
| Sequence replay | Sequence ≤ highest seen → strike (sliding window W = 64 under `paper_v2`) | `replay.rs` | Signed only |
| Repeat offender | Reputation 0.5, −0.15 per strike, banned below 0.1; banned senders are dropped on admission | `sybil.rs` | Signed only |
| Message flood | Token bucket per sender (20 messages, +5/s) | `sybil.rs` | Signed only |
| Malformed input | Severity range, payload size, node-ID format, sequence overflow, rate limit | `validation.rs`, `integration.rs` | Both |
| Oversized / slow TCP frames | 1 MiB frame limit, 5 s read timeouts, 100 concurrent connections | `transport/tcp.rs` | TCP |
| Data at rest | AES-256-GCM, key derived with Argon2id | `storage/encrypted.rs` | Local storage |
| Panics across FFI | Rust panics surface as Kotlin exceptions (recorded run: `docs/evidence/android-bft038-2026-08-23/`) | `ffi.rs` | Android |

## Known gaps (open, with the task that addresses each)

1. **The TCP path has no isolation mechanism.** `crci-node` exchanges unsigned `PipelineMessage` values and
   never calls `NodeRuntime`; signatures, replay filter, strikes and bans are not applied to network
   traffic (planned).
2. **The transport sender is self-declared.** The TCP listener uses the message's `origin_node` field as
   the sender. Any connection can claim any identity, so strikes cannot be bound to a real neighbour
   (authenticated handshake planned). There is no handshake, and TCP traffic is neither authenticated nor encrypted.
3. **Lifetime signature-check cap.** After 5 signature verifications for an origin, all later messages from
   that origin are dropped, honest or not (fix planned).
4. **Unsigned mass-casualty events.** Messages of type `mce` skip signature and key checks on the signed
   path (fix planned).
5. **Idle peers get banned.** Reputation decays at 0.05/s after 2 s of silence and the decay can cross the
   ban threshold, so an honest peer silent for about 10 s is banned (fix planned).
6. **Sliding window is off by default.** The default build rejects any out-of-order sequence as a replay
   (planned).
7. **Paced replays evade eviction.** An adversary that sends enough valid traffic between replays recovers
   reputation faster than it loses it (measured in [benchmarks.md](benchmarks.md); strike-damped recovery planned).
8. **Duplicate-ID cache resets.** The seen-message cache is cleared completely at 5 000 entries, after which
   earlier IDs can be replayed once more without a duplicate strike (the sequence check still applies).
9. **No partition recovery protocol.** There is no anti-entropy or digest exchange after a partition
   (planned).
10. **Sybil resistance is minimal.** `sybil.rs` has a proof-of-work check, but only tests call it, and a
    "start at 0.2 if the /24 subnet has a banned peer" rule, which only applies to sender IDs that parse
    as socket addresses. Fresh key pairs are otherwise free.
