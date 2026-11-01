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
- `recovery_paper_v2_run2_0255.txt` is the source of the preprint's paced-attack result
  (Rapid Attack: banned; Paced Attack with 300 recovery packets: not banned).
  `recovery_paper_v2_run1_0243.txt` is an earlier run from a different build that reports different
  results (both conditions survive). It is kept because it was observed, not because it is correct.

## `archive/`

| File | SHA-256 |
|---|---|
| `byzantine_eviction_tcp-garbage-harness_2026-09-16.csv` | `a1829240b0a2296805733f1e3ff36e1c22a59053fb3b4db853568367485cfcb5` |

- Output of an earlier `byzantine_bench` that sent garbage bytes over loopback TCP and waited on a fixed
  500 ms poll (mean ≈ 514 ms). That harness no longer exists. Moved here from
  `crates/crci-node/benches/results/byzantine_eviction.csv` on 2026-11-01. **Not used by the preprint.**
