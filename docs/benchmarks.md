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
pipeline messages per second, discovery convergence time, Ed25519 sign/verify cost,
and any multi-machine or physical-radio measurement.
