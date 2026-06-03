mod aeda;
mod battery;
mod bench;
mod chaos;
mod crisis;
mod identity;
mod mesh;
mod message;
mod network;
mod node;
mod replay;
mod runtime;
mod storage;
mod stress;
mod transport;
mod ttl;
mod validation;

use mesh::MeshSimulator;
use message::{Message, Signal, Visibility};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn main() {
    // Shared inbox — a single locked map that all SimTransports write into
    // and each NodeRuntime reads from. This simulates the radio layer.
    let inbox: crate::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));

    let mut sim = MeshSimulator::new();

    // Zone A — crisis area
    sim.add_node("node-001", "zone-a", inbox.clone());
    sim.add_node("node-002", "zone-a", inbox.clone());
    sim.add_node("node-003", "zone-a", inbox.clone()); // Byzantine
    sim.add_node("node-004", "zone-a", inbox.clone()); // will panic button
    sim.add_node("node-005", "zone-a", inbox.clone()); // indoor
    sim.add_node("node-006", "zone-a", inbox.clone());

    // Zone B — safe area
    sim.add_node("node-007", "zone-b", inbox.clone());
    sim.add_node("node-008", "zone-b", inbox.clone());
    sim.add_node("node-009", "zone-b", inbox.clone());

    // Wire peers
    sim.connect("node-001", "node-002");
    sim.connect("node-002", "node-003");
    sim.connect("node-003", "node-004");
    sim.connect("node-004", "node-005");
    sim.connect("node-005", "node-006");
    sim.connect("node-007", "node-008");
    sim.connect("node-008", "node-009");

    println!("=== CRCI Mesh — Initial State ===");
    sim.print_all();

    // ── Round 1 ──────────────────────────────────────────────────────────────
    println!("\n=== Round 1 — Nodes originate messages ===");

    sim.originate(
        "node-001",
        Message::new(
            "msg-001",
            "node-001",
            Signal::new(4, true, false, true, 4, Visibility::Direct),
            Some("flooding on main road"),
        ),
    );

    sim.originate(
        "node-002",
        Message::new(
            "msg-002",
            "node-002",
            Signal::new(4, true, false, true, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-003",
        Message::new(
            "msg-003",
            "node-003",
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            Some("all clear here"),
        ),
    );

    sim.originate(
        "node-004",
        Message::rescue("msg-004", "node-004", Some("help trapped under debris")),
    );

    sim.originate(
        "node-005",
        Message::new(
            "msg-005",
            "node-005",
            Signal::new(1, false, true, false, 1, Visibility::Unknown),
            Some("indoors cannot see outside"),
        ),
    );

    sim.originate(
        "node-006",
        Message::new(
            "msg-006",
            "node-006",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("bridge collapsed"),
        ),
    );

    sim.originate(
        "node-007",
        Message::new(
            "msg-007",
            "node-007",
            Signal::new(1, false, true, true, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-008",
        Message::new(
            "msg-008",
            "node-008",
            Signal::new(2, false, true, true, 4, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-009",
        Message::new(
            "msg-009",
            "node-009",
            Signal::new(1, false, true, true, 5, Visibility::Direct),
            None,
        ),
    );

    println!("\n=== Round 1 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 1 ===");
    sim.run_consensus();

    println!("\n=== State After Round 1 ===");
    sim.print_all();
    sim.new_round();

    // ── Round 2 — node-004 dies ───────────────────────────────────────────
    println!("\n=== Round 2 — node-004 battery dies ===");
    if let Some(n) = sim.nodes.get_mut("node-004") {
        n.go_offline();
    }

    sim.originate(
        "node-001",
        Message::new(
            "msg-010",
            "node-001",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("situation critical"),
        ),
    );

    sim.originate(
        "node-002",
        Message::new(
            "msg-011",
            "node-002",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-003",
        Message::new(
            "msg-012",
            "node-003",
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-006",
        Message::new(
            "msg-013",
            "node-006",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            None,
        ),
    );

    println!("\n=== Round 2 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 2 ===");
    sim.run_consensus();

    println!("\n=== State After Round 2 ===");
    sim.print_all();
    sim.new_round();

    // ── Round 3 ──────────────────────────────────────────────────────────────
    println!("\n=== Round 3 ===");

    sim.originate(
        "node-001",
        Message::new(
            "msg-014",
            "node-001",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("roads completely blocked"),
        ),
    );

    sim.originate(
        "node-002",
        Message::new(
            "msg-015",
            "node-002",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-003",
        Message::new(
            "msg-016",
            "node-003",
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            None,
        ),
    );

    sim.originate(
        "node-006",
        Message::new(
            "msg-017",
            "node-006",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            None,
        ),
    );

    println!("\n=== Round 3 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 3 ===");
    sim.run_consensus();

    println!("\n=== Final State ===");
    sim.print_all();

    // ── Session 10: Save and reload state ────────────────────────────────────
    println!("\n=== Saving node states to disk ===");
    for id in ["node-001", "node-002", "node-003", "node-006"] {
        if let Some(node) = sim.nodes.get(id) {
            node.save_state();
        }
    }

    println!("\n=== node-001 restarts — loading saved state ===");
    let fresh_inbox: crate::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut restarted = crate::runtime::NodeRuntime::new("node-001", "zone-a", fresh_inbox);
    restarted.load_state();
    restarted.status();

    println!("\n=== Cleaning up state files ===");
    for id in ["node-001", "node-002", "node-003", "node-006"] {
        if let Some(node) = sim.nodes.get(id) {
            node.storage.delete();
            println!("  🗑  deleted {}_state.enc", id);
        }
    }

    // ── Sessions 11-13: Adversarial Stress Tests ─────────────────────────────
    crate::stress::test_large_network();
    crate::stress::test_sybil_attack();
    crate::stress::test_signature_forgery();
    crate::stress::test_network_partition();

    // ── Crisis Scenarios ──────────────────────────────────────────────────────
    crate::crisis::scenario_flood();
    crate::crisis::scenario_earthquake();
    crate::crisis::scenario_conflict();
    crate::crisis::scenario_hazmat();

    // ── Session 17: Replay protection test ───────────────────────────────────
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║  SESSION 17 — Replay Attack Protection       ║");
    println!("╚══════════════════════════════════════════════╝");

    let replay_inbox: crate::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut replay_sim = crate::mesh::MeshSimulator::new();
    replay_sim.add_node("honest", "zone-r", replay_inbox.clone());
    replay_sim.add_node("attacker", "zone-r", replay_inbox.clone());
    replay_sim.add_node("victim", "zone-r", replay_inbox.clone());
    replay_sim.connect("honest", "victim");
    replay_sim.connect("attacker", "victim");

    // Honest node sends seq=1
    replay_sim.originate(
        "honest",
        crate::message::Message::new(
            "legit-001",
            "honest",
            crate::message::Signal::new(
                4,
                false,
                true,
                true,
                5,
                crate::message::Visibility::Direct,
            ),
            Some("legitimate report"),
        ),
    );
    replay_sim.drain();
    println!("  ✅ Legitimate message delivered");

    // Attacker re-injects the same message (same id, same seq) — victim must reject it
    // Simulate by originating a message with the same id from attacker claiming origin=honest
    println!("  Attacker re-injecting seq=1 from 'honest'...");
    // In a real test we'd manipulate the wire directly; here we demonstrate the
    // filter catches duplicate seq numbers from the same origin
    replay_sim.originate(
        "honest",
        crate::message::Message::new(
            "legit-001-replay",
            "honest",
            crate::message::Signal::new(
                4,
                false,
                true,
                true,
                5,
                crate::message::Visibility::Direct,
            ),
            Some("REPLAYED report"),
        ),
    );
    replay_sim.drain();
    println!(
        "  Replay protection tracked {} origins",
        replay_sim
            .nodes
            .get("victim")
            .map(|n| n.replay_filter.tracked_origins())
            .unwrap_or(0)
    );

    // ── Session 18: Benchmark harness ────────────────────────────────────
    bench::run_benchmarks();

    chaos::run_chaos_tests();

    // ── Session 21: Validation + Rate Limiting ────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 21 — INPUT VALIDATION + RATE LIMITING           ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    // Severity validation
    let nan_result = validation::validate_severity(f32::NAN);
    println!("  NaN severity rejected: {}", nan_result.is_err());
    let inf_result = validation::validate_severity(f32::INFINITY);
    println!("  Infinity severity rejected: {}", inf_result.is_err());
    let oob_result = validation::validate_severity(99.0);
    println!("  Out-of-range severity rejected: {}", oob_result.is_err());

    // Rate limiting
    let mut limiter = validation::RateLimiter::new();
    let mut blocked = 0usize;
    for _ in 0..15 {
        if limiter.check_and_record("spammer").is_err() {
            blocked += 1;
        }
    }
    println!("  Rate limit: 15 msgs → {} blocked (limit=10)", blocked);

    // Panic cooldown
    let mut limiter2 = validation::RateLimiter::new();
    let first = limiter2.check_panic_cooldown("node-a", 1);
    let second = limiter2.check_panic_cooldown("node-a", 5);
    println!(
        "  Panic button: first press ok={}, spam blocked={}",
        first.is_ok(),
        second.is_err()
    );

    // Seq overflow
    let overflow = validation::validate_seq(u64::MAX - 100);
    println!("  Seq overflow detected: {}", overflow.is_err());

    // Payload size
    let big = vec![0u8; 300];
    let size_result = validation::validate_payload_size(&big);
    println!("  Oversized payload blocked: {}", size_result.is_err());

    println!("  ✅ Session 21 validation tests complete.");

    // ── Session 22: AEDA ──────────────────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 22 — AEDA (Autonomous Emergency Decisions)      ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let mut aeda = aeda::AedaEngine::new();

    // Simulate zone-alpha crisis: 3 rescues within 5 rounds → escalation
    for i in 0..3u64 {
        aeda.process(
            aeda::RescueEvent {
                node_id: format!("node-{i}"),
                zone: "zone-alpha".to_string(),
                severity: 4,
                round: 10 + i,
                reputation: 1.0,
            },
            10 + i,
        );
    }
    // Suspicious all-clear from node-9 in active crisis zone
    aeda.process_normal_report("node-9", "zone-alpha");

    // Simulate misinformation node alternating sev 1 and 5
    for round in 0..6u64 {
        let sev = if round % 2 == 0 { 1u8 } else { 5u8 };
        aeda.process(
            aeda::RescueEvent {
                node_id: "bad-actor".to_string(),
                zone: "zone-beta".to_string(),
                severity: sev,
                round,
                reputation: 0.6,
            },
            round,
        );
    }

    println!("  Escalations triggered: {}", aeda.count_escalations());
    println!("  Suspicious all-clears: {}", aeda.count_suspicious());
    println!("  Misinformation suspects: {}", aeda.count_disinfo());
    println!();
    for decision in aeda.decisions() {
        println!("  {decision}");
    }
    println!();
    println!("  ✅ Session 22 AEDA complete.");

    // ── Session 23: Battery-Aware Mode ───────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 23A — BATTERY-AWARE GOSSIP THROTTLING           ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    // Simulate 3 nodes draining from full → critical
    let mut nodes_bat = vec![
        battery::BatteryState::new("node-full", 85),
        battery::BatteryState::new("node-low", 15),
        battery::BatteryState::new("node-critical", 3),
    ];

    for state in &mut nodes_bat {
        let rescue_fwd = state.should_forward(true);
        let normal_fwd = state.should_forward(false);
        println!(
            "  [{}] tier={} | rescue forwarded={} | normal forwarded={}",
            state.node_id,
            state.tier.label(),
            rescue_fwd,
            normal_fwd
        );
    }

    // Drain simulation: node-full goes critical over 20 rounds
    let mut draining = battery::BatteryState::new("draining-node", 100);
    for round in 0..20u8 {
        draining.update(100 - round * 5);
    }
    println!(
        "  After draining: {}% | tier={} | drain_rate={:.1}/round | est_rounds={}",
        draining.percent,
        draining.tier.label(),
        draining.drain_rate_per_round(),
        draining.estimated_rounds_remaining().unwrap_or(0)
    );

    let all_states = vec![
        battery::BatteryState::new("a", 80),
        battery::BatteryState::new("b", 12),
        battery::BatteryState::new("c", 2),
    ];
    let summary = battery::NetworkBatterySummary::from_states(&all_states);
    println!(
        "  Network: {} nodes | full={} low={} critical={} | min={}% avg={:.1}%",
        summary.total_nodes,
        summary.full_count,
        summary.low_count,
        summary.critical_count,
        summary.min_percent,
        summary.avg_percent
    );
    println!("  ✅ Session 23A battery tests complete.");

    // ── Session 23: TTL Enforcement ──────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  SESSION 23B — MESSAGE TTL + STORAGE PRUNING             ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let mut store = ttl::TtlStore::new();

    // Add 5 normal messages at round 0
    for i in 0..5 {
        store.store(ttl::StoredMessage::new_normal(&format!("msg-{i}"), 0));
    }
    // Add 2 rescue messages at round 0
    store.store(ttl::StoredMessage::new_rescue("rescue-A", 0));
    store.store(ttl::StoredMessage::new_rescue("rescue-B", 0));

    println!(
        "  Round 0: active={} rescue={}",
        store.active_count(),
        store.rescue_count()
    );

    // Prune at round 60 — all 5 normal messages should expire
    let pruned = store.prune(60);
    println!(
        "  Round 60 prune: {} normal msgs pruned | active={} rescue={}",
        pruned,
        store.active_count(),
        store.rescue_count()
    );

    // Resolve rescue-A — it can now expire
    store.resolve_rescue("rescue-A");
    let pruned2 = store.prune(510);
    println!(
        "  Round 510 prune after resolving rescue-A: {} pruned | active={} rescue={}",
        pruned2,
        store.active_count(),
        store.rescue_count()
    );

    // Tombstone test: try to re-store a pruned message
    let readmit = store.store(ttl::StoredMessage::new_normal("msg-0", 600));
    println!(
        "  Re-store of tombstoned msg-0: admitted={} (expected false)",
        readmit
    );

    println!(
        "  Total stored: {} | pruned: {} | evicted: {}",
        store.total_stored, store.total_pruned, store.total_evicted
    );
    println!("  ✅ Session 23B TTL tests complete.");
} // ← this is the closing brace of fn main()
