# Evidence Index

Raw outputs from real runs, kept so every published number can be traced to a file.
Files are stored verbatim. The only change ever made is UTF-16 → UTF-8 conversion and CRLF → LF
line endings, noted per folder below. SHA-256 hashes are of the files as committed.

Benchmark CSVs that feed the paper live next to their harness in
`crates/crci-node/benches/results/` (see [docs/benchmarks.md](../benchmarks.md)).

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

## `archive/`

| File | SHA-256 |
|---|---|
| `byzantine_eviction_tcp-garbage-harness_2026-09-16.csv` | `a1829240b0a2296805733f1e3ff36e1c22a59053fb3b4db853568367485cfcb5` |

- Output of an earlier `byzantine_bench` that sent garbage bytes over loopback TCP and waited on a fixed
  500 ms poll (mean ≈ 514 ms). That harness no longer exists. Moved here from
  `crates/crci-node/benches/results/byzantine_eviction.csv` on 2026-11-01. **Not used by the preprint.**
