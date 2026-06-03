// ─── Mesh Simulator ───────────────────────────────────────────────────────────
// In real life: this doesn't exist. In the real system, the transport layer
// (Bluetooth/WiFi Direct/LoRa) handles delivery automatically based on
// physical proximity. Here we simulate that — the MeshSimulator knows
// which nodes are within range of each other and delivers bytes between them.
//
// Critically: the MeshSimulator knows NOTHING about messages. It only
// moves bytes. It doesn't understand what's in them. This is correct
// architecture — the transport layer is dumb, the nodes are smart.

use std::collections::{HashMap, HashSet};
use crate::runtime::NodeRuntime;
use crate::transport::SharedInbox;
use crate::message::Message;

pub struct MeshSimulator {
    pub nodes: HashMap<String, NodeRuntime>,
    pub declared_mce_zones: HashSet<String>,
}

impl MeshSimulator {
    pub fn new() -> MeshSimulator {
        MeshSimulator {
            nodes: HashMap::new(),
            declared_mce_zones: HashSet::new(),
        }
    }

    // Create a shared inbox and add a node to the mesh
    pub fn add_node(&mut self, id: &str, zone: &str, inbox: SharedInbox) {
        let node = NodeRuntime::new(id, zone, inbox);
        self.nodes.insert(id.to_string(), node);
    }

    // Wire two nodes as peers — bidirectional
    pub fn connect(&mut self, a: &str, b: &str) {
        let key_a = self.nodes.get(a)
            .map(|n| (n.id.clone(), n.identity.verifying_key.to_bytes().to_vec()));
        let key_b = self.nodes.get(b)
            .map(|n| (n.id.clone(), n.identity.verifying_key.to_bytes().to_vec()));

        if let Some(node) = self.nodes.get_mut(a) {
            node.add_peer(b);
            if let Some((id_b, kb)) = &key_b {
                node.known_keys.insert(id_b.clone(), kb.clone());
            }
        }

        if let Some(node) = self.nodes.get_mut(b) {
            node.add_peer(a);
            if let Some((id_a, ka)) = &key_a {
                node.known_keys.insert(id_a.clone(), ka.clone());
            }
        }
    }

    // A node originates a message
    pub fn originate(&mut self, node_id: &str, msg: Message) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.originate(msg);
        }
    }

    // Run one delivery pass — all nodes process their inboxes
    // In real life: this happens continuously on each phone.
    // Here we run it in passes to simulate time steps.
    pub fn deliver_pass(&mut self) {
        let ids: Vec<String> = self.nodes.keys().cloned().collect();
        for id in ids {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.process_inbox();
            }
        }
    }

    // Run multiple passes until no messages remain in flight
    pub fn drain(&mut self) {
        for _ in 0..10 {
            self.deliver_pass();
        }
    }

    // Run consensus across all nodes' collected observations
    pub fn run_consensus(&mut self) {
        // Gather all observations from all nodes, grouped by zone
        let mut by_zone: HashMap<String, Vec<(String, u8, u8, bool)>> = HashMap::new();

        for node in self.nodes.values() {
            for obs in &node.observations {
                by_zone.entry(node.zone.clone())
                    .or_default()
                    .push(obs.clone());
            }
        }

        // Deduplicate by origin node_id per zone
        let mut deduped_by_zone: HashMap<String, HashMap<String, (u8, u8, bool)>> = HashMap::new();
        for (zone, obs_list) in &by_zone {
            for (node_id, sev, conf, unknown_vis) in obs_list {
                deduped_by_zone
                    .entry(zone.clone())
                    .or_default()
                    .insert(node_id.clone(), (*sev, *conf, *unknown_vis));
            }
        }

        for (zone, reporters) in &deduped_by_zone {
            println!("  [Zone {}] {} reporter(s)", zone, reporters.len());

            if reporters.len() < 3 {
                println!("  [Zone {}] ⚠ Insufficient reporters — skipped", zone);
                continue;
            }

            let confident: Vec<u8> = reporters.values()
                .filter(|(_, conf, unknown)| *conf >= 3 && !unknown)
                .map(|(sev, _, _)| *sev)
                .collect();

            if confident.len() < 2 {
                println!("  [Zone {}] ⚠ Not enough confident observers — skipped", zone);
                continue;
            }

            let mut sorted = confident.clone();
            sorted.sort();
            let mid = sorted.len() / 2;
            let median = if sorted.len().is_multiple_of(2) {
                (sorted[mid - 1] + sorted[mid]) / 2
            } else { sorted[mid] };

            println!("  [Zone {}] Median severity: {}", zone, median);

            let mut any_anomaly = false;
            let mut rescue_count = 0u32;

            for (node_id, (severity, confidence, unknown_vis)) in reporters {
                if *unknown_vis {
                    println!("  [Zone {}] ℹ [{}] sev {} — unknown vis, not penalised", zone, node_id, severity);
                    continue;
                }
                if *confidence < 3 {
                    println!("  [Zone {}] ℹ [{}] sev {} — low confidence, not penalised", zone, node_id, severity);
                    continue;
                }

                let deviation = (*severity as i16 - median as i16).unsigned_abs();
                if deviation > 2 {
                    any_anomaly = true;
                    println!("  [Zone {}] ⚠ ANOMALY: [{}] sev {} (dev {} from {})", zone, node_id, severity, deviation, median);
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        node.penalize();
                        println!("  [Zone {}] ✗ [{}] penalised → rep {:.2}", zone, node_id, node.reputation);
                    }
                }

                rescue_count += self.nodes.get(node_id)
                    .map(|n| n.persistent_messages.values()
                        .filter(|m| m.message_type == "rescue").count() as u32)
                    .unwrap_or(0);
            }

            if !any_anomaly {
                println!("  [Zone {}] ✓ No anomalies", zone);
            }

            if rescue_count >= 3 && !self.declared_mce_zones.contains(zone) {
                self.declared_mce_zones.insert(zone.clone());
                println!("  [Zone {}] 🚨 MASS CASUALTY EVENT — {} rescue requests", zone, rescue_count);
            }
        }

        // Clear observations for next round
        for node in self.nodes.values_mut() {
            node.observations.clear();
        }
    }

    pub fn new_round(&mut self) {
        for node in self.nodes.values_mut() {
            node.seen_messages.clear();
        }

        // Re-announce persistent rescue requests from offline nodes
        let offline_rescues: Vec<(String, String)> = self.nodes.values()
            .flat_map(|n| n.persistent_messages.values()
                .filter(|m| m.message_type == "rescue")
                .map(move |m| (n.id.clone(), m.id.clone())))
            .collect();

        for (holder_id, msg_id) in offline_rescues {
            let origin = self.nodes.values()
                .find(|n| n.persistent_messages.contains_key(&msg_id))
                .and_then(|n| n.persistent_messages.get(&msg_id))
                .map(|m| m.origin.clone());

            if let Some(origin_id) = origin {
                let origin_online = self.nodes.get(&origin_id)
                    .map(|n| n.is_online).unwrap_or(false);
                if !origin_online {
                    println!("  📡 [{}] keeping rescue '{}' alive — origin OFFLINE", holder_id, msg_id);
                }
            }
        }
    }

    pub fn print_all(&self) {
        let mut ids: Vec<&String> = self.nodes.keys().collect();
        ids.sort();
        for id in ids {
            self.nodes[id].status();
        }
    }
}