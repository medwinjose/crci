use crci_core::{
    identity::Identity,
    message::{Message, Signal},
    runtime::{NodeRuntime, WireMessage},
};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

fn main() {
    println!("Evaluating reputation recovery vs attacker pacing...\n");
    
    // Condition 1: Rapid Attack (Burst of 3 replays)
    let is_banned_rapid = test_evasion(0, 0);
    println!("Condition 1 (Rapid Attack, 0 recovery pkts): Banned? {}", is_banned_rapid);

    // Condition 2: Evasion Pacing (300 recovery pkts between replays)
    let is_banned_paced = test_evasion(300, 10);
    println!("Condition 2 (Paced Attack, 300 recovery pkts): Banned? {}", is_banned_paced);
    
    println!("\n=== RECOVERY ABLATION RESULTS ===");
    println!("Condition 1 (Rapid Attack, 0 recovery pkts): Survival: {}", !is_banned_rapid);
    println!("Condition 2 (Paced Attack, 300 recovery pkts): Survival: {}", !is_banned_paced);
}

fn test_evasion(recovery_packets: usize, delay_ms: u64) -> bool {
    let inbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("node-a", "zone-a", inbox.clone());
    node.replay_filter.window_size = 0; // Strict monotonic so replays are always rejected
    
    let attacker = Identity::new("node-h");
    
    let mut seq = 1;
    
    // Attack round 1
    inject_replay(&mut node, &inbox, &attacker, seq, seq); // 1 is <= 1 (replay)
    seq += 1;
    
    for _ in 0..recovery_packets {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        inject_honest(&mut node, &inbox, &attacker, seq);
        seq += 1;
    }
    
    // Attack round 2
    inject_replay(&mut node, &inbox, &attacker, seq, seq-1);
    seq += 1;
    
    for _ in 0..recovery_packets {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        inject_honest(&mut node, &inbox, &attacker, seq);
        seq += 1;
    }
    
    // Attack round 3
    inject_replay(&mut node, &inbox, &attacker, seq, seq-1);
    
    match node.sybil_guard.check(&"node-h".to_string()) {
        Err(crci_core::sybil::SybilError::Banned(_)) => true,
        _ => false,
    }
}

fn inject_replay(node: &mut NodeRuntime, inbox: &Arc<Mutex<HashMap<String, Vec<(String, Vec<u8>)>>>>, attacker: &Identity, max_seq: u64, replay_seq: u64) {
    let mut msg_max = Message::new(&format!("msg-max-{}", max_seq), "node-h", Signal::panic(), None);
    msg_max.seq = max_seq;
    let wire_max = WireMessage::from_message(&msg_max, attacker);
    let raw_max = serde_json::to_vec(&wire_max).unwrap();
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw_max));
    node.process_inbox();
    node.sig_verifications_count.clear();
    
    let mut msg_reordered = Message::new(&format!("msg-reordered-{}", replay_seq), "node-h", Signal::panic(), None);
    msg_reordered.seq = replay_seq;
    let wire_reordered = WireMessage::from_message(&msg_reordered, attacker);
    let raw_reordered = serde_json::to_vec(&wire_reordered).unwrap();
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw_reordered));
    node.process_inbox();
    node.sig_verifications_count.clear();
}

fn inject_honest(node: &mut NodeRuntime, inbox: &Arc<Mutex<HashMap<String, Vec<(String, Vec<u8>)>>>>, attacker: &Identity, seq: u64) {
    let mut msg = Message::new(&format!("msg-honest-{}", seq), "node-h", Signal::panic(), None);
    msg.seq = seq;
    let wire = WireMessage::from_message(&msg, attacker);
    let raw = serde_json::to_vec(&wire).unwrap();
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw));
    node.process_inbox();
    node.sig_verifications_count.clear();
}
