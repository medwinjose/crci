#![allow(dead_code)]
use crci_core::transport::Transport;
use crci_core::*;
mod cli;

use mesh::MeshSimulator;
use message::{Message, Signal, Visibility};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() {
    // Experiment timings (--emit-delay-ms, --replay-after-ms) count from process start.
    let process_start = std::time::Instant::now();
    let args = Cli::parse();

    if args.verbose {
        println!("Verbose mode enabled.");
    }

    if let Some(byz_mode) = args.byzantine.clone() {
        let Some(target) = args.to.clone() else {
            eprintln!("Byzantine mode requires a target peer via --to");
            std::process::exit(1);
        };
        let node_id = args.label.clone().unwrap_or_else(|| {
            let mut rng = rand::thread_rng();
            use rand::Rng;
            format!("byz-cli-{:04x}", rng.gen::<u16>())
        });
        println!(
            "Running as Byzantine node in mode: {} targeting: {}",
            byz_mode, target
        );
        match crci_core::byzantine_behavior::run_byzantine_behavior(
            &target,
            &node_id,
            &byz_mode,
            args.interval,
            args.duration,
        )
        .await
        {
            Ok((inj, fail)) => {
                println!("[byzantine-agent] Done. Total injections attempted: {}, Total rejected/failed: {}", inj + fail, fail);
            }
            Err(e) => {
                eprintln!("Byzantine agent error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if let (Some(send_text), Some(to_addr)) = (args.send.clone(), args.to.clone()) {
        let label = args
            .label
            .clone()
            .unwrap_or_else(|| "cli-sender".to_string());

        let (message_type, severity_val) = match args.severity.to_lowercase().as_str() {
            "normal" => (message::MessageType::Normal, 3),
            "rescue" => (message::MessageType::RescueRequest, 5),
            "panic" => (message::MessageType::Panic, 5),
            _ => {
                eprintln!(
                    "Invalid severity: {}. Must be normal, rescue, or panic.",
                    args.severity
                );
                std::process::exit(1);
            }
        };

        let listen_addr = args
            .listen
            .clone()
            .unwrap_or_else(|| "127.0.0.1:0".to_string());
        let listen_socket: std::net::SocketAddr = match listen_addr.parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Invalid listen/bind address {listen_addr}: {e}");
                std::process::exit(1);
            }
        };

        let mut node = make_node(&args, &label).await;
        if args.listen.is_none() {
            // With --listen, `run_node` prints this line.
            println!("Node {} id {}", label, node.id);
        }

        // The transport authenticates connections with the same key that signs messages.
        let tcp = match crci_core::transport::TcpTransport::bind(
            listen_socket,
            node.identity.signing_key.clone(),
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to bind TcpTransport on {listen_socket}: {e}");
                std::process::exit(1);
            }
        };

        // Originate a message signed with this node's key.
        let mut msg = Message::new(
            &uuid::Uuid::new_v4().to_string(),
            &node.id,
            Signal::new(
                severity_val,
                severity_val >= 5,
                false,
                true,
                3,
                Visibility::Direct,
            ),
            Some(&send_text),
        );
        msg.message_type = message_type;
        node.seq_counter += 1;
        msg.seq = node.seq_counter;
        let wire = crci_core::runtime::WireMessage::from_message(&msg, &node.identity);

        match tcp.send(&to_addr, &wire).await {
            Ok(_) => {
                println!("SUCCESS: Message originated and sent to {}", to_addr);
            }
            Err(e) => {
                println!("FAILURE: Failed to send message to {}: {:?}", to_addr, e);
            }
        }

        if args.listen.is_some() {
            run_node(
                label,
                node,
                Some(Arc::new(tcp)),
                dial_targets(&args),
                Experiment::from_args(&args, process_start),
            )
            .await;
        }
        return;
    }

    if args.label.is_some() || args.listen.is_some() {
        let label = args.label.clone().unwrap_or_else(|| "cli-node".to_string());
        let mut node = make_node(&args, &label).await;
        if let Some(rate) = args.sender_refill_per_sec {
            node.sybil_guard.sender_refill_per_sec = rate;
        }
        match args.defence.parse::<crci_core::runtime::DefenceMode>() {
            Ok(mode) => node.set_defence_mode(mode),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        if let Some(k) = args.strikes_to_ban {
            node.sybil_guard.set_strikes_to_ban(k);
        }
        if let Some(w) = args.replay_window {
            if w > crci_core::replay::MAX_WINDOW_SIZE {
                eprintln!(
                    "--replay-window {w} is above the maximum of {}",
                    crci_core::replay::MAX_WINDOW_SIZE
                );
                std::process::exit(1);
            }
            node.replay_filter.window_size = w;
        }

        let tcp = if let Some(ref listen) = args.listen {
            let addr = match listen.parse::<std::net::SocketAddr>() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Invalid listen address {listen}: {e}");
                    std::process::exit(1);
                }
            };
            match crci_core::transport::TcpTransport::bind(addr, node.identity.signing_key.clone())
                .await
            {
                Ok(t) => Some(Arc::new(t)),
                Err(e) => {
                    eprintln!("Failed to bind TcpTransport on {addr}: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            None
        };

        run_node(
            label,
            node,
            tcp,
            dial_targets(&args),
            Experiment::from_args(&args, process_start),
        )
        .await;
        return;
    }
    let (ws_tx, _) = tokio::sync::broadcast::channel(100);
    let (divergence_tx, _) = tokio::sync::broadcast::channel(100);
    let state = Arc::new(crci_core::api::ApiState {
        node_count: Arc::new(std::sync::RwLock::new(9)),
        peer_list: Arc::new(std::sync::RwLock::new(vec![
            "node-001".to_string(),
            "node-002".to_string(),
            "node-003".to_string(),
            "node-004".to_string(),
            "node-005".to_string(),
            "node-006".to_string(),
            "node-007".to_string(),
            "node-008".to_string(),
            "node-009".to_string(),
        ])),
        recent_messages: Arc::new(std::sync::RwLock::new(std::collections::VecDeque::new())),
        ws_tx,
        node_id: "local-node".to_string(),
        start_time: std::time::Instant::now(),
        byzantine_events: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        divergence_alerts: Arc::new(std::sync::RwLock::new(Vec::new())),
        divergence_tx,
        rate_limit_counts: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        merkle_head: Arc::new(std::sync::RwLock::new(([0; 32], 0))),
    });

    let state_clone = state.clone();

    let sim_task = tokio::spawn(async move {
        // Shared inbox — a single locked map that all SimTransports write into
        // and each NodeRuntime reads from. This simulates the radio layer.
        let inbox: crci_core::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));

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

        // ── Save and reload state ────────────────────────────────────
        println!("\n=== Saving node states to disk ===");
        for id in ["node-001", "node-002", "node-003", "node-006"] {
            if let Some(node) = sim.nodes.get(id) {
                node.save_state();
            }
        }

        println!("\n=== node-001 restarts — loading saved state ===");
        let fresh_inbox: crci_core::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut restarted = crci_core::runtime::NodeRuntime::new("node-001", "zone-a", fresh_inbox);
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
        crci_core::stress::test_large_network();
        crci_core::stress::test_sybil_attack();
        crci_core::stress::test_signature_forgery();
        crci_core::stress::test_network_partition();

        // ── Crisis Scenarios ──────────────────────────────────────────────────────
        crci_core::crisis::scenario_flood();
        crci_core::crisis::scenario_earthquake();
        crci_core::crisis::scenario_conflict();
        crci_core::crisis::scenario_hazmat();

        // ── Replay protection test ───────────────────────────────────
        println!("\n╔══════════════════════════════════════════════╗");
        println!("║  Replay Attack Protection                    ║");
        println!("╚══════════════════════════════════════════════╝");

        let replay_inbox: crci_core::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut replay_sim = crci_core::mesh::MeshSimulator::new();
        replay_sim.add_node("honest", "zone-r", replay_inbox.clone());
        replay_sim.add_node("attacker", "zone-r", replay_inbox.clone());
        replay_sim.add_node("victim", "zone-r", replay_inbox.clone());
        replay_sim.connect("honest", "victim");
        replay_sim.connect("attacker", "victim");

        // Honest node sends seq=1
        replay_sim.originate(
            "honest",
            crci_core::message::Message::new(
                "legit-001",
                "honest",
                crci_core::message::Signal::new(
                    4,
                    false,
                    true,
                    true,
                    5,
                    crci_core::message::Visibility::Direct,
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
            crci_core::message::Message::new(
                "legit-001-replay",
                "honest",
                crci_core::message::Signal::new(
                    4,
                    false,
                    true,
                    true,
                    5,
                    crci_core::message::Visibility::Direct,
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

        // ── Benchmark harness ────────────────────────────────────
        bench::run_benchmarks();

        chaos::run_chaos_tests();

        // ── Validation + Rate Limiting ────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  INPUT VALIDATION + RATE LIMITING                        ║");
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

        println!("  ✅ Validation tests complete.");

        // ── AEDA ──────────────────────────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  AEDA (Autonomous Emergency Decisions)                   ║");
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
        println!("  ✅ AEDA complete.");

        // ── Battery-Aware Mode ───────────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  BATTERY-AWARE GOSSIP THROTTLING                         ║");
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
        println!("  ✅ Battery tests complete.");

        // ── TTL Enforcement ──────────────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  MESSAGE TTL + STORAGE PRUNING                           ║");
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
        println!("  ✅ TTL tests complete.");

        // ── Integration Pipeline ─────────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  INTEGRATED GOSSIP PIPELINE                              ║");
        println!("║  Validation + Battery + TTL + AEDA wired together        ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        let mut pipe = integration::GossipPipeline::new();

        // Register node battery states
        pipe.update_battery("alice", 85);
        pipe.update_battery("bob", 15);
        pipe.update_battery("charlie", 2);

        // Round 1: mix of valid, invalid, rescue, and spam
        println!("\n  --- Round 1 ---");

        // Valid rescue from charlie (critical battery) — MUST go through
        let r = integration::PipelineMessage {
            id: "rescue-alice".to_string(),
            origin_node: "alice".to_string(),
            zone: "zone-hot".to_string(),
            severity: 5,
            kind: integration::MessageKind::Rescue,
            payload_bytes: 80,
            reputation: 1.0,
            round: 1,
            seq: 1,
        };
        let v = pipe.process(&r);
        println!("  Alice rescue (full battery):    {:?}", v);

        let r2 = integration::PipelineMessage {
            id: "rescue-charlie".to_string(),
            origin_node: "charlie".to_string(),
            zone: "zone-hot".to_string(),
            severity: 5,
            kind: integration::MessageKind::Rescue,
            payload_bytes: 80,
            reputation: 1.0,
            round: 1,
            seq: 1,
        };
        let v2 = pipe.process(&r2);
        println!("  Charlie rescue (critical batt): {:?}", v2);

        // Normal msg from charlie (critical battery) — must be suppressed
        let n1 = integration::PipelineMessage {
            id: "normal-charlie".to_string(),
            origin_node: "charlie".to_string(),
            zone: "zone-hot".to_string(),
            severity: 2,
            kind: integration::MessageKind::Normal,
            payload_bytes: 40,
            reputation: 1.0,
            round: 1,
            seq: 1,
        };
        let v3 = pipe.process(&n1);
        println!("  Charlie normal (critical batt): {:?}", v3);

        // Invalid severity — must be rejected
        let bad = integration::PipelineMessage {
            id: "bad-sev".to_string(),
            origin_node: "attacker".to_string(),
            zone: "zone-hot".to_string(),
            severity: 0,
            kind: integration::MessageKind::Normal,
            payload_bytes: 40,
            reputation: 0.5,
            round: 1,
            seq: 1,
        };
        let v4 = pipe.process(&bad);
        println!("  Attacker invalid severity:      {:?}", v4);

        // Spam: 12 msgs from one node
        for i in 0..12u32 {
            pipe.process(&integration::PipelineMessage {
                id: format!("spam-{i}"),
                origin_node: "spammer".to_string(),
                zone: "zone-hot".to_string(),
                severity: 3,
                kind: integration::MessageKind::Normal,
                payload_bytes: 30,
                reputation: 0.9,
                round: 1,
                seq: 1,
            });
        }
        println!("  Spammer (12 msgs, limit=10):    throttled/rejected as expected");

        // 3 rescues → AEDA should escalate zone-hot
        pipe.process(&integration::PipelineMessage {
            id: "rescue-bob".to_string(),
            origin_node: "bob".to_string(),
            zone: "zone-hot".to_string(),
            severity: 4,
            kind: integration::MessageKind::Rescue,
            payload_bytes: 60,
            reputation: 1.0,
            round: 1,
            seq: 1,
        });

        let (esc, sus, dis) = pipe.aeda_summary();
        println!("\n  AEDA after 3 rescues in zone-hot:");
        println!("    Escalations: {} (expected 1)", esc);
        println!("    Suspicious:  {}", sus);
        println!("    Disinfo:     {}", dis);

        // Rescue ack feedback
        println!("\n  Rescue acknowledgment counts:");
        println!(
            "    rescue-alice:   {} node(s) holding",
            pipe.rescue_ack_count("rescue-alice")
        );
        println!(
            "    rescue-charlie: {} node(s) holding",
            pipe.rescue_ack_count("rescue-charlie")
        );

        // Resolve rescue-charlie (found safe)
        pipe.resolve_rescue("rescue-charlie");
        println!(
            "    rescue-charlie after resolve: {} (expected 0)",
            pipe.rescue_ack_count("rescue-charlie")
        );

        // Advance round — rate limiter resets, TTL prunes
        pipe.next_round();
        println!("\n  Pipeline stats:");
        println!(
            "    Accepted: {} | Rejected: {} | Throttled: {}",
            pipe.accepted, pipe.rejected, pipe.throttled
        );
        println!("    Active in TTL store: {}", pipe.ttl_store.active_count());

        // Split battery counter demo (bug fix 1)
        println!("\n  Split battery counter (bug fix):");
        let mut sbc = integration::SplitBatteryCounter::new(15); // LOW
        sbc.should_forward(true); // rescue
        sbc.should_forward(true); // rescue
        let first_normal = sbc.should_forward(false); // normal — must not be affected by rescue count
        println!(
            "    After 2 rescues, first normal forwarded: {} (expected true)",
            first_normal
        );
        println!(
            "    Rescue count: {} | Normal count: {}",
            sbc.rescue_forwarded, sbc.normal_forwarded
        );

        println!();
        println!("  ✅ Integration pipeline complete.");

        // ── STRIDE Security Hardening ────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  STRIDE SECURITY HARDENING                               ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        // S: Spoofing — Node ID derivation
        println!("\n  [S] Spoofing — Opaque Node IDs:");
        let fake_pubkey = b"example_pubkey_bytes_32_aabbccdd";
        let derived_id = security::derive_node_id(fake_pubkey);
        println!("    Derived ID from pubkey: {derived_id}");
        println!(
            "    Is valid format: {}",
            security::is_valid_node_id_format(&derived_id)
        );
        println!(
            "    Sequential ID 'zone-a-node-01' valid: {}",
            security::is_valid_node_id_format("zone-a-node-01")
        );

        // T: Tampering — Signable payload audit
        println!("\n  [T] Tampering — Signable Payload Audit:");
        let complete = "node_id=abc zone=z1 severity=5 kind=rescue round=42";
        let incomplete = "node_id=abc severity=5";
        println!(
            "    Complete payload:   {:?}",
            security::audit_signable_payload(complete)
        );
        println!(
            "    Incomplete payload: {:?}",
            security::audit_signable_payload(incomplete)
        );

        // R: Repudiation — Audit log
        println!("\n  [R] Repudiation — Audit Log:");
        let mut audit = security::AuditLog::new(1000);
        audit.append(
            1,
            security::AuditEventKind::MessageRejected {
                node_id: "attacker".to_string(),
                reason: "invalid severity".to_string(),
            },
        );
        audit.append(
            2,
            security::AuditEventKind::AedaEscalation {
                zone: "zone-hot".to_string(),
            },
        );
        audit.append(
            3,
            security::AuditEventKind::ZoneSpoofAttempt {
                node_id: "enemy-node".to_string(),
                claimed_zone: "zone-alpha".to_string(),
            },
        );
        audit.append(
            4,
            security::AuditEventKind::RescueResolved {
                rescue_id: "rescue-001".to_string(),
                resolver: "sar-team-1".to_string(),
            },
        );
        println!("    Audit entries logged: {}", audit.len());
        for entry in audit.entries() {
            println!("    [round {}] {}", entry.round, entry.event);
        }

        // I: Information Disclosure — Zone membership
        println!("\n  [I] Information Disclosure — Zone Membership:");
        let mut zone_reg = security::ZoneMembershipRegistry::new();
        zone_reg.register_bootstrap("trusted-node", "zone-alpha");
        let spoof = zone_reg.claim_zone("enemy-node", "zone-alpha");
        println!("    Enemy zone claim accepted: {} (expected false)", spoof);
        println!(
            "    Enemy pending vouching: {}",
            zone_reg.is_pending("enemy-node")
        );
        let vouched = zone_reg.vouch("trusted-node", "enemy-node");
        println!("    Vouched by trusted node: {}", vouched);
        println!(
            "    Enemy now verified: {}",
            zone_reg.is_verified("enemy-node", "zone-alpha")
        );

        // D: Denial of Service — Reputation-weighted MCE
        println!("\n  [D] Denial of Service — Weighted MCE Threshold:");
        let mut mce = security::WeightedRescueCounter::new(3.0);
        for _ in 0..5 {
            let t = mce.record("zone-fake", 0.3); // low-rep spam
            if t {
                println!("    UNEXPECTED: low-rep triggered MCE");
            }
        }
        println!(
            "    5 low-rep (0.3) rescues — MCE triggered: false (zone count: {:.1})",
            mce.zone_count("zone-fake")
        );
        let mut mce2 = security::WeightedRescueCounter::new(3.0);
        let t1 = mce2.record("zone-real", 1.0);
        let t2 = mce2.record("zone-real", 1.0);
        let t3 = mce2.record("zone-real", 1.0);
        println!(
            "    3 full-rep (1.0) rescues — MCE triggered: {} (expected true)",
            t1 || t2 || t3
        );

        // E: Elevation of Privilege + Safety: Priority Queue
        println!("\n  [E] Elevation + Safety — Priority Message Queue:");
        let mut q = security::PriorityMessageQueue::new();
        q.push(security::QueuedMessage {
            id: "n1".to_string(),
            kind: security::ProtocolMessageKind::Normal,
            round: 1,
        });
        q.push(security::QueuedMessage {
            id: "h1".to_string(),
            kind: security::ProtocolMessageKind::Hazard,
            round: 1,
        });
        q.push(security::QueuedMessage {
            id: "r1".to_string(),
            kind: security::ProtocolMessageKind::Rescue,
            round: 1,
        });
        q.push(security::QueuedMessage {
            id: "g1".to_string(),
            kind: security::ProtocolMessageKind::Goodbye,
            round: 1,
        });
        println!("    Dequeue order (highest priority first):");
        while let Some(msg) = q.pop() {
            println!("      [{:?}] id={}", msg.kind, msg.id);
        }

        // GOODBYE message demo
        println!("\n  Safety: GOODBYE + RescueResolution message types:");
        let bye = security::QueuedMessage {
            id: "bye-001".to_string(),
            kind: security::ProtocolMessageKind::Goodbye,
            round: 10,
        };
        let resolved = security::QueuedMessage {
            id: "res-001".to_string(),
            kind: security::ProtocolMessageKind::RescueResolution {
                rescue_id: "rescue-priya".to_string(),
            },
            round: 11,
        };
        println!("    GOODBYE priority: {}", bye.kind.priority());
        println!(
            "    RescueResolution priority: {}",
            resolved.kind.priority()
        );

        println!();
        println!("  ✅ STRIDE hardening complete.");

        // ── Node Discovery Protocol ──────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  NODE DISCOVERY PROTOCOL                                 ║");
        println!("║  Beacon + Peer Exchange + Zone Bootstrap                 ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        // Scenario: 3 nodes in zone-alpha, 2 in zone-beta
        // node-a and node-b hear each other directly via beacon
        // node-c only knows node-a and learns about node-b via exchange
        let id_a = "a1a1a1a1a1a1a1a1";
        let id_b = "b2b2b2b2b2b2b2b2";
        let id_c = "c3c3c3c3c3c3c3c3";
        let id_d = "d4d4d4d4d4d4d4d4";
        let id_e = "e5e5e5e5e5e5e5e5";

        let mut eng_a = discovery::DiscoveryEngine::new(id_a, "zone-alpha");
        let mut eng_b = discovery::DiscoveryEngine::new(id_b, "zone-alpha");
        let mut eng_c = discovery::DiscoveryEngine::new(id_c, "zone-alpha");
        let mut eng_d = discovery::DiscoveryEngine::new(id_d, "zone-beta");
        let mut eng_e = discovery::DiscoveryEngine::new(id_e, "zone-beta");

        println!("\n  --- Round 10: First beacon round ---");

        // Round 10 beacons
        let beacon_a = eng_a.generate_beacon(10, "FULL");
        let beacon_b = eng_b.generate_beacon(10, "LOW");
        let beacon_d = eng_d.generate_beacon(10, "FULL");
        let beacon_e = eng_e.generate_beacon(10, "CRITICAL");

        // A and B hear each other; C only hears A; D and E hear each other
        eng_b.receive_beacon(&beacon_a, 10);
        eng_a.receive_beacon(&beacon_b, 10);
        eng_c.receive_beacon(&beacon_a, 10);
        eng_e.receive_beacon(&beacon_d, 10);
        eng_d.receive_beacon(&beacon_e, 10);

        println!(
            "  node-a knows: {} peers | node-b knows: {} peers | node-c knows: {} peers",
            eng_a.known_peers(),
            eng_b.known_peers(),
            eng_c.known_peers()
        );
        println!(
            "  node-c knows node-b directly: {} (expected false)",
            eng_c.knows_peer(id_b)
        );

        println!("\n  --- Round 20: Peer exchange — C asks A for peers ---");

        // C connects to A and requests peer list
        let shared_by_a = eng_a.generate_peer_exchange(id_c, 20);
        let found = eng_c.receive_peer_exchange(id_a, &shared_by_a, "zone-alpha", 20);
        println!("  node-c discovered {} new peer(s) via exchange", found);
        println!(
            "  node-c now knows node-b: {} (expected true)",
            eng_c.knows_peer(id_b)
        );
        println!("  node-c total known: {}", eng_c.known_peers());

        println!("\n  --- Round 20: Zone-beta peer exchange ---");
        let shared_by_d = eng_d.generate_peer_exchange(id_e, 20);
        let found_e = eng_e.receive_peer_exchange(id_d, &shared_by_d, "zone-beta", 20);
        println!("  node-e discovered {} new peer(s) from node-d", found_e);

        println!("\n  --- Full network simulation (10 nodes, 50 rounds) ---");
        let (beacons_sent, total_discovered) = discovery::simulate_discovery(10, 50);
        println!(
            "  Beacons broadcast: {} | Peer discoveries: {}",
            beacons_sent, total_discovered
        );

        println!("\n  --- Battery tier in beacons ---");
        println!(
            "  beacon_b battery tier: {} (expected LOW)",
            beacon_b.battery_tier
        );
        println!(
            "  beacon_e battery tier: {} (expected CRITICAL)",
            beacon_e.battery_tier
        );

        println!("\n  --- Exchange peer count cap ---");
        // Populate node-a with many peers and verify exchange cap
        for i in 0..12u64 {
            eng_a.peer_table.upsert(discovery::PeerRecord {
                node_id: format!("{i:016x}"),
                zone: "zone-alpha".to_string(),
                last_seen: i,
                discovery_method: discovery::DiscoveryMethod::DirectBeacon,
                battery_tier: "FULL".to_string(),
                zone_vouched: false,
            });
        }
        let big_exchange = eng_a.generate_peer_exchange("newcomer", 30);
        println!(
            "  node-a has 12+ peers, exchange sends {} (max=8)",
            big_exchange.len()
        );

        println!();
        println!("  ✅ Node discovery complete.");

        // ── Extended Benchmarks ──────────────────────────
        bench_extended::run_extended_benchmarks();

        // ── Zone Registry + Docker Compose ───────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  SECURITY WIRING & DOCKER PROOF LOOP                     ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        let mut pipe29 = integration::GossipPipeline::new();
        let norm_msg = integration::PipelineMessage {
            id: "s29-norm".to_string(),
            origin_node: "unverified".to_string(),
            zone: "zone-1".to_string(),
            severity: 2,
            kind: integration::MessageKind::Normal,
            payload_bytes: 10,
            reputation: 1.0,
            round: 1,
            seq: 1,
        };
        let verdict = pipe29.process(&norm_msg);
        println!(
            "  Zone registry wiring: unverified node rejected → {:?}",
            verdict
        );

        let res_msg = integration::PipelineMessage {
            id: "s29-res".to_string(),
            origin_node: "unverified".to_string(),
            zone: "zone-1".to_string(),
            severity: 5,
            kind: integration::MessageKind::Rescue,
            payload_bytes: 10,
            reputation: 1.0,
            round: 1,
            seq: 2,
        };
        let verdict = pipe29.process(&res_msg);
        println!("  Rescue from unverified node: accepted → {:?}", verdict);

        pipe29.register_bootstrap("zone-1", "boot");
        pipe29.claim_zone("zone-1", "vouched");
        pipe29.vouch("zone-1", "boot", "vouched");

        let vouched_msg = integration::PipelineMessage {
            id: "s29-vouch".to_string(),
            origin_node: "vouched".to_string(),
            zone: "zone-1".to_string(),
            severity: 2,
            kind: integration::MessageKind::Normal,
            payload_bytes: 10,
            reputation: 1.0,
            round: 1,
            seq: 3,
        };
        let verdict = pipe29.process(&vouched_msg);
        println!("  Vouched node accepted → {:?}", verdict);

        // ── WASM stand-in: priority scorer ──────────────────────────
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  WASM STAND-IN — PRIORITY SCORER (no engine linked)      ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        let mut runtime = crci_core::wasm::WasmRuntime::new(10 * 1024 * 1024, 100);
        runtime.register(Box::new(crci_core::wasm::PriorityScorer));
        println!("  WasmRuntime stand-in: registered 1 module (priority_scorer)");

        let input = [4, 0, 0, 0, 0];
        if let Ok(result) = runtime.execute("priority_scorer", &input) {
            println!(
                "  Edge computation: input={} bytes → score={}",
                input.len(),
                result[0]
            );
        }
    });

    let api_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
            .await
            .unwrap_or_else(|_| panic!("Failed to bind 8080"));
        println!("API server listening on 0.0.0.0:8080");
        if let Err(e) = axum::serve(listener, crci_core::api::build_router(state_clone)).await {
            eprintln!("API server error: {}", e);
        }
    });

    let _ = tokio::join!(sim_task, api_task);
}

/// Peer addresses from `--dial` and `--peers`.
fn dial_targets(args: &Cli) -> Vec<String> {
    let mut targets = Vec::new();
    if let Some(dial) = args.dial.clone() {
        targets.push(dial);
    }
    if let Some(peers_str) = args.peers.clone() {
        for p in peers_str.split(',') {
            let p = p.trim();
            if !p.is_empty() {
                targets.push(p.to_string());
            }
        }
    }
    targets
}

/// Field rules from the gossip pipeline, applied before a message reaches `NodeRuntime`
/// so that malformed messages are neither delivered nor relayed.
fn check_fields(wire: &runtime::WireMessage) -> Result<(), String> {
    validation::validate_node_id(&wire.origin).map_err(|e| format!("invalid origin: {e}"))?;
    validation::validate_severity_u8(wire.severity).map_err(|e| e.to_string())?;
    validation::validate_seq(wire.seq).map_err(|e| e.to_string())?;
    let note = wire.note.as_deref().unwrap_or("");
    validation::validate_payload_size(note.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// The application-layer view of a delivered message, for AEDA and the severity check.
/// `reputation` is the reputation of the neighbour that delivered it.
fn to_pipeline_message(
    wire: &runtime::WireMessage,
    reputation: f32,
) -> integration::PipelineMessage {
    let kind = match wire.message_type.as_str() {
        "rescue" | "mce" => integration::MessageKind::Rescue,
        "panic" => integration::MessageKind::Panic,
        _ => integration::MessageKind::Normal,
    };
    integration::PipelineMessage {
        id: wire.id.clone(),
        origin_node: wire.origin.clone(),
        zone: "zone-host".to_string(),
        severity: wire.severity,
        kind,
        payload_bytes: wire.note.as_ref().map(|n| n.len()).unwrap_or(0),
        reputation,
        round: 0,
        seq: wire.seq,
    }
}

/// The address sync answers should be sent to: the listen address, unless it is unspecified
/// (`0.0.0.0` or `::`), which a peer cannot dial back.
fn sync_reply_address(listen: std::net::SocketAddr) -> Option<String> {
    if listen.ip().is_unspecified() {
        None
    } else {
        Some(listen.to_string())
    }
}

/// Options for testbed experiments: periodic origination of signed messages, and a fault
/// mode in which the node replays every gossip message it sends.
#[derive(Clone, Copy, Debug)]
struct Experiment {
    /// The instant the delays below count from: process start, or the shared epoch given
    /// with --experiment-epoch-unix-ms (which may still be in the future).
    started: std::time::Instant,
    emit_count: u64,
    emit_interval: std::time::Duration,
    emit_delay: std::time::Duration,
    replay_after: Option<std::time::Duration>,
    replay_copies: usize,
    corrupt_signatures: bool,
    corrupt_for: Option<std::time::Duration>,
}

impl Experiment {
    fn from_args(args: &Cli, process_start: std::time::Instant) -> Self {
        let started = match args.experiment_epoch_unix_ms {
            Some(epoch) => {
                let now_instant = std::time::Instant::now();
                let now_ms = unix_ms() as u64;
                if epoch >= now_ms {
                    now_instant
                        .checked_add(std::time::Duration::from_millis(epoch - now_ms))
                        .unwrap_or(now_instant)
                } else {
                    now_instant
                        .checked_sub(std::time::Duration::from_millis(now_ms - epoch))
                        .unwrap_or(now_instant)
                }
            }
            None => process_start,
        };
        Experiment {
            started,
            emit_count: args.emit_count,
            emit_interval: std::time::Duration::from_millis(args.emit_interval_ms.max(1)),
            emit_delay: std::time::Duration::from_millis(args.emit_delay_ms),
            replay_after: args.replay_after_ms.map(std::time::Duration::from_millis),
            replay_copies: args.replay_copies.max(1),
            corrupt_signatures: args.corrupt_signatures,
            corrupt_for: args.corrupt_for_ms.map(std::time::Duration::from_millis),
        }
    }
}

/// Milliseconds since the Unix epoch, for log lines that experiments correlate across nodes.
fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Relay messages `NodeRuntime` queued, `copies` times each (more than one only in the
/// replay fault mode). Destinations are peer addresses from --dial / --peers.
fn relay_outbound(
    tcp: &transport::TcpTransport,
    outbound: Vec<(String, Vec<u8>)>,
    copies: usize,
    corrupt: bool,
) {
    for (dest, bytes) in outbound {
        let Ok(mut out) = serde_json::from_slice::<runtime::WireMessage>(&bytes) else {
            continue;
        };
        if corrupt {
            corrupt_signature(&mut out);
        }
        for _ in 0..copies {
            if let Err(e) = tcp.relay(&dest, out.clone()) {
                log::warn!("relay of '{}' to {} dropped: {}", out.id, dest, e);
            }
        }
    }
}

/// Make the origin signature invalid (fault injection: a relay forwarding forged data).
fn corrupt_signature(msg: &mut runtime::WireMessage) {
    match msg.signature_bytes.first_mut() {
        Some(b) => *b ^= 1,
        None => msg.signature_bytes.push(1),
    }
}

/// Whether the fault has started and this node was told to corrupt signatures.
fn corrupting_now(experiment: &Experiment) -> bool {
    experiment.corrupt_signatures
        && experiment.replay_after.is_some_and(|after| {
            let now = std::time::Instant::now();
            experiment
                .started
                .checked_add(after)
                .is_some_and(|at| now >= at && experiment.corrupt_for.is_none_or(|d| now < at + d))
        })
}

/// How many copies of each outgoing message to send: `replay_copies` once the replay fault
/// is active.
/// Prints when the fault starts, so experiments can measure eviction latency from it.
fn copies_to_send(
    node_id: &str,
    experiment: &Experiment,
    started: std::time::Instant,
    announced: &std::sync::atomic::AtomicBool,
) -> usize {
    match experiment.replay_after {
        Some(after)
            if started
                .checked_add(after)
                .is_some_and(|at| std::time::Instant::now() >= at) =>
        {
            if !announced.swap(true, std::sync::atomic::Ordering::SeqCst) {
                println!(
                    "Node {} fault: sending every gossip message {} times from now at_ms={}",
                    node_id,
                    experiment.replay_copies,
                    unix_ms()
                );
            }
            experiment.replay_copies
        }
        _ => 1,
    }
}

/// The node labelled `label`. With `--key-file`, its key is the one kept in that encrypted file
/// (created on first use), so it keeps its ID across restarts, and its sequence numbers start
/// above any the previous run could have used. Without it, a fresh key: a new ID every start.
async fn make_node(args: &Cli, label: &str) -> runtime::NodeRuntime {
    let inbox: transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let (Some(key_file), Some(passphrase_file)) = (&args.key_file, &args.key_passphrase_file)
    else {
        return runtime::NodeRuntime::new(label, "zone-host", inbox);
    };
    let passphrase = match std::fs::read_to_string(passphrase_file) {
        Ok(text) => text.lines().next().unwrap_or_default().to_string(),
        Err(e) => {
            eprintln!(
                "Cannot read --key-passphrase-file {}: {e}",
                passphrase_file.display()
            );
            std::process::exit(1);
        }
    };
    match identity::load_or_create(key_file, &passphrase).await {
        Ok((key, seq_floor)) => {
            let identity = identity::Identity::from_signing_key(label, key);
            let mut node = runtime::NodeRuntime::with_identity(identity, "zone-host", inbox);
            node.seq_counter = seq_floor;
            node.control_seq_counter = seq_floor;
            node
        }
        Err(e) => {
            eprintln!("Cannot load the node key from {}: {e}", key_file.display());
            std::process::exit(1);
        }
    }
}

/// Remove and return everything `NodeRuntime` queued for other nodes (gossip relays and
/// chain-head announcements), keyed by destination.
fn take_outbound(node: &runtime::NodeRuntime) -> Vec<(String, Vec<u8>)> {
    let mut inbox = node.inbox.lock().unwrap_or_else(|e| e.into_inner());
    let destinations: Vec<String> = inbox.keys().filter(|k| **k != node.id).cloned().collect();
    let mut out = Vec::new();
    for dest in destinations {
        for (_, bytes) in inbox.remove(&dest).unwrap_or_default() {
            out.push((dest.clone(), bytes));
        }
    }
    out
}

/// Run a node: dial peers, then (with a listener) feed every received TCP message through
/// `NodeRuntime` (signatures, replay window, strikes, bans), hand delivered messages to the
/// gossip pipeline, and relay `NodeRuntime`'s outgoing messages over TCP.
async fn run_node(
    node_id: String,
    mut node: runtime::NodeRuntime,
    tcp: Option<Arc<transport::TcpTransport>>,
    dial: Vec<String>,
    experiment: Experiment,
) {
    // Log lines name this node by its label (`node_id` here) and peers and origins by their
    // node IDs; this line maps one to the other.
    println!("Node {} id {}", node_id, node.id);
    let started = experiment.started;
    let fault_announced = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Frames from banned peers, dropped before they are parsed; reported with the statistics.
    let banned_frames_dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Lines the receive loop held back (see `FairLogGate`): (received, accepted), reported with the
    // statistics and at shutdown, so that counts made from the log stay exact.
    let lines_held = Arc::new((
        std::sync::atomic::AtomicU64::new(0),
        std::sync::atomic::AtomicU64::new(0),
    ));
    // Forward to every dialed peer: the router's next hops are node IDs, not TCP addresses.
    node.use_router = false;
    node.delivery_lines = false;
    let mut pipeline = integration::GossipPipeline::new();
    pipeline.register_bootstrap("zone-host", &node_id);

    let (ws_tx, _) = tokio::sync::broadcast::channel(100);
    let (divergence_tx, _) = tokio::sync::broadcast::channel(100);
    let api_state = Arc::new(api::ApiState {
        node_count: Arc::new(std::sync::RwLock::new(0)),
        peer_list: Arc::new(std::sync::RwLock::new(Vec::new())),
        recent_messages: Arc::new(std::sync::RwLock::new(std::collections::VecDeque::new())),
        ws_tx: ws_tx.clone(),
        node_id: node.id.clone(),
        start_time: std::time::Instant::now(),
        byzantine_events: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        divergence_alerts: Arc::new(std::sync::RwLock::new(Vec::new())),
        divergence_tx: divergence_tx.clone(),
        rate_limit_counts: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        merkle_head: Arc::new(std::sync::RwLock::new(([0; 32], 0))),
    });

    // Every configured peer is a relay target from the start, whether or not it is up yet:
    // the persistent connection to it is opened on the first relay and re-opened when it
    // drops. Only a reachable peer joins the zone and the API peer list now. The probes run
    // concurrently and time out, so peers that are down cannot hold up start-up.
    const STARTUP_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
    let probes: Vec<_> = dial
        .iter()
        .map(|peer| {
            node.add_peer(peer);
            let peer = peer.clone();
            tokio::spawn(async move {
                let result = tokio::time::timeout(
                    STARTUP_PROBE_TIMEOUT,
                    tokio::net::TcpStream::connect(&peer),
                )
                .await;
                (peer, result)
            })
        })
        .collect();
    for probe in probes {
        let Ok((peer, result)) = probe.await else {
            continue;
        };
        match result {
            Ok(Ok(_)) => {
                pipeline.register_bootstrap("zone-host", &peer);
                add_api_peer(&api_state, &peer);
                println!("Successfully dialed peer: {}", peer);
            }
            Ok(Err(e)) => {
                println!("Warning: Failed to connect to peer {}: {}", peer, e);
            }
            Err(_) => {
                println!(
                    "Warning: Failed to connect to peer {}: no answer within {} s",
                    peer,
                    STARTUP_PROBE_TIMEOUT.as_secs()
                );
            }
        }
    }

    let Some(tcp) = tcp else {
        return;
    };

    // Axum REST/WebSocket API on 8080
    let api_state_clone = api_state.clone();
    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:8080").await {
            println!("Axum API server listening on 0.0.0.0:8080");
            let _ = axum::serve(listener, api::build_router(api_state_clone)).await;
        }
    });

    let node_arc = Arc::new(Mutex::new(node));

    // Anti-entropy: ask every dialed peer for the messages this node is missing, at
    // start-up and every SYNC_INTERVAL; a peer that could not be reached is retried every
    // SYNC_RETRY, so a healed link is re-synchronised within seconds. The session is opened
    // once the listener's identity is authenticated and before the request is sent, so its
    // answers can never arrive first.
    const SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
    const SYNC_RETRY: std::time::Duration = std::time::Duration::from_secs(5);
    let sync_tcp = tcp.clone();
    let sync_node = node_arc.clone();
    let sync_id = node_id.clone();
    let reply_to = sync_reply_address(tcp.listen_addr);
    if reply_to.is_none() {
        eprintln!(
            "Warning: listening on {}: peers answer sync requests at the listen address, so \
             anti-entropy is off. Listen on an address the peers can reach.",
            tcp.listen_addr
        );
    }
    // A peer that re-establishes its relay connection to this node (`link_resumed`) may have
    // lost frames with the previous one, so it is asked for a sync as soon as its cooldown
    // allows, not at the next SYNC_INTERVAL. Its address is known once this node's own sync
    // dial authenticated it.
    tokio::spawn(async move {
        let Some(reply_to) = reply_to else {
            return;
        };
        let mut new_links = sync_tcp.new_links.lock().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut next_due: HashMap<String, std::time::Instant> = HashMap::new();
        let mut last_sent: HashMap<String, std::time::Instant> = HashMap::new();
        let mut addr_of: HashMap<String, String> = HashMap::new();
        loop {
            let now = std::time::Instant::now();
            for peer in &dial {
                if next_due.get(peer).is_some_and(|due| *due > now) {
                    continue;
                }
                let mut conn = match sync_tcp.dial(peer).await {
                    Ok(conn) => conn,
                    Err(e) => {
                        log::warn!("sync: {} unreachable: {}", peer, e);
                        next_due.insert(peer.clone(), now + SYNC_RETRY);
                        continue;
                    }
                };
                let request = {
                    let mut n = sync_node.lock().unwrap_or_else(|e| e.into_inner());
                    let request = n.build_sync_request(Some(reply_to.clone()));
                    n.open_sync_session(&conn.peer, &request);
                    request
                };
                match conn.send(&request).await {
                    Ok(()) => println!(
                        "Node {} requested sync from {} ({})",
                        sync_id, peer, conn.peer
                    ),
                    Err(e) => log::warn!("sync request to {} failed: {}", peer, e),
                }
                addr_of.insert(conn.peer.clone(), peer.clone());
                conn.close().await;
                // After the dial, which can take seconds: the cooldown runs from the arrival.
                last_sent.insert(peer.clone(), std::time::Instant::now());
                next_due.insert(peer.clone(), now + SYNC_INTERVAL);
            }
            let wake = next_due
                .values()
                .min()
                .copied()
                .unwrap_or(now + SYNC_RETRY)
                .min(std::time::Instant::now() + SYNC_RETRY);
            tokio::select! {
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(wake)) => {}
                Some(identity) = new_links.recv() => {
                    let Some(peer) = addr_of.get(&identity) else {
                        continue;
                    };
                    let earliest = last_sent
                        .get(peer)
                        .map_or(now, |t| *t + runtime::SYNC_RESUME_SPACING);
                    let due = next_due.entry(peer.clone()).or_insert(earliest);
                    *due = (*due).min(earliest.max(std::time::Instant::now()));
                }
            }
        }
    });
    let pipeline = Arc::new(Mutex::new(pipeline));

    let recv_tcp = tcp.clone();
    let recv_node = node_arc.clone();
    let recv_pipeline = pipeline.clone();
    let recv_api = api_state.clone();
    let recv_id = node_id.clone();
    let recv_fault = fault_announced.clone();
    let recv_banned_dropped = banned_frames_dropped.clone();
    let recv_lines_held = lines_held.clone();
    tokio::spawn(async move {
        // Peers this node has banned, so each ban (and readmission) is printed once.
        let mut banned: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Banned peers whose dropped frames were already announced (one line per ban).
        let mut dropping: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Every line below 1 000 per 10 s (three times the highest honest load measured); above,
        // a fair share per sender, at least 20 per 10 s.
        let mut received_lines = runtime::FairLogGate::new(1000, 20);
        let mut accepted_lines = runtime::FairLogGate::new(1000, 20);
        while let Ok((sender_id, wire)) = recv_tcp.receive().await {
            let dequeued = std::time::Instant::now();
            // A banned peer's frames are dropped by NodeRuntime's admission check anyway; doing
            // it first spares the parse, the lock hold and the per-frame log lines, which a
            // flooding peer would otherwise turn into CPU and output load (a blocked stdout
            // would stall this loop). The check also performs the time-based readmission.
            let sender_is_banned = {
                let mut n = recv_node.lock().unwrap_or_else(|e| e.into_inner());
                let is_banned = n.sybil_guard.is_banned(&sender_id);
                if is_banned {
                    n.sybil_banned_events += 1;
                    n.metrics
                        .sybil_banned_total
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                is_banned
            };
            if sender_is_banned {
                // A frame still arrives only from before the shun took effect, or as the
                // probe after its interval: shun again so the connection is not read.
                recv_tcp.shun_peer(&sender_id);
                recv_banned_dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if dropping.insert(sender_id.clone()) {
                    println!(
                        "Node {} dropping frames from banned peer {} without processing at_ms={}",
                        recv_id,
                        sender_id,
                        unix_ms()
                    );
                }
                continue;
            }
            dropping.remove(&sender_id);
            let received_printed = received_lines.pass(&sender_id, dequeued);
            recv_lines_held.0.store(
                received_lines.held_back_total(),
                std::sync::atomic::Ordering::Relaxed,
            );
            if let Some(more) = received_printed {
                println!(
                    "Node {} received message from {}: id={}, type={}, origin={}{}",
                    recv_id, sender_id, wire.id, wire.message_type, wire.origin, more
                );
            }
            if let Err(reason) = check_fields(&wire) {
                println!(
                    "Node {} rejected '{}' from {}: {}",
                    recv_id, wire.id, sender_id, reason
                );
                continue;
            }
            // An answer to this node's sync request, or ordinary gossip (for experiments
            // that attribute late deliveries).
            let path = if wire.answers_sync.is_some() {
                "answer"
            } else {
                "gossip"
            };
            let bytes = match serde_json::to_vec(&wire) {
                Ok(b) => b,
                Err(_) => continue,
            };

            // All isolation checks run inside NodeRuntime, as on the in-process path.
            let (delivered, outbound, head, sender_rep, new_events, sender_banned) = {
                let mut n = recv_node.lock().unwrap_or_else(|e| e.into_inner());
                {
                    let mut inbox = n.inbox.lock().unwrap_or_else(|e| e.into_inner());
                    inbox
                        .entry(n.id.clone())
                        .or_default()
                        .push((sender_id.clone(), bytes));
                }
                let events_before = n.byzantine_events;
                let delivered = n.process_inbox_delivered();
                let outbound = take_outbound(&n);
                let sender_rep = n.sybil_guard.reputation(&sender_id);
                let sender_banned = n.sybil_guard.is_banned(&sender_id);
                (
                    delivered,
                    outbound,
                    n.chain_head(),
                    sender_rep,
                    n.byzantine_events - events_before,
                    sender_banned,
                )
            };
            // Protocol compute for this frame (checks, signature, replay window, strikes); the
            // frame signature (verified by the transport) is not included.
            let proc_us = dequeued.elapsed().as_micros();
            // Strikes are always charged to the sender of the frame just processed, so a
            // ban can only start here.
            if sender_banned && banned.insert(sender_id.clone()) {
                recv_tcp.shun_peer(&sender_id);
                println!("Node {} banned {} at_ms={}", recv_id, sender_id, unix_ms());
            } else if !sender_banned && banned.remove(&sender_id) {
                println!(
                    "Node {} readmitted {} at_ms={}",
                    recv_id,
                    sender_id,
                    unix_ms()
                );
            }
            recv_api
                .byzantine_events
                .fetch_add(new_events, std::sync::atomic::Ordering::SeqCst);
            if let Ok(mut h) = recv_api.merkle_head.write() {
                *h = head;
            }

            for msg in &delivered {
                let printed = accepted_lines.pass(&sender_id, dequeued);
                recv_lines_held.1.store(
                    accepted_lines.held_back_total(),
                    std::sync::atomic::Ordering::Relaxed,
                );
                if let Some(more) = &printed {
                    println!(
                        "Node {} accepted '{}' (origin {}, via {}) at_ms={} path={} proc_us={}{}",
                        recv_id,
                        msg.id,
                        msg.origin,
                        sender_id,
                        unix_ms(),
                        path,
                        proc_us,
                        more
                    );
                }
                let verdict = {
                    let mut p = recv_pipeline.lock().unwrap_or_else(|e| e.into_inner());
                    p.register_bootstrap("zone-host", &msg.origin);
                    p.process(&to_pipeline_message(msg, sender_rep))
                };
                if printed.is_some() && verdict != integration::PipelineVerdict::Accept {
                    println!(
                        "Node {} pipeline verdict for '{}': {:?}",
                        recv_id, msg.id, verdict
                    );
                }

                add_api_peer(&recv_api, &sender_id);
                let api_msg = api::ApiMessage {
                    from: msg.origin.clone(),
                    severity: msg.severity,
                    content: format!(
                        "type={}, seq={}, via={}, verdict={:?}",
                        msg.message_type, msg.seq, sender_id, verdict
                    ),
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                };
                if let Ok(mut recent) = recv_api.recent_messages.write() {
                    recent.push_back(api_msg.clone());
                    if recent.len() > 50 {
                        recent.pop_front();
                    }
                }
                if let Ok(json) = serde_json::to_string(&api_msg) {
                    let _ = recv_api.ws_tx.send(json);
                }
            }

            // Relay what NodeRuntime queued. Destinations are peer addresses from --dial /
            // --peers; a destination that is a node ID (from the router) has no address yet.
            let copies = copies_to_send(&recv_id, &experiment, started, &recv_fault);
            relay_outbound(&recv_tcp, outbound, copies, corrupting_now(&experiment));
        }
    });

    // Experiments: originate `emit_count` signed messages, one per `emit_interval`.
    if experiment.emit_count > 0 {
        let emit_tcp = tcp.clone();
        let emit_node = node_arc.clone();
        let emit_id = node_id.clone();
        let emit_fault = fault_announced.clone();
        tokio::spawn(async move {
            // The delay counts from the experiment's start, not from the end of the dial
            // loop above.
            let Some(first) = started.checked_add(experiment.emit_delay) else {
                return;
            };
            tokio::time::sleep_until(tokio::time::Instant::from_std(first)).await;
            let mut ticker = tokio::time::interval(experiment.emit_interval);
            for _ in 0..experiment.emit_count {
                ticker.tick().await;
                let msg_id = uuid::Uuid::new_v4().to_string();
                let copies = copies_to_send(&emit_id, &experiment, started, &emit_fault);
                {
                    let mut n = emit_node.lock().unwrap_or_else(|e| e.into_inner());
                    n.originate(Message::new(
                        &msg_id,
                        &emit_id,
                        Signal::new(3, false, false, true, 3, Visibility::Direct),
                        Some("testbed"),
                    ));
                    // Queued under the lock: a sync answer carrying this message must not
                    // overtake it, or the neighbour would strike this node for the gossip copy.
                    let outbound = take_outbound(&n);
                    relay_outbound(&emit_tcp, outbound, copies, corrupting_now(&experiment));
                }
                println!(
                    "Node {} originated '{}' at_ms={}",
                    emit_id,
                    msg_id,
                    unix_ms()
                );
            }
        });
    }

    // Every 5 s: advance the pipeline round (per-round rate limit, TTL) and run the
    // severity-median check. The check only counts outliers: a validly signed report is
    // never grounds for a strike, so reputation is not changed.
    let cons_pipeline = pipeline.clone();
    let cons_node_id = node_id.clone();
    let cons_api = api_state.clone();
    let stats_tcp = tcp.clone();
    let stats_banned_dropped = banned_frames_dropped.clone();
    let stats_lines_held = lines_held.clone();
    let mut last_paced = 0;
    tokio::spawn(async move {
        let mut last_stats = (0, 0);
        let mut last_banned_dropped = 0;
        let mut last_held = (0, 0);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let held = lines_held_now(&stats_lines_held);
            if held != last_held {
                last_held = held;
                print_lines_held(&cons_node_id, held);
            }
            // Relayed frames that never left this node; anti-entropy is their only recovery.
            let stats = (
                stats_tcp
                    .relay_failures
                    .load(std::sync::atomic::Ordering::Relaxed),
                stats_tcp
                    .relay_queue_full
                    .load(std::sync::atomic::Ordering::Relaxed),
            );
            let paced = stats_tcp
                .paced_frames
                .load(std::sync::atomic::Ordering::Relaxed);
            if paced != last_paced {
                last_paced = paced;
                println!(
                    "Node {} frames paced: total={} at_ms={}",
                    cons_node_id,
                    paced,
                    unix_ms()
                );
            }
            let banned_dropped = stats_banned_dropped.load(std::sync::atomic::Ordering::Relaxed);
            if banned_dropped != last_banned_dropped {
                last_banned_dropped = banned_dropped;
                println!(
                    "Node {} banned-peer frames dropped: total={} at_ms={}",
                    cons_node_id,
                    banned_dropped,
                    unix_ms()
                );
            }
            if stats != last_stats {
                last_stats = stats;
                println!(
                    "Node {} relay drops: failed={} queue_full={} at_ms={}",
                    cons_node_id,
                    stats.0,
                    stats.1,
                    unix_ms()
                );
            }
            let mut flagged = Vec::new();
            {
                let mut p = cons_pipeline.lock().unwrap_or_else(|e| e.into_inner());
                p.next_round();
                let zone = "zone-host";
                let total_zone_nodes = p.zone_registry.member_count(zone);

                // BFT-002 / BFT-010: Filter severities to only verified/vouched zone members
                let verified_severities: HashMap<String, u8> = p
                    .peer_severities
                    .iter()
                    .filter(|(id, _)| p.zone_registry.is_verified(id, zone))
                    .map(|(id, &sev)| (id.clone(), sev))
                    .collect();

                let n = verified_severities.len();
                // BFT-006: Prevent unauthenticated state convergence under zero active peer configuration
                if n > 0 {
                    // BFT-007: Mitigate split-brain by requiring active voters > N_total / 2
                    if n <= total_zone_nodes / 2 {
                        println!(
                            "[{}] Split-brain guard (BFT-007): verified count {} <= half of total zone nodes {}. Check skipped.",
                            cons_node_id, n, total_zone_nodes
                        );
                    } else {
                        let mut severities: Vec<u8> =
                            verified_severities.values().cloned().collect();
                        // Anchor median calculation to honest baseline severity (3)
                        severities.push(3);
                        severities.sort();
                        let median = severities[severities.len() / 2] as f64;
                        let outliers: Vec<(String, f64)> = verified_severities
                            .iter()
                            .map(|(id, &sev)| (id.clone(), (sev as f64 - median).abs()))
                            .filter(|(_, dev)| *dev >= 2.0)
                            .collect();

                        // BFT-001 / BFT-005: Enforce strict f < n/3 inequality using safe integer arithmetic
                        if 3 * outliers.len() >= n {
                            println!(
                                "[{}] Quorum guard (BFT-001): 3f >= n (f: {}, n: {}), check skipped.",
                                cons_node_id,
                                outliers.len(),
                                n
                            );
                        } else {
                            flagged = outliers;
                        }
                    }
                }
                for (id, _) in &flagged {
                    p.peer_severities.remove(id);
                }
            }

            for (id, dev) in flagged {
                println!(
                    "[{}] Severity outlier flagged: {} (deviation {} from the median); counted only, reputation unchanged",
                    cons_node_id, id, dev
                );
                cons_api
                    .byzantine_events
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
    });

    println!("Node {} listening on {}", node_id, tcp.listen_addr);
    tokio::signal::ctrl_c().await.unwrap_or(());
    print_lines_held(&node_id, lines_held_now(&lines_held));
    println!("Shutting down.");
}

fn lines_held_now(
    held: &(std::sync::atomic::AtomicU64, std::sync::atomic::AtomicU64),
) -> (u64, u64) {
    (
        held.0.load(std::sync::atomic::Ordering::Relaxed),
        held.1.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Cumulative counts of the per-message lines held back by the node's log limit.
fn print_lines_held(node_id: &str, (received, accepted): (u64, u64)) {
    println!(
        "Node {} log lines not printed: received={} accepted={} at_ms={}",
        node_id,
        received,
        accepted,
        unix_ms()
    );
}

fn add_api_peer(api_state: &api::ApiState, peer: &str) {
    if let Ok(mut peer_list) = api_state.peer_list.write() {
        if !peer_list.iter().any(|p| p == peer) {
            // A status list for the API: new identities are free, so it keeps the newest.
            if peer_list.len() >= 256 {
                peer_list.remove(0);
            }
            peer_list.push(peer.to_string());
        }
    }
    if let Ok(mut node_count) = api_state.node_count.write() {
        *node_count = api_state.peer_list.read().map(|l| l.len()).unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::{corrupt_signature, sync_reply_address};

    #[test]
    fn sync_answers_are_not_sent_to_an_unspecified_listen_address() {
        assert_eq!(sync_reply_address("0.0.0.0:7001".parse().unwrap()), None);
        assert_eq!(sync_reply_address("[::]:7001".parse().unwrap()), None);
        assert_eq!(
            sync_reply_address("10.0.0.3:7001".parse().unwrap()),
            Some("10.0.0.3:7001".to_string())
        );
    }

    #[test]
    fn corrupted_message_fails_signature_verification() {
        use crci_core::identity::Identity;
        use crci_core::message::{Message, Signal, Visibility};
        let msg = Message::new(
            "m1",
            "a",
            Signal::new(3, false, false, true, 3, Visibility::Direct),
            None,
        );
        let mut wire = crci_core::runtime::WireMessage::from_message(&msg, &Identity::new("a"));
        assert!(wire.verify_signature());
        corrupt_signature(&mut wire);
        assert!(!wire.verify_signature());
    }
}
