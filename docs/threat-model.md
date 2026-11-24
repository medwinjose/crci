# Threat Model

What CRCI defends against today, where the defence lives, and what is not defended. Every message, in-process
or over TCP, goes through the same checks in `NodeRuntime` ([architecture.md](architecture.md#message-path)).

## Goal and scope

CRCI aims to let each node **locally** isolate a neighbour that commits observable protocol violations,
without voting rounds or global agreement. It is not a consensus protocol and gives no agreement,
ordering or safety guarantee across nodes.

**Adversary considered:** a peer that commits acts of *commission* that a receiver can check by itself:
replaying messages it already sent or forwarding messages with invalid signatures. The adversary cannot forge Ed25519 signatures of keys it does not hold.

**Not considered:** Sybil attacks at scale with many network addresses (many keys on one machine are
considered: the transport and the message budget charge each source host, item 8), collusion,
majority-Byzantine networks, semantically false but correctly signed reports, radio jamming, side channels,
compromised devices.

## Defences that exist

| Threat | Defence | Where |
|---|---|---|
| Forged or altered message content | Ed25519 signature on every `WireMessage`, every message type included, covering every field in an unambiguous, domain-separated encoding; invalid → dropped, sender struck | `runtime.rs` |
| Impersonating an origin (claiming its ID) | A node ID is derived from its public key (first 16 bytes of SHA-256 over a domain tag and the key); a message is authentic only if its origin is the ID derived from the key it carries and the signature verifies. A claim of another node's ID needs a second preimage (about 2^128 key generations); without one the message is a forgery: dropped and its sender struck. Which message arrives first does not matter | `identity.rs`, `runtime.rs` |
| Direct replay | Same message (signing key, sequence number and ID) from the same sender → strike, judged before the rate limit so a sender over budget cannot hide it; from a different sender → ignored | `runtime.rs` |
| Suppressing a message by signing another under its ID | A message ID is chosen by its signer, so the seen cache and the rescue store key messages by signing key, sequence number and ID: a message under an honest message's ID, or a frame rejected for its key, is a different message and suppresses nothing. Found while writing the TLA+ model; TLC reproduces the loss with the old key ([formal-verification.md](formal-verification.md)) | `runtime.rs` |
| Framing an honest relay by reusing a message ID | A signer can sign a second message under one of its IDs with a new sequence number; a relay whose seen cache has cleared forwards it as new. The sequence number is part of the seen-cache key, so a neighbour that still holds the first does not take the second for a replay. Found while proving that honest neighbours are never struck; TLC reproduces the strike with the old key | `runtime.rs` |
| Sequence replay | Sliding window W = 64 per origin: same sequence from the same sender → strike; from another sender → ignored; older than the window → dropped without a strike. The sequence number is signed | `replay.rs` |
| Repeat offender | Reputation 0.5, −0.15 per strike, banned below 0.1; banned senders are dropped on admission | `sybil.rs` |
| Paced replays | Recovery depends on elapsed time only (0.0005/s, capped at the initial 0.5, so credit cannot be banked); two strikes within 300 s cap recovery at 0.18 | `sybil.rs` |
| Strikes for honest re-sync after a partition | Digest-based anti-entropy: a node asks for what it lacks; answers are tagged with the request (covered by the TCP frame signature) and a tagged answer that repeats gossip is excused once, however late it arrives, and so is one gossip copy that follows its answer, since the two can take different connections, whether that answer was accepted or dropped (for example as a copy of what a third neighbour delivered first); every other untagged or unsolicited duplicate strikes. Requests are signed, answered only from their author, rate-limited and batched | `runtime.rs` |
| Message flood | Token bucket per sender (20 messages, +20/s) for messages not seen before, applied before the signature check; copies of known messages are dropped first, by signing key, sequence number and ID | `sybil.rs`, `runtime.rs` |
| Flood of fresh, validly signed messages from one origin | Token bucket per authenticated origin (20 messages, +5/s), charged after the origin is authenticated; excess dropped without a strike, so one origin cannot take the network's capacity through honest relays | `runtime.rs`, `sybil.rs` |
| Signature-verification flood | Verifications bounded by the sender's token bucket; each invalid signature is a strike, so three bad signatures ban the sender; a banned peer's connection is closed and refused (30 s at a time), so its frames are neither read nor verified (measured: [benchmarks.md](benchmarks.md#invalid-signature-flood)) | `runtime.rs`, `sybil.rs`, `transport/tcp.rs` |
| Flood of frames from one host (including a sender that is never struck) | The listener reads at most 200 frames per second from all the connections of one source host together (burst 400); beyond that the reader waits and TCP flow control slows the sender. No strike: an honest burst is never punished, and honest links stay far below the cap | `transport/tcp.rs` |
| Many keys on one machine (identity rotation) | Keys are free, so the listener charges its limits to the connection's source host (an IPv4 address or an IPv6 /64), not to the identity: at most 8 connections and 16 handshakes at once, then 2 per second, per host; frame pacing shared by the host; and `crci-node` charges every identity of a host to one message budget (20, then 20/s). A host costs a node at most that much however many keys it uses (proved: [formal-verification.md](formal-verification.md#analytical-bounds), Theorem 6) | `transport/tcp.rs`, `sybil.rs`, `crci-node` `main.rs` |
| Locking honest neighbours out with idle connections | When all 100 slots are taken, a connection from a host holding fewer connections than the largest host is admitted by closing the least recently active connection of that host; a host with no connection is never refused, and displacing an honest host with c connections takes at least (100 − honest connections) / c adversary hosts (Lemma 3) | `transport/tcp.rs` |
| Malformed input | Origin-ID length, severity range, sequence overflow and note size checked before `NodeRuntime` on a TCP node | `validation.rs`, `crci-node` `main.rs` |
| Framing an author by replaying its message | Strikes fall on the connection that delivered a message, never on the message's author | `runtime.rs`, `transport/tcp.rs` |
| Framing a neighbour by claiming its ID | Mutual Ed25519 challenge-response on every TCP connection; a peer whose declared node ID is not derived from the key it proved is refused, and strikes bind to that authenticated node ID | `transport/tcp.rs` |
| Framing a node by aiming its sync answers at a third node | An answer leaves only to the node that asked: the request ID names its author, the request is answered only if it does, and the transport sends a tagged answer only to a listener that authenticated as that author. Before, one signed request naming a neighbour's address made the answerer send that neighbour copies of what it had relayed to it, three strikes and a ban | `runtime.rs`, `transport/tcp.rs` |
| Framing a node through a second address (a peer that forwards its connections to another peer, one node configured under two addresses) | Gossip leaves to each authenticated node at one address only: the sender remembers the last 1 024 messages per listener and the address each went to | `transport/tcp.rs` |
| A disguised replay (the copy's unauthenticated origin or type changed so it looks stale) | A frame counts as a copy only if its origin is the ID of the key it carries, and control messages have seen-cache keys of their own; anything else is judged as a new message, whose signature then fails | `runtime.rs` |
| Oversized fields (memory and log growth inside every budget) | IDs, types, visibility, notes, answer tags and reply addresses have size limits, only a sync request carries a digest, printed fields hold no control character; outside them a message is dropped without a strike before anything is stored or printed | `runtime.rs`, `crci-node` `main.rs` |
| Injecting, altering or replaying frames on the path | Every frame is signed by the dialer for the session (transcript hash) and numbered; a bad signature or counter closes the connection before the frame is processed | `transport/tcp.rs` |
| Oversized / slow TCP frames | 1 MiB frame limit, 5 s to finish a frame that has started (a silent authenticated connection is closed after 10 min), 100 concurrent connections, admitted per source host | `transport/tcp.rs` |
| Data at rest | AES-256-GCM, each record bound to its ID, key derived with Argon2id (19 MiB, 2 passes, 1 lane, written out in the code); with `--key-file` the node key is kept this way, and a wrong passphrase stops the node instead of creating a new key | `storage/encrypted.rs`, `identity.rs` |
| A restarted node reusing sequence numbers | With `--key-file` a restarted node keeps its ID; each start reserves the next 2^32 sequence numbers in the key file before sending, so neighbours never see a number twice from it | `identity.rs` |
| A restarted relay struck for relaying old messages again | With `--key-file` the node also keeps, encrypted beside the key, the highest sequence number accepted per origin (saved at most 5 s after a change, at once for a new origin, and on a clean stop). A restarted node accepts older messages and answers sync requests with them but does not relay them, plus 45 messages per origin after a run that did not stop cleanly | `identity.rs`, `runtime.rs` |
| Panics across FFI | Rust panics surface as Kotlin exceptions (recorded run: `docs/evidence/android-bft038-2026-08-23/`) | `ffi.rs` |

## Known gaps (open, with the task that addresses each)

1. **Identities are free and not bound to names.** A banned peer can return with a fresh key pair (a new
   identity starts at 0.5). Node IDs are derived from keys, so no one can take an existing node's ID, but
   nothing ties a key to a person or device, and labels are not authenticated. A node started without
   `--key-file` has a new key, and so a new identity, at every start. The key file is only as safe as its
   passphrase file and the device. TCP traffic is authenticated but not encrypted. What a fresh key buys is
   bounded by the machine it comes from, not by the key: a node charges its connection, handshake and frame
   limits and its message budget to each source host (item 8).
2. **Stale floods are not penalised.** A packet older than the replay window is dropped without a strike,
   so an adversary can resend old signed messages without losing reputation. Each one still costs a
   signature verification; the cost is bounded only by the per-sender token bucket (20 messages, then 20/s).
3. **Paced replays are bounded, not eliminated.** Recovery forgives one strike per 300 s, so an adversary
   that replays at most once every 300 s is never banned: 12 replays per hour per identity, and no identity
   that is never banned takes more than 14 strikes in any hour (proved: [formal-verification.md](formal-verification.md#analytical-bounds);
   the score's rounding to 0.001 after each strike makes a replay every 299 s sustainable too). Faster
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
   every 30 s, and also whenever that neighbour re-establishes its relay connection (`link_resumed`) or
   its last answer filled a whole batch of 16, at most once per 8 s. A malicious neighbour that reconnects
   on purpose, or always answers with 16 messages, can therefore send at most 16 unpunished duplicates per
   8 s (otherwise 16 per 30 s), only of messages the requester
   lacked when it asked; each is dropped, and each costs one signature check within the neighbour's token
   bucket. One untagged copy of a message that arrived as an answer to that request is excused too (an
   answer can overtake the gossip copy on another connection), within the same limits; every other untagged
   repeat, and any further repeat, is a strike. Under link flapping (20 % and 50 % of the links down for 5–10 s at a time,
   90 honest testbed runs) no neighbour was struck or banned, because a resumed link's sync answers are
   tagged ([benchmarks.md](benchmarks.md#link-flapping)). Answers go to the `reply_to` address named in the signed
   request, but are sent only if the node that answers at that address authenticates as the requester, so a
   requester can still make a node open connections to an address of its choice, not send it messages. Answers
   are limited per requester (one per 5 s) and in total (a burst of 4, then one per second).
6. **New messages above the rate limit are throttled, not struck.** New messages beyond a neighbour's
   token bucket are dropped without a strike, so that honest bursts (for example after a partition) are
   never punished. Repeats are judged before the bucket, so a replay is struck whether or not the sender
   is over budget. A neighbour that floods new messages faster than its budget loses the excess, which also
   crowds out its own relays: persistent over-budget sending degrades it to an omission fault, which the
   mesh tolerates only through path redundancy.
   On TCP every frame's session signature is verified before any of these checks (about 36 µs, about
   100 µs with the read, the parse and the log lines). A banned neighbour is disconnected, so this cost
   stops after the ban; a neighbour that is never struck is limited by frame pacing to 200 frames/s per
   connection, which is about 2 % of a core at the measured per-frame cost. Measured with a neighbour that
   floods 1 000 valid fresh messages/s: 1.92 CPU seconds per neighbour over a 60 s run against 1.18 for a
   40 messages/s reference, 38.4 MiB of logs per run against 25.2 MiB (on an earlier build without pacing:
   6.3 s and 174 MiB).
   What remains: each source host can still make its receiver do that much (the pacing is shared by all of a
   host's connections), and up to 16 handshakes at once, then 2 per second, however many keys it uses.
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
   second either (an honest source at 8 messages/s lost 635–1 684 messages per run). A node that signs
   its flood with many fresh identities is not bounded by the per-origin budget, only by its host's budget at
   each node it reaches directly: 20 new messages per second per receiver (item 8), more than the 16
   messages/s the network carried without an honest loss, so such a flood still crowds out honest traffic
   where its messages are relayed.
8. **Sybil resistance stops at the network address.** Fresh key pairs are free, so every limit of a TCP
   node that an adversary could multiply with keys is charged to the source host of the connection instead:
   connections (8), handshakes (16, then 2/s), frames (400, then 200/s) and new messages (20, then 20/s) per
   host, whatever the number of identities (Theorem 6 in
   [formal-verification.md](formal-verification.md#analytical-bounds); tests over real sockets in
   `tcp_host_limits_tests.rs`). This only moves the cost from keys to addresses: an adversary with k hosts
   gets k times every budget, and on IPv6 a whole /64 counts as one host. Several honest nodes behind one
   address (a NAT) would share one budget; the mesh setting assumes one address per node. Within its budget a
   host can still inject fresh, validly signed messages under new origins, which the per-origin budget does
   not stop and honest relays carry on (item 7). `sybil.rs` also has a proof-of-work check, which only tests
   call, and a "start at 0.2 if the /24 subnet has a banned peer" rule, which only applies to sender IDs that
   parse as socket addresses (no TCP sender's does: TCP senders are key-derived IDs).
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
    forgets the least active first. A forgotten origin loses its replay window but keeps its floor, the highest
    sequence number accepted, in a table of 3 584 floors (oldest first out): an old message of that origin is
    stale, dropped without a strike and never accepted or relayed again. Before floors such a message was
    accepted again and relayed, and a neighbour that still remembered the origin struck the honest relay for
    the repeat: on the testbed one host flooding new origins inside its budget got 9 honest nodes banned
    ([formal-verification.md](formal-verification.md#found-while-bounding-what-one-host-can-do)). What is left:
    an adversary that pushes an origin's floor out too, with 3 584 more forgotten origins at that relay while a
    neighbour still remembers the origin, gets the old effect back; the model checker shows both sides
    (`origin-forgotten-*` pass, `floor-forgotten-*` break), and tests on the real code check the floor and its
    limit. Messages of a forgotten origin that the node never received are not recovered below the floor.
    Limits on the rest of the state have smaller
    costs: a clean transport peer that is forgotten loses nothing, and a banned one goes only when 1 024 peers
    are all banned; the recent store under a flood keeps less to answer sync requests with; a rescue message
    can be pushed out of the store by 256 newer ones, as the store has no way to tell a real rescue from a forged
    claim by a new identity. Deriving origin IDs from keys does not change this, since fresh keys are free; a cost
    to mint an identity (measured, not used: [benchmarks.md](benchmarks.md#proof-of-work-identity-minting-cost))
    would only raise the price.
    Measured through a real node, 90 fresh identities sending 10⁶ validly signed messages inside every budget
    leave its memory at about 19 MiB and its log at 32 bytes per message, and an honest neighbour loses nothing
    ([benchmarks.md](benchmarks.md#memory-per-node)).

13. **The key file is not protected against rollback, and some local message fields never leave the node.**
    Each record in the encrypted key file is authenticated and tied to its ID, so a record cannot be altered
    or passed off under another ID. The file as a whole is not protected against being replaced by an older
    authentic copy of itself or of one record (for example the sequence-number floor): that needs a trusted
    monotonic counter outside the file, which a phone or a Raspberry Pi does not offer. A node whose key
    file is rolled back can reuse sequence numbers, and its neighbours then drop or strike those messages;
    the attacker needs write access to the device, which the model does not consider (see Not considered).
    The same holds for the record of what the node relayed, kept beside the key file: rolled back or deleted, it
    lets a restarted node relay old messages again and be struck for them. It also does not name origins the node
    had forgotten (above 896), nor a new origin heard in the last second before a crash.
    Separately, the in-process `Message` has fields that `WireMessage` does not carry (position,
    resource type, expiry, priority, hop count); they are not sent, not signed, and come back as defaults
    on the receiving side. The protocol transports the fields of `WireMessage` and no others.

14. **A peer that forwards connections can stand on another peer's link.** If an address leads, through a
    malicious peer, to a node this node also reaches directly, each message goes out on whichever of the two
    links sends it first. On the forwarded link the adversary can read (nothing is encrypted), delay or withhold
    frames, but not alter or repeat them. That is omission, which the mechanism does not punish; other paths and
    the next sync recover the message.
