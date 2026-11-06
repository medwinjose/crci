# Evidence Index

Raw outputs from real runs, kept so every published number can be traced to a file.
Files are stored verbatim. The only change ever made is UTF-16 → UTF-8 conversion and CRLF → LF
line endings, noted per folder below. SHA-256 hashes are of the files as committed.

Benchmark CSVs that feed the paper live next to their harness in
`crates/crci-node/benches/results/` (see [docs/benchmarks.md](../benchmarks.md)).

## `ablation-2026-11-06/`

Baselines (defence modes) and the strike-threshold and replay-window ablations. Each CSV in `simulation/` and
`testbed/` is regenerated from the run summaries by `scripts/sim/ablation_table.py` and
`scripts/testbed/baselines_table.py`; the window and paced-attacker tables are the committed bench CSVs
`crates/crci-node/benches/results/ablation_window.csv` and `ablation_paced.csv` (`cargo bench -p crci-node
--bench ablation_bench`, deterministic).

| Folder / file | What it is | SHA-256 |
|---|---|---|
| `simulation/results.csv` | 234 in-process runs (N = 100, rgg and mesh, seeds 1–3): 108 strike-threshold runs and 126 defence-mode runs; `mesh_sim` binary `0f8dcceb26e5a786f8e867da2527bd0377edcdd1e8accd4f4b85d9f0a28bbaa6` | `bbe919675de6e108b48a165958d5e0b02f3e0fbfa45a520ae257e8a93dd22ded` |
| `simulation/k_tradeoff.csv` | the threshold table, means over runs | `4b05a0bacee6332d808200ac9879a62a38ca364521e2146ecd57ab9c3b6f1c21` |
| `simulation/modes.csv` | the defence-mode table, means over runs | `3e52715468a6bf36fac366cfd3805da0e96b67540918f6117c66a7f126a94d87` |
| `testbed/baselines.csv` | 63 real-process runs (rgg N = 50, seeds 1–3), `crci-node` binary `aa619cb0e480cd729e282cf4800974e6c984e4e83438dd057ad6510f42b48ad8` | `12ca5704d6913c9b73a38f45c70b44f86b4b80c2db9a22d1ae931244b098b494` |
| `testbed/results.csv` | the generic per-run table of the same runs | `9984b18da839e01500eb65194d60f460a3a9757cfdeb3adb32ca787b1dfc8419` |
| `crates/crci-node/benches/results/ablation_window.csv` | window ablation | `49f07c4d8705368821be80db56800ed904f2d7d94a0dcba32855568d2e3ea0ae` |
| `crates/crci-node/benches/results/ablation_paced.csv` | strike threshold against a paced attacker | `14f7550e4b812564952f841ca9487c8f97fa3b4289c051df99ad525214346933` |

`testbed/first-attempt/` holds the `summary.json` of eight runs that failed only the "no swap I/O" criterion
and were repeated; `testbed/runs/` has the repeats. Per-run raw logs are not kept (about 15 MiB each).

- **What it proves:** against replay only the ban isolates the attacker (10 of 10 neighbours in all nine
  replay runs; 0 for the baselines); against a fresh-message flood nothing is struck and the per-origin
  budget is what keeps delivery at 1.0 (`static-bucket` falls to 0.74); removing the admission limit keeps
  delivery but multiplies the neighbours' CPU by up to six; the strike threshold trades honest-link bans
  under duplicate frames against a faster sustainable paced replay; a strict-monotonic window loses
  reordered messages and bans nobody.
- **What it does not prove:** behaviour above N = 100, colluding or churning identities, multi-neighbour
  floods, or any real network's duplicate rate (the duplicates are injected). The simulation has no
  capacity model, so its delivery numbers for `vanilla` say nothing about overload; three `vanilla` runs at
  200 messages/s exceed the testbed's CPU limit and are marked not faithful.

## `flood-2026-11-06/`

Six run sets of the invalid-signature flood (`scripts/testbed/run_flood.sh`; rgg N = 50, seeds 1–3), one
folder per node binary, in the order they were measured. Every run folder holds its `summary.json` and the
topology line; each set holds `flood.csv`, `flood.md` (means and the pass/fail criteria), `results.csv`, the
figures and the runner's log.

| Folder | Node binary | What it shows |
|---|---|---|
| `first-set/` | `7644ec0d` | before any fix: every frame of a banned peer parsed and logged (299.5 MiB of logs at 2 560 forged frames/s per link) |
| `second-set/` | `4a2e7963` | banned peers' frames dropped before parsing, logs flat at 13 MiB; no CPU data |
| `third-set/` | `4a2e7963` | with CPU seconds per process: a banned peer still costs its neighbours 6.55 s (reference 1.0 s) |
| `fourth-set/` | `77f1a010` (SHA-256 `77f1a01033400efc381e0361863e2378973fa0635e43c646cf91baaacf93d220`) | banned peers' connections closed: 0.56 s; readmission 10/10; the valid fresh flood (`fresh-1000`) costs 6.3 s and 174 MiB |
| `fifth-set/` | `22703b6d` | frame pacing; also the honest-throughput runs (`load/`) and the attacker sets (`attacks/`) |
| `sixth-set/` | `cc996fbb` (SHA-256 `cc996fbb38a70afbc1349bba3af096d9422add4ad4e5bb990df71e1ddbeac8c4`) | final binary: pacing counter, flood set, `load/`, `attacks/` |

Raw logs are kept for the final binary only (`sixth-set/`): one 2 560 frames/s run, the readmission run and one
valid fresh flood run.

| File | SHA-256 |
|---|---|
| `sixth-set/flood.csv` | `67536e1e808c38e197f56bbc30eca79e98a895a2cef531451e5a911e19c332f5` |
| `sixth-set/raw-logs-forged-x64-s1.tar.gz` | `5aacf0a2204a87847f758389e06148c1c0e4b37ce581706b8cb2bb7ce592e114` |
| `sixth-set/raw-logs-readmit-s1.tar.gz` | `73e64338a903a0a48894ea69c483d932ab34e8bfff1716c4624493c86c0411bb` |
| `sixth-set/raw-logs-fresh-1000-s1.tar.gz` | `b1b439344ed2bedb1c3ee9fe0b224fb636144237025581bf2019b4d16dda76cb` |

- **What it proves:** a relay that forges signatures is banned by all its neighbours in 32–136 ms (median per run) with 0 honest
  false evictions; after the ban it costs them no more CPU than the same load with valid signatures; a banned
  peer is readmitted through the probe; a sender that is never struck is bounded by frame pacing, which no
  honest link ever reaches (counter 0 in every load run).
- **What it does not prove:** behaviour above N = 100 under flood, identity churn (a banned peer returning
  with a new key pair), or a flood from several neighbours at once. Raw logs of the earlier binaries are not
  kept; their numbers are in each set's `flood.csv` and `summary.json` files.
- **Criteria failed as written:** log size compared with a reference that lacked the attacker's valid traffic;
  the first neighbour-CPU bound; the 3 454 honest-frame bound for load 32 taken from one earlier set (the three
  recorded sets span 2 343–3 615 for the same seeds). They are listed in
  [benchmarks.md](../benchmarks.md#invalid-signature-flood).

## `ci-wan-smoke-2026-09-13/`

| File | SHA-256 |
|---|---|
| `ubuntu_log.txt` | `eb9cdd931dafe525471b921431d6434b380df4fffad9c38e57f8a7d41d5363c4` |

- **What it is:** the full GitHub Actions log of CI run 34750740352 (job 103706730226, `ubuntu-latest`,
  2026-09-13), exported as UTF-16LE. Bytes are unchanged, so the hash still matches the original export.
- **What it proves:** `test_real_wan_chaos_netns` ran to completion. It created 5 network namespaces with
  `tc netem` (50 ms delay, 5 % loss), started 5 `crci-node` processes, waited 10 s and read their memory.
- **What it does not prove:** anything about isolation, partitions or delivery. The test induces no
  partition and its only assertion is that setup commands succeed; it is a smoke test
  (see [docs/benchmarks.md](../benchmarks.md#wan-smoke-test)).

## `android-bft038-2026-08-23/`

| File | SHA-256 |
|---|---|
| `instrumentation-output.txt` | `5fc958c0513428166d5edc8a29d3195c0d61c74d9a4d708d1364d27088c32ff5` |
| `junit-report.xml` | `af1a98a5e18b751d5df8e4a6c8ba4f04a57188a51553972dbaf878a319f836a2` |
| `logcat.txt` | `a0440ba0fab37ce083cf6b76e50e855f255a88e74505d69641eb3904f97efd1b` |
| `test-result.textproto` | `93e56a54ce87dd735a51f4edd281ca548265389c8069c54b4522506f19e960d9` |

- **What it is:** Gradle `connectedAndroidTest` output for `Bft038PanicBoundaryTest`, run on the
  emulator `medium_phone(AVD) - 16` (x86_64) on 2026-08-23. Copied from `crci-android/app/build/`
  on 2026-11-01 (line endings normalised to LF).
- **What it proves:** `testBft038_FfiPanicDoesNotCrashJvm` passed (`OK (1 test)`) against the
  `libcrci_core.so` built at that time (crate version 0.1.0): a Rust panic behind the FFI surfaced as a
  Kotlin exception instead of crashing the JVM.
- **Caveat:** the test asserts `crciVersion() == "0.1.0"`, and `crci_version()` in
  `crates/crci-core/src/ffi.rs` returns a hard-coded `"0.1.0"` even though the crate is 2.1.0. The test
  has not been re-run since 2026-08-23.

## `paper-v1-benches-2026-09-25/`

PowerShell `>` redirects of `cargo bench` taken on 2026-09-25 (local Windows host, crate version 0.1.0).
Converted from UTF-16LE to UTF-8; the `Γ£ì`-style characters are the original console mis-encoding.

| File | Command (from the file's own header) | SHA-256 |
|---|---|---|
| `ablation_default-build_run1_0235.txt` | `cargo bench --bench ablation_bench` | `6fda9820fff585269e72a4f6041c9f4cc34c6b94727381fc3fa49f469f77c89e` |
| `ablation_default-build_run2_0235.txt` | `cargo bench --bench ablation_bench` | `20d4a53e922ba3762fb1ac1a5a4c5df44b8d18f57a2d46821412c41401e5b056` |
| `ablation_paper_v2_0237.txt` | `cargo bench --bench ablation_bench --features paper_v2` | `3226bcc024b4e0f9391975b51e259079899ddbd5c3c37e786208aa086feb32c5` |
| `recovery_paper_v2_run1_0243.txt` | `cargo bench --bench recovery_bench --features paper_v2` | `1388e197feb4cd9f3b44f0e85527427e3b7acdb41e44686cadf1f5a1af06c97b` |
| `recovery_paper_v2_run2_0255.txt` | `cargo bench --bench recovery_bench --features paper_v2` | `8b35fa25a73557c12354c4c1502a881fe47ec3aca06381587b2ee6a188368882` |

What these files show, stated plainly:

- `ablation_paper_v2_0237.txt` is the source of the W = 8/16/32/64 columns of the preprint's Table IV.
- Both default-build ablation runs report **Survival in every cell**. In the default build the window
  size is ignored (strict-monotonic replay check), so these runs are the only recorded strict-monotonic
  results, and they do **not** support the preprint's "Strict-Monotonic (W = 0): False-Positive Ban"
  column. That column has no recorded run and is to be re-measured.
- The logs show `BFT-008 Signature verification limit exceeded … dropping message`, i.e. the per-origin
  signature-check cap was dropping honest traffic during these runs.
- `recovery_paper_v2_run2_0255.txt` was the source of the preprint's first paced-attack result, now
  superseded by `paced-attack-2026-11-02/`
  (Rapid Attack: banned; Paced Attack with 300 recovery packets: not banned).
  `recovery_paper_v2_run1_0243.txt` is an earlier run from a different build that reports different
  results (both conditions survive). It is kept because it was observed, not because it is correct.

## `paced-attack-2026-11-02/`

| File | SHA-256 |
|---|---|
| `recovery_bench_summary.txt` | `34f5b0970983e6e8acc3a8197fee1f3a0c94623e21e86c52771b7092042bc842` |
| `before_realtime_paced_replay.txt` | `1bcfe189f1515786655e2b97f22f859b6b55febd62068b7ad8a63e12e010aee6` |
| `before_harness_paced_before.rs.txt` | `86a09fad584dca2f94e6530db833eec6dfaf3872c7799320ceb94810d3beed42` |

- `recovery_bench_summary.txt`: the summary lines of `cargo bench -p crci-node --bench recovery_bench`
  (`grep -E "^Paced|^ +period_s|^ +[0-9]+ +[0-9]+ +[0-9]+|^Wrote"`; the full output also contains one log
  line per processed message). The CSVs it wrote are in `crates/crci-node/benches/results/`
  (`recovery_paced_sweep.csv` SHA-256 `0b7c3b49e972b6ec89b8977159de92d1e48bf672ede867c5847fe2092924c516`,
  `recovery_trajectory.csv` SHA-256 `ef657bba88ccc0ef11b8bf3f81add0497d0be7061037733caca55342d783274c`).
- `before_realtime_paced_replay.txt`: the same attack on the previous recovery rules (commit 7d70aff),
  in real time, from `before_harness_paced_before.rs.txt` placed at
  `crates/crci-node/tests/paced_before.rs` and run with
  `cargo test --release -p crci-node --test paced_before -- --nocapture` (lines matching `^P=` and the
  test result).

## `testbed-2026-11-02/`

| File | SHA-256 |
|---|---|
| `results.csv` | `695aa8ef9c2c2df41d48ae306bd8582e8e2024f0c4f6d688f9052ffdca253b11` |
| `rgg-50-raw.tar.gz` | `d3e07fc73f51d999e9c45331ac9b4f5ca89d0c545f28796438d9057c789cc19e` |
| `run_all.txt` | `38a719d24473cf81ebf9611a58760b241f4fd65fdef147b561a439707206e9d6` |
| `binary.sha256` | `6ab3bfda37f5d5bed46ef7cdf13c90dfdd2ccac3214fd68a81ad3a7bf8919382` |

- **What it is:** the emulation testbed runs of 2026-11-02 (`scripts/testbed/run_all.sh`, WSL2 on the
  i5-12450HX host): RGG and 4-regular mesh at N = 20, 50, 100, a capacity sweep at N = 200, 300, 500,
  and load runs at 4 and 6 messages/s. `run_all.txt` is the script's console output (renamed from
  `.log`); `binary.sha256` is the hash of the `crci-node` release binary every run used.
- `runs/<run>/`: `summary.json` (every metric and the fidelity verdict), `bans.csv`, `meta.json`
  (configuration, timings, roles, traffic plan) and `topology.txt` (generator output).
- `rgg-50-raw.tar.gz`: the complete raw output of the N = 50 eviction run (every node's log, ping output,
  host samples, per-node RSS); `scripts/testbed/analyze.py` regenerates that run's summary from it.
  Raw logs of the other runs (about 530 MB) are not committed.
- **What it shows and does not show:** see [benchmarks.md](../benchmarks.md#emulation-testbed-real-processes-one-host).
  **Superseded** by `testbed-final-2026-11-03/` (one seed per configuration, earlier protocol).

## `netem-2026-11-02/`

| File | SHA-256 |
|---|---|
| `results.csv` | `56ebbb854c80e6c6bcc3c015c7dacc45c4f0ab74efb7d31b4be3d27a3fbcc785` |
| `run_netem.txt` | `aa232717124be61ed399e3d461b64fe3d5b499d17628e4aade53545c7b7fe8fc` |
| `netem-l20-rgg-100-s1-raw.tar.gz` | `5a55ecf254f8052c45801053a60e5744a9d0f110f11288e2fda588febf5eac47` |
| `netem/summary.csv` | `894ddd638e3a67550002a29fe7d5fec9283c90b165d6cf166dcb360a8653b1d5` |
| `binary.sha256` | `6ab3bfda37f5d5bed46ef7cdf13c90dfdd2ccac3214fd68a81ad3a7bf8919382` |

- **What it is:** the first packet-loss, jitter and reordering sweep (`scripts/testbed/run_netem.sh`,
  162 runs) on the protocol of 2026-11-02. `netem/` holds one CSV per condition; `runs/<run>/` the
  per-run summaries; the tarball is the complete raw output of the run with the most honest bans.
- **What it shows:** honest nodes were banned from 10 % loss on (13 of 162 runs). The cause, found in the
  tarball's logs, was a late answer to an anti-entropy request struck as a replay after the requester's
  10 s session had expired; fixed since (sync answers are tagged with their request). **Superseded** by
  `testbed-final-2026-11-03/`; kept as the before-state.

## `testbed-final-2026-11-03/`

| File | SHA-256 |
|---|---|
| `binary.sha256` | `d0a96f4e9afc89962d44d84f6353f91e57b2303ab4845290bf7ede9d55eb432b` |
| `criteria.txt` | `343ba3ed5d488a4dc11456cb3888bb91c47f389f8427f60bc474833a52b65156` |
| `progress.txt` | `a9a12af0ede438a5d7c5d1b7c54725827cbba2aab7fea01bb5e7db7c94493eff` |
| `r4/results.csv` | `235d2ca2313c63b2947c82d7ae85192d539c5b9e01ece6e998deffc2117978db` |
| `netem/results.csv` | `7c426235c34dc4ae8c55965ebe12cc7ba361bcfdd864afbeabcaafb86ded0a7d` |
| `netem/conditions/conditions.md` | `0bd09d8ac82271b20e1ea6547f7d25f8f17b9c72d0f1b3b5d5d68688af288b10` |
| `r3b/results.csv` | `2d144acd089eb761864f7252ec929f4d896e43b030a7cf155036d79e3d36aa4a` |
| `fl/results.csv` | `d426469c9c43e435a35035646e89b6b13ce28a9f2156fb9853e097972aaf986e` |
| `ev20/results.csv` | `6f46131cda53f056ad720552554f4c58c2bcd333e9d89d1a72294dcb60783abe` |
| `before-origin-budget/fl/results.csv` | `f155cc01007d5437acd0cdd39ab74dc461137927d18ebda81ffdc1a37de2ffe3` |
| `ev20-rgg-20-s1-raw.tar.gz` | `35610e6ddebdcd1beee6c22c9a36ed2082fe7804ed22e8643ec3fe23aee6e3ff` |
| `fl-new-refill20-s3-raw.tar.gz` | `aa571f5c2fc841f6ef27201c6819926685fab8a6d04f81a1a30617a0d64d0e63` |

- **What it is:** every testbed number in [benchmarks.md](../benchmarks.md#emulation-testbed-real-processes-one-host),
  from one `crci-node` build (`binary.sha256`; the second hash is the older build used for the
  before/after replayer runs). `r4/` = `run_all.sh` (48 runs), `netem/` = `run_netem.sh` (162 runs),
  `r3b/`, `fl/`, `ev20/` = the over-budget replayer, fresh-message flood and 20 % loss with a 60 s drain
  (`run_attacks.sh` runs the same configurations), 30 runs. `criteria.txt` holds the pass criteria, written
  before the runs (one internal label removed). `console.txt` files are the drivers' console output.
- `runs/<run>/`: `summary.json`, `bans.csv`, `meta.json` per run. Raw logs (about 3 GB) are not committed
  except the two tarballs: the slowest delivery at 20 % loss (60 s) and one fresh-message flood run.
- `before-origin-budget/fl/`: the same flood on the build before the per-origin budget (honest delivery
  82–98 %).
- **Superseded** by `testbed-fast-recovery-2026-11-04/` (every configuration re-run on the build that
  recovers frames lost with an aborted connection); kept as the before-state.

## `testbed-fast-recovery-2026-11-04/`

| File | SHA-256 |
|---|---|
| `binary.sha256` | `e21018b18ea74ee1395467eb7331bba231bf659e88358bf2c12d812f2994ff0d` |
| `criteria.txt` | `82c9f227051e5748413e6a03e81213ac2bc343a6df53c345076a2045f4cf8c3f` |
| `progress.txt` | `d024999a029badbcd362fbb54bc1dfbf83b13dcabfb1cd77454e4a6532e83c91` |
| `summary.txt` | `5af4e65be897c78f6892febc1cc72031c7ef1963d1289e7cc0b44d0034965fb7` |
| `netem/results.csv` | `b5ca57b4ab59be990d6bd29be72822aa4515c589e2c54766192ff788de1bc875` |
| `netem/conditions/conditions.md` | `ae8adfdca36330a8b415e30cb4967064ca4988902c7c4ba8ebc5d8b2b0363701` |
| `r4/results.csv` | `9a8897b32b2735a2a837e843ed9784c5eb782146619fa170366df7024f3ec0ad` |
| `attacks/over-budget/results.csv` | `410740577c9a0393e5fe084177612092156f62ba4bfc84201dc82409f47c0f54` |
| `attacks/flood/results.csv` | `e3ea78337725574cf04477a0216149ae54577ae7b7772e5d05a550f22fb4f1af` |
| `attacks/eventual/results.csv` | `fe0347d8c2aeb366578a35f9cf9d3bad735c8a8db6aac3b71615f0d4aacf1428` |
| `l20-rgg-20-s3-raw.tar.gz` | `30eec98446852bbcee56b98fa7e529f699a8977a15ad37bad5968396b7576789` |
| `first-fix/summary.txt` | `6b9cd02f6cbe6354df47a9ad7af1a195074308ee93cbeefa868c3e1eb53450ee` |
| `first-fix/binary.sha256` | `769895d88ce464f1db3ae2ea80275a6382178e6928ece76176846ea261cb433e` |
| `first-fix/l20-rgg-20-s1-raw.tar.gz` | `b50625109a5164f970b106e4b3903b4454d29213ad3b6e0281c86fd3645610ff` |

- **What it is:** every testbed number in [benchmarks.md](../benchmarks.md#emulation-testbed-real-processes-one-host)
  since 2026-11-04, from one `crci-node` build (`binary.sha256`, commit efbf48e). `r4/` = `run_all.sh`
  (48 runs), `netem/` = `run_netem.sh` (162 runs), `attacks/` = `run_attacks.sh` (27 runs: over-budget
  replayer, fresh-message flood, and 20 % loss with a 60 s drain in `eventual/`). `criteria.txt` holds the
  pass criteria, written before the first of the two builds below was run (internal labels replaced).
  `summary.txt` is the verbatim comparison with the previous build; `progress.txt` includes the
  20 % loss gate the driver checked before running the rest.
- **Result against the criteria:** worst post-ban delivery at 20 % loss 25.3 s (criterion < 30 s; previous
  build 60.0 s), 99th percentile 3.4 s (criterion ≤ 4.7 s); every other criterion of the previous runs
  held. One criterion failed: deliveries later than 5 s were to fall at 10, 15 and 20 % loss and fell
  only at 15 % (37 → 142, 293 → 156, 833 → 882). That column counts deliveries, not messages; at 10 % one
  message accounts for 96 of the 142. Deliveries later than 20 s fell from 0 / 51 / 72 to 0 / 0 / 3.
- `runs/<run>/`: `summary.json`, `bans.csv`, `meta.json` per run, plus `latency.csv.gz` (every delivery)
  for the 20 % loss runs in `attacks/eventual/`. Raw logs are not committed except the tarball of the
  slowest of those runs (25.3 s).
- `first-fix/`: the first version of the change (commit 5c363f7), which asked a peer for a sync only
  when the peer opened a new relay connection. It **failed** the latency criterion (worst 65.5 s, p99
  5.9 s): a sender with nothing more to send never reconnected, so frames lost just before a link fell
  quiet still waited for the 30 s exchange. Results, summary and the raw logs of its slowest run are kept.

## `sim-2026-11-04/`

| File | SHA-256 |
|---|---|
| `before-fixes/criteria-check-echo-defect.txt` | `41a2309e0d2f33347bded3623d17e2af670b024d7338fee85870c7ede406e9f7` |
| `before-fixes/results-echo-defect.csv` | `59b9c35ed48fb0f88d37ecf67152889de29a6cc36df47c5c945b56c596c121f5` |
| `binary.sha256` | `61b603ee5c2f04b2d8a8c5efc262a04f0b9f362d00f98061e9258a89eef721be` |
| `console.txt` | `decb4cf61bbf7b353a5c6bfb47898a929006cdf8bc1fa18bca82593474cc3465` |
| `criteria-check.txt` | `527fe717fb77ce939744d4da7eb9022160a8a50b1b2528f3b4574bc8bbbe3dc4` |
| `criteria.txt` | `ab3ca45d8537a7ad50560a42093352cd8ea45fc4554d446df796e0195338cbca` |
| `cross-check.csv` | `405dd3b23fed2596298ece22f1cc79082cd5b6faf5024cfba8d37bc0e390fa1d` |
| `figures/cost-vs-n.svg` | `4f3bb77463971b625ce43969d6ff15ec9560650e69303f97fc6ebb780c64e4dc` |
| `figures/delivery-vs-attackers.svg` | `d281aa71f479ff19059dec342e7d6b633146841f6a045b7738925e1ae1e6f8b8` |
| `figures/largest-component-vs-attackers.svg` | `7d9a2a9b4aa0806cd3a0a0030d0952999bb2a03d30483cf07a8c3a54f06fc344` |
| `recheck/00e4b508-1-eventual-results.csv` | `d7cb2c5c23bcd21d1275f6faaceec51d990e0f538cd57c38b64415c9a48ad0ff` |
| `recheck/00e4b508-1-flood-results.csv` | `36266cc39d59acb0f222826654aabb164eda1c3cf464cc86ef2c9fcd82692823` |
| `recheck/00e4b508-1-over-budget-results.csv` | `3566223d320c8db58e27b43327edeae0291b4a44375d4af0398c793813bc4ffb` |
| `recheck/00e4b508-2-eventual-results.csv` | `982070e8f1b5d971a68e3a0c4f8ddb9e0c026dbf87bfff03a79043f414e044c1` |
| `recheck/04ec7be5-2-eventual-results.csv` | `439ed9789e95ce0d306d42b96ae0bc3fbd83f10bbaf9972e3ed936deeaa8a478` |
| `recheck/b17f6f0c-1-eventual-results.csv` | `9cd6cdd41d047393cb7f6603a0655687591feaddf81410c3f8c6de07d4521fe6` |
| `recheck/b17f6f0c-1-flood-results.csv` | `fa9fd66d49c5f065100d0f73406026a2a36c90bc14d0bd8b63b7059cea51ea2b` |
| `recheck/b17f6f0c-1-over-budget-results.csv` | `61b1796d14c50ef10d115c0b8101a39b54df686da0ce10f7058204764672869d` |
| `recheck/b17f6f0c-2-eventual-results.csv` | `49329d4c394cf2f056f6ee50a26c4a8ae396bac88e37a99262d7a750f40cf149` |
| `recheck/builds.txt` | `5d7187af1039c461a06b844254c6bdf9b46a14a7b87f2670a96a964b246d0078` |
| `recheck/cfe7d248-1-eventual-results.csv` | `587344447aaf4fcfeae6b72eb2a6308c748f1106a7799b9eba1b11d3c32c0673` |
| `recheck/cfe7d248-1-over-budget-results.csv` | `dcbcf4696e6b1057a2c3e33d62efaa53a2c3ba4a5495bdb4006986ff9cdf8bc4` |
| `recheck/cfe7d248-2-eventual-results.csv` | `22245e22483475c7f6f6d282cd628e0fcc93afe8dceb45b6914023bbcd1da7eb` |
| `recheck/compare_sets.py` | `f75854c7609f5831fba1b8b007cc41e8b04f2150972fb5cdff1b8882e2e2ce1b` |
| `recheck/per-run.csv` | `b76c4020628f5fe79ad9056abce9879f7b5e6f61454c946c48b449a1f7f50e40` |
| `recheck/sets.csv` | `a354c7e904d6ae2662a2cbceb08e1e312b39ae87465e05849094d91d7adeb899` |
| `recheck/sim-on-final-source.txt` | `eaaedc479599c01b80678c6aab0260a10cfd434a6e1cc016c36c60711ff8bca4` |
| `recheck/traced-33459ms-b17f6f0c-1-l20-rgg-20-s2.tar.gz` | `7bdb83a437a8b04d34fe49a87240687e7c7ffd5ecfadf2f25226006fd0397c7f` |
| `recheck/traced-34144ms-cfe7d248-2-l20-rgg-20-s1.tar.gz` | `a68f0964d8042e1797d34c7663505a6ebc1c56e8c8162c2049ee5173901ec182` |
| `recheck/traced-47610ms-00e4b508-2-l20-rgg-20-s1.tar.gz` | `7d7abec23e6e17bde1c24eaea7ab04fd5dacdc3b9e895ad5982c1868be1d55d5` |
| `results.csv` | `63791a2f5eee9f740a46812973cc171288e06d31b8cd1f6440ec947fb2e5ba4e` |
| `tables.md` | `b067cf94b36357854f2a554280956bc0e26f9ee706dc053dbb82ea61f72d1616` |
| `timing.csv` | `4950b48abe255af9ec9310a3e6b6efe3a30587c2bbe65679037d2b294dc2ca19` |

- In-process simulation: `mesh_sim` build `9c151bb1`, 210 runs from seeds (`make sim`), with the criteria fixed
  before the runs (`criteria.txt`), the cross-check against the testbed and the figures. `before-fixes/` holds the
  runs that showed honest senders struck for an echo of their own message.
- `recheck/`: repeat sets of the 18 eventual runs at 20 % loss on four `crci-node` builds (`builds.txt`), which put
  the single set in `testbed-fast-recovery-2026-11-04/` in context: its p99 of 3.4 s and maximum of 25.3 s are one
  set, not the build. `sets.csv` and `per-run.csv` come from `compare_sets.py` over the raw run folders, which are
  not kept except the three traced runs (`traced-*.tar.gz`). `sim-on-final-source.txt`: the simulator rebuilt from
  the final source gives the same outcomes on 90 of these runs.

## `archive/`

| File | SHA-256 |
|---|---|
| `byzantine_eviction_tcp-garbage-harness_2026-09-16.csv` | `a1829240b0a2296805733f1e3ff36e1c22a59053fb3b4db853568367485cfcb5` |

- Output of an earlier `byzantine_bench` that sent garbage bytes over loopback TCP and waited on a fixed
  500 ms poll (mean ≈ 514 ms). That harness no longer exists. Moved here from
  `crates/crci-node/benches/results/byzantine_eviction.csv` on 2026-11-01. **Not used by the preprint.**
