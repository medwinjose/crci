mod identity;
mod message;
mod node;
mod network;
mod transport;
mod runtime;
mod mesh;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use message::{Message, Signal, Visibility};
use mesh::MeshSimulator;

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

    sim.originate("node-001", Message::new("msg-001", "node-001",
        Signal::new(4, true, false, true, 4, Visibility::Direct),
        Some("flooding on main road")));

    sim.originate("node-002", Message::new("msg-002", "node-002",
        Signal::new(4, true, false, true, 5, Visibility::Direct), None));

    sim.originate("node-003", Message::new("msg-003", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct),
        Some("all clear here")));

    sim.originate("node-004", Message::rescue("msg-004", "node-004",
        Some("help trapped under debris")));

    sim.originate("node-005", Message::new("msg-005", "node-005",
        Signal::new(1, false, true, false, 1, Visibility::Unknown),
        Some("indoors cannot see outside")));

    sim.originate("node-006", Message::new("msg-006", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("bridge collapsed")));

    sim.originate("node-007", Message::new("msg-007", "node-007",
        Signal::new(1, false, true, true, 5, Visibility::Direct), None));

    sim.originate("node-008", Message::new("msg-008", "node-008",
        Signal::new(2, false, true, true, 4, Visibility::Direct), None));

    sim.originate("node-009", Message::new("msg-009", "node-009",
        Signal::new(1, false, true, true, 5, Visibility::Direct), None));

    println!("\n=== Round 1 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 1 ===");
    sim.run_consensus();

    println!("\n=== State After Round 1 ===");
    sim.print_all();
    sim.new_round();

    // ── Round 2 — node-004 dies ───────────────────────────────────────────
    println!("\n=== Round 2 — node-004 battery dies ===");
    if let Some(n) = sim.nodes.get_mut("node-004") { n.go_offline(); }

    sim.originate("node-001", Message::new("msg-010", "node-001",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("situation critical")));

    sim.originate("node-002", Message::new("msg-011", "node-002",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    sim.originate("node-003", Message::new("msg-012", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct), None));

    sim.originate("node-006", Message::new("msg-013", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Round 2 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 2 ===");
    sim.run_consensus();

    println!("\n=== State After Round 2 ===");
    sim.print_all();
    sim.new_round();

    // ── Round 3 ──────────────────────────────────────────────────────────────
    println!("\n=== Round 3 ===");

    sim.originate("node-001", Message::new("msg-014", "node-001",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("roads completely blocked")));

    sim.originate("node-002", Message::new("msg-015", "node-002",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    sim.originate("node-003", Message::new("msg-016", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct), None));

    sim.originate("node-006", Message::new("msg-017", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Round 3 — Mesh delivery ===");
    sim.drain();

    println!("\n=== Consensus — Round 3 ===");
    sim.run_consensus();

    println!("\n=== Final State ===");
    sim.print_all();
}