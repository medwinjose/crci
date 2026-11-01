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
- `recovery_bench` and `ablation_bench` call `node.sig_verifications_count.clear()` between messages.
  Without that call, `NodeRuntime` drops every message from an origin after its 5th signature check
  (`crates/crci-core/src/runtime.rs`, the BFT-008 throttle, which is never reset). The results of those two
  harnesses therefore describe the reputation logic with that throttle bypassed.

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

## Paced attack vs. reputation recovery (preprint §IV-D)

- **Harness:** `crates/crci-node/benches/recovery_bench.rs`
- **Command (recorded run):** `cargo bench -p crci-node --bench recovery_bench --features paper_v2`
- **Procedure:** with `window_size = 0`, the adversary sends 3 replays. "Rapid": back to back. "Paced":
  between replays the adversary itself sends 300 valid, fresh messages 10 ms apart, so its reputation
  recovers before the next strike.
- **Raw output:** [`evidence/paper-v1-benches-2026-09-25/recovery_paper_v2_run2_0255.txt`](evidence/paper-v1-benches-2026-09-25/recovery_paper_v2_run2_0255.txt)

| Condition | Adversary banned? |
|---|---|
| Rapid (0 recovery packets) | yes |
| Paced (300 recovery packets between replays) | no |

An earlier run from a different build (`recovery_paper_v2_run1_0243.txt`) reports both conditions not
banned. The paced-attack result is a known evasion; strike-damped recovery is planned to address it.

**Current rule:** each "replay" in this harness is a fresh message carrying an older sequence number, not a
repeat of an earlier message. Under the stale-packet rule that packet is dropped without a strike, so the
harness no longer exercises a replay and its output is not a measurement of the evasion. It is planned to be
rewritten with true duplicate replays together with strike-damped recovery.

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
