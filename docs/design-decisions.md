# Architecture Decision Records (ADRs)

## ADR 001: Transition to Real Network-Namespace WAN Chaos Testing
**Date:** 2026-09-08 (Session 168 / Item 8)

**Decision:** Replace the in-process 100-node simulated WAN chaos test (`chaos_wan_100_tests.rs`) with a 5-node test executing across real Linux network namespaces (`chaos_wan_real_tests.rs`) using `tc` (traffic control) and `netem` (network emulator).

**Alternatives Considered:** 
- Retaining the 100-node simulation test as the primary benchmark.
- Running a large-scale real WAN test on CI.

**Why Rejected:** 
- The in-process simulation (`chaos_wan_100_tests.rs`) bypasses the actual `rust-libp2p` TCP stack, failing to validate real network socket exhaustion or kernel-level packet drops.
- Running 100 real network namespaces on standard CI runners (e.g., GitHub Actions Ubuntu-latest) causes Out-Of-Memory (OOM) kills and CPU starvation, breaking the test artificially due to hardware limits rather than protocol limits.

**Outcome:** The real WAN chaos test is capped at a 5-node topology to ensure stable, deterministic execution in CI, proving real kernel-level TCP fault tolerance. The 100-node simulation remains in the test suite purely as an algorithmic benchmark.

## ADR 002: Cargo Workspace Reorganization
**Date:** 2026-09-12 (Session 173 / Item 12)

**Decision:** Split the monolithic CRCI codebase into a Cargo workspace containing `crci-core` (library logic) and `crci-node` (binary and API).

**Alternatives Considered:** 
- Maintaining the monolithic crate and relying on feature flags to exclude CLI/REST dependencies when compiling for Android.

**Why Rejected:** 
- Feature flag management became excessively complex, risking the inclusion of heavy asynchronous runtimes (like `actix-web`) in the Android `.so` bundle. A workspace creates a hard architectural boundary.

**Outcome:** The `crci-core` crate is now completely decoupled from binary execution, drastically simplifying the UniFFI bindings generation for `crci-android`. Tests and benchmarks were relocated to their respective crates.
