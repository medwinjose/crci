# Formal Verification (TLA+)

Files in [`docs/tla/`](tla/):

| File | Purpose |
|---|---|
| `CRCITypes.tla` | Constants and message record type. |
| `CRCI.tla` | State machine: `Originate`, `Gossip`, `Receive`, `Byzantine`; invariants and temporal properties. |
| `MC.tla`, `MC.cfg` | Model-checking configuration used for the recorded run. |
| `CRCI.cfg` | A larger configuration (5 nodes, 2 Byzantine, `MaxSeq = 4`, also `GossipProgress`). No recorded run. |
| `tla2tools.jar` | TLC, pinned: TLA+ Tools 2.0 2024-08-08, tag v1.7.4 (revision `5a47802`). Requires Java 11+. |

## Running TLC

```bash
cd docs/tla
java -XX:+UseParallelGC -cp tla2tools.jar tlc2.TLC -config MC.cfg MC.tla
```

TLC writes scratch state to `docs/tla/states/` (gitignored).

## What `MC.cfg` checks

- **Constants:** `Nodes = {n1, n2, n3, n4}`, `ByzantineNodes = {n4}`, `MaxSeq = 0`, `MinTrustedRep = 5`,
  one message ID (`m1`), one zone, one message kind (`RESCUE`).
- **Invariants:** `NoForgedOrigin`, `ReplayNeverDeliveredTwice`, `ByzantineContainment`,
  `RescueNeverDropped`.
- **Temporal property:** `RescueLiveness`. `GossipProgress` is **not** checked by `MC.cfg`.
- **Fairness:** `Spec` uses weak fairness on the whole next-state relation, `WF_vars(Next)`.

**Reported result:** 649 states generated, 81 distinct, no violations (2026-09-08, commit `cc5e805`).
This run has not been repeated in this repository and its output is not in `docs/evidence/`; it is re-run
and recorded when the specification is updated.

## What the model does and does not cover

Stated plainly, so that no stronger claim is made from it:

- With `MaxSeq = 0` the `Originate` action is never enabled (`seqCounter[n] < 0` is false), so honest
  nodes send nothing. The only message in the model is a single `RESCUE` message injected by the Byzantine
  node, which honest nodes then gossip and receive.
- `reputation` is initialised to 10 and **no action changes it**, so `ByzantineContainment` holds
  trivially. The model has no strikes, no penalties, no bans and no eviction.
- `NoForgedOrigin` holds by construction: every message in `ValidMessages` has an origin in `Nodes`.
  Signatures are not modelled.
- `ReplayNeverDeliveredTwice` is enforced directly by the guard of `Receive`. The model does not
  contain the sliding window or the strict-monotonic filter.
- Battery state never changes from `FULL`, so the battery rule in `Gossip` is never exercised.

In short, the current specification checks dissemination of one message among 4 nodes. It does **not**
model the local isolation mechanism that the preprint describes. Bringing the spec in line with the
final protocol (strikes, ban threshold, sliding window, recovery) is planned work. The comments inside
`CRCI.tla` describe the properties more strongly than the model supports.
