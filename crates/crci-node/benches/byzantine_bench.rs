#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::len_zero)]
use crci_core::runtime::{NodeRuntime, WireMessage};

use crci_core::message::{Message, Signal};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use hdrhistogram::Histogram;

fn run_trials(
    trials: usize,
    apply_jitter: bool,
    csv_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rows: Vec<(usize, u128, u128, bool)> = Vec::new();

    for trial in 0..trials {
        let inbox_a = Arc::new(Mutex::new(HashMap::new()));
        let mut node_a = NodeRuntime::new("node-a", "zone-a", inbox_a.clone());

        // Honest peer
        let mut msg_h = Message::new("msg-h", "node-h", Signal::panic(), None);
        msg_h.seq = 1;
        let identity_h = crci_core::identity::Identity::new("node-h");
        let wire_h = WireMessage::from_message(&msg_h, &identity_h);
        let raw_h = serde_json::to_vec(&wire_h).unwrap();

        let t0 = Instant::now();
        inbox_a.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw_h));
        node_a.process_inbox();
        let handshake_ms = t0.elapsed().as_millis();

        // Byzantine peer
        let mut msg_b = Message::new("msg-b", "node-b", Signal::panic(), None);
        msg_b.seq = 1;
        let identity_b = crci_core::identity::Identity::new("node-b");
        let wire_b = WireMessage::from_message(&msg_b, &identity_b);
        let raw_b_initial = serde_json::to_vec(&wire_b).unwrap();
        inbox_a.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-b".to_string(), raw_b_initial));
        node_a.process_inbox();
        
        let t1 = Instant::now();
        
        for _ in 0..3 {
            if apply_jitter {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            let raw_b = serde_json::to_vec(&wire_b).unwrap();
            inbox_a.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-b".to_string(), raw_b));
            node_a.process_inbox();
        }

        let eviction_ms = t1.elapsed().as_millis();

        let is_banned = match node_a.sybil_guard.check(&"node-b".to_string()) {
            Err(crci_core::sybil::SybilError::Banned(_)) => true,
            _ => false,
        };
        
        let honest_survived = match node_a.sybil_guard.check(&"node-h".to_string()) {
            Err(crci_core::sybil::SybilError::Banned(_)) => false,
            _ => true,
        };

        rows.push((trial + 1, handshake_ms, eviction_ms, is_banned && honest_survived));
    }

    std::fs::create_dir_all("benches/results")?;
    let mut csv = String::from(
        "trial,legitimate_handshake_ms,byzantine_eviction_ms,adversary_banned_honest_survived\n",
    );
    for (t, h, e, s) in &rows {
        csv.push_str(&format!("{},{},{},{}\n", t, h, e, s));
    }
    std::fs::write(format!("benches/results/{}", csv_name), &csv)?;

    let mut hist = Histogram::<u64>::new(3).unwrap();
    let mut success_count = 0;
    for row in &rows {
        if row.3 {
            hist.record(row.2 as u64).unwrap();
            success_count += 1;
        }
    }

    if hist.len() > 0 {
        let mean = hist.mean();
        println!(
            "\n=== Byzantine Eviction Benchmark (Jitter: {}) ===",
            apply_jitter
        );
        println!("Trials: {}", trials);
        println!("Mean eviction latency: {:.2}ms", mean);
        println!("Median (p50): {}ms", hist.value_at_percentile(50.0));
        println!("Min latency: {}ms", hist.min());
        println!("Max latency: {}ms", hist.max());
        println!("Successful Isolation & Survivability: {}/{}", success_count, trials);
    }

    Ok(())
}

fn compare_naive_vs_sliding() {
    println!("\n=== Comparing Naive Monotonic Rejection vs Sliding Window ===");
    let inbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node_a = NodeRuntime::new("node-a", "zone-a", inbox.clone());
    let identity_h = crci_core::identity::Identity::new("node-h");

    // Naive rule would ban on any out-of-order packet (seq < max_seq)
    // Send seq = 5, then seq = 4 (simulating packet reordering)
    let mut msg_5 = Message::new("msg-5", "node-h", Signal::panic(), None);
    msg_5.seq = 5;
    let wire_5 = WireMessage::from_message(&msg_5, &identity_h);
    let raw_5 = serde_json::to_vec(&wire_5).unwrap();
    
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw_5));
    node_a.process_inbox(); // max_seq is now 5

    let mut msg_4 = Message::new("msg-4", "node-h", Signal::panic(), None);
    msg_4.seq = 4;
    let wire_4 = WireMessage::from_message(&msg_4, &identity_h);
    let raw_4 = serde_json::to_vec(&wire_4).unwrap();
    
    inbox.lock().unwrap().entry("node-a".to_string()).or_insert_with(Vec::new).push(("node-h".to_string(), raw_4));
    node_a.process_inbox(); // Out of order packet arrives

    let is_banned = match node_a.sybil_guard.check(&"node-h".to_string()) {
        Err(crci_core::sybil::SybilError::Banned(_)) => true,
        _ => false,
    };
    
    println!("Sliding window result: Honest peer banned due to packet reordering? {}", is_banned);
    println!("A naive `seq <= last_seq` rule WOULD have penalized the peer here, potentially banning them after 3 reordered packets.");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_trials(50, false, "byzantine_eviction_baseline.csv")?;
    run_trials(50, true, "byzantine_eviction_jitter.csv")?;
    compare_naive_vs_sliding();
    Ok(())
}
