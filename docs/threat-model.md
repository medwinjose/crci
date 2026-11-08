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
| Direct replay | Same message (signing key, sequence number and ID) from the same sender → strike, judged before the rate limit so a sender over budget cannot hide it; from a different sender → ignored | `runtime.rs` |
| Suppressing a message by signing another under its ID | A message ID is chosen by its signer, so the seen cache and the rescue store key messages by signing key, sequence number and ID: a message under an honest message's ID, or a frame rejected for its key, is a different message and suppresses nothing. Found while writing the TLA+ model; TLC reproduces the loss with the old key ([formal-verification.md](formal-verification.md)) | `runtime.rs` |
| Framing an honest relay by reusing a message ID | A signer can sign a second message under one of its IDs with a new sequence number; a relay whose seen cache has cleared forwards it as new. The sequence number is part of the seen-cache key, so a neighbour that still holds the first does not take the second for a replay. Found while proving that honest neighbours are never struck; TLC reproduces the strike with the old key | `runtime.rs` |
| Sequence replay | Sliding window W = 64 per origin: same sequence from the same sender → strike; from another sender → ignored; older than the window → dropped without a strike. The sequence number is signed | `replay.rs` |
| Repeat offender | Reputation 0.5, −0.15 per strike, banned below 0.1; banned senders are dropped on admission | `sybil.rs` |
| Paced replays | Recovery depends on elapsed time only (0.0005/s, capped at the initial 0.5, so credit cannot be banked); two strikes within 300 s cap recovery at 0.18 | `sybil.rs` |
| Strikes for honest re-sync after a partition | Digest-based anti-entropy: a node asks for what it lacks; answers are tagged with the request (covered by the TCP frame signature) and a tagged answer that repeats gossip is excused once, however late it arrives, and so is one gossip copy that follows its answer, since the two can take different connections, whether that answer was accepted or dropped (for example as a copy of what a third neighbour delivered first); every other untagged or unsolicited duplicate strikes. Requests are signed, answered only from their author, rate-limited and batched | `runtime.rs` |
| Message flood | Token bucket per sender (20 messages, +20/s) for messages not seen before, applied before the signature check; copies of known messages are dropped first, by signing key, sequence number and ID | `sybil.rs`, `runtime.rs` |
| Flood of fresh, validly signed messages from one origin | Token bucket per authenticated origin (20 messages, +5/s), charged after the signature and pinned-key checks; excess dropped without a strike, so one origin cannot take the network's capacity through honest relays | `runtime.rs`, `sybil.rs` |
| Signature-verification flood | Verifications bounded by the sender's token bucket; each invalid signature is a strike, so three bad signatures ban the sender; a banned peer's connection is closed and refused (30 s at a time), so its frames are neither read nor verified (measured: [benchmarks.md](benchmarks.md#invalid-signature-flood)) | `runtime.rs`, `sybil.rs`, `transport/tcp.rs` |
| Flood of frames on one connection (including a sender that is never struck) | The listener reads at most 200 frames per second from a connection (burst 400); beyond that the reader waits and TCP flow control slows the sender. No strike: an honest burst is never punished, and honest links stay far below the cap | `transport/tcp.rs` |
| Malformed input | Origin-ID length, severity range, sequence overflow and note size checked before `NodeRuntime` on a TCP node | `validation.rs`, `crci-node` `main.rs` |
| Framing an author by replaying its message | Strikes fall on the connection that delivered a message, never on the message's author | `runtime.rs`, `transport/tcp.rs` |
| Framing a neighbour by claiming its ID | Mutual Ed25519 challenge-response on every TCP connection; strikes bind to the authenticated identity (declared ID + proved key), so a claimed ID with another key is another identity | `transport/tcp.rs` |
| Injecting, altering or replaying frames on the path | Every frame is signed by the dialer for the session (transcript hash) and numbered; a bad signature or counter closes the connection before the frame is processed | `transport/tcp.rs` |
| Oversized / slow TCP frames | 1 MiB frame limit, 5 s to finish a frame that has started (a silent authenticated connection is closed after 10 min), 100 concurrent connections | `transport/tcp.rs` |
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
   signature verification; the cost is bounded only by the per-sender token bucket (20 messages, then 20/s).
3. **Paced replays are bounded, not eliminated.** Recovery forgives one strike per 300 s, so an adversary
   that replays at most once every 300 s is never banned: at most 12 replays per hour per identity. Faster
   pacing is banned (a replay every 290 s after 10 150 s, every 120 s after 480 s). An adversary that keeps
   replaying through repeated bans gets about 15 replays per hour past admission while banned 99.6 % of the time. Every
   replay is dropped and costs one signature check (measured in
   [benchmarks.md](benchmarks.md#paced-attack-vs-reputation-recovery)).
4. **Duplicate-ID cache resets.** The seen-message cache is cleared completely at 1 792 entries, after which
   earlier IDs can be resent. Inside the replay window the sequence filter still strikes a repeat from the
   same sender; outside it the packet is dropped without a strike.
5. **Re-synchronisation has a small excused set.** A neighbour that was asked for a sync may repeat a
   message the requester did not list as held once without a strike, if it tags the repeat as an answer
   to that request (within 120 s), for at most 16 messages per request. A TCP node asks each neighbour
   every 30 s, and also whenever that neighbour re-establishes its relay connection (`link_resumed`), at
   most once per 8 s. A malicious neighbour that reconnects on purpose can therefore send at most 16
   unpunished duplicates per 8 s (without reconnecting, 16 per 30 s), only of messages the requester
   lacked when it asked; each is dropped, and each costs one signature check within the neighbour's token
   bucket. One untagged copy of a message that arrived as an answer to that request is excused too (an
   answer can overtake the gossip copy on another connection), within the same limits; every other untagged
   repeat, and any further repeat, is a strike. Under link flapping (20 % and 50 % of the links down for 5–10 s at a time,
   90 honest testbed runs) no neighbour was struck or banned, because a resumed link's sync answers are
   tagged ([benchmarks.md](benchmarks.md#link-flapping)). Answers go to the `reply_to` address named in the signed
   request, so a requester can aim an answer (at most 16 messages) at a third party. Answers are limited per requester
   (one per 5 s) and in total (a burst of 4, then one per second), which bounds that reflection but does
   not remove it.
6. **New messages above the rate limit are throttled, not struck.** New messages beyond a neighbour's
   token bucket are dropped without a strike, so that honest bursts (for example after a partition) are
   never punished. Repeats are judged before the bucket, so a replay is struck whether or not the sender
   is over budget. A neighbour that floods new messages faster than its budget loses the excess, which also
   crowds out its own relays: persistent over-budget sending degrades it to an omission fault, which the
   mesh tolerates only through path redundancy.
   On TCP every frame's session signature is verified before any of these checks (about 37 µs, about
   100 µs with the read, the parse and the log lines). A banned neighbour is disconnected, so this cost
   stops after the ban; a neighbour that is never struck is limited by frame pacing to 200 frames/s per
   connection, which is about 2 % of a core at the measured per-frame cost. Measured with a neighbour that
   floods 1 000 valid fresh messages/s: 1.98 CPU seconds per neighbour over a 60 s run against 1.0 for a
   40 messages/s reference, 68 MiB of logs per run against 31 MiB (without pacing: 6.3 s and 174 MiB).
   What remains: each connection can still make its receiver do that much; a sender that reconnects pays a
   handshake each time, the same as any stranger.
7. **Aggregate gossip rate is bounded by the per-neighbour budget.** Every node relays every new message
   to all its neighbours, so each link carries every message once. Copies of messages the receiver already
   has spend no budget, but a node whose first copies all arrive through one neighbour (a node of degree 1,
   for example) needs that neighbour's budget for the whole network's traffic. The token bucket (20 per
   second) therefore caps the network-wide sustained message rate near its refill rate, whatever the number
   of nodes; beyond it, honest frames are dropped (without strikes) and delivery relies on redundant paths.
   Measured on the testbed (random geometric graph, 50 nodes, 3 seeds, load spread over many origins): no
   honest frame dropped at 16 messages/s; at 32 messages/s, 2 343–3 454 dropped per run and 91–99 % of
   messages delivered. Each origin is limited to 5 new messages per second at every node (burst 20), so one
   flooding identity takes at most that share, and an honest node cannot sustain more than 5 messages per
   second either (an honest source at 8 messages/s lost 635–1 684 messages per run). A node that floods
   with many fresh identities is not bounded this way (item 8).
8. **Sybil resistance is minimal.** `sybil.rs` has a proof-of-work check, but only tests call it, and a
   "start at 0.2 if the /24 subnet has a banned peer" rule, which only applies to sender IDs that parse
   as socket addresses. Fresh key pairs are otherwise free, so the per-origin budget (5 messages per
   second per origin) does not bound a node that signs its flood with many new identities.
9. **A neighbour whose frames are duplicated is struck.** A repeat of a message ID from the same
   connection is a strike, whoever caused it, so an honest neighbour behind a transport that duplicates
   frames loses reputation. The TCP transport numbers its frames and never duplicates one, and no honest
   strike occurred in any testbed run (flapping links included), but the protocol does not tell a duplicating link from a replaying
   peer. Measured with an injected duplicate fault (simulation, N = 100; not a property of any measured
   network): with the shipped three strikes no honest link is banned up to 3 × 10⁻⁵ duplicates per frame, 0.5 %
   are at 10⁻⁴, 11 % at 3 × 10⁻⁴ and 66 % at 10⁻³ (five strikes: 0.03 %, 1 % and 42 %; one strike: 40 %, 71 % and
   88 %). The price of a higher threshold is a faster sustainable paced replay: one per 300 s at three strikes,
   one per 180 s at five ([benchmarks.md](benchmarks.md#strike-threshold-k)).
10. **Without the per-origin budget a flood costs delivery; without bans a replayer is never isolated.**
    Switching defences off shows which one does what (emulation, 50 nodes): with only the per-sender
    bucket, delivery falls to 74 % under a flood of 200 fresh messages/s and 89–91 % under a replayer that
    also sends 40/s; with no admission limit at all delivery holds but a neighbour of the attacker spends up
    to six times the CPU ([benchmarks.md](benchmarks.md#defence-modes-on-real-processes)). A strict-monotonic
    replay window (W = 0) loses reordered messages and strikes nobody; a replay older than W is dropped
    without a strike, so the window also bounds how old a replay can be and still be punished
    ([benchmarks.md](benchmarks.md#replay-window-w)).
11. **Banning an attacker removes its links, and that can cut honest nodes off.** The isolation is local and
    correct (every honest neighbour of every replayer banned it in all 90 emulated runs with 10–45 % of the nodes
    replaying, 0 honest nodes banned), but a node whose paths to the source all ran through attackers is
    disconnected once they are banned: 27 of the 72 emulated runs with attackers left honest nodes unreachable
    from the source, and a graph computation over 200 topology instances per point puts the probability that
    at least one honest node is cut off at 1–19 % with 10 % of the nodes replaying and 85–100 % with 45 %
    (mostly single nodes up to 20 %). The protocol does not rebuild the overlay; path redundancy is the only
    remedy, and an attacker with no honest neighbour is banned by nobody. Replay-only attackers that do not
    collude and do not originate; N ≤ 100 on real processes
    ([benchmarks.md](benchmarks.md#share-of-replaying-nodes)).
12. **Bounded memory forgets, and identities are free.** A node keeps state for at most 896 origins and
    forgets the least active first. A forgotten origin loses its pinned key and its replay window, so an old
    message of that origin is accepted again as new and relayed, and a neighbour that still remembers the
    origin strikes the honest relay for the repeat. Three such replays ban it there. The adversary has to
    displace the origin first, by signing more messages than the origin has had accepted on each of 896 fresh
    identities, within the budgets of the neighbours it reaches; an origin with few messages is cheap to displace
    and a busy one is not. Both effects are shown by the model checker (`origin-forgotten-delivery`,
    `origin-forgotten-framing`) and by a test on the real code. Limits on the rest of the state have smaller
    costs: a clean transport peer that is forgotten loses nothing, and a banned one goes only when 1 024 peers
    are all banned; the recent store under a flood keeps less to answer sync requests with; a rescue message
    can be pushed out of the store by 256 newer ones, as the store has no way to tell a real rescue from a forged
    claim by a new identity. Key-derived origin IDs and identity minting cost are planned, and only raise the price.
    Measured through a real node, 90 fresh identities sending 3 × 10⁶ validly signed messages inside every budget
    leave its memory at about 20 MiB and its log at 35 bytes per message, and an honest neighbour loses nothing
    ([benchmarks.md](benchmarks.md#memory-per-node)).
