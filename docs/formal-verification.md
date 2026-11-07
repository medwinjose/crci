# Formal Verification (TLA+)

The isolation protocol is specified in TLA+ and model-checked with TLC, and the specification is tied to
the Rust code by replaying what TLC explores through the real implementation.

**What this is:** exhaustive checking of small instances, up to 42 million distinct states each. **What it
is not:** a proof for every network size. A property below holds for the stated constants; nothing here
says it holds at any N, though the mechanism does not depend on N and the emulation and simulation runs
([benchmarks.md](benchmarks.md)) test larger networks.

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
make tla                                 # every check: about 35 min on 12 cores, several GB of temporary disk
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

Run on 2026-11-07 on an Intel i5-12450HX (12 logical CPUs); 24 of 24 checks gave the expected outcome,
51.8 min in total with other jobs on the host (36.5 min alone, before the last two checks were added). *Diameter* is the depth of TLC's complete state-graph search.

### Reputation (one observer, one peer)

| Check | Setting | Outcome | Generated | Distinct | Diameter |
|---|---|---|---|---|---|
| `rep-free` | Every schedule of up to 6 strikes, 20 s grid | 4 invariants hold | 36 557 | 30 104 | 91 |
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
| `honest-loss-sync` | Line | 2, 2 | 2, 2 | none | Seen cache may clear | 135 910 941 | 26 658 112 | 33 |
| `adversary` | Kite | 2, 2 | 0, 0 | 4 | | 50 474 929 | 9 913 640 | 25 |
| `adversary-sync` | Kite | 1, 2 | 0, 1 | 1 | | 42 825 864 | 8 344 894 | 31 |
| `adversary-loss` | Line + a | 1, 2 | 1, 1 | 1 | | 10 495 981 | 2 228 776 | 28 |
| `adversary-time` | Line + a | 1, 2 | 0, 1 | 3 | Time passes | 240 021 567 | 42 189 740 | 38 |
| `reordering-links` | Line | 2, 2 | 1, 2 | none | Links reorder frames | 54 648 052 | 10 205 074 | 26 |
| `window-slides` | Line | 3, 1 | 0, 1 | none | | 7 485 | 3 053 | 21 |
| `delivery` | Line | 2, 2 | 1, 2 | none | `EventualDelivery` | 434 253 | 133 030 | 26 |
| `eviction` | Kite | 1, 2 | 0, 0 | Unbounded replays | `EveryNeighbourBans` | 119 957 | 26 048 | 29 |

### Protocol: remove one thing, and TLC finds the failure

Each row removes one assumption or defence. The counterexample shows that the property is not vacuous and
that the assumption is needed. These are the assumptions the results above rest on.

| Check | Removed | Violated | Trace (states) | What it means |
|---|---|---|---|---|
| `link-duplicates` | A network that does not duplicate | `HonestNeverStruck` | 6 | A duplicated frame strikes an honest neighbour (threat model, gap 9). TCP does not duplicate frames. |
| `session-expires` | Answers arriving within their session | `HonestNeverStruck` | 9 | An answer later than 120 s, or beyond the four newest sessions per peer, is a strike. |
| `no-excuse` | The excuse for answers | `HonestNeverStruck` | 8 | Why the excuse exists. |
| `no-window` | The replay window | `NoDoubleDelivery` | 6 | After the seen cache clears, a replay would be delivered again. |
| `no-signatures` | Signature checks | `NoForgedDelivery` | 3 | |
| `seen-by-id` | The signing key in the seen-cache key | `DeliveredWhenQuiescent` | 14 | The defect below. |
| `window-too-small` | W at least the loss depth (W = 1, 3 messages, 1 loss) | `DeliveredWhenQuiescent` | 11 | A message that falls out of every neighbour's window before it is recovered is lost for good. |
| `origin-forgotten-delivery` | Remembering every origin (forgetting switched on) | `NoDoubleDelivery` | 8 | After a node forgets an origin and clears its seen cache, a replay is delivered again. |
| `origin-forgotten-framing` | The same | `HonestNeverStruck` | 8 | The node relays that replay, and a neighbour that still remembers the origin strikes it. |
| `paced-replayer-spared` | Replays faster than recovery (a fair clock) | `EveryNeighbourBans` | 25 | The residual bound: an adversary that replays at most once per 300 s is never banned. |

## Defects this work found

The seen cache and the rescue store were keyed by the message ID, which the signer chooses. A validly signed
message under an honest message's ID, or a frame rejected at the key pin, made every later copy of the honest
message look already seen, through gossip and sync answers alike, with no strike for anyone: one frame
suppressed a message for good at that node. The defect came up while writing the seen cache into the model;
failing tests in `crates/crci-node/tests/message_id_tests.rs` confirmed it on the code, and TLC reproduces
it with the old key (`seen-by-id`, a 14-state trace, in 5.9 s). The cache and the store are now keyed by signing
key and ID.

A second one came from the model's assumptions. Removing order on a link gave a 6-state counterexample
([`no-fifo-before-the-fix.txt`](evidence/tla-2026-11-07/no-fifo-before-the-fix.txt)): a sync
answer overtakes the gossip copy of the same message, and the copy strikes an honest neighbour. At first that looked
like an assumption TCP satisfies, until a two-process test failed under load for this very reason: an answer goes
to the address in the request and gossip to the address configured for the peer, and when those differ they take
different connections. One gossip copy that follows its accepted answer is now excused, within the same limit of one
excuse per message per request, so an adversary gains no free repeats. With that fix order on a link is no longer
assumed: `reordering-links` passes.

## How the model is tied to the code

A specification only says something about the code if the two agree, so two tests check that they do:

- **`reputation_model_conformance`:** every transition of the `rep-free` state graph, 12 552 in all
  (2 527 strikes, 10 025 ticks), is replayed through `PeerReputation`, which must reach the same score, strike
  ages and ban. This covers the whole reachable reputation state space on the 20 s grid.
- **`protocol_model_conformance`:** behaviours of `CRCI.tla` generated by TLC (simulation mode, fixed seeds)
  are replayed through real `NodeRuntime` instances, one per honest node, with real signatures and serialized
  frames. After every step every node must hold what the model says: accepted messages, each neighbour's
  score and ban, the seen cache and the replay window; digests, answer batches and relay targets are compared
  too. The 200 committed behaviours have 7 063 steps: 1 633 messages accepted, 402 strikes, 625 excused
  answers, 46 frames of banned peers, 466 forged frames, 63 stale frames (23 of them repeats), 29 own echoes, 516
  gossip copies, 516 answers, 784 seen-cache clears, 229 clock ticks and 221 frames reusing another message's ID;
  30 of them reorder frames on a link. The test fails if any of these branches is never reached.

Each test was checked by breaking the code. Changing any of five reputation constants fails the first test. The
second caught 18 of 18 logic changes: the seen-cache key, each condition of both excuses, the stale drop, each
condition of an answer, relaying to the origin, the strike for a bad signature, the check for banned senders, the
own-echo check, the size of the recent store, sender recording, and the window's three rules
([`evidence/tla-2026-11-07/mutants.txt`](evidence/tla-2026-11-07/mutants.txt)).

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
