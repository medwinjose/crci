# CRCI Benchmark Reference

This document contains only real, verified benchmarks that are continuously tested against the current codebase.

## 1. Byzantine Eviction Latency (Baseline)
Tested in `crates/crci-node/benches/byzantine_bench.rs`.
Raw results available at: `crates/crci-node/benches/results/byzantine_eviction.csv`

| Metric | Value |
|--------|-------|
| Trials | 50 |
| Byzantine Nodes | 1 |
| Honest Nodes | 1 |
| Max Eviction Latency | 527 ms |
| Honest Peer Survivability | 50 / 50 trials (100%) |

*Interpretation: Under direct adversarial connection without artificial network jitter, the CRCI reputation engine detects and evicts a Byzantine peer in approximately half a second, ensuring the honest peer's connection survives without being partitioned.*

## 2. Byzantine Eviction Latency (With Network Jitter)
Tested in `crates/crci-node/benches/byzantine_bench.rs`.
Raw results available at: `crates/crci-node/benches/results/byzantine_eviction_jitter.csv`

| Metric | Value |
|--------|-------|
| Trials | 50 |
| Byzantine Nodes | 1 |
| Honest Nodes | 1 |
| Max Eviction Latency | 850 ms |
| Honest Peer Survivability | 50 / 50 trials (100%) |

*Interpretation: When introducing simulated packet delay/jitter, the protocol remains stable. The worst-case eviction latency extends to 850ms, but the honest peer survives 100% of the attacks.*
