use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::mesh::MeshSimulator;
use crate::message::{Message, Signal, Visibility};
use crate::transport::SharedInbox;

// ─── Test 1: Large Network ────────────────────────────────────────────────────

pub fn test_large_network() {
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║  STRESS TEST 1 — Large Network (30 nodes)   ║");
    println!("╚══════════════════════════════════════════════╝");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    for zone in &["zone-a", "zone-b", "zone-c"] {
        for i in 1..=10 {
            let id = format!("{}-node-{:02}", zone, i);
            sim.add_node(&id, zone, inbox.clone());
        }
    }

    for zone in &["zone-a", "zone-b", "zone-c"] {
        for i in 1..9usize {
            let a = format!("{}-node-{:02}", zone, i);
            let b = format!("{}-node-{:02}", zone, i + 1);
            sim.connect(&a, &b);
        }
    }

    println!("Network: 30 nodes across 3 zones, linear topology");

    for zone in &["zone-a", "zone-b", "zone-c"] {
        for i in 1..=10usize {
            let id = format!("{}-node-{:02}", zone, i);
            let is_byzantine = i == 1;
            let sev = if is_byzantine { 1 } else { 5 };
            let msg_id = format!("{}-msg-r1-{:02}", zone, i);
            sim.originate(&id, Message::new(
                &msg_id, &id,
                Signal::new(sev, !is_byzantine, is_byzantine, !is_byzantine, 5, Visibility::Direct),
                None,
            ));
        }
    }

    sim.drain();

    println!("\n--- Consensus ---");
    sim.run_consensus();

    let mut caught = 0;
    for zone in &["zone-a", "zone-b", "zone-c"] {
        let id = format!("{}-node-01", zone);
        if let Some(node) = sim.nodes.get(&id) {
            if node.reputation < 1.0 {
                caught += 1;
                println!("  ✅ [{}] penalised (rep: {:.2})", id, node.reputation);
            } else {
                println!("  ❌ [{}] not penalised (rep: {:.2})", id, node.reputation);
            }
        }
    }
    println!("\nResult: {}/3 Byzantine nodes penalised (full isolation requires 3 rounds)", caught);
}

// ─── Test 2: Sybil Attack ─────────────────────────────────────────────────────

pub fn test_sybil_attack() {
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║  STRESS TEST 2 — Sybil Attack (4 vs 3)      ║");
    println!("╚══════════════════════════════════════════════╝");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    for i in 1..=7usize {
        let id = format!("node-{:02}", i);
        sim.add_node(&id, "zone-attack", inbox.clone());
    }
    for i in 1..6usize {
        sim.connect(&format!("node-{:02}", i), &format!("node-{:02}", i + 1));
    }

    println!("Setup: nodes 01-04 Byzantine (sev 1), nodes 05-07 honest (sev 5)");
    println!("Byzantine nodes outnumber honest 4:3 — majority is lying\n");

    for i in 1..=4usize {
        let id = format!("node-{:02}", i);
        let msg_id = format!("msg-byz-{:02}", i);
        sim.originate(&id, Message::new(
            &msg_id, &id,
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            Some("all clear"),
        ));
    }

    for i in 5..=7usize {
        let id = format!("node-{:02}", i);
        let msg_id = format!("msg-hon-{:02}", i);
        sim.originate(&id, Message::new(
            &msg_id, &id,
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("critical"),
        ));
    }

    sim.drain();

    println!("--- Consensus (Byzantine majority scenario) ---");
    sim.run_consensus();

    println!("\nAnalysis: When Byzantine nodes hold majority, the median shifts");
    println!("to their false value. This is the known limit of majority-vote");
    println!("Byzantine tolerance — requires >2/3 honest nodes to hold.");
    println!("CRCI mitigation: flag zones where rescue requests contradict");
    println!("the consensus (honest nodes filed rescues, median says clear).");
}

// ─── Test 3: Signature Forgery ────────────────────────────────────────────────

pub fn test_signature_forgery() {
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║  STRESS TEST 3 — Signature Forgery Attempt  ║");
    println!("╚══════════════════════════════════════════════╝");

    use crate::runtime::WireMessage;
    use crate::identity::Identity;
    use crate::transport::Transport;

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    sim.add_node("node-001", "zone-a", inbox.clone());
    sim.add_node("node-002", "zone-a", inbox.clone());
    sim.add_node("node-attacker", "zone-a", inbox.clone());
    sim.connect("node-001", "node-002");
    sim.connect("node-002", "node-attacker");

    println!("Setup: attacker tries to impersonate node-001\n");

    let attacker_identity = Identity::new("attacker");
    let fake_payload = format!("{}:{}:{}", "msg-forged", "node-001", 1u8);
    let fake_sig = attacker_identity.sign(fake_payload.as_bytes());

    let forged = WireMessage {
        id: "msg-forged".to_string(),
        origin: "node-001".to_string(),
        origin_pubkey: attacker_identity.verifying_key.to_bytes().to_vec(),
        severity: 1,
        confidence: 5,
        needs_help: false,
        can_help_others: true,
        location_confirmed: false,
        visibility: "direct".to_string(),
        note: Some("FORGED: all clear".to_string()),
        message_type: "normal".to_string(),
        origin_active: true,
        signature_bytes: fake_sig.to_bytes().to_vec(),
        seq: 1,
    };

    let raw = serde_json::to_vec(&forged).unwrap();
    {
        let mut ib = inbox.lock().unwrap();
        ib.entry("node-002".to_string()).or_default().push(raw);
    }

    println!("Injecting forged message into node-002's inbox...");
    sim.drain();

    if let Some(node) = sim.nodes.get("node-002") {
        if node.observations.is_empty() {
            println!("\n  ✅ Forged message REJECTED — node-002 recorded 0 observations");
            println!("  ✅ Pubkey mismatch detected and dropped at receive");
        } else {
            println!("\n  ❌ Forged message ACCEPTED — security failure!");
        }
    }
}

// ─── Test 4: Network Partition ────────────────────────────────────────────────

pub fn test_network_partition() {
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║  STRESS TEST 4 — Network Partition           ║");
    println!("╚══════════════════════════════════════════════╝");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    sim.add_node("left-001", "zone-a", inbox.clone());
    sim.add_node("left-002", "zone-a", inbox.clone());
    sim.add_node("left-003", "zone-a", inbox.clone());
    sim.add_node("right-001", "zone-a", inbox.clone());
    sim.add_node("right-002", "zone-a", inbox.clone());
    sim.add_node("right-003", "zone-a", inbox.clone());

    sim.connect("left-001", "left-002");
    sim.connect("left-002", "left-003");
    sim.connect("right-001", "right-002");
    sim.connect("right-002", "right-003");

    println!("Partition active: left-001/002/003 isolated from right-001/002/003\n");

    sim.originate("left-001", Message::rescue(
        "msg-rescue-left", "left-001",
        Some("trapped, need help"),
    ));
    sim.drain();

    let right_has_rescue = sim.nodes.get("right-001")
        .map(|n| n.persistent_messages.contains_key("msg-rescue-left"))
        .unwrap_or(false);

    println!("Right partition received rescue during partition: {}", right_has_rescue);
    if !right_has_rescue {
        println!("  ✅ Partition correctly isolated — message did not cross");
    }

    println!("\nHealing partition: connecting left-003 ↔ right-001...");
    sim.connect("left-003", "right-001");
    sim.new_round();

    sim.originate("left-001", Message::rescue(
        "msg-rescue-left-2", "left-001",
        Some("still trapped, partition healed"),
    ));
    sim.drain();

    let right_has_rescue_after = sim.nodes.get("right-003")
        .map(|n| !n.persistent_messages.is_empty())
        .unwrap_or(false);

    if right_has_rescue_after {
        println!("  ✅ After healing — rescue reached right-003");
    } else {
        println!("  ⚠ Message did not propagate after healing");
    }
}