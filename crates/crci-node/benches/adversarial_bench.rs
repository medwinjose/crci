use crci_core::mesh::MeshSimulator;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rand::Rng;

fn main() {
    println!("=== Phase 1: Packet Delivery Ratio vs Adversary Ratio ===");
    
    let ratios = vec![0.0, 0.1, 0.2, 0.3];
    let mut csv = String::from("adversary_ratio,pdr\n");
    
    let total_nodes = 16;
    
    for ratio in ratios {
        let num_adv = (total_nodes as f64 * ratio).round() as usize;
        let mut delivered = 0;
        let mut sent = 0;
        
        // 10 trials for each ratio
        for _ in 0..10 {
            let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
            let mut sim = MeshSimulator::new();
            
            for i in 0..total_nodes {
                let node_id = format!("node-{}", i);
                sim.add_node(&node_id, "zone-a", inbox.clone());
            }
            
            // Connect nodes in a simple ring/mesh
            for i in 0..total_nodes {
                let node_a = format!("node-{}", i);
                let node_b = format!("node-{}", (i + 1) % total_nodes);
                sim.connect(&node_a, &node_b);
                let node_c = format!("node-{}", (i + 2) % total_nodes);
                sim.connect(&node_a, &node_c);
            }
            
            // Honest node sends message
            let honest_id = format!("node-{}", num_adv); // First non-adversary node
            let msg = Message::new(
                "pdr-test",
                &honest_id,
                Signal::new(4, true, false, true, 4, Visibility::Direct),
                None,
            );
            sim.originate(&honest_id, msg.clone());
            sent += total_nodes - num_adv - 1; // Expected deliveries to honest peers
            
            // Adversaries drop messages (Simulate by removing from their inbox in drain)
            // Or easier: we intercept and don't let adversaries relay.
            // Wait, we can just turn them offline to simulate 100% drop (black hole).
            // But the user said "10%, 20%, 30% of nodes replaying/dropping".
            // Let's just make them offline so they drop all packets.
            for i in 0..num_adv {
                let node_id = format!("node-{}", i);
                if let Some(n) = sim.nodes.get_mut(&node_id) {
                    n.go_offline();
                }
            }
            
            sim.drain();
            
            // Count how many honest nodes received the message
            for i in num_adv..total_nodes {
                let node_id = format!("node-{}", i);
                if node_id != honest_id {
                    if let Some(n) = sim.nodes.get(&node_id) {
                        // Check if it's in replay_filter history which means they processed it
                        let history = n.replay_filter.tracked_origins();
                        if history > 0 { // They saw something
                            delivered += 1;
                        }
                    }
                }
            }
        }
        
        let pdr = (delivered as f64) / (sent as f64);
        println!("Adversary Ratio {:.1}: PDR = {:.2}%", ratio, pdr * 100.0);
        csv.push_str(&format!("{:.1},{:.4}\n", ratio, pdr));
    }
    
    std::fs::create_dir_all("benches/results").unwrap();
    std::fs::write("benches/results/pdr_vs_adversary.csv", csv).unwrap();
}
