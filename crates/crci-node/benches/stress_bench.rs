use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use crci_core::message::{Message, Signal, Visibility};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn main() {
    println!("=== Phase 1 Stress Tests ===");

    // Test a) Gossip duplicate relay
    println!("\n[Test A] Gossip Duplicate Relay");
    let inbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node_a = NodeRuntime::new("node-a", "zone-a", inbox.clone());
    
    // Original message from node-c
    let mut msg_c = Message::new("msg-c-1", "node-c", Signal::panic(), None);
    msg_c.seq = 100;
    let ident_c = crci_core::identity::Identity::new("node-c");
    let wire_c = WireMessage::from_message(&msg_c, &ident_c);
    let raw_c = serde_json::to_vec(&wire_c).unwrap();

    // node-b1 relays it to node-a
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-b1".to_string(), raw_c.clone()));
    node_a.process_inbox();
    let rep_c1 = node_a.reputation_floor();
    println!("After path 1 (node-b1), node-c rep: {}", node_a.sybil_guard.check(&"node-c".to_string()).is_ok());

    // node-b2 relays the EXACT SAME MESSAGE to node-a
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-b2".to_string(), raw_c.clone()));
    node_a.process_inbox();
    
    // Check if node-c was penalized
    // Note: since we pass raw_c into inbox directly, there is no "transport" peer. It looks like it came from itself or anonymous in SimTransport?
    // Actually, process_inbox loops through `inbox` values, decoding `WireMessage`.
    // Wait, `process_inbox` in SimTransport gets messages from `inbox.remove(&self.id)`. The messages are just `Vec<u8>`. 
    // It extracts `wire.origin` and uses it for `sybil_guard`.
    match node_a.sybil_guard.check(&"node-c".to_string()) {
        Ok(_) => println!("Result: node-c is NOT banned. Duplicate relay is ignored gracefully."),
        Err(e) => println!("Result: node-c IS BANNED. Error: {}", e),
    }

    // Test b) Partition-and-heal
    println!("\n[Test B] Partition-and-Heal (Stale legitimate message)");
    let mut msg_c_stale = Message::new("msg-c-stale", "node-c", Signal::panic(), None);
    msg_c_stale.seq = 10; // Old sequence number (current is 100)
    let wire_c_stale = WireMessage::from_message(&msg_c_stale, &ident_c);
    let raw_c_stale = serde_json::to_vec(&wire_c_stale).unwrap();

    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-b3".to_string(), raw_c_stale));
    node_a.process_inbox();

    // Check penalty on node-c
    match node_a.sybil_guard.check(&"node-c".to_string()) {
        Ok(_) => println!("Result: node-c is NOT banned. Stale message absorbed safely."),
        Err(e) => println!("Result: node-c IS BANNED! Error: {}", e),
    }

    // Check penalty on relaying peers
    let rep_b2 = node_a.sybil_guard.reputation(&"node-b2".to_string());
    let rep_b3 = node_a.sybil_guard.reputation(&"node-b3".to_string());
    let rep_c = node_a.sybil_guard.reputation(&"node-c".to_string());
    
    println!("Final reputations:");
    println!("node-c (Author): {:.2}", rep_c);
    println!("node-b2 (Relayed duplicate): {:.2}", rep_b2);
    println!("node-b3 (Relayed stale): {:.2}", rep_b3);
    
    if rep_c == 0.5 && rep_b2 < 0.5 && rep_b3 < 0.5 {
        println!("SUCCESS: Dual-layer trust correctly penalizes relaying neighbors instead of claimed authors!");
    } else {
        println!("FAILURE: Dual-layer trust failed.");
    }
}
