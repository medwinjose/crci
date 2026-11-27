# Formal Verification (TLA+)

The isolation protocol is specified in TLA+ and model-checked with TLC, and the specification is tied to
the Rust code by replaying what TLC explores through the real implementation.

**What this is:** exhaustive checking of small instances, up to about 104 million distinct states each. **What it
is not:** a proof for every network size. A property below holds for the stated constants; nothing here
says it holds at any N, though the mechanism does not depend on N and the emulation and simulation runs
([benchmarks.md](benchmarks.md)) test larger networks. The [analytical bounds](#analytical-bounds) further down
are proved for any N: each concerns one observer and one neighbour at a time.

## Files

| File | Purpose |
|---|---|
| [`tla/CRCITypes.tla`](tla/CRCITypes.tla) | The reputation rule: initial score, penalty, ban, readmission, damping, recovery. |
| [`tla/Reputation.tla`](tla/Reputation.tla) | One observer and one peer against every strike schedule on a time grid. |
| [`tla/CRCI.tla`](tla/CRCI.tla) | The protocol as `NodeRuntime::process_inbox_delivered` runs it, in its order of checks: seen cache, sliding window, strikes, bans, recovery, signatures, anti-entropy with tagged answers, relaying, lossy FIFO links, and an adversary. |
| [`tla/MC.tla`](tla/MC.tla), [`tla/MC.cfg`](tla/MC.cfg) | Topologies, and one configuration (`adversary`) for running TLC by hand. |
| [`tla/check.py`](tla/check.py) | Every configuration below with its expected outcome; exports what the conformance tests replay. |
| `tla/reputation-transitions.csv`, `tla/protocol-traces.jsonl` | The exports, committed. CI regenerates them and fails if they differ from the specifications. |
| `tla/tla2tools.jar` | TLC, pinned: `TLC2 Version 2.19 of 08 August 2024 (rev: 5a47802)`. Requires Java 11+. |

## Running

```bash
make tla                                 # every check: 80 to 100 min on 12 cores, several GB of temporary disk
python3 docs/tla/check.py --quick        # the checks that take about a minute or less (run in CI)
python3 docs/tla/check.py seen-by-id     # one check
cargo test -p crci-core --test reputation_model_conformance --test protocol_model_conformance
```

Raw TLC output of the recorded run: [`evidence/tla-2026-11-07/`](evidence/tla-2026-11-07/).

## The adversary and the network

The adversary holds no honest key, cannot pose as another sender (connections are authenticated) and cannot
claim another node's ID (IDs are derived from keys, so such a claim is a forgery like a bad signature). It
replays anything it has received, with any sync tag; forges frames whose signature does not verify; signs
its own messages under any message ID it has seen; and otherwise stays silent, omitting whatever it likes.
There is one adversary and no collusion. Links are queues that can lose frames, FIFO as one TCP connection per
neighbour gives, and in `reordering-links` and one set of replayed traces they also reorder, as two connections
to one neighbour can. A frame dropped by a token bucket has the same effect as a lost one, so budgets are
modelled as loss. Honest nodes keep the seen cache, window and recent store as the code does, and the seen
cache can be cleared whole, as its limit of 1 792 entries does. A switch lets a node forget an origin whole, as its
limit on tracked origins does, keeping the origin's floor (the highest sequence number accepted; everything at or
below it stays stale) as the code does; the safety checks with it on pass, with an adversary
(`origin-forgotten-delivery`, `origin-forgotten-framing`) and with catch-up (`origin-forgotten-sync`,
`origin-forgotten-triangle`). Forgetting costs delivery: the origin's kept messages go
with it, and the floor stops the node from fetching them again, so a neighbour that missed one and has no other
source never gets it (`origin-forgotten-gap`). A second switch lets the node lose that floor too, as the floor table's own limit
does; with it the two adversary checks break (`floor-forgotten-delivery`, `floor-forgotten-framing`): bounded memory
still has a price while identities are free, now behind 3 584 floors as well as 896 origins.

## Properties

| Property | Statement |
|---|---|
| `HonestNeverStruck` | No honest node ever loses reputation for what an honest neighbour sent. |
| `OnlyAdversariesBanned` | A node bans only an adversary. |
| `NoDoubleDelivery` | A node accepts each (origin, sequence number) at most once. |
| `NoForgedDelivery` | A node accepts for an honest origin only what that origin signed. |
| `WindowConsistent` | What the window remembers lies inside the window and within what was accepted. |
| `DeliveredWhenQuiescent` | Once no honest step is left, every message of an honest source has reached every honest node. |
| `EventualDelivery` (temporal) | Every message of an honest source reaches every honest node. |
| `EveryNeighbourBans` (temporal) | A replayer that does not stop is banned by every honest neighbour. |
| `ScoreInRange`, `NoBanWithoutStrike`, `BurstOfThreeBans`, `BannedBelowUnban` | The score stays in [0, 0.5]; inactivity never lowers it; three strikes in a row ban from any state; a banned peer stays below the readmission score. |
| `EventuallyForgiven` (temporal) | After finitely many strikes a peer returns to the initial score, unbanned. |

### Fairness

Safety needs none. Liveness needs the following, and no more:

- `EventualDelivery`: weak fairness of honest steps as a group, `WF_vars(HonestStep)`: honest nodes keep
  receiving, answering and asking. Loss, the clock and the adversary get no fairness. Losses are finite in
  every configuration, and the honest nodes stay connected without the adversary. Honest steps are finite in
  these models, so every behaviour reaches a state where none is enabled; that is why the invariant
  `DeliveredWhenQuiescent`, far cheaper for TLC, gives delivery in the larger configurations.
- `EveryNeighbourBans`: weak fairness of each link's delivery and of the replayer's sending, and no fairness
  for the clock. With a fair clock the property is false (`paced-replayer-spared`): that is the residual
  bound below.
- `EventuallyForgiven`: weak fairness of the clock.

## Results

Run on 2026-11-25 on an Intel i5-12450HX (12 logical CPUs): 33 of 33 checks gave the expected outcome,
100 min in total ([`evidence/tla-2026-11-25/`](evidence/tla-2026-11-25/)). Every check that passes explored exactly
as many states as in the run of 2026-11-17 (`evidence/host-limits-2026-11-17/tla/`). With 12 workers the depth
TLC reports for a search, and the counterexample it meets first, vary a little from run to run (`rep-free`: depth
91, 92 and 95); the tables give this run's. Keying the seen
cache by sequence number too (below) roughly quadruples `honest-loss-sync`, the largest check. *Diameter* is the depth of TLC's complete state-graph search.

### Reputation (one observer, one peer)

| Check | Setting | Outcome | Generated | Distinct | Diameter |
|---|---|---|---|---|---|
| `rep-free` | Every schedule of up to 6 strikes, 20 s grid | 4 invariants hold | 36 557 | 30 104 | 95 |
| `rep-forgiven` | The same, time fair | `EventuallyForgiven` holds | 36 557 | 30 104 | 91 |
| `rep-spaced-300` | Strikes at least 300 s apart, 10 s grid, no history limit | Never banned | 63 | 61 | 61 |
| `rep-paced-290` | A strike every 290 s | Banned at 10 150 s | 1 051 | 1 051 | 1 051 |
| `rep-paced-120` | A strike every 120 s | Banned at 480 s | 53 | 53 | 53 |

The last three reproduce exactly the times `recovery_bench` measures on the real code
([benchmarks.md](benchmarks.md#paced-attack-vs-reputation-recovery)).

### Protocol: the properties hold

Nodes: `s` (source), `h`, `r`, and adversary `a`. Line: `s-h-r`. Line + a: also `a-r`. Kite: also `a-h`, `a-r`.
Requests: sync requests per ordered pair that can race frames in flight; each fault earns one more on a
quiet link. All invariants but `DeliveredWhenQuiescent` hold in `window-slides` too, which leaves it out.

| Check | Topology | Messages, W | Faults, requests | Adversary frames | Other | Generated | Distinct | Diameter |
|---|---|---|---|---|---|---|---|---|
| `honest-loss-sync` | Line | 2, 2 | 2, 2 | none | Seen cache may clear | 462 676 391 | 104 091 287 | 36 |
| `adversary` | Kite | 2, 2 | 0, 0 | 4 | | 50 474 929 | 9 913 640 | 25 |
| `adversary-sync` | Kite | 1, 2 | 0, 1 | 1 | | 42 838 888 | 8 348 878 | 31 |
| `adversary-loss` | Line + a | 1, 2 | 1, 1 | 1 | | 10 803 517 | 2 312 816 | 28 |
| `adversary-time` | Line + a | 1, 2 | 0, 1 | 3 | Time passes | 272 092 207 | 48 344 100 | 38 |
| `reordering-links` | Line | 2, 2 | 1, 2 | none | Links reorder frames | 54 648 052 | 10 205 074 | 26 |
| `reused-id` | Kite | 0, 2 | 0, 0 | 3, two sequence numbers, IDs reused | Seen cache may clear | 8 173 033 | 2 014 341 | 21 |
| `triangle-reordering` | Triangle `s-h-r-s` | 1, 2 | 1, 2 | none | Links reorder, seen cache may clear | 171 975 801 | 29 737 911 | 30 |
| `window-slides` | Line | 3, 1 | 0, 1 | none | | 7 485 | 3 053 | 21 |
| `delivery` | Line | 2, 2 | 1, 2 | none | `EventualDelivery` | 434 253 | 133 030 | 26 |
| `eviction` | Kite | 1, 2 | 0, 0 | Unbounded replays | `EveryNeighbourBans` | 119 957 | 26 048 | 29 |
| `origin-forgotten-delivery` | Kite | 1, 2 | 0, 0 | 1 | Origins may be forgotten (their floors kept), seen cache may clear; `NoDoubleDelivery` | 2 494 390 | 380 811 | 23 |
| `origin-forgotten-framing` | Kite | 1, 2 | 0, 0 | 1 | The same; `HonestNeverStruck` | 2 494 390 | 380 811 | 22 |
| `origin-forgotten-sync` | Line | 2, 2 | 0, 1 | none | The same; every invariant but `DeliveredWhenQuiescent` | 2 391 279 | 548 901 | 25 |
| `origin-forgotten-triangle` | Triangle `s-h-r-s` | 1, 2 | 1, 1 | none | The same, and links reorder | 2 792 030 | 521 449 | 21 |

### Protocol: remove one thing, and TLC finds the failure

Each row removes one assumption or defence. The counterexample shows that the property is not vacuous and
that the assumption is needed. These are the assumptions the results above rest on.

| Check | Removed | Violated | Trace (states) | What it means |
|---|---|---|---|---|
| `link-duplicates` | A network that does not duplicate | `HonestNeverStruck` | 7 | A duplicated frame strikes an honest neighbour (threat model, gap 9). TCP does not duplicate frames. |
| `session-expires` | Answers arriving within their session | `HonestNeverStruck` | 8 | An answer later than 120 s, or beyond the four newest sessions per peer, is a strike. |
| `no-excuse` | The excuse for answers | `HonestNeverStruck` | 8 | Why the excuse exists. |
| `no-window` | The replay window | `NoDoubleDelivery` | 6 | After the seen cache clears, a replay would be delivered again. |
| `no-signatures` | Signature checks | `NoForgedDelivery` | 3 | |
| `seen-by-id` | The signing key in the seen-cache key | `DeliveredWhenQuiescent` | 14 | The first defect below. |
| `seen-without-seq` | The sequence number in the seen-cache key | `HonestNeverStruck` | 8 | A reused message ID frames an honest relay ([below](#found-while-proving-the-bounds)). |
| `copied-answers-ignored` | Noting answers that were dropped as copies | `HonestNeverStruck` | 8 | The gossip copy an answer overtook is struck ([below](#found-while-proving-the-bounds)). |
| `window-too-small` | W at least the loss depth (W = 1, 3 messages, 1 loss) | `DeliveredWhenQuiescent` | 11 | A message that falls out of every neighbour's window before it is recovered is lost for good. |
| `origin-forgotten-gap` | A node that remembers the origin | `DeliveredWhenQuiescent` | 5 | The only relay forgets a message it held; the copy it had sent on is lost; its digest counts the message as held, so it is not sent to it again, and the node behind it never gets it. |
| `floor-forgotten-delivery` | A forgotten origin's floor (it is lost too) | `NoDoubleDelivery` | 8 | After a node forgets an origin and its floor and clears its seen cache, a replay is delivered again. |
| `floor-forgotten-framing` | The same | `HonestNeverStruck` | 9 | The node relays that replay, and a neighbour that still remembers the origin strikes it. |
| `paced-replayer-spared` | Replays faster than recovery (a fair clock) | `EveryNeighbourBans` | 24 | The residual bound: an adversary that replays at most once per 300 s is never banned. |

## Analytical bounds

TLC checks small instances. The statements below hold for any network size, because each concerns one
observer and one neighbour (Theorems 1–3), the checks one node applies to one frame (Theorem 4) or what one
host can make one node do (Lemma 3, Theorem 6). Each proof
was written against the code, and each reputation theorem is also checked against the code by a test that fails
when a constant or rule it depends on changes (`sybil.rs`: `no_strike_schedule_beats_the_strike_budget`,
`periodic_strikes_ban_exactly_when_faster_than_the_forgiveness_period`, `a_ban_lasts_between_300_and_500_seconds`;
five planted changes, each caught by at least one of them).

**Notation** (`sybil.rs`): initial and highest score σ₀ = 0.5, penalty π = 0.15, ban threshold β = 0.1,
readmission threshold υ = 0.25, recovery rate ρ = 0.0005 per second, damped ceiling c = 0.18, strike memory
M = 300 s, forgiveness period T = π / ρ = 300 s. A strike sets the score s to max(0, s − π) and bans the peer if
the result is below β. Between strikes the score rises at rate ρ up to a cap: c while two or more strikes lie in
the last M seconds, σ₀ otherwise; a score above the cap stays where it is. A ban is lifted once the score
reaches υ. The theorems use exact arithmetic; the last paragraph says what the code's arithmetic changes.

**Recovery can undo strikes.** Recovery forgives one strike per T by design, so a proof cannot assume that it
never undoes a strike. Theorem 1 states how much it undoes instead.

**Lemma 1.** The score stays in [0, σ₀]; it falls only at a strike; between strikes it rises by at most ρ per
second; a peer that is never struck is never banned. *Proof.* A strike clamps at 0, the cap is at most σ₀,
recovery never lowers a score, and only a strike sets the ban. ∎

Theorems 1–3 are about one record of one peer. A node keeps 1 024 such records; past that it forgets the
longest idle first, records with remembered strikes after clean ones and banned ones last (`make_room`). A forgotten peer
starts again at σ₀, which gains it nothing that a fresh key would not.

**Theorem 1 (strike budget).** If a peer takes k strikes at t₁ ≤ … ≤ t_k and none of them bans it, then
k π ≤ s(t₁⁻) − β + ρ (t_k − t₁) ≤ σ₀ − β + ρ (t_k − t₁).
*Proof.* Each strike leaves a score of at least β > 0, so none was clamped and each removed exactly π. Between
t₁ and t_k recovery added at most ρ (t_k − t₁) (Lemma 1). So β ≤ s(t_k⁺) ≤ s(t₁⁻) − kπ + ρ (t_k − t₁). ∎
With the shipped values, any k strikes within L < 300 k − 800 seconds ban the peer: three strikes within 100 s,
four within 400 s. An identity that is never banned takes at most ⌊(0.4 + 0.0005 L) / 0.15⌋ strikes in any L
seconds: 14 in an hour, and at most one per T in the long run. Theorem 2 shows that one per T is reached, so the
long-run bound is tight. The theorem does not depend on damping. For the ablation's strike threshold k
(π = 0.45 / k), a single strike bans at k = 1, and at k = 5 the bound is 24 strikes per hour (measured: 20).

**Theorem 2 (periodic pacing).** Let a peer start at σ₀ and be struck every P seconds from time P.
(a) If P ≥ T it is never banned: its score alternates between σ₀ − π = 0.35 and σ₀.
(b) If P < 100 s it is banned at the 3rd strike; if 100 ≤ P < 150 s, at the 4th; if 150 ≤ P < T, by the
N(P)-th strike, N(P) = 3 + ⌊(0.1 + ρP) / (ρ (300 − P))⌋.
*Proof.* (a) Strike n is forgotten at nP + M ≤ (n + 1)P, so at most one strike is remembered at a time, the cap
is σ₀, and ρP ≥ π of recovery restores σ₀ before each strike. (b) Let a_n be the score after strike n: a₁ = 0.35
and, with one strike remembered in between, a₂ = 0.2 + ρP, which is above c. For P < 150 s, strike n − 1 is still
remembered at strike n + 1, so from strike 2 on the cap is c and a score above c does not rise: a₃ = 0.05 + ρP,
below β when P < 100 s; otherwise a₃ < c and a₄ ≤ c − π = 0.03, below β. For 150 ≤ P < T, between strikes n and
n + 1 (n ≥ 2) strike n − 1 is remembered for the first 300 − P seconds (cap c) and only strike n for the
remaining 2P − 300 (cap σ₀), so
a_{n+1} = min(max(a_n, min(a_n + ρ(300 − P), c)) + ρ(2P − 300), σ₀) − π.
With δ = ρ(300 − P) > 0 this is a_n − 2δ while a_n ≥ c and at most a_n − δ below c, so the first n with
a_n < β is at most 3 + ⌊(a₂ − β) / δ⌋ = N(P). ∎
The recurrence gives exactly what `recovery_bench` and TLC measure: P = 290 s is banned at the 35th strike
(10 150 s), P = 120 s at the 4th (480 s); N(290) = 52 is only the bound. Damping does not move the threshold;
while the score is above c it doubles the loss per period.

**Theorem 3 (length of a ban).** If the strike at t_b bans a peer and no strike follows, the ban is lifted at a
time t_r with 300 s ≤ t_r − t_b ≤ 500 s.
*Proof.* At t_b the score is below β and must reach υ, rising at most ρ per second: at least
(υ − β) / ρ = 300 s. Every strike is at or before t_b, so all are forgotten by t_b + M and the cap is σ₀ from
then on; before that the cap is at least c, so the score is at least min(ρM, c) = 0.15 at t_b + M and reaches υ
within (υ − 0.15) / ρ = 200 s more. ∎
No strike can follow while a peer is banned: `process_inbox_delivered` checks the ban before every strike site,
and a TCP node also closes the banned peer's connections (its frames are read again at most 30 s after the ban
is lifted).

**Lemma 2 (window).** In `ReplayFilter` a peer's bit for slot seq mod (W + 1) is set only for sequence numbers
in [seq_max − W, seq_max], where the slot is unique. So the filter returns `Replayed` for (origin, seq) from p
only if p delivered that (origin, seq) before and it is still in the window, and it accepts each (origin, seq) at
most once while it remembers the origin (seq_max never decreases). Peers are compared by keyed 64-bit tags; the
lemma assumes two neighbours' tags differ, which fails with probability below n² / 2⁶³ for n peers of an origin,
and the key is random per node, so a collision cannot be aimed at anyone.

**Theorem 4 (no strike without a forgery or a repeat).** A node strikes transport peer p only if p delivered
(a) a message that is not authentic (its signature does not verify, or its origin is not the node ID derived
from its key), or (b) two frames with the same signed origin and sequence
number (at the seen-cache check: the same signing key, by its first 16 bytes, which takes 2¹²⁸ work to match,
the same sequence number and the same ID), the second while that sequence
number was inside the node's window and not excused as an answer to the node's own sync request. The node's own
messages have no window at it, so a second echo of one of them counts as inside.
*Proof.* There are three strike sites, all in `process_inbox_delivered`: the seen-cache repeat (the entry holds
p's tag only if p delivered the same key since the cache was last cleared; out-of-window and excused repeats
are dropped before the strike), the authenticity check, which is (a), and a `Replayed` verdict that is not
excused (Lemma 2). ∎

**Corollary 5 (honest neighbours).** Let p and r be honest neighbours, and suppose (A1) the link from p to r
never duplicates a frame (TCP numbers frames, the node never re-sends one, and it sends r gossip at one address
only, whatever addresses lead to r), (A2) p loses the floor of no origin whose
messages it relays (a forgotten origin keeps its floor, and p loses a floor only after 3 584 more origins are
forgotten; a p restarted from a key file keeps its relay marks, which play the part of the floors), and (A3) p
sends an answer only to the node that asked, and each sync answer p sends r, and each gossip copy that such
an answer overtakes, arrives while r's session for that request is open (120 s; at most the four newest per peer,
which requests spaced 8 s apart turn into at least 32 s; at most 96 over all peers, oldest closed first, so the 32 s holds for a node with up to 24 neighbours). Then r
never strikes p, and by Lemma 1 never bans it.
*Proof.* (a) is impossible: p forwards only messages whose signature verified at p, the signature covers every
field, and verification is deterministic. For (b): p sends r a given (origin, seq) at most once as gossip (it
relays only what its own window accepted, which happens once by Lemma 2 while p tracks the origin and, after p
forgets it, never again for a sequence number at or below its floor, which A2 keeps; A1 rules out a second copy),
and at most once per request of r as an answer, tagged with that request and only if r's digest did not list it.
Take any frame from p that repeats an earlier one. A tagged answer is excused under its own request: its session
is open (A3), the digest did not list the message, and its batch holds at most 16 messages, the per-request
limit. An untagged repeat is p's one gossip copy, so the earlier frame was an answer to some request q; that
answer was recorded as answered under q whatever its verdict (it entered the seen cache, as a copy or after its
signature verified), so the gossip copy is excused under q, once, while q is open (A3). A frame of p that r's
budget dropped was never recorded and repeats nothing. Every repeat is excused, so (b) never holds. ∎
Honest strikes can therefore come only from A1–A3 failing, and a check above shows each one is needed
(`link-duplicates`, `floor-forgotten-framing`, `session-expires`). A reused ID with a new sequence number is
covered: the two messages have different keys. Writing this proof found the first two defects listed under
[found while proving the bounds](#found-while-proving-the-bounds).

**What one host can make a node do.** Theorems 1–4 are about one identity, and identities are free: a banned
peer comes back under a new key. Theorem 6 bounds what a whole machine can do with as many keys as it likes,
by charging the TCP listener's limits and the message budget to the connection's source host (an IPv4 address,
or an IPv6 /64; [architecture.md](architecture.md#over-tcp-the-crci-node-binary)). Notation (`tcp.rs`,
`sybil.rs`): M = 100 connections per listener, m = 8 per host, a = 16 handshakes at once and α = 2 per second
per host, f = 400 frames at once and φ = 200 per second per host, b = 20 new messages at once and q = 20 per
second per sender.

**Lemma 3 (who a full listener displaces).** Suppose a connection from host g arrives while all M slots are
taken. If some host holds more connections than g, the least recently active connection of one such host is
closed and g's is admitted; otherwise g's is refused. Hence (a) a host with no connection is never refused for
lack of slots; (b) a connection of host h, which holds c connections, is closed to admit another host's only if
every host holds at most c connections at that moment; (c) if honest hosts hold H connections in all and h holds
c of them, an adversary must control at least (M − H) / c hosts before it can close any of h's connections
this way, however often it reconnects.
*Proof.* `ConnRegistry::admit` compares g's count with the largest count and evicts from the largest host only
when that count exceeds g's: (a) and (b). For (c), at the moment of (b) the M slots are full and every host
holds at most c, so the adversary's M − H connections need at least (M − H) / c hosts. A host's other limits
close only its own connections (m per host, 4 per identity). ∎

**Theorem 6 (one host's cost to one node).** Let r be a TCP node with the default limits and h a host that may
use any number of identities and origins. Over any interval of length t:
(a) h holds at most m of r's connections at any time, and r runs at most a + αt handshakes for h (each one
Ed25519 signature and one verification);
(b) r reads at most f + φt frames from h, so it verifies at most that many frame signatures for h;
(c) at most b + qt frames charged to h pass `SybilGuard::check`, where a frame is charged to the host of its
sender's latest authenticated connection. Every message-signature verification, every strike for an
invalid signature or a forged origin, every acceptance and every relay of a new message comes after that
check, so each of them is bounded by b + qt too. Strikes for a repeat come before it and cost no signature
check; each needs a frame, so (b) bounds them;
(d) by Theorem 1 each identity of h is banned by its third strike within 100 s. Every ban needs a strike and
every strike a frame, so r bans at most f + φt of h's identities in the interval, and at most (f + φt) / 3 of
those that had no strike before it began.
An identity that connects from several hosts can move its frames from one to another, so for an adversary with
k hosts the bounds hold for the k together: k (b + qt) in (c), and so on.
*Proof.* (a) A connection is admitted only with a handshake token of its host (capacity a, refill α), and an
admission that brings the host above m closes its least recently active connection first. (b) Each frame read
takes a token from a bucket shared by the host's connections (capacity f, refill φ); the reader waits when it is
empty. (c) `crci-node` sets the sender's host (`SybilGuard::set_sender_host`) before each frame is processed, and
`check` takes a token from the host's bucket (capacity b, refill q) as well as from the identity's; the order of
`process_inbox_delivered` puts every signature check, strike for an invalid signature, acceptance and relay
after `check`, and the seen-cache repeat check before it. (d) The strike that bans lies in the interval; from the
initial score it is at least the third, as two leave 0.2. In all three buckets the state of a host is forgotten only once its bucket is full again
(`forget_idle_hosts`, the listener's host table), so a forgotten host comes back exactly as it would have been;
the identities' own records can be forgotten (`make_room`), which gives an identity a fresh budget of its own but
not its host. ∎
With the shipped values a host costs a node at most 2 handshakes, 200 frame checks and 20 message checks per
second however many keys it holds, against unlimited before; an honest node is one host with one identity and
never reaches any of these limits (it relays at most about 32 frames per second at the measured network
capacity). What Theorem 6 does not bound: an adversary with many addresses (the bounds are per host and add
up); the valid, fresh messages a host may inject inside its budget, which honest relays carry on to everyone
(threat model, item 7); and the slots of Lemma 3 (c) against an adversary that controls that many hosts.
Tests in `sybil.rs` (`identities_of_one_host_share_one_budget`, `a_host_with_one_identity_sees_the_same_budget_as_before`,
`a_host_gains_nothing_when_its_identities_are_forgotten`), in `tcp.rs` (`admission_tests`, nine) and over real
sockets and processes (`crates/crci-node/tests/tcp_host_limits_tests.rs`, five) check the lemma and the theorem
against the code; nine planted changes to the rules (no per-host eviction, `>=` for `>`, IPv6 per address,
no host budget, no identity limit, no handshake rate, the busiest connection evicted instead of the idlest,
the node not passing the host, the smallest host taken for the largest, and either table forgetting a host whose
budget is not full again) are each caught: eleven in all.

**What the code's arithmetic changes.** The code rounds a score to 0.001 after each strike, which can raise it
by 0.0005, the recovery of one second. In Theorem 1 the penalty becomes π − 0.0005 (three strikes ban within
97 s; still at most 14 strikes per hour). In Theorem 2 rounding gives back up to one second per period: a strike
every 299 s is also never banned (checked up to 3 000 strikes), and the bound on the banning strike becomes
3 + ⌊(0.101 + ρP) / (ρ(299 − P))⌋, which every P from 150 to 298 s meets. Recovery is applied in steps of at
least one second (`RECOVERY_STEP`), so a ban can be lifted up to a second late (2 000 bans in the test:
303–501 s). The score is an `f32`; in steps of a second its rounding changes recovery by less than 0.01 %.

## Defects this work found

The seen cache and the rescue store were keyed by the message ID, which the signer chooses. A validly signed
message under an honest message's ID, or a frame rejected at the key pin, made every later copy of the honest
message look already seen, through gossip and sync answers alike, with no strike for anyone: one frame
suppressed a message for good at that node. The defect came up while writing the seen cache into the model;
failing tests in `crates/crci-node/tests/message_id_tests.rs` confirmed it on the code, and TLC reproduces
it with the old key (`seen-by-id`, a 14-state trace, in 5.9 s). The cache and the store were then keyed by signing
key and ID, and later by sequence number too ([below](#found-while-proving-the-bounds)).

A second one came from the model's assumptions. Removing order on a link gave a 6-state counterexample
([`no-fifo-before-the-fix.txt`](evidence/tla-2026-11-07/no-fifo-before-the-fix.txt)): a sync
answer overtakes the gossip copy of the same message, and the copy strikes an honest neighbour. At first that looked
like an assumption TCP satisfies, until a two-process test failed under load for this very reason: an answer goes
to the address in the request and gossip to the address configured for the peer, and when those differ they take
different connections. One gossip copy that follows its answer was then excused if that answer had been accepted,
and later whatever happened to the answer ([below](#found-while-proving-the-bounds)), within the same limit of one
excuse per message per request, so an adversary gains no free repeats. With that fix order on a link is no longer
assumed: `reordering-links` passes.

### Found while proving the bounds

**A reused message ID framed an honest relay.** A signer chooses its IDs, so an adversary origin can sign a second
message under the ID of its first with a new sequence number. A relay whose seen cache had cleared since the first
accepted the second as new and forwarded it; a neighbour whose cache still held the ID took it for the relay
repeating itself and struck it, and three such messages banned it. The cache was keyed by signing key and ID; it is
now keyed by signing key, sequence number and ID. Test first:
`a_reused_message_id_with_a_new_sequence_number_does_not_strike_the_relay` (`state_bound_tests.rs`). TLC
reproduces the strike with the old key (`seen-without-seq`, 8 states), and none of the earlier checks could:
their adversary signed only one sequence number.

**An answer that was dropped did not excuse the gossip copy it overtook.** The excuse for a gossip copy that
follows its answer required the answer to have been accepted. When a third neighbour had delivered the message
first, the answer was a copy, was not recorded, and the asked neighbour's own gossip copy was then struck. The
same happened when the answer met a cleared seen cache (the window judged it delivered by another peer) or a key
pinned to another origin by an adversary squatting the origin's ID (pinning has since been removed: origin IDs are
derived from keys). A tagged answer now counts as answered as soon
as it enters the seen cache, whatever its verdict after that; the limit of one excuse per message per request and
16 per request is unchanged, so an adversary gains no repeat it could not already get by sending the answer first.
Tests first: the three `a_gossip_copy_after_its_answer_is_not_struck_when_…` tests (`partition_heal_tests.rs`).
TLC reproduces the strike with the old rule on a triangle with reordering links (`copied-answers-ignored`,
8 states); `reordering-links` could not, because its line has no third path.

**Recovery ran fast when applied often.** The score is an `f32` and recovery was applied on every frame. Adding a
tiny increment rounds, and for a peer sending a frame every millisecond recovery ran 1.3 % fast over 200 s (0.14 %
at one every 5 ms): the peer's own traffic bought it forgiveness, slightly. Recovery is now applied in steps of at
least one second, except in full right before a strike; at every rate measured the result is within 0.01 % of
the exact value.

### Found while bounding what one host can do

**A flood of new origins got honest relays banned.** Measured on the testbed with the two fixes below in place
(`make host-attack`, one seed, build `25e245ec`): a host sending 100 fresh messages per second, each from a new origin and
under an identity switched every 20, inside every budget, got 9 honest nodes banned with per-host limits on and 10
with them off. The flood pushed quiet honest origins out of the relays' tables (896 origins, the least active
first); a relay that forgot an origin accepted one of its old messages again when it arrived once more, relayed
it, and a neighbour that still remembered the origin struck the relay for the repeat, three times. This was the
documented cost of bounded memory (the model's `origin-forgotten-framing`), and the measurement showed that one
host inside its budget can make it happen. Fixed by keeping each forgotten origin's floor
(`ReplayFilter::forget`): its messages at or below the floor are stale, so none is accepted, delivered or relayed
twice. Tests first (`a_forgotten_origin_keeps_a_floor_so_its_old_messages_are_stale`,
`a_replay_after_forgetting_does_not_get_an_honest_relay_struck`, `floors_are_bounded_and_the_oldest_goes_first`
in `state_bound_tests.rs`); the model's two forgetting checks now pass, and fail again once the floor can be lost.

**Idle connections locked honest neighbours out.** The listener refused every new connection once 100 were
open, and an authenticated connection may stay idle for 10 minutes. One machine with one key could open 100
connections and send nothing: no strike, no ban, and every reconnect of an honest neighbour, every sync request
(a short connection of its own) and every sync answer to that node was refused from then on. Shown first by a
test that failed on the old listener (`without_per_host_limits_idle_connections_lock_an_honest_neighbour_out`
keeps that behaviour under `--per-host-limits off`); fixed by admission per source host (Lemma 3).

**Every new key brought a new budget.** The message budget, the frame pacing and the connection limit were
per identity or per connection, so a host that switched keys got a fresh burst each time: 40 identities of one
host had 800 fresh messages accepted where one identity gets 20, then 20 per second
(`without_per_host_limits_every_identity_brings_a_fresh_budget`; with the fix 262 in 12.1 s, the bound being
20 + 20 × 12.1 = 262). Fixed by charging all of a host's identities to one budget (Theorem 6).

### Found while reading the code line by line

Reading every line of the protocol core against Theorem 4 and Corollary 5 found ways to make an assumption of
the corollary false at will. Each was reproduced first by a test that failed (`framing_tests.rs`; the first
three on real processes, each ending in a ban of the honest node) and then fixed.

- **Answers aimed at a third node (A3).** A sync request names the address its answer goes to. Any peer could
  name a neighbour r of p: p then sent r up to 16 messages it had already relayed to it, tagged with a request
  r never made. One signed request, three strikes, p banned by r. Answers now leave only to a listener that
  authenticates as the requester.
- **One node behind two addresses (A1).** A malicious peer that forwards the connections it accepts to r (or an
  operator that lists r under two addresses) made p send r every message twice, over two sessions both
  correctly authenticated as p. Gossip now leaves to a node at one address only.
- **A restart (A2).** With a persistent key a restarted relay keeps its ID and loses its windows. It caught up
  from one neighbour, accepted the messages as new and relayed them to another that had them from the earlier
  run, which banned it. Relay marks now survive the restart.
- **Unauthenticated fields in the repeat check.** A copy is recognised before its signature is checked, and its
  own `origin` and type chose the window that decides whether a repeat is stale. Naming an origin whose window
  is far ahead, or a control type, made a replay look stale: no strike. This framed nobody; it let a replayer
  avoid strikes. The origin must now be the ID of the key, and control messages have their own keys.

The model has none of the four: it has no addresses and no restarts, and its adversary cannot change a field
of a copy without it being a different message. None of them contradicts Theorem 4; they are reasons the
assumptions of Corollary 5 did not hold in the code, which the proof had taken for granted.

### Found while comparing the model's limits with the code's

The model has an answer carry everything its digest lacks and a digest list everything a node holds. The code
limits both (16 messages, 256 origins), and "what the model leaves out" listed that without asking what it costs.

- **Useless answers crowded out lost messages.** An origin a digest leaves out looks unknown, so its messages are
  sent again at every request. A digest listed the 256 origins with the highest sequence numbers, which the signer
  chooses, and an answer took the first 16 missing messages by origin ID, the same ones every time. Once a node
  tracked more than 256 origins, with no adversary needed, a lost message whose origin sorted after 16 such
  repeats was never recovered from that neighbour; an adversary could arrange it for every honest origin with
  300 signed messages, one per key, inside every budget. No strike was involved, so no theorem is touched:
  it is `EventualDelivery` that the code did not inherit from the model. Reproduced by the four tests at the end of
  `partition_heal_tests.rs`, each failing before. A digest now lists the most active half of the origins, as
  forgetting ranks them, and fills up with a different random choice each time; an answer gives every origin one
  turn after another in an order that changes with every request, origins the digest lists first, the others at least a
  quarter of the batch; and a node asks again early only after a full answer that brought something new. Recovery above 256 tracked
  origins is still slower than below, and the model still has neither limit. Recorded data had shown it unnoticed: the six
  long simulation runs with 300 authors (no loss, no adversary in three of them) excused 352 000–419 000 repeats each,
  10.1–10.3 per request without attackers; on the new build 274 000–338 000, 9.3 per request. Bans, strikes and delivery are
  the same in all 210 simulation runs.
- **The model's digest left out what a floor covers.** A node whose origin was forgotten and came back lists the
  sequence numbers below the kept floor as held; the model did not. Harmless (the answers were stale), but the
  two differed and no test could see it, because no replayed behaviour forgot an origin. The model is corrected and
  30 behaviours with forgetting are replayed.
- **Theorem 6 (d) was too strong.** It bounded the identities banned in any interval by a third of the frames,
  which overlooks identities struck twice before the interval began: one frame bans each of those.

## How the model is tied to the code

A specification only says something about the code if the two agree, so two tests check that they do:

- **`reputation_model_conformance`:** every transition of the `rep-free` state graph, 12 552 in all
  (2 527 strikes, 10 025 ticks), is replayed through `PeerReputation`, which must reach the same score, strike
  ages and ban. This covers the whole reachable reputation state space on the 20 s grid.
- **`protocol_model_conformance`:** behaviours of `CRCI.tla` generated by TLC (simulation mode, fixed seeds)
  are replayed through real `NodeRuntime` instances, one per honest node, with real signatures and serialized
  frames. After every step every node must hold what the model says: accepted messages, each neighbour's
  score and ban, the seen cache and the replay window; digests, answer batches and relay targets are compared
  too, and so is the floor of every forgotten origin. The 290 committed behaviours (260 exported on 2026-11-17, and 30 that forget origins
  added on 2026-11-25, the others unchanged) have 10 588 steps: 2 297 messages accepted, 497 strikes,
  839 excused answers, 64 frames of banned peers, 507 forged frames, 243 stale frames (28 of them repeats), 40 own
  echoes, 611 gossip copies, 771 answers, 1 603 seen-cache clears, 195 forgotten origins, 222 clock ticks, 197 frames reusing another
  message's ID and 176 messages reusing their author's own ID with a new sequence number; 60 of them reorder
  frames on a link, 30 of those on a triangle. The test fails if any of these branches is never reached.

Each test was checked by breaking the code. Changing any of five reputation constants fails the first test. The
second caught 18 of 18 logic changes: the seen-cache key, each condition of both excuses, the stale drop, each
condition of an answer, relaying to the origin, the strike for a bad signature, the check for banned senders, the
own-echo check, the size of the recent store, sender recording, and the window's three rules
([`evidence/tla-2026-11-07/mutants.txt`](evidence/tla-2026-11-07/mutants.txt)); it also catches both rules
replaced while proving the bounds (the seen-cache key without the sequence number, and noting only accepted
answers). Those changes were tried against the earlier export of the behaviours, not against the current one.

The replay covers random behaviours, not all of them, and it checks the runtime, not the TCP node around it.

## What the model leaves out

- **Size:** at most 4 nodes, 3 messages, 4 adversary frames (or an unbounded single replay), 2 faults. A
  defect that needs more does not show up here.
- **One adversary, no collusion.** No Sybil identities.
- **Node IDs.** Keys are not separate from origins in the model: a node's ID stands for its key, as in the
  code, where the ID is derived from the key. A message claiming another node's ID is a forgery there and is
  covered by the model's frames with an invalid signature. The derivation is assumed second-preimage resistant
  (a 128-bit SHA-256 prefix).
- **Transport:** the handshake and frame authentication are assumed, not modelled. So are the per-host
  admission limits (Lemma 3 and Theorem 6 are proved on the code instead), frame pacing and the
  shun of banned peers, the 5 s answer cooldown and the answer budget. Token buckets are modelled as loss.
- **Sizes of digests and answers:** a digest lists every origin and an answer carries everything it lacks
  ([what the limits cost](#found-while-comparing-the-models-limits-with-the-codes)).
- **Echoes to the author:** a node does not send a message to its origin. A TCP node does, once, because it knows
  its peers by address; the author drops the echo as a copy.
- **Sessions** never expire in the checks that pass: answers are assumed to arrive within 120 s, and at most
  four sessions per peer exist. `session-expires` shows what happens otherwise.
- **Addresses and restarts:** a node has one link to each neighbour, answers go to the node that asked, and no
  node restarts ([found while reading the code](#found-while-reading-the-code-line-by-line)).
- **Chain-head announcements** are left out; they have their own counter and window.
- **Periodic requests** are modelled as requests with something to answer, plus the bound on requests above.
- **Time** is a grid: 20 s or 10 s in `Reputation.tla`, 100 s in `CRCI.tla`.
