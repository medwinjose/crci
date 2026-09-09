// NOTE: this suite is an in-process simulation, not the real tc/netem WAN chaos suite
// required by locked scope Item 8. Superseded by chaos_wan_real_tests.rs — see CURRENT_STATE.md.

//! 100-node WAN chaos suite (Session 153, Step 8).
//!
//! Extends the existing `chaos.rs` pattern to 100 in-process nodes with
//! simulated WAN conditions: variable latency, packet loss, and a deliberate
//! network partition with heal + convergence measurement.
//!
//! Measures three real metrics:
//!   (a) Sustained message throughput (messages delivered per simulated round)
//!   (b) Partition-recovery time (rounds from heal to full re-convergence)
//!   (c) Per-node memory footprint (heap-estimated bytes per ChaosWanNode)

use std::collections::HashSet;
use std::time::Instant;

// ── Deterministic RNG (same LCG pattern as chaos.rs) ──────────────

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg {
            state: seed.wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (u32::MAX as f32)
    }

    /// Returns a value in [lo, hi) range.
    fn next_range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + (self.next_u64() % (hi - lo))
    }
}

// ── Minimal node for WAN chaos sim ────────────────────────────────

struct ChaosWanNode {
    #[allow(dead_code)]
    id: usize,
    received: HashSet<String>,
    peers: Vec<usize>,
    online: bool,
}

impl ChaosWanNode {
    fn new(id: usize) -> Self {
        ChaosWanNode {
            id,
            received: HashSet::new(),
            peers: Vec::new(),
            online: true,
        }
    }

    /// Estimate heap memory usage in bytes.
    /// HashSet<String>: each entry is ~(key_len + 56) bytes for HashMap overhead.
    /// Vec<usize>: capacity * 8.
    fn estimated_heap_bytes(&self) -> usize {
        let set_overhead = 56; // per-entry HashMap overhead estimate
        let set_bytes: usize = self.received.iter().map(|s| s.len() + set_overhead).sum();
        let peers_bytes = self.peers.capacity() * std::mem::size_of::<usize>();
        let id_bytes = std::mem::size_of::<usize>(); // stack, but for completeness
        set_bytes + peers_bytes + id_bytes
    }
}

// ── Topology: ring with k=3 cross-links for resilience ───────────

fn build_wan_ring(node_count: usize, cross_links: usize) -> Vec<ChaosWanNode> {
    let mut nodes: Vec<ChaosWanNode> = (0..node_count).map(ChaosWanNode::new).collect();
    for i in 0..node_count {
        for offset in 1..=cross_links {
            let peer = (i + offset) % node_count;
            if !nodes[i].peers.contains(&peer) {
                nodes[i].peers.push(peer);
            }
            if !nodes[peer].peers.contains(&i) {
                nodes[peer].peers.push(i);
            }
        }
    }
    nodes
}

// ── Main scenario ─────────────────────────────────────────────────

struct WanChaosResult {
    node_count: usize,
    total_rounds: usize,
    total_deliveries: u64,
    avg_deliveries_per_round: f64,
    partition_heal_round: usize,
    convergence_round: Option<usize>,
    recovery_rounds: Option<usize>,
    min_node_heap_bytes: usize,
    max_node_heap_bytes: usize,
    avg_node_heap_bytes: usize,
    wall_time_ms: u128,
    all_received_msg_a: bool,
    all_received_msg_b: bool,
    partition_isolated: bool,
    packet_loss_rate: f32,
}

fn run_wan_chaos_100(node_count: usize, loss_rate: f32) -> WanChaosResult {
    let t0 = Instant::now();
    let mut rng = Lcg::new(153);
    let mut nodes = build_wan_ring(node_count, 3);

    // Partition parameters
    let partition_start = 20usize;
    let partition_heal = 60usize;
    let total_rounds = 120usize;
    let partition_boundary = node_count / 5; // isolate 20% of nodes (nodes 0..boundary)

    // Originate messages at partition start so they don't pre-propagate across boundary
    let msg_a = "wan-chaos-rescue-A".to_string();
    let msg_b = "wan-chaos-rescue-B".to_string();

    let mut total_deliveries: u64 = 0;
    let mut partition_isolated = true;
    let mut convergence_round: Option<usize> = None;

    // Track WAN latency simulation: we model latency as delayed delivery
    // (messages sent in round N arrive in round N + latency_rounds).
    // latency_rounds drawn from [1, 4] to simulate 50-200ms at 50ms/round tick.
    struct DelayedMsg {
        target: usize,
        source: usize, // track source for partition enforcement on delivery
        msg: String,
        deliver_at: usize,
    }
    let mut delayed_queue: Vec<DelayedMsg> = Vec::new();

    for round in 0..total_rounds {
        let in_partition = round >= partition_start && round < partition_heal;

        // Originate messages exactly when partition starts
        if round == partition_start {
            nodes[0].received.insert(msg_a.clone());
            nodes[partition_boundary + 1].received.insert(msg_b.clone());
        }

        // Deliver delayed messages that are due this round
        let mut due_now: Vec<(usize, usize, String)> = Vec::new();
        delayed_queue.retain(|dm| {
            if dm.deliver_at <= round {
                due_now.push((dm.target, dm.source, dm.msg.clone()));
                false
            } else {
                true
            }
        });
        for (target, source, msg) in due_now {
            // Enforce partition on delivery too: if partition is active,
            // block cross-boundary delivery
            if in_partition {
                let src_in_a = source < partition_boundary;
                let tgt_in_a = target < partition_boundary;
                if src_in_a != tgt_in_a {
                    continue; // partition blocks this delayed delivery
                }
            }
            if nodes[target].online && !nodes[target].received.contains(&msg) {
                nodes[target].received.insert(msg);
                total_deliveries += 1;
            }
        }

        // Gossip round
        let mut new_delayed: Vec<DelayedMsg> = Vec::new();

        for i in 0..node_count {
            if !nodes[i].online {
                continue;
            }
            let peers = nodes[i].peers.clone();
            let msgs: Vec<String> = nodes[i].received.iter().cloned().collect();

            for msg in &msgs {
                for &peer in &peers {
                    if !nodes[peer].online {
                        continue;
                    }

                    // Enforce partition: if in partition window, block cross-boundary gossip
                    if in_partition {
                        let i_in_a = i < partition_boundary;
                        let peer_in_a = peer < partition_boundary;
                        if i_in_a != peer_in_a {
                            continue; // blocked by partition
                        }
                    }

                    if nodes[peer].received.contains(msg) {
                        continue; // already has it
                    }

                    // Packet loss
                    if rng.next_f32() < loss_rate {
                        continue; // dropped
                    }

                    // WAN latency: deliver after 1-4 rounds
                    let latency = rng.next_range(1, 5) as usize;
                    new_delayed.push(DelayedMsg {
                        target: peer,
                        source: i,
                        msg: msg.clone(),
                        deliver_at: round + latency,
                    });
                }
            }
        }

        delayed_queue.extend(new_delayed);

        // Check partition isolation: during partition, msg_a must stay in partition A,
        // msg_b must stay in partition B
        if in_partition {
            let a_has_b = nodes[..partition_boundary]
                .iter()
                .any(|n| n.received.contains(&msg_b));
            let b_has_a = nodes[partition_boundary..]
                .iter()
                .any(|n| n.received.contains(&msg_a));
            if a_has_b || b_has_a {
                partition_isolated = false;
            }
        }

        // Check convergence (post-heal)
        if round >= partition_heal && convergence_round.is_none() {
            let all_have_a = nodes.iter().all(|n| n.received.contains(&msg_a));
            let all_have_b = nodes.iter().all(|n| n.received.contains(&msg_b));
            if all_have_a && all_have_b {
                convergence_round = Some(round);
            }
        }
    }

    // Drain remaining delayed queue
    for dm in &delayed_queue {
        if nodes[dm.target].online && !nodes[dm.target].received.contains(&dm.msg) {
            nodes[dm.target].received.insert(dm.msg.clone());
            total_deliveries += 1;
        }
    }

    let wall_time_ms = t0.elapsed().as_millis();

    // Memory measurement
    let heap_sizes: Vec<usize> = nodes.iter().map(|n| n.estimated_heap_bytes()).collect();
    let min_heap = *heap_sizes.iter().min().unwrap_or(&0);
    let max_heap = *heap_sizes.iter().max().unwrap_or(&0);
    let avg_heap = if heap_sizes.is_empty() {
        0
    } else {
        heap_sizes.iter().sum::<usize>() / heap_sizes.len()
    };

    let all_have_a = nodes.iter().all(|n| n.received.contains(&msg_a));
    let all_have_b = nodes.iter().all(|n| n.received.contains(&msg_b));
    let recovery_rounds = convergence_round.map(|c| c - partition_heal);

    let gossip_rounds = total_rounds;
    let avg_deliveries = if gossip_rounds > 0 {
        total_deliveries as f64 / gossip_rounds as f64
    } else {
        0.0
    };

    WanChaosResult {
        node_count,
        total_rounds,
        total_deliveries,
        avg_deliveries_per_round: avg_deliveries,
        partition_heal_round: partition_heal,
        convergence_round,
        recovery_rounds,
        min_node_heap_bytes: min_heap,
        max_node_heap_bytes: max_heap,
        avg_node_heap_bytes: avg_heap,
        wall_time_ms,
        all_received_msg_a: all_have_a,
        all_received_msg_b: all_have_b,
        partition_isolated,
        packet_loss_rate: loss_rate,
    }
}

fn print_result(r: &WanChaosResult) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║  WAN CHAOS: {}-node, {:.0}% loss, {} rounds{:>width$}║",
        r.node_count,
        r.packet_loss_rate * 100.0,
        r.total_rounds,
        "",
        width = 18
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Throughput: {} total deliveries, {:.1} msgs/round{:>width$}║",
        r.total_deliveries,
        r.avg_deliveries_per_round,
        "",
        width = 15
    );
    println!(
        "║  Partition heal round: {}. Convergence round: {}{:>width$}║",
        r.partition_heal_round,
        r.convergence_round
            .map(|c| c.to_string())
            .unwrap_or_else(|| "DNF".to_string()),
        "",
        width = 20
    );
    println!(
        "║  Recovery time: {} rounds after heal{:>width$}║",
        r.recovery_rounds
            .map(|rr| rr.to_string())
            .unwrap_or_else(|| "DNF".to_string()),
        "",
        width = 25
    );
    println!(
        "║  Memory (heap est): min={} avg={} max={} bytes/node{:>width$}║",
        r.min_node_heap_bytes,
        r.avg_node_heap_bytes,
        r.max_node_heap_bytes,
        "",
        width = 3
    );
    println!(
        "║  Partition isolated: {} | All got msg_a: {} | All got msg_b: {}  ║",
        if r.partition_isolated { "YES" } else { "NO " },
        if r.all_received_msg_a { "YES" } else { "NO " },
        if r.all_received_msg_b { "YES" } else { "NO " },
    );
    println!(
        "║  Wall time: {} ms{:>width$}║",
        r.wall_time_ms,
        "",
        width = 43
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn test_wan_chaos_100_nodes_3pct_loss() {
    let result = run_wan_chaos_100(100, 0.03);
    print_result(&result);

    // Invariant: partition must have been correctly enforced
    assert!(
        result.partition_isolated,
        "Messages must not cross partition boundary during isolation window"
    );

    // Invariant: after heal + convergence, all nodes must have both messages
    assert!(
        result.all_received_msg_a,
        "All 100 nodes must receive msg_a after partition heal"
    );
    assert!(
        result.all_received_msg_b,
        "All 100 nodes must receive msg_b after partition heal"
    );

    // Convergence must have happened within the remaining rounds
    assert!(
        result.convergence_round.is_some(),
        "Network must converge within {} rounds after heal",
        result.total_rounds - result.partition_heal_round
    );

    // Throughput must be positive
    assert!(
        result.total_deliveries > 0,
        "Must have delivered at least some messages"
    );

    // Recovery time must be finite
    assert!(
        result.recovery_rounds.is_some(),
        "Recovery must complete within the simulation window"
    );
}

#[test]
fn test_wan_chaos_100_nodes_5pct_loss() {
    let result = run_wan_chaos_100(100, 0.05);
    print_result(&result);

    assert!(
        result.partition_isolated,
        "Messages must not cross partition boundary"
    );
    assert!(
        result.all_received_msg_a,
        "All nodes must receive msg_a after heal"
    );
    assert!(
        result.all_received_msg_b,
        "All nodes must receive msg_b after heal"
    );
    assert!(
        result.convergence_round.is_some(),
        "Network must converge after heal"
    );
}
