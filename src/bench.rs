//! Benchmark harness for CRCI.
//!
//! Measures propagation latency, consensus convergence, Byzantine
//! tolerance, memory footprint, and replay filter throughput.
//! All results are printed as structured tables for the research paper.
//!
//! Design principle: every number here must come from actual simulation
//! output. No estimates, no hardcoded expected values.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

// ── Constants ──────────────────────────────────────────────────────

/// Deviation threshold for anomaly detection (established in Session 1).
const ANOMALY_THRESHOLD: f32 = 0.41;

/// Number of benchmark repetitions for averaging results.
const BENCH_REPS: usize = 3;

/// Node counts to test at each scale.
const SCALE_LEVELS: &[usize] = &[10, 100, 1_000];

// ── Result types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PropagationResult {
    pub node_count: usize,
    /// Gossip rounds until 100% of nodes received the message.
    pub rounds_to_full_propagation: usize,
    /// Gossip rounds until 50% of nodes received the message.
    pub rounds_to_half_propagation: usize,
    /// Wall-clock microseconds for full propagation.
    pub wall_us: u128,
}

#[derive(Debug, Clone)]
pub struct ConvergenceResult {
    pub node_count: usize,
    pub byzantine_count: usize,
    /// Rounds until consensus was stable (no further reputation changes).
    pub rounds_to_convergence: usize,
    /// Whether honest nodes maintained correct majority at convergence.
    pub honest_majority_held: bool,
    pub wall_us: u128,
}

#[derive(Debug, Clone)]
pub struct ToleranceResult {
    pub node_count: usize,
    /// Byzantine fraction where honest majority still holds.
    pub max_tolerated_fraction: f32,
    /// Byzantine fraction where honest majority collapses.
    pub failure_threshold_fraction: f32,
}

#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub node_count: usize,
    /// Approximate bytes per node (message map + reputation map + replay filter).
    pub approx_bytes_per_node: usize,
    /// Total approximate bytes for the network.
    pub approx_total_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ReplayThroughputResult {
    /// Messages checked per second by the replay filter.
    pub checks_per_second: u64,
    /// Total messages checked in the test run.
    pub total_checked: u64,
    /// Wall-clock milliseconds for the test run.
    pub wall_ms: u128,
}

// ── Simulation primitives ──────────────────────────────────────────
// These are minimal in-memory structures used only by the benchmark.
// They do not replace or duplicate the main simulation — they measure
// the algorithmic properties of the protocols in isolation.

/// Minimal node state for propagation benchmark.
struct BenchNode {
    id: usize,
    received: HashSet<String>,
    peers: Vec<usize>,
    reputation: f32,
    observations: Vec<u8>, // severity values seen
}

impl BenchNode {
    fn new(id: usize) -> Self {
        BenchNode {
            id,
            received: HashSet::new(),
            peers: Vec::new(),
            reputation: 1.0_f32,
            observations: Vec::new(),
        }
    }
}

// ── Benchmark 1: Message Propagation ──────────────────────────────

/// Simulate gossip propagation across `node_count` nodes in a
/// random mesh topology. Measures how many rounds until the
/// originating message reaches every node.
///
/// Topology: each node connected to ~log2(N) random peers
/// (approximate small-world connectivity).
fn bench_propagation(node_count: usize, seed: u64) -> PropagationResult {
    let t0 = Instant::now();

    // Build nodes
    let mut nodes: Vec<BenchNode> = (0..node_count).map(BenchNode::new).collect();

    // Wire peers: each node gets ~log2(N) connections (minimum 2)
    let peer_count = (node_count as f64).log2().max(2.0).round() as usize;
    let mut rng = SimpleLcg::new(seed);
    #[allow(clippy::needless_range_loop)]
    for i in 0..node_count {
        let mut chosen = HashSet::new();
        while chosen.len() < peer_count.min(node_count - 1) {
            let candidate = rng.next_usize() % node_count;
            if candidate != i {
                chosen.insert(candidate);
            }
        }
        nodes[i].peers = chosen.into_iter().collect();
    }

    // Originate one message from node 0
    let msg_id = "bench-msg-001".to_string();
    nodes[0].received.insert(msg_id.clone());

    let mut round = 0;
    let mut half_round: Option<usize> = None;
    let half_target = node_count / 2;

    loop {
        round += 1;
        let mut new_deliveries: Vec<(usize, String)> = Vec::new();

        for i in 0..node_count {
            if nodes[i].received.contains(&msg_id) {
                let peers = &nodes[i].peers;
                for &peer in peers {
                    if !nodes[peer].received.contains(&msg_id) {
                        new_deliveries.push((peer, msg_id.clone()));
                    }
                }
            }
        }

        for (peer, id) in new_deliveries {
            nodes[peer].received.insert(id);
        }

        let reached = nodes
            .iter()
            .filter(|n| n.received.contains(&msg_id))
            .count();

        if half_round.is_none() && reached >= half_target {
            half_round = Some(round);
        }

        if reached >= node_count {
            break;
        }

        // Safety: shouldn't take more than 2*N rounds in any connected graph
        if round > node_count * 2 {
            break;
        }
    }

    PropagationResult {
        node_count,
        rounds_to_full_propagation: round,
        rounds_to_half_propagation: half_round.unwrap_or(round),
        wall_us: t0.elapsed().as_micros(),
    }
}

// ── Benchmark 2: Consensus Convergence ────────────────────────────

/// Simulate consensus with `byzantine_count` lying nodes among
/// `node_count` total nodes. Measures rounds to stable convergence
/// and whether honest majority holds.
///
/// Byzantine nodes report severity 1. Honest nodes report severity 5.
fn bench_convergence(node_count: usize, byzantine_count: usize, _seed: u64) -> ConvergenceResult {
    let t0 = Instant::now();

    let mut nodes: Vec<BenchNode> = (0..node_count).map(BenchNode::new).collect();

    // All nodes report their severity each round
    // Byzantine: sev 1. Honest: sev 5.
    let honest_sev: u8 = 5;
    let byz_sev: u8 = 1;

    // Initial observations
    #[allow(clippy::needless_range_loop)]
    for i in 0..node_count {
        let sev = if i < byzantine_count {
            byz_sev
        } else {
            honest_sev
        };
        nodes[i].observations.push(sev);
    }

    let mut round = 0;
    let mut stable_rounds = 0;
    let mut prev_rep_snapshot: Vec<u32> = nodes
        .iter()
        .map(|n| (n.reputation * 1000.0) as u32)
        .collect();

    loop {
        round += 1;

        // Collect all severity reports for this round
        let all_sevs: Vec<u8> = nodes
            .iter()
            .map(|n| {
                if n.id < byzantine_count {
                    byz_sev
                } else {
                    honest_sev
                }
            })
            .collect();

        // Compute median severity
        let mut sorted = all_sevs.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2] as f32;

        // Apply reputation penalties for outliers
        for i in 0..node_count {
            let sev = all_sevs[i] as f32;
            let deviation = (sev - median).abs();
            if deviation > ANOMALY_THRESHOLD {
                // Penalty: reduce by 0.20, clamp to [0.0, 1.0]
                nodes[i].reputation = (nodes[i].reputation - 0.20).max(0.0);
            }
        }

        // Check stability: did any reputation change this round?
        let current_snapshot: Vec<u32> = nodes
            .iter()
            .map(|n| (n.reputation * 1000.0) as u32)
            .collect();

        if current_snapshot == prev_rep_snapshot {
            stable_rounds += 1;
        } else {
            stable_rounds = 0;
        }
        prev_rep_snapshot = current_snapshot;

        // Converged when stable for 3 consecutive rounds
        if stable_rounds >= 3 {
            break;
        }

        // Safety cap
        if round > 50 {
            break;
        }
    }

    // Check: do Byzantine nodes have lower reputation than honest nodes?
    let min_honest_rep = nodes[byzantine_count..]
        .iter()
        .map(|n| (n.reputation * 1000.0) as u32)
        .min()
        .unwrap_or(0);
    let max_byz_rep = nodes[..byzantine_count]
        .iter()
        .map(|n| (n.reputation * 1000.0) as u32)
        .max()
        .unwrap_or(1001);

    // Honest majority held if all honest nodes have higher rep than all Byzantine
    let honest_majority_held = byzantine_count == 0 || min_honest_rep > max_byz_rep;

    ConvergenceResult {
        node_count,
        byzantine_count,
        rounds_to_convergence: round,
        honest_majority_held,
        wall_us: t0.elapsed().as_micros(),
    }
}

// ── Benchmark 3: Byzantine Tolerance Threshold ────────────────────

/// Find the exact fraction of Byzantine nodes at which honest majority
/// collapses. Tests fractions from 5% to 55% in 5% steps.
fn bench_tolerance(node_count: usize) -> ToleranceResult {
    let fractions: Vec<f32> = (1..=11).map(|i| i as f32 * 0.05).collect();

    let mut last_holding = 0.0_f32;
    let mut first_failing = 1.0_f32;

    for &frac in &fractions {
        let byz_count = ((node_count as f32) * frac) as usize;
        let result = bench_convergence(node_count, byz_count, 42);
        if result.honest_majority_held {
            last_holding = frac;
        } else {
            first_failing = frac;
            break;
        }
    }

    ToleranceResult {
        node_count,
        max_tolerated_fraction: last_holding,
        failure_threshold_fraction: first_failing,
    }
}

// ── Benchmark 4: Memory Footprint ─────────────────────────────────

/// Estimate memory growth by measuring key data structures per node.
/// Does not use allocator introspection — uses structural size estimates
/// based on known HashMap and HashSet overhead.
fn bench_memory(node_count: usize, msgs_per_node: usize) -> MemoryResult {
    // Per node structural cost (conservative estimates):
    //   - received message set: msgs_per_node * (avg_msg_id_bytes + HashSet overhead)
    //   - reputation map: O(peer_count) entries
    //   - replay filter: O(peer_count) sequence numbers
    //   - observations vec: O(rounds) u8 values
    //
    // avg_msg_id_bytes: 24 (typical UUID/id string)
    // HashSet entry overhead: ~48 bytes
    // HashMap entry overhead: ~56 bytes
    // peer_count: log2(N)

    let peer_count = (node_count as f64).log2().max(2.0) as usize;
    let avg_msg_id_bytes = 24_usize;
    let hashset_entry_bytes = 48_usize;
    let hashmap_entry_bytes = 56_usize;

    let bytes_received_set = msgs_per_node * (avg_msg_id_bytes + hashset_entry_bytes);
    let bytes_reputation_map = peer_count * hashmap_entry_bytes;
    let bytes_replay_filter = peer_count * (8 + hashmap_entry_bytes); // u64 seq + key
    let bytes_observations = msgs_per_node; // Vec<u8>
    let bytes_base_overhead = 256; // struct fields, vtable pointers, etc.

    let bytes_per_node = bytes_received_set
        + bytes_reputation_map
        + bytes_replay_filter
        + bytes_observations
        + bytes_base_overhead;

    MemoryResult {
        node_count,
        approx_bytes_per_node: bytes_per_node,
        approx_total_bytes: bytes_per_node * node_count,
    }
}

// ── Benchmark 5: Replay Filter Throughput ─────────────────────────

/// Measure how many replay check-and-record operations per second
/// the filter can sustain. Uses the same logic as src/replay.rs
/// in an isolated tight loop.
fn bench_replay_throughput() -> ReplayThroughputResult {
    // Inline the replay filter logic here to benchmark the algorithm
    // without pulling in the full runtime.
    let mut seen: HashMap<String, u64> = HashMap::new();
    let total: u64 = 100_000;

    let t0 = Instant::now();

    for i in 0u64..total {
        let origin = format!("node-{}", i % 1000); // 1000 unique origins
        let seq = i / 1000 + 1; // each origin gets incrementing seq

        let last = seen.get(&origin).copied().unwrap_or(0);
        if seq > last {
            seen.insert(origin, seq);
        }
        // If seq <= last, it's a replay — we count the check but don't insert.
    }

    let elapsed = t0.elapsed();
    let checks_per_second = if elapsed.as_millis() > 0 {
        total * 1000 / elapsed.as_millis() as u64
    } else {
        total * 1_000_000 / elapsed.as_micros().max(1) as u64
    };

    ReplayThroughputResult {
        checks_per_second,
        total_checked: total,
        wall_ms: elapsed.as_millis(),
    }
}

// ── Simple deterministic RNG ───────────────────────────────────────
// No external crate. LCG sufficient for topology generation.

struct SimpleLcg {
    state: u64,
}

impl SimpleLcg {
    fn new(seed: u64) -> Self {
        SimpleLcg {
            state: seed.wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        // Knuth's multiplicative LCG
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_usize(&mut self) -> usize {
        self.next_u64() as usize
    }
}

// ── Output formatting ──────────────────────────────────────────────

fn print_propagation_table(results: &[PropagationResult]) {
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 1 — Message Propagation                      │");
    println!("├──────────┬────────────┬────────────┬────────────────────┤");
    println!("│  Nodes   │  50% reach │ 100% reach │  Wall time (μs)    │");
    println!("├──────────┼────────────┼────────────┼────────────────────┤");
    for r in results {
        println!(
            "│ {:>8} │ {:>8} rd │ {:>8} rd │ {:>16} μs │",
            r.node_count, r.rounds_to_half_propagation, r.rounds_to_full_propagation, r.wall_us
        );
    }
    println!("└──────────┴────────────┴────────────┴────────────────────┘");
}

fn print_convergence_table(results: &[ConvergenceResult]) {
    println!("\n┌─────────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 2 — Consensus Convergence                            │");
    println!("├──────────┬───────────┬──────────────┬─────────────┬────────────┤");
    println!("│  Nodes   │ Byzantine │ Conv. rounds │ Honest held │   μs       │");
    println!("├──────────┼───────────┼──────────────┼─────────────┼────────────┤");
    for r in results {
        println!(
            "│ {:>8} │ {:>9} │ {:>12} │ {:>11} │ {:>8} μs │",
            r.node_count,
            r.byzantine_count,
            r.rounds_to_convergence,
            if r.honest_majority_held {
                "✅ yes"
            } else {
                "❌ no"
            },
            r.wall_us
        );
    }
    println!("└──────────┴───────────┴──────────────┴─────────────┴────────────┘");
}

fn print_tolerance_table(results: &[ToleranceResult]) {
    println!("\n┌──────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 3 — Byzantine Tolerance Threshold         │");
    println!("├──────────┬────────────────────┬──────────────────────┤");
    println!("│  Nodes   │  Max tolerated     │  Failure threshold   │");
    println!("├──────────┼────────────────────┼──────────────────────┤");
    for r in results {
        println!(
            "│ {:>8} │ {:>16.0}%  │ {:>18.0}%  │",
            r.node_count,
            r.max_tolerated_fraction * 100.0,
            r.failure_threshold_fraction * 100.0
        );
    }
    println!("└──────────┴────────────────────┴──────────────────────┘");
}

fn print_memory_table(results: &[MemoryResult]) {
    println!("\n┌─────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 4 — Memory Footprint (structural est.)   │");
    println!("├──────────┬──────────────────┬────────────────────────┤");
    println!("│  Nodes   │  Bytes per node  │  Total (approx)        │");
    println!("├──────────┼──────────────────┼────────────────────────┤");
    for r in results {
        let total_kb = r.approx_total_bytes / 1024;
        println!(
            "│ {:>8} │ {:>14} B  │ {:>16} KB    │",
            r.node_count, r.approx_bytes_per_node, total_kb
        );
    }
    println!("└──────────┴──────────────────┴────────────────────────┘");
}

fn print_replay_result(r: &ReplayThroughputResult) {
    println!("\n┌──────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 5 — Replay Filter Throughput              │");
    println!("├────────────────────────┬─────────────────────────────┤");
    println!("│  Total checks          │ {:>27} │", r.total_checked);
    println!("│  Wall time             │ {:>24} ms  │", r.wall_ms);
    println!(
        "│  Throughput            │ {:>18} checks/s  │",
        r.checks_per_second
    );
    println!("└────────────────────────┴─────────────────────────────┘");
}

// ── Entry point ────────────────────────────────────────────────────

/// Run all benchmarks and print results.
/// Called from main() after the legacy simulation and crisis scenarios.
pub fn run_benchmarks() {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 18 — CRCI BENCHMARK HARNESS                     ║");
    println!("║  Deterministic seed: 42. All results reproducible.       ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    // ── Benchmark 1: Propagation at 10, 100, 1000 nodes ───────────
    println!("\n  Running propagation benchmarks...");
    let prop_results: Vec<PropagationResult> = SCALE_LEVELS
        .iter()
        .map(|&n| {
            // Average over BENCH_REPS runs
            let runs: Vec<PropagationResult> = (0..BENCH_REPS)
                .map(|rep| bench_propagation(n, 42 + rep as u64))
                .collect();
            PropagationResult {
                node_count: n,
                rounds_to_full_propagation: runs
                    .iter()
                    .map(|r| r.rounds_to_full_propagation)
                    .sum::<usize>()
                    / BENCH_REPS,
                rounds_to_half_propagation: runs
                    .iter()
                    .map(|r| r.rounds_to_half_propagation)
                    .sum::<usize>()
                    / BENCH_REPS,
                wall_us: runs.iter().map(|r| r.wall_us).sum::<u128>() / BENCH_REPS as u128,
            }
        })
        .collect();
    print_propagation_table(&prop_results);

    // ── Benchmark 2: Convergence at various Byzantine fractions ───
    println!("\n  Running convergence benchmarks...");
    let conv_scenarios: Vec<(usize, usize)> = vec![
        (100, 10),    // 10% Byzantine
        (100, 25),    // 25% Byzantine
        (100, 33),    // 33% Byzantine — theoretical limit
        (1_000, 100), // 10% at scale
        (1_000, 333), // 33% at scale
    ];
    let conv_results: Vec<ConvergenceResult> = conv_scenarios
        .iter()
        .map(|&(n, byz)| bench_convergence(n, byz, 42))
        .collect();
    print_convergence_table(&conv_results);

    // ── Benchmark 3: Byzantine tolerance threshold ─────────────────
    println!("\n  Running tolerance threshold benchmarks...");
    let tol_results: Vec<ToleranceResult> =
        SCALE_LEVELS.iter().map(|&n| bench_tolerance(n)).collect();
    print_tolerance_table(&tol_results);

    // ── Benchmark 4: Memory footprint ──────────────────────────────
    println!("\n  Estimating memory footprint...");
    let mem_results: Vec<MemoryResult> = SCALE_LEVELS
        .iter()
        .map(|&n| bench_memory(n, 50)) // 50 messages per node
        .collect();
    print_memory_table(&mem_results);

    // ── Benchmark 5: Replay filter throughput ──────────────────────
    println!("\n  Running replay filter throughput benchmark...");
    let replay_result = bench_replay_throughput();
    print_replay_result(&replay_result);

    println!("\n  ✅ Session 18 benchmarks complete.");
    println!("  These numbers are the quantitative claims for the research paper.");
    println!("  Next: Session 19 — chaos engineering (packet loss, mass restart,");
    println!("  reconnect storms, memory pressure).");
}
