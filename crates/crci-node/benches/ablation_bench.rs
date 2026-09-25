#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::single_char_add_str)]
use crci_core::runtime::{NodeRuntime, WireMessage};

use crci_core::message::{Message, Signal};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn test_reorder(window: u64, depth: u64) -> bool {
    let inbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("node-a", "zone-a", inbox.clone());
    node.replay_filter.window_size = window;
    let identity = crci_core::identity::Identity::new("node-h");

    // send 3 pairs of packets
    let mut current_seq = 1;
    for _ in 1..=3 {
        let max_seq = current_seq + depth;
        let reordered_seq = current_seq;

        let mut msg_max = Message::new(
            &format!("msg-max-{}", max_seq),
            "node-h",
            Signal::panic(),
            None,
        );
        msg_max.seq = max_seq;
        let wire_max = WireMessage::from_message(&msg_max, &identity);
        let raw_max = serde_json::to_vec(&wire_max).unwrap();
        inbox
            .lock()
            .unwrap()
            .entry("node-a".to_string())
            .or_insert_with(Vec::new)
            .push(("node-h".to_string(), raw_max));
        node.process_inbox();

        let mut msg_reordered = Message::new(
            &format!("msg-reordered-{}", reordered_seq),
            "node-h",
            Signal::panic(),
            None,
        );
        msg_reordered.seq = reordered_seq;
        let wire_reordered = WireMessage::from_message(&msg_reordered, &identity);
        let raw_reordered = serde_json::to_vec(&wire_reordered).unwrap();
        inbox
            .lock()
            .unwrap()
            .entry("node-a".to_string())
            .or_insert_with(Vec::new)
            .push(("node-h".to_string(), raw_reordered));
        node.process_inbox();
        node.sig_verifications_count.clear();

        current_seq += depth + 1;
    }

    let is_banned = match node.sybil_guard.check(&"node-h".to_string()) {
        Err(crci_core::sybil::SybilError::Banned(_)) => true,
        _ => false,
    };

    !is_banned // Return true if survived
}

fn main() {
    let windows = [8, 16, 32, 64];
    let depths = [1, 2, 4, 8, 16, 32];

    let mut out = String::new();
    out.push_str("| Reorder depth | W=8 | W=16 | W=32 | W=64 |\n");
    out.push_str("| :--- | :--- | :--- | :--- | :--- |\n");

    for &d in &depths {
        out.push_str(&format!("| {} |", d));
        for &w in &windows {
            let survived = test_reorder(w, d);
            if survived {
                out.push_str(" survival |");
            } else {
                out.push_str(" false-positive ban |");
            }
        }
        out.push_str("\n");
    }

    println!("\n\n\n=== ABLATION RESULTS ===");
    println!("{}", out);
}
