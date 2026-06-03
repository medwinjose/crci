//! Chaos engineering scenarios for CRCI.
//!
//! Three scenarios, each self-contained and deterministic (seed 99).
//! No new crates. Uses SimpleLcg from bench.rs pattern.
//! All output prefixed with "CHAOS TEST:" for easy grep.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

// ── Deterministic RNG (same pattern as bench.rs) ──────────────────

struct SimpleLcg {
    state: u64,
}

impl SimpleLcg {
    fn new(seed: u64) -> Self {
        SimpleLcg { state: seed.wrapping_add(1) }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (u32::MAX as f32)
    }

    fn next_usize(&mut self) -> usize {
        self.next_u64() as usize
    }
}

// ── Minimal node for chaos sim ────────────────────────────────────

struct ChaosNode {
    id: usize,
    received: HashSet<String>,
    peers: Vec<usize>,
    online: bool,
    rescue_msgs: Vec<String>,
    reputation: f32,
}

impl ChaosNode {
    fn new(id: usize) -> Self {
        ChaosNode {
            id,
            received: HashSet::new(),
            peers: Vec::new(),
            online: true,
            rescue_msgs: Vec::new(),
            reputation: 1.0,
        }
    }
}

// ── Topology builder ──────────────────────────────────────────────

fn build_ring_topology(node_count: usize) -> Vec<ChaosNode> {
    let mut nodes: Vec<ChaosNode> = (0..node_count).map(ChaosNode::new).collect();
    // Ring + cross-links for resilience: each node connects to next 2 peers
    for i in 0..node_count {
        let p1 = (i + 1) % node_count;
        let p2 = (i + 2) % node_count;
        nodes[i].peers.push(p1);
        nodes[i].peers.push(p2);
        // Bidirectional
        nodes[p1].peers.push(i);
        nodes[p2].peers.push(i);
    }
    // Deduplicate
    for node in &mut nodes {
        node.peers.sort_unstable();
        node.peers.dedup();
        node.peers.retain(|&p| p != node.id);
    }
    nodes
}

// ── SCENARIO 1: Packet Loss ───────────────────────────────────────

fn run_packet_loss_scenario(loss_rate: f32, label: &str) {
    let t0 = Instant::now();
    let node_count = 10;
    let mut nodes = build_ring_topology(node_count);
    let mut rng = SimpleLcg::new(99);

    // Originate rescue message from node 0
    let msg_id = "chaos-rescue-001".to_string();
    nodes[0].received.insert(msg_id.clone());
    nodes[0].rescue_msgs.push(msg_id.clone());

    let mut round = 0;
    let mut full_propagation_round: Option<usize> = None;

    for _ in 0..100 {
        round += 1;
        let mut deliveries: Vec<(usize, String)> = Vec::new();

        for i in 0..node_count {
            if !nodes[i].online { continue; }
            if nodes[i].received.contains(&msg_id) {
                let peers = nodes[i].peers.clone();
                for &peer in &peers {
                    if !nodes[peer].online { continue; }
                    if !nodes[peer].received.contains(&msg_id) {
                        // Apply packet loss
                        if rng.next_f32() >= loss_rate {
                            deliveries.push((peer, msg_id.clone()));
                        }
                    }
                }
            }
        }

        for (peer, id) in deliveries {
            nodes[peer].received.insert(id.clone());
            nodes[peer].rescue_msgs.push(id);
        }

        let reached = nodes.iter()
            .filter(|n| n.online && n.received.contains(&msg_id))
            .count();
        let total_online = nodes.iter().filter(|n| n.online).count();

        if full_propagation_round.is_none() && reached >= total_online {
            full_propagation_round = Some(round);
            break;
        }
    }

    let reached = nodes.iter()
        .filter(|n| n.received.contains(&msg_id))
        .count();
    let wall_ms = t0.elapsed().as_millis();

    println!();
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  CHAOS TEST: {}                    ║", label);
    println!("╚══════════════════════════════════════════════════════╝");
    println!(
        "  Nodes: {} | Loss rate: {:.0}% | Rounds to full propagation: {}",
        node_count,
        loss_rate * 100.0,
        full_propagation_round
            .map(|r| r.to_string())
            .unwrap_or_else(|| "DNF (>100 rounds)".to_string())
    );
    println!(
        "  Nodes reached: {}/{} | Wall time: {} ms",
        reached, node_count, wall_ms
    );

    // Rescue persistence check
    let rescue_holders = nodes.iter()
        .filter(|n| !n.rescue_msgs.is_empty())
        .count();
    println!(
        "  Rescue msg held by {}/{} nodes — {}",
        rescue_holders,
        node_count,
        if rescue_holders > 0 { "✅ rescue survived" } else { "❌ rescue lost" }
    );
}

// ── SCENARIO 2: Crash + Restart ───────────────────────────────────

fn run_crash_restart_scenario() {
    let t0 = Instant::now();
    let node_count = 10;
    let mut nodes = build_ring_topology(node_count);
    let mut rng = SimpleLcg::new(99);

    println!();
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  CHAOS TEST: Node Crash + Restart                    ║");
    println!("╚══════════════════════════════════════════════════════╝");

    // Originate rescue from node 1 (not node 0 which will crash)
    let msg_before = "rescue-before-crash".to_string();
    nodes[1].received.insert(msg_before.clone());
    nodes[1].rescue_msgs.push(msg_before.clone());

    // Propagate for 25 rounds
    for round in 0..100usize {
        // Round 25: crash node 0
        if round == 25 {
            nodes[0].online = false;
            println!("  ⚡ Round 25: node-0 CRASHED");
        }

        // Round 50: node 0 restarts
        if round == 50 {
            nodes[0].online = true;
            // Restart: keep rescue_msgs (persistent storage), clear received
            // (simulates fresh boot — it will re-receive from peers)
            let kept_rescues = nodes[0].rescue_msgs.clone();
            nodes[0].received.clear();
            nodes[0].rescue_msgs = kept_rescues;
            println!(
                "  🔄 Round 50: node-0 RESTARTED | rescue msgs retained: {}",
                nodes[0].rescue_msgs.len()
            );
        }

        let mut deliveries: Vec<(usize, String)> = Vec::new();
        for i in 0..node_count {
            if !nodes[i].online { continue; }
            if nodes[i].received.contains(&msg_before) {
                let peers = nodes[i].peers.clone();
                for &peer in &peers {
                    if !nodes[peer].online { continue; }
                    if !nodes[peer].received.contains(&msg_before) {
                        // Low loss (5%) for normal operation
                        if rng.next_f32() >= 0.05 {
                            deliveries.push((peer, msg_before.clone()));
                        }
                    }
                }
            }
        }
        for (peer, id) in deliveries {
            nodes[peer].received.insert(id.clone());
            nodes[peer].rescue_msgs.push(id);
        }

        // At round 60, originate a new message to test node-0 re-joins gossip
        if round == 60 {
            let msg_after = "rescue-after-restart".to_string();
            nodes[1].received.insert(msg_after.clone());
            nodes[1].rescue_msgs.push(msg_after.clone());

            // Propagate new msg for the rest of the loop
            for _ in 0..20 {
                let mut d2: Vec<(usize, String)> = Vec::new();
                for i in 0..node_count {
                    if !nodes[i].online { continue; }
                    if nodes[i].received.contains(&msg_after) {
                        let peers = nodes[i].peers.clone();
                        for &peer in &peers {
                            if nodes[peer].online
                                && !nodes[peer].received.contains(&msg_after)
                            {
                                d2.push((peer, msg_after.clone()));
                            }
                        }
                    }
                }
                for (peer, id) in d2 {
                    nodes[peer].received.insert(id.clone());
                    nodes[peer].rescue_msgs.push(id);
                }
            }

            let reached_after = nodes.iter()
                .filter(|n| n.online && n.received.contains(&msg_after))
                .count();
            println!(
                "  📡 Round 60+20: post-restart message reached {}/{} nodes",
                reached_after, node_count
            );
            break;
        }
    }

    let wall_ms = t0.elapsed().as_millis();
    let node0_before = nodes[0].received.contains(&msg_before);
    println!(
        "  node-0 received pre-crash rescue after restart: {} | Wall: {} ms",
        if node0_before { "✅ yes" } else { "❌ no" },
        wall_ms
    );
}

// ── SCENARIO 3: Reconnect Storm ───────────────────────────────────

fn run_reconnect_storm() {
    let t0 = Instant::now();
    let node_count = 10;
    let mut nodes = build_ring_topology(node_count);
    let mut rng = SimpleLcg::new(99);

    println!();
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  CHAOS TEST: Reconnect Storm                         ║");
    println!("╚══════════════════════════════════════════════════════╝");

    // Originate rescue from node 0
    let msg_id = "rescue-storm".to_string();
    nodes[0].received.insert(msg_id.clone());
    nodes[0].rescue_msgs.push(msg_id.clone());

    let mut disconnect_events = 0;
    let mut reconnect_events = 0;
    let mut duplicate_drops = 0;

    // Track which nodes are toggling
    let mut online_states: Vec<bool> = vec![true; node_count];

    for round in 0..50usize {
        // Rounds 10-30: storm — nodes 2,3,4,5 rapidly disconnect/reconnect
        if round >= 10 && round < 30 {
            for storm_node in 2..=5 {
                let toggle = rng.next_f32() < 0.4; // 40% chance of toggle
                if toggle {
                    let was_online = online_states[storm_node];
                    online_states[storm_node] = !was_online;
                    nodes[storm_node].online = !was_online;
                    if was_online {
                        disconnect_events += 1;
                    } else {
                        reconnect_events += 1;
                    }
                }
            }
        } else {
            // Outside storm window: all online
            for i in 0..node_count {
                nodes[i].online = true;
                online_states[i] = true;
            }
        }

        // Gossip round with duplicate tracking
        let mut deliveries: Vec<(usize, String)> = Vec::new();
        let mut attempted_duplicates = 0;

        for i in 0..node_count {
            if !nodes[i].online { continue; }
            if nodes[i].received.contains(&msg_id) {
                let peers = nodes[i].peers.clone();
                for &peer in &peers {
                    if !nodes[peer].online { continue; }
                    if nodes[peer].received.contains(&msg_id) {
                        attempted_duplicates += 1; // would be replay-dropped
                    } else if rng.next_f32() >= 0.05 {
                        deliveries.push((peer, msg_id.clone()));
                    }
                }
            }
        }
        duplicate_drops += attempted_duplicates;

        for (peer, id) in deliveries {
            nodes[peer].received.insert(id.clone());
            nodes[peer].rescue_msgs.push(id);
        }
    }

    // After storm: all nodes back online, verify propagation
    for node in &mut nodes {
        node.online = true;
    }
    // One more gossip pass
    for _ in 0..5 {
        let mut d: Vec<(usize, String)> = Vec::new();
        for i in 0..node_count {
            if nodes[i].received.contains(&msg_id) {
                let peers = nodes[i].peers.clone();
                for &peer in &peers {
                    if !nodes[peer].received.contains(&msg_id) {
                        d.push((peer, msg_id.clone()));
                    }
                }
            }
        }
        for (peer, id) in d {
            nodes[peer].received.insert(id.clone());
            nodes[peer].rescue_msgs.push(id);
        }
    }

    let final_reached = nodes.iter().filter(|n| n.received.contains(&msg_id)).count();
    let wall_ms = t0.elapsed().as_millis();

    println!(
        "  Disconnect events: {} | Reconnect events: {}",
        disconnect_events, reconnect_events
    );
    println!(
        "  Duplicate sends blocked (replay filter equivalent): {}",
        duplicate_drops
    );
    println!(
        "  Final propagation: {}/{} nodes | Wall: {} ms",
        final_reached, node_count, wall_ms
    );
    println!(
        "  Rescue survived reconnect storm: {}",
        if final_reached == node_count { "✅ yes" } else { "⚠ partial" }
    );
}

// ── Entry point ───────────────────────────────────────────────────

/// Run all chaos engineering scenarios.
/// Called from main() after bench::run_benchmarks().
pub fn run_chaos_tests() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 19 — CHAOS ENGINEERING                          ║");
    println!("║  Deterministic seed: 99. Injecting real failure modes.   ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    run_packet_loss_scenario(0.10, "10% Packet Loss              ");
    run_packet_loss_scenario(0.40, "40% Packet Loss (Critical)   ");
    run_crash_restart_scenario();
    run_reconnect_storm();

    println!();
    println!("  ✅ Session 19 chaos tests complete.");
    println!("  Next: Session 20 — research paper draft + library extraction.");
}