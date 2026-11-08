# Formal Verification (TLA+)

The isolation protocol is specified in TLA+ and model-checked with TLC, and the specification is tied to
the Rust code by replaying what TLC explores through the real implementation.

**What this is:** exhaustive checking of small instances, up to 42 million distinct states each. **What it
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
make tla                                 # every check: about 85 min on 12 cores, several GB of temporary disk
python3 docs/tla/check.py --quick        # the checks that take about a minute or less (run in CI)
python3 docs/tla/check.py seen-by-id     # one check
cargo test -p crci-core --test reputation_model_conformance --test protocol_model_conformance
```

Raw TLC output of the recorded run: [`evidence/tla-2026-11-07/`](evidence/tla-2026-11-07/).

## The adversary and the network

The adversary holds no honest key and cannot pose as another sender (connections are authenticated). It
replays anything it has received, with any sync tag; forges frames whose signature does not verify; signs
its own messages under any message ID it has seen; and otherwise stays silent, omitting whatever it likes.
There is one adversary and no collusion. Links are queues that can lose frames, FIFO as one TCP connection per
neighbour gives, and in `reordering-links` and one set of replayed traces they also reorder, as two connections
to one neighbour can. A frame dropped by a token bucket has the same effect as a lost one, so budgets are
modelled as loss. Honest nodes keep the seen cache, window and recent store as the code does, and the seen
cache can be cleared whole, as its limit of 1 792 entries does. A switch lets a node forget an origin whole, as its
limit on tracked origins does; it is off in every check that must hold, because the two checks with it on
(`origin-forgotten-delivery`, `origin-forgotten-framing`) must break: bounded memory has a price while identities are free.

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

Run on 2026-11-08 on an Intel i5-12450HX (12 logical CPUs); 28 of 28 checks gave the expected outcome,
84.0 min in total ([`evidence/theorems-2026-11-08/tla/`](evidence/theorems-2026-11-08/tla/)). Keying the seen
cache by sequence number too (below) roughly quadruples `honest-loss-sync`, the largest check. *Diameter* is the depth of TLC's complete state-graph search.

### Reputation (one observer, one peer)

| Check | Setting | Outcome | Generated | Distinct | Diameter |
|---|---|---|---|---|---|
| `rep-free` | Every schedule of up to 6 strikes, 20 s grid | 4 invariants hold | 36 557 | 30 104 | 92 |
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

### Protocol: remove one thing, and TLC finds the failure

Each row removes one assumption or defence. The counterexample shows that the property is not vacuous and
that the assumption is needed. These are the assumptions the results above rest on.

| Check | Removed | Violated | Trace (states) | What it means |
|---|---|---|---|---|
| `link-duplicates` | A network that does not duplicate | `HonestNeverStruck` | 5 | A duplicated frame strikes an honest neighbour (threat model, gap 9). TCP does not duplicate frames. |
| `session-expires` | Answers arriving within their session | `HonestNeverStruck` | 7 | An answer later than 120 s, or beyond the four newest sessions per peer, is a strike. |
| `no-excuse` | The excuse for answers | `HonestNeverStruck` | 6 | Why the excuse exists. |
| `no-window` | The replay window | `NoDoubleDelivery` | 6 | After the seen cache clears, a replay would be delivered again. |
| `no-signatures` | Signature checks | `NoForgedDelivery` | 3 | |
| `seen-by-id` | The signing key in the seen-cache key | `DeliveredWhenQuiescent` | 14 | The first defect below. |
| `seen-without-seq` | The sequence number in the seen-cache key | `HonestNeverStruck` | 8 | A reused message ID frames an honest relay ([below](#found-while-proving-the-bounds)). |
| `copied-answers-ignored` | Noting answers that were dropped as copies | `HonestNeverStruck` | 8 | The gossip copy an answer overtook is struck ([below](#found-while-proving-the-bounds)). |
| `window-too-small` | W at least the loss depth (W = 1, 3 messages, 1 loss) | `DeliveredWhenQuiescent` | 11 | A message that falls out of every neighbour's window before it is recovered is lost for good. |
| `origin-forgotten-delivery` | Remembering every origin (forgetting switched on) | `NoDoubleDelivery` | 7 | After a node forgets an origin and clears its seen cache, a replay is delivered again. |
| `origin-forgotten-framing` | The same | `HonestNeverStruck` | 8 | The node relays that replay, and a neighbour that still remembers the origin strikes it. |
| `paced-replayer-spared` | Replays faster than recovery (a fair clock) | `EveryNeighbourBans` | 18 | The residual bound: an adversary that replays at most once per 300 s is never banned. |

## Analytical bounds

TLC checks small instances. The statements below hold for any network size, because each concerns one
observer and one neighbour (Theorems 1–3) or the checks one node applies to one frame (Theorem 4). Each proof
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
(a) a message whose signature does not verify, or (b) two frames with the same signed origin and sequence
number (at the seen-cache check: the same signing key, by its first 16 bytes, which takes 2¹²⁸ work to match,
the same sequence number and the same ID), the second while that sequence
number was inside the node's window and not excused as an answer to the node's own sync request.
*Proof.* There are three strike sites, all in `process_inbox_delivered`: the seen-cache repeat (the entry holds
p's tag only if p delivered the same key since the cache was last cleared; out-of-window and excused repeats
are dropped before the strike), the signature check, which is (a), and a `Replayed` verdict that is not
excused (Lemma 2). ∎

**Corollary 5 (honest neighbours).** Let p and r be honest neighbours, and suppose (A1) the link from p to r
never duplicates a frame (TCP numbers frames and the node never re-sends one), (A2) p forgets no origin whose
messages it relays (it tracks fewer than 896), and (A3) each sync answer p sends r, and each gossip copy that such
an answer overtakes, arrives while r's session for that request is open (120 s, the four newest per peer). Then r
never strikes p, and by Lemma 1 never bans it.
*Proof.* (a) is impossible: p forwards only messages whose signature verified at p, the signature covers every
field, and verification is deterministic. For (b): p sends r a given (origin, seq) at most once as gossip (it
relays only what its own window accepted, which happens once by Lemma 2 and A2; A1 rules out a second copy),
and at most once per request of r as an answer, tagged with that request and only if r's digest did not list it.
Take any frame from p that repeats an earlier one. A tagged answer is excused under its own request: its session
is open (A3), the digest did not list the message, and its batch holds at most 16 messages, the per-request
limit. An untagged repeat is p's one gossip copy, so the earlier frame was an answer to some request q; that
answer was recorded as answered under q whatever its verdict (it entered the seen cache, as a copy or after its
signature verified), so the gossip copy is excused under q, once, while q is open (A3). A frame of p that r's
budget dropped was never recorded and repeats nothing. Every repeat is excused, so (b) never holds. ∎
Honest strikes can therefore come only from A1–A3 failing, and a check above shows each one is needed
(`link-duplicates`, `origin-forgotten-framing`, `session-expires`). A reused ID with a new sequence number is
covered: the two messages have different keys. Writing this proof found the first two defects listed under
[found while proving the bounds](#found-while-proving-the-bounds).

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
pinned to another origin by an adversary squatting the origin's ID. A tagged answer now counts as answered as soon
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

## How the model is tied to the code

A specification only says something about the code if the two agree, so two tests check that they do:

- **`reputation_model_conformance`:** every transition of the `rep-free` state graph, 12 552 in all
  (2 527 strikes, 10 025 ticks), is replayed through `PeerReputation`, which must reach the same score, strike
  ages and ban. This covers the whole reachable reputation state space on the 20 s grid.
- **`protocol_model_conformance`:** behaviours of `CRCI.tla` generated by TLC (simulation mode, fixed seeds)
  are replayed through real `NodeRuntime` instances, one per honest node, with real signatures and serialized
  frames. After every step every node must hold what the model says: accepted messages, each neighbour's
  score and ban, the seen cache and the replay window; digests, answer batches and relay targets are compared
  too. The 260 committed behaviours have 9 233 steps: 2 000 messages accepted, 470 strikes, 842 excused
  answers, 50 frames of banned peers, 487 forged frames, 40 stale frames (8 of them repeats), 36 own echoes, 543
  gossip copies, 740 answers, 1 348 seen-cache clears, 206 clock ticks, 199 frames reusing another message's ID
  and 200 messages reusing their author's own ID with a new sequence number; 60 of them reorder frames on a
  link, 30 of those on a triangle. The test fails if any of these branches is never reached.

Each test was checked by breaking the code. Changing any of five reputation constants fails the first test. The
second caught 18 of 18 logic changes: the seen-cache key, each condition of both excuses, the stale drop, each
condition of an answer, relaying to the origin, the strike for a bad signature, the check for banned senders, the
own-echo check, the size of the recent store, sender recording, and the window's three rules
([`evidence/tla-2026-11-07/mutants.txt`](evidence/tla-2026-11-07/mutants.txt)); it also catches both rules
replaced while proving the bounds (the seen-cache key without the sequence number, and noting only accepted
answers).

The replay covers random behaviours, not all of them, and it checks the runtime, not the TCP node around it.

## What the model leaves out

- **Size:** at most 4 nodes, 3 messages, 4 adversary frames (or an unbounded single replay), 2 faults. A
  defect that needs more does not show up here.
- **One adversary, no collusion.** No Sybil identities.
- **Key pinning.** An adversary announcing an honest node's ID first (threat model, gap 1) is not modelled:
  keys are not separate from origins, so the adversary signs only as itself. That denial of service is known
  and open.
- **Transport:** the handshake and frame authentication are assumed, not modelled. So are frame pacing and the
  shun of banned peers, the limits on batch and origin counts in a digest, the 5 s answer cooldown and the answer
  budget. Token buckets are modelled as loss.
- **Sessions** never expire in the checks that pass: answers are assumed to arrive within 120 s, and at most
  four sessions per peer exist. `session-expires` shows what happens otherwise.
- **Chain-head announcements** are left out; they have their own counter and window.
- **Periodic requests** are modelled as requests with something to answer, plus the bound on requests above.
- **Time** is a grid: 20 s or 10 s in `Reputation.tla`, 100 s in `CRCI.tla`.
