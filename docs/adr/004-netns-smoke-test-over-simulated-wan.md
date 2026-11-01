# ADR 004: Network-Namespace Smoke Test in CI instead of a Simulated 100-Node WAN

- **Date:** 2026-09-08
- **Status:** Accepted (the 5-node limit applies to CI only; the evaluation testbed is Task 12)

## Context

`crates/crci-node/tests/chaos_wan_100_tests.rs` simulates 100 nodes in one process. It never opens a
socket, so it cannot show anything about real network behaviour. Running 100 real network stacks on a
GitHub-hosted runner is not practical because of memory and CPU limits.

## Decision

Add `crates/crci-node/tests/chaos_wan_real_tests.rs`. It starts 5 `crci-node` processes in separate Linux
network namespaces with `tc netem` delay and loss, and runs as an `#[ignore]` test in CI on
`ubuntu-latest`. Keep the 100-node simulation as an in-process test.

## Consequences

- CI shows that real `crci-node` processes start, bind and run for 10 s under netem.
- The test asserts nothing about delivery, isolation or partitions (see
  [benchmarks.md](../benchmarks.md#wan-smoke-test)). It is a smoke test, not an evaluation.
- Scale experiments need a separate 20–50 node testbed.
