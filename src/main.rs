mod identity;
mod message;
mod node;
mod network;

use message::{Message, Signal, Visibility};
use node::Node;
use network::Network;

fn main() {
    let mut net = Network::new();

    net.add_node(Node::new("node-001", true,  "zone-a"));
    net.add_node(Node::new("node-002", true,  "zone-a"));
    net.add_node(Node::new("node-003", false, "zone-a")); // Byzantine
    net.add_node(Node::new("node-004", true,  "zone-a")); // will panic button
    net.add_node(Node::new("node-005", true,  "zone-a")); // indoor
    net.add_node(Node::new("node-006", true,  "zone-a"));
    net.add_node(Node::new("node-007", true,  "zone-b"));
    net.add_node(Node::new("node-008", true,  "zone-b"));
    net.add_node(Node::new("node-009", true,  "zone-b"));

    net.connect("node-001", "node-002");
    net.connect("node-002", "node-003");
    net.connect("node-003", "node-004");
    net.connect("node-004", "node-005");
    net.connect("node-005", "node-006");
    net.connect("node-007", "node-008");
    net.connect("node-008", "node-009");

    println!("=== CRCI Network — Initial State ===");
    net.print_all();

    // ── Round 1 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 1 ===");
    net.gossip("node-001", Message::new("msg-001", "node-001", Signal::new(4, true, false, true, 4, Visibility::Direct), Some("flooding on main road")));
    net.gossip("node-002", Message::new("msg-002", "node-002", Signal::new(4, true, false, true, 5, Visibility::Direct), None));
    net.gossip("node-003", Message::new("msg-003", "node-003", Signal::new(1, false, true, false, 5, Visibility::Direct), Some("all clear here")));
    net.gossip("node-004", Message::rescue("msg-004", "node-004", Some("help trapped under debris")));
    net.gossip("node-005", Message::new("msg-005", "node-005", Signal::new(1, false, true, false, 1, Visibility::Unknown), Some("indoors cannot see outside")));
    net.gossip("node-006", Message::new("msg-006", "node-006", Signal::new(5, true, false, true, 5, Visibility::Direct), Some("bridge collapsed")));
    net.gossip("node-007", Message::new("msg-007", "node-007", Signal::new(1, false, true, true, 5, Visibility::Direct), None));
    net.gossip("node-008", Message::new("msg-008", "node-008", Signal::new(2, false, true, true, 4, Visibility::Direct), None));
    net.gossip("node-009", Message::new("msg-009", "node-009", Signal::new(1, false, true, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 1 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 1 ===");
    net.print_all();
    net.new_round();

    // ── Round 2 — node-004 goes offline ──────────────────────────────────────
    println!("\n=== Round 2 — node-004 battery dies ===");
    if let Some(n) = net.nodes.get_mut("node-004") { n.go_offline(); }

    net.gossip("node-001", Message::new("msg-010", "node-001", Signal::new(5, true, false, true, 5, Visibility::Direct), Some("situation critical")));
    net.gossip("node-002", Message::new("msg-011", "node-002", Signal::new(5, true, false, true, 5, Visibility::Direct), None));
    net.gossip("node-003", Message::new("msg-012", "node-003", Signal::new(1, false, true, false, 5, Visibility::Direct), None));
    net.gossip("node-006", Message::new("msg-013", "node-006", Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 2 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 2 ===");
    net.print_all();
    net.new_round();

    // ── Round 3 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 3 ===");
    net.gossip("node-001", Message::new("msg-014", "node-001", Signal::new(5, true, false, true, 5, Visibility::Direct), Some("roads completely blocked")));
    net.gossip("node-002", Message::new("msg-015", "node-002", Signal::new(5, true, false, true, 5, Visibility::Direct), None));
    net.gossip("node-003", Message::new("msg-016", "node-003", Signal::new(1, false, true, false, 5, Visibility::Direct), None));
    net.gossip("node-006", Message::new("msg-017", "node-006", Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 3 ===");
    net.run_consensus();
    println!("\n=== Final Network State ===");
    net.print_all();
}