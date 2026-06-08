# Benchmark Results

CRCI's resilience and performance are rigorously tested through automated benchmarking suites. Our methodology emphasizes quantifiable, reproducible metrics so that fault-tolerance claims can be strictly verified.

## Overall Performance

*All benchmarks run on a single machine (simulated). See `docs/BENCHMARKS.md` for full methodology.*

| Metric | Condition | Performance |
|--------|-----------|-------------|
| Message Propagation | 10 / 100 / 1000 nodes | 3 / 8 / 15 rounds to full reach |
| Consensus Convergence | 100 nodes, 33% Byzantine | 8 rounds |
| Replay Filter Throughput | In-memory cache | 2.2M checks/s |
| Pipeline Throughput | End-to-end processing | ~94K msg/s |
| Discovery Convergence | 100 nodes | 10 rounds (<70ms) |

## Byzantine Fault Injection Tolerance

To prove that a CRCI node correctly survives active DoS attacks without sacrificing legitimate peer connections, we built the `benches/byzantine_bench.rs` harness. 

### Methodology
The harness automates a localized integration test consisting of 20 independent trials. In each trial:
1. Two honest nodes (Node A and Node B) bind to dynamically allocated loopback ports (e.g., 19005 and 19006).
2. Node B dials Node A to establish a legitimate handshake. The time for `peer_count` to reach `1` is recorded (`legitimate_handshake_ms`).
3. A "Byzantine" adversary node connects to Node A via a raw `TcpStream` and immediately blasts unparseable garbage bytes (`BYZANTINE_GARBAGE\n`) over the socket.
4. The test measures the time until the system stabilizes, ensuring the bad connection is dropped and the honest `peer_count` remains securely at `1` (`byzantine_eviction_ms` and `legitimate_peer_survived`).

### Summary Metrics

Byzantine fault injection across 20 trials on loopback (single machine, no network latency):

| Metric | Value |
|---|---|
| Trials | 20 |
| Legitimate handshake (mean) | 15ms |
| Byzantine eviction/ignore (mean) | 510ms |
| Byzantine eviction/ignore (p95) | 512ms |
| Legitimate peer survived | 20/20 |

> Results generated automatically via `cargo test --test byzantine_bench -- --nocapture`.

### Raw Data

The raw timings exported from the most recent run are logged below. Note that the eviction timeout includes a fixed `500ms` internal async poll threshold to guarantee complete peer table stability.

```csv
trial,legitimate_handshake_ms,byzantine_eviction_ms,legitimate_peer_survived
1,16,510,true
2,14,511,true
3,14,512,true
4,14,510,true
5,14,511,true
6,14,506,true
7,14,510,true
8,14,511,true
9,14,511,true
10,30,508,true
11,14,506,true
12,14,512,true
13,14,511,true
14,14,505,true
15,13,507,true
16,14,508,true
17,14,512,true
18,14,510,true
19,14,512,true
20,14,529,true
```
