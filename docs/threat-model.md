# Threat Model

What CRCI defends against today, where the defence lives, and what is not defended. Every message, in-process
or over TCP, goes through the same checks in `NodeRuntime` ([architecture.md](architecture.md#message-path)).

## Goal and scope

CRCI aims to let each node **locally** isolate a neighbour that commits observable protocol violations,
without voting rounds or global agreement. It is not a consensus protocol and gives no agreement,
ordering or safety guarantee across nodes.

**Adversary considered:** a peer that commits acts of *commission* that a receiver can check by itself:
replaying messages it already sent or forwarding messages with invalid signatures. The adversary cannot forge Ed25519 signatures of keys it does not hold.

**Not considered:** Sybil attacks at scale (many identities), collusion, majority-Byzantine networks,
semantically false but correctly signed reports, radio jamming, side channels, compromised devices.

## Defences that exist

| Threat | Defence | Where |
|---|---|---|
| Forged or altered message content | Ed25519 signature on every `WireMessage`, every message type included, covering every field in an unambiguous, domain-separated encoding; invalid → dropped, sender struck | `runtime.rs` |
| Impersonating a known origin | Public key pinned on first sight; a different key → dropped (no strike: the receiver cannot tell which key is genuine) | `runtime.rs` |
| Direct replay | Same message ID from the same sender → strike; from a different sender → ignored | `runtime.rs` |
| Sequence replay | Sliding window W = 64 per origin: same sequence from the same sender → strike; from another sender → ignored; older than the window → dropped without a strike. The sequence number is signed | `replay.rs` |
| Repeat offender | Reputation 0.5, −0.15 per strike, banned below 0.1; banned senders are dropped on admission | `sybil.rs` |
| Paced replays | Recovery depends on elapsed time only (0.0005/s, capped at the initial 0.5, so credit cannot be banked); two strikes within 300 s cap recovery at 0.18 | `sybil.rs` |
| Strikes for honest re-sync after a partition | Digest-based anti-entropy: a node asks for what it lacks; answers that cross with gossip are excused once; unsolicited duplicates still strike. Requests are signed, answered only from their author, rate-limited and batched | `runtime.rs` |
| Message flood | Token bucket per sender (20 messages, +5/s), applied before the signature check | `sybil.rs` |
| Signature-verification flood | Verifications bounded by the sender's token bucket; each invalid signature is a strike, so three bad signatures ban the sender | `runtime.rs`, `sybil.rs` |
| Malformed input | Origin-ID length, severity range, sequence overflow and note size checked before `NodeRuntime` on a TCP node | `validation.rs`, `crci-node` `main.rs` |
| Framing an author by replaying its message | Strikes fall on the connection that delivered a message, never on the message's author | `runtime.rs`, `transport/tcp.rs` |
| Framing a neighbour by claiming its ID | Mutual Ed25519 challenge-response on every TCP connection; strikes bind to the authenticated identity (declared ID + proved key), so a claimed ID with another key is another identity | `transport/tcp.rs` |
| Injecting, altering or replaying frames on the path | Every frame is signed by the dialer for the session (transcript hash) and numbered; a bad signature or counter closes the connection before the frame is processed | `transport/tcp.rs` |
| Oversized / slow TCP frames | 1 MiB frame limit, 5 s read timeouts, 100 concurrent connections | `transport/tcp.rs` |
| Data at rest | AES-256-GCM, key derived with Argon2id | `storage/encrypted.rs` |
| Panics across FFI | Rust panics surface as Kotlin exceptions (recorded run: `docs/evidence/android-bft038-2026-08-23/`) | `ffi.rs` |

## Known gaps (open, with the task that addresses each)

1. **Identities are free and not bound to names.** A banned peer can return with a fresh key pair (a new
   identity starts at 0.5), and keys are generated at start-up, so a restarted node is a new identity too.
   Origin IDs are pinned to the first key seen: an adversary that announces an honest node's ID first makes
   this receiver drop that node's messages (denial of service, no strike). Origin IDs derived from keys are
   planned. TCP traffic is authenticated but not encrypted.
2. **Stale floods are not penalised.** A packet older than the replay window is dropped without a strike,
   so an adversary can resend old signed messages without losing reputation. Each one still costs a
   signature verification; the cost is bounded only by the per-sender token bucket (20 messages, then 5/s).
3. **Paced replays are bounded, not eliminated.** Recovery forgives one strike per 300 s, so an adversary
   that replays at most once every 300 s is never banned: at most 12 replays per hour per identity. Faster
   pacing is banned (a replay every 290 s after 10 150 s, every 120 s after 480 s). An adversary that keeps
   replaying through repeated bans lands about 15 replays per hour while banned 99.6 % of the time. Every
   replay is dropped and costs one signature check (measured in
   [benchmarks.md](benchmarks.md#paced-attack-vs-reputation-recovery)).
4. **Duplicate-ID cache resets.** The seen-message cache is cleared completely at 5 000 entries, after which
   earlier IDs can be resent. Inside the replay window the sequence filter still strikes a repeat from the
   same sender; outside it the packet is dropped without a strike.
5. **Re-synchronisation has a small excused set.** During a 30 s sync session the peer that was asked
   may repeat each message the requester did not list as held once without a strike. A malicious peer
   that is asked can therefore send at most one unpunished duplicate per such message; it still costs a
   signature check and is dropped. Answers go to the `reply_to` address named in the signed request, so a
   requester can aim an answer (at most 16 messages) at a third party. Answers are limited per requester
   (one per 5 s) and in total (a burst of 4, then one per second), which bounds that reflection but does
   not remove it.
6. **Sybil resistance is minimal.** `sybil.rs` has a proof-of-work check, but only tests call it, and a
   "start at 0.2 if the /24 subnet has a banned peer" rule, which only applies to sender IDs that parse
   as socket addresses. Fresh key pairs are otherwise free.
