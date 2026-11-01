# Threat Model

What CRCI defends against today, where the defence lives, and what is not defended. Every message, in-process
or over TCP, goes through the same checks in `NodeRuntime` ([architecture.md](architecture.md#message-path)).

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

| Threat | Defence | Where |
|---|---|---|
| Forged or altered message content | Ed25519 signature on every `WireMessage`, every message type included, covering every field in an unambiguous, domain-separated encoding; invalid → dropped, sender struck | `runtime.rs` |
| Impersonating a known origin | Public key pinned on first sight; a different key → dropped, sender struck | `runtime.rs` |
| Direct replay | Same message ID from the same sender → strike; from a different sender → ignored | `runtime.rs` |
| Sequence replay | Sliding window W = 64 per origin: same sequence from the same sender → strike; from another sender → ignored; older than the window → dropped without a strike. The sequence number is signed | `replay.rs` |
| Repeat offender | Reputation 0.5, −0.15 per strike, banned below 0.1; banned senders are dropped on admission | `sybil.rs` |
| Message flood | Token bucket per sender (20 messages, +5/s), applied before the signature check | `sybil.rs` |
| Signature-verification flood | Verifications bounded by the sender's token bucket; each invalid signature is a strike, so three bad signatures ban the sender | `runtime.rs`, `sybil.rs` |
| Malformed input | Origin-ID length, severity range, sequence overflow and note size checked before `NodeRuntime` on a TCP node | `validation.rs`, `crci-node` `main.rs` |
| Framing an author by replaying its message | On TCP each frame names its sender separately from the message's author; strikes fall on the sender | `transport/tcp.rs` |
| Oversized / slow TCP frames | 1 MiB frame limit, 5 s read timeouts, 100 concurrent connections | `transport/tcp.rs` |
| Data at rest | AES-256-GCM, key derived with Argon2id | `storage/encrypted.rs` |
| Panics across FFI | Rust panics surface as Kotlin exceptions (recorded run: `docs/evidence/android-bft038-2026-08-23/`) | `ffi.rs` |

## Known gaps (open, with the task that addresses each)

1. **The transport sender is self-declared.** A TCP frame names its sender, and `NodeRuntime` admits,
   strikes and bans that ID, but nothing on the connection proves it. An adversary can change its sender ID
   to escape a ban, or claim an honest neighbour's ID so that its own replays are struck against that
   neighbour (authenticated handshake planned). TCP traffic is neither authenticated nor encrypted.
2. **Stale floods are not penalised.** A packet older than the replay window is dropped without a strike,
   so an adversary can resend old signed messages without losing reputation. Each one still costs a
   signature verification; the cost is bounded only by the per-sender token bucket (20 messages, then 5/s).
3. **Paced replays evade eviction.** An adversary that sends enough valid traffic between replays recovers
   reputation faster than it loses it (measured in [benchmarks.md](benchmarks.md); strike-damped recovery planned).
4. **Duplicate-ID cache resets.** The seen-message cache is cleared completely at 5 000 entries, after which
   earlier IDs can be resent. Inside the replay window the sequence filter still strikes a repeat from the
   same sender; outside it the packet is dropped without a strike.
5. **No partition recovery protocol.** There is no anti-entropy or digest exchange after a partition
   (planned).
6. **Sybil resistance is minimal.** `sybil.rs` has a proof-of-work check, but only tests call it, and a
   "start at 0.2 if the /24 subnet has a banned peer" rule, which only applies to sender IDs that parse
   as socket addresses. Fresh key pairs are otherwise free.
