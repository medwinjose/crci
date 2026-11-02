# Benchmarks

Every number below comes from a recorded run. Each entry names the harness, the exact command, and the
raw output file. Anything without a recorded run is listed under [Not measured](#not-measured).

## How the benchmarks run (read this first)

All three paper benchmarks drive `NodeRuntime` **in-process**. A test writes JSON-encoded `WireMessage`
bytes into a shared `HashMap` inbox and calls `NodeRuntime::process_inbox()` directly. There are no
sockets, no TCP, no second process and no network stack. Latencies are therefore processing time plus any
`std::thread::sleep` the harness inserts, measured with `Instant::elapsed().as_millis()` (whole
milliseconds, truncated).

Two harness details affect what the results mean:

- The harnesses register **one honest peer that sends a single message** per trial. "Honest survival"
  means that peer is not banned after that message.
- The recorded runs of `recovery_bench` and `ablation_bench` below reset a per-origin signature-check
  counter between messages, because `NodeRuntime` then dropped every message from an origin after its 5th
  signature check. That counter has been removed (signature checks are now bounded by the per-sender token
  bucket) and the harnesses no longer reset anything.
- The per-sender token bucket (20 messages, then 5 per second) applies in every harness.
- `recovery_bench` runs on a simulated clock (`SybilGuard::use_manual_clock` and `advance_time`), so it
  covers six hours per condition in seconds and produces byte-identical output on every run.

Bench targets have `test = false`, so `cargo test` never runs them or rewrites their CSVs.

## Byzantine eviction (preprint Table III)

- **Harness:** `crates/crci-node/benches/byzantine_bench.rs`
- **Command:** `cargo bench -p crci-node --bench byzantine_bench` (writes to `crates/crci-node/benches/results/`)
- **Procedure per trial:** honest peer `node-h` sends one signed message; adversary `node-b` sends one
  signed message and then replays it 3 times. Each replay is a duplicate message ID from the same
  sender, which costs −0.15 reputation: 0.5 → 0.35 → 0.2 → 0.05 < 0.1 = banned. The timer starts before
  the first replay and stops after the third. The "15 ms delay" condition sleeps 15 ms before each replay.
- **Raw data:** `byzantine_eviction_baseline.csv`, `byzantine_eviction_jitter.csv`

| Condition | Trials | Strikes to ban | Eviction time, mean (min / max) | Adversary banned and honest peer not banned |
|---|---|---|---|---|
| No delay | 50 | 3 | 0.00 ms (0 / 0) | 50 / 50 |
| 15 ms sleep before each replay | 50 | 3 | 45.82 ms (45 / 46) | 50 / 50 |

The procedure is deterministic, so 50 trials show the harness is consistent, not statistical spread. The
45.82 ms is almost entirely the 3 × 15 ms of sleep.

## Sliding window vs. reorder depth (preprint Table IV)

- **Harness:** `crates/crci-node/benches/ablation_bench.rs`
- **Command (recorded run):** `cargo bench -p crci-node --bench ablation_bench --features paper_v2`
  (the `paper_v2` feature has since been removed; the sliding window is now always on)
- **Procedure:** for W ∈ {8, 16, 32, 64} and reorder depth D ∈ {1, 2, 4, 8, 16, 32}, the honest peer
  sends 3 pairs of packets: first sequence s + D, then the delayed sequence s. The cell reads
  "false-positive ban" if the honest peer ends up banned. In the default build `window_size` is ignored
  (strict-monotonic check); the `paper_v2` feature enables the window.
- **Raw output:** [`evidence/paper-v1-benches-2026-09-25/ablation_paper_v2_0237.txt`](evidence/paper-v1-benches-2026-09-25/ablation_paper_v2_0237.txt)

| D | W = 8 | W = 16 | W = 32 | W = 64 |
|---|---|---|---|---|
| 1, 2, 4, 8 | survival | survival | survival | survival |
| 16 | false-positive ban | survival | survival | survival |
| 32 | false-positive ban | false-positive ban | survival | survival |

**Strict-monotonic column:** the preprint's Table IV also shows a "Strict-Monotonic (W = 0)" column with a
false-positive ban in every row. The only recorded strict-monotonic runs (default build,
[`ablation_default-build_run1_0235.txt`](evidence/paper-v1-benches-2026-09-25/ablation_default-build_run1_0235.txt)
and `_run2_0235.txt`) report **survival in every cell**. That column is not supported by any recorded run and
must be re-measured before submission.

**Current rule:** the recorded runs above predate the stale-packet rule. Today a packet more than W behind
is dropped without a strike, so reordering can no longer ban an honest peer at any W, and this harness
shows survival in every cell by construction. What W now controls is delivery: packets beyond the window
are lost. Re-measuring the window ablation as delivered packets, not bans, is planned.

## Paced attack vs. reputation recovery

- **Harness:** `crates/crci-node/benches/recovery_bench.rs`
- **Command:** `cargo bench -p crci-node --bench recovery_bench`
- **Procedure:** one receiver, one adversarial transport peer that authors its own messages. The adversary
  sends a fresh, validly signed message every 10 s and, every *P* seconds, resends its latest message
  unchanged (an exact duplicate from the same peer inside the window: one strike). Simulated clock, 6 h
  (21 600 s) per period, one step per simulated second.
- **Raw data:** [`recovery_paced_sweep.csv`](../crates/crci-node/benches/results/recovery_paced_sweep.csv)
  (one row per *P*), [`recovery_trajectory.csv`](../crates/crci-node/benches/results/recovery_trajectory.csv)
  (score every 10 s for *P* = 120, 290, 330 s); console summary in
  [`evidence/paced-attack-2026-11-02/`](evidence/paced-attack-2026-11-02/). Two runs gave byte-identical CSVs.

| Replay period *P* (s) | Strikes landed in 6 h | First ban (s) | Time banned |
|---|---|---|---|
| 1 | 89 | 3 | 99.6 % |
| 15 | 85 | 45 | 95.2 % |
| 60 | 74 | 180 | 82.9 % |
| 120 | 74 | 480 | 75.3 % |
| 240 | 73 | 1 680 | 26.8 % |
| 290 | 72 | 10 150 | 2.9 % |
| 300 | 72 | never | 0 % |
| 600 | 36 | never | 0 % |
| 1 200 | 18 | never | 0 % |

The measured threshold is the forgiveness period, *T* = penalty / recovery rate = 0.15 / 0.0005 = 300 s:
an adversary pacing at *P* ≥ 300 s is never banned and gets at most 12 replays per hour past admission; below it the
adversary is banned, sooner the faster it replays. Strike damping does not move this threshold; it halves
the time a sub-threshold adversary survives (net change per period 0.001 *P* − 0.3 instead of
0.0005 *P* − 0.15). An adversary that keeps replaying through bans (*P* = 1 s) gets about 15 replays per
hour past admission across ban cycles while being banned 99.6 % of the time. Every replay is dropped and costs one
signature check.

**Before this rule** (same commit lineage, previous recovery rules; real-time run, adversary sending valid
traffic at 4 messages per second; harness and output in
[`evidence/paced-attack-2026-11-02/`](evidence/paced-attack-2026-11-02/)): an adversary replaying every
20 s or 30 s was never banned and its score rose to 0.85 after 8 replays; every 10 s its score fell by
0.03 per replay; every 5 s it was banned after 5 replays. Recovery then grew with the adversary's own
traffic (+0.005·√Δt per admitted message), so sending more valid messages bought faster forgiveness.

The 2026-09-25 runs (`recovery_paper_v2_*` in
[`evidence/paper-v1-benches-2026-09-25/`](evidence/paper-v1-benches-2026-09-25/)) measured a different
harness (fresh messages with an older sequence number, which the current rules drop without a strike) and
are kept as history only.

## Ed25519 cost

- **Harness:** `crates/crci-node/benches/crypto_bench.rs` (criterion, 100 samples per case; signing and
  verification go through `WireMessage::sign_with` / `verify_signature`, i.e. `verify_strict` on the
  canonical signed bytes)
- **Command:** `cargo bench -p crci-node --bench crypto_bench`
- **Recorded run (2026-11-02, Intel i5-12450HX, Windows 11, one thread):** keygen 13.4 µs, sign 15.2 µs,
  verify 37.0 µs for a valid and 36.9 µs for an invalid signature (about 27 000 verifications per second per
  core), canonical encoding 44 ns. Criterion's console output is not committed; the numbers are for this
  CPU only.

## Emulation testbed (real processes, one host)

- **Harness:** `scripts/testbed/` (see its [README](../scripts/testbed/README.md)): one `crci-node` process
  per Linux network namespace, one veth link each to a shared bridge, `tc netem` 10 ms one-way delay per
  hop (20 ms round trip), the overlay topology given with `--peers`. Emulated on one host (WSL2 on an
  Intel i5-12450HX, 12 logical CPUs, 7.6 GiB), not on separate machines or radios.
- **Command:** `make testbed OUT=…` (= `sudo scripts/testbed/run_all.sh OUT target/release/crci-node`;
  one build for every run; SHA-256 of the binary in the evidence folder).
- **Each run:** seed 1, a 60 s window. Traffic 1 signed message/s in total unless stated (0.5/s from a
  source S, 0.5/s from the other nodes). A relay B whose removal keeps the graph connected, chosen to
  maximise the set D of nodes whose every shortest path from S runs through B, repeats every message once
  from 20 s on. Fidelity criteria (all must hold): every node listening at the start, mean host CPU below
  70 %, no swap activity, no kernel input-backlog drops, ICMP RTT on 10 sampled overlay links with median
  within 10 % and 95th percentile within 25 % of 20 ms. **Every run below met all of them.**
- **Recorded runs (2026-11-02):** [`evidence/testbed-2026-11-02/`](evidence/testbed-2026-11-02/).

| Run | N | Topology (mean/max degree, diameter, avg. path, clustering) | Delivery after eviction | Latency median / p95 after eviction | B banned by | Eviction latency (median) | Honest false evictions | Host CPU mean / max | RSS per node |
|---|---|---|---|---|---|---|---|---|---|
| rgg-20 | 20 | 4.4 / 7, 6, 2.56, 0.50 | 100 % | 42 / 64 ms | 6/6 | 4.0 s | 0 | 0.3 / 1.9 % | 6.7 MiB |
| rgg-50 | 50 | 6.0 / 11, 10, 3.67, 0.58 | 100 % | 42 / 74 ms | 10/10 | 4.0 s | 0 | 0.9 / 4.4 % | 6.9 MiB |
| rgg-100 | 100 | 7.6 / 15, 12, 4.79, 0.60 | 100 % | 48 / 92 ms | 14/14 | 2.0 s | 0 | 2.3 / 8.8 % | 6.9 MiB |
| mesh-20 | 20 | 4 / 4, 4, 2.16, 0.10 | 100 % | 21 / 42 ms | 4/4 | 4.0 s | 0 | 0.3 / 1.6 % | 6.7 MiB |
| mesh-50 | 50 | 4 / 4, 5, 2.93, 0.03 | 100 % | 32 / 52 ms | 4/4 | 2.0 s | 0 | 0.8 / 3.8 % | 6.7 MiB |
| mesh-100 | 100 | 4 / 4, 6, 3.58, 0.04 | 100 % | 43 / 58 ms | 4/4 | 2.0 s | 0 | 1.1 / 5.9 % | 6.7 MiB |
| sweep-rgg-200 | 200 | 9.1 / 19, 14, 6.17, 0.66 | 100 % | 74 / 127 ms | 16/16 | 2.0 s | 0 | 5.3 / 19.6 % | 7.0 MiB |
| sweep-rgg-300 | 300 | 10.6 / 22, 16, 6.56, 0.63 | 100 % | 91 / 171 ms | 22/22 | 2.0 s | 0 | 9.5 / 37.5 % | 7.1 MiB |
| sweep-rgg-500 | 500 | 11.2 / 20, 20, 7.98, 0.62 | 100 % | 140 / 280 ms | 18/18 | 2.0 s | 0 | 17.0 / 76.9 % | 7.3 MiB |
| stress4-rgg-500 (4 msg/s) | 500 | as above | 100 % | 140 / 296 ms | 18/18 | 0.5 s | 0 | 63.7 / 99.9 % | 8.3 MiB |
| load4-rgg-50 (4 msg/s) | 50 | as rgg-50 | 100 % | 42 / 74 ms | 10/10 | 1.0 s | 0 | 2.7 / 5.6 % | 7.3 MiB |
| load6-rgg-50 (6 msg/s) | 50 | as rgg-50 | 86 % (mean per message) | 42 / 94 ms | 10/10 | 2.7 s | 0 | 3.8 / 6.7 % | 7.4 MiB |

What the runs show, stated plainly:

- **Eviction at an intermediate hop.** In every run all of B's neighbours banned it, no honest node was
  banned, and after the ban no node accepted anything relayed by B. S's messages still reached every node
  of D (e.g. rgg-50: 306/306 deliveries to the 18 downstream nodes after the ban; their median latency rose
  from 44 to 54 ms because the alternate paths are longer). Eviction latency is the time for B to relay
  three repeated messages to a neighbour, so it depends on the message rate (about 2 s at 1 msg/s, 0.5 s at
  4 msg/s). Some neighbours needed about 4 s: a repeat that arrives within 10 s of a sync request is
  excused once (at most 16 per request) as a possible sync answer. In rgg-50 the five neighbours that
  banned B at 4 s are exactly the five that excused two of its repeats.
- **Capacity sweep.** At 1 msg/s every N up to 500 met every fidelity criterion (mean CPU 17 % at
  N = 500). At 4 msg/s, close to the protocol's capacity (below), N = 500 still met them but with mean CPU
  64 % and peaks at 100 %: 500 nodes is the practical ceiling of this host. Larger N was not tried.
- **Host effects grow with N.** The median latency above the configured delay per hop is 0.5–1.0 ms up to
  N = 100, 1.4 ms at 200, 3.0 ms at 300 and 6.8 ms at 500. Part of that is processing in the protocol and
  part is many processes sharing one host; the runs cannot separate the two.
- **Capacity is set by the per-neighbour budget.** Every link carries every message, so the token bucket
  (20, then 5 per second per neighbour) caps the network-wide message rate at about 5 per second for any
  N. At 6 msg/s, 7 397 honest frames were dropped by rate limiting (no strikes, no false evictions) and
  messages reached 86 % of nodes on average within the window after the eviction. 241 deliveries arrived
  more than 5 s after origination (at most 28 s), consistent with recovery by the 30 s anti-entropy
  exchange; the logs do not record which path delivered them.
- **Not shown:** link failures, more than one seed per configuration in the table above (the impairment
  runs below use three), and any result beyond one host.

### Packet loss, jitter and reordering

- **Harness:** the same testbed with `tc netem` on every node's egress: one-way delay 40 ms with ±15 ms
  uniform jitter, plus packet loss and reordering (`netem reorder`: that share of packets skips the delay).
  Every overlay hop crosses one egress, so loss applies per hop, below the TCP connections the nodes use.
- **Command:** `make netem OUT=…` (= `sudo scripts/testbed/run_netem.sh OUT-netem target/release/crci-node`).
  Conditions: loss 0/5/10/15/20 %, reordering 5/10/15 %, and loss 10 % with reordering 10 %; random geometric
  and 4-regular mesh topologies, N = 20/50/100, seeds 1–3 (the seed selects the topology instance, the roles
  and the traffic schedule; the kernel's netem randomness is not seeded). 9 conditions × 2 × 3 × 3 = 162
  runs, 60 s window, 1 message/s, relay B repeating every message from 20 s on, as above. One build, SHA-256
  in the evidence folder.
- **Fidelity:** same criteria as above; the round-trip criterion compares the sampled ICMP median and 95th
  percentile with the median and 95th percentile of the configured two-hop delay distribution (Monte Carlo).
  **All 162 runs met every criterion** (host CPU mean at most 2.7 %, peak 11.2 %).
- **Recorded runs (2026-11-02):** [`evidence/netem-2026-11-02/`](evidence/netem-2026-11-02/): per-condition
  CSVs, `netem/summary.csv`, per-run `summary.json`, and the raw logs of the run with the most false
  evictions.

Each row aggregates the 18 runs of a condition (2 topologies × 3 sizes × 3 seeds).

| Condition | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit |
|---|---|---|---|---|---|---|---|
| loss 0 % (jitter only) | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 4.1 s (8.0 s) | 0 |
| loss 5 % | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 6.1 s (12.1 s) | 0 |
| loss 10 % | 2 | 2, 1 | 18/18 | 100 % | 100 % (100 %) | 6.2 s (12.2 s) | 0 |
| loss 15 % | 2 | 2, 1 | 18/18 | 100 % | 100 % (100 %) | 6.2 s (38.8 s) | 262 |
| loss 20 % | 9 | 12, 3 | 17/18 | 99.4 % | 96.1 % (63 %) | 7.2 s (33.4 s) | 540 |
| reordering 5 % | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 4.1 s (8.1 s) | 0 |
| reordering 10 % | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 4.1 s (8.1 s) | 0 |
| reordering 15 % | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 4.1 s (8.0 s) | 0 |
| loss 10 % + reordering 10 % | 0 | 0 | 18/18 | 100 % | 100 % (100 %) | 8.0 s (12.7 s) | 3 |

What the runs show, stated plainly:

- **Reordering is harmless; loss is not.** Reordering of up to 15 % of packets changed nothing measurable
  (no false evictions, full delivery, eviction latency unchanged). Packet loss makes eviction slower
  (median about 6 s at 5–15 % loss against 4 s without) and, from 10 % loss on, causes honest nodes to be
  banned.
- **Honest nodes were banned.** 13 of the 162 runs banned at least one honest node (2 at 10 % loss,
  2 at 15 %, 9 at 20 %; up to 3 nodes in one run, at 20 % loss, N = 100). None occurred at 5 % loss or
  below, nor with reordering alone. In the run with the most (`netem-l20-rgg-100-s1`) an honest neighbour
  delivered messages it had already delivered, in one burst, and the receiver struck it as a replay and
  banned it, before B started its fault. The cause of the repeated delivery is not yet established (a
  hypothesis: frames re-sent after a loss-induced reconnect of the TCP connection); it is a protocol-level
  weakness, not a testbed artifact, and is not fixed here.
- **At 20 % loss the network degrades.** Delivery after the ban fell to 96 % on average and to 63 % in the
  worst run, frames were dropped by the rate limit (540 across the condition; rate-limited frames never strike), and in
  one run (rgg, N = 20) B was banned by only some of its neighbours.
- **Eviction of B is robust up to 15 % loss.** In 17 of 18 runs at 20 % and all runs at lower loss every
  neighbour of B banned it.
- **Limits.** Loss is applied per egress and independently of the TCP connection; burst loss, asymmetric
  loss and link failures were not tested. Three seeds per cell is too few for confidence intervals; the table
  reports means and extremes only.

## WAN smoke test

- **Test:** `crates/crci-node/tests/chaos_wan_real_tests.rs::test_real_wan_chaos_netns` (`#[ignore]`,
  Linux + root only; CI runs it on `ubuntu-latest`)
- **What it does:** creates 5 network namespaces on a bridge, applies `tc netem delay 50ms loss 5%`,
  starts 5 `crci-node --listen … --peers …` processes, sleeps 10 s, reads each process's `VmRSS`,
  and kills them.
- **What it checks:** only that the setup commands succeed. It creates no partition, sends no attack
  traffic and asserts nothing about delivery or isolation. `partition_recovery_ms` in its output is the
  elapsed time of the sleep, and `throughput_msgs` is the constant 42.
- **Recorded run:** CI run 34750740352, log in
  [`evidence/ci-wan-smoke-2026-09-13/`](evidence/ci-wan-smoke-2026-09-13/).

## Not measured

The following appear in older material but have **no recorded run** in this repository:
propagation rounds for 10/100/1000 nodes, consensus convergence, replay-filter checks per second,
pipeline messages per second, discovery convergence time,
and any multi-machine or physical-radio measurement.
