use std::collections::{HashMap, HashSet};

// ─── Structured Signal (Layer 1) ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Signal {
    severity: u8,
    needs_help: bool,
    can_help_others: bool,
    location_confirmed: bool,
}

impl Signal {
    fn new(severity: u8, needs_help: bool, can_help_others: bool, location_confirmed: bool) -> Signal {
        assert!(severity >= 1 && severity <= 5, "Severity must be 1–5");
        Signal { severity, needs_help, can_help_others, location_confirmed }
    }
}

// ─── Message ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Message {
    id: String,
    origin: String,
    signal: Signal,
    note: Option<String>,
}

impl Message {
    fn new(id: &str, origin: &str, signal: Signal, note: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            origin: origin.to_string(),
            signal,
            note: note.map(|s| s.to_string()),
        }
    }
}

// ─── Node ────────────────────────────────────────────────────────────────────

struct Node {
    id: String,
    is_honest: bool,
    reputation: f64,
    zone: String,
    peers: Vec<String>,
    seen_messages: HashSet<String>,
}

impl Node {
    fn new(id: &str, is_honest: bool, zone: &str) -> Node {
        Node {
            id: id.to_string(),
            is_honest,
            reputation: 1.0,
            zone: zone.to_string(),
            peers: Vec::new(),
            seen_messages: HashSet::new(),
        }
    }

    fn add_peer(&mut self, peer_id: &str) {
        if !self.peers.contains(&peer_id.to_string()) {
            self.peers.push(peer_id.to_string());
        }
    }

    fn penalize(&mut self) {
        self.reputation -= 0.2;
        if self.reputation < 0.0 {
            self.reputation = 0.0;
        }
    }

    fn is_trusted(&self) -> bool {
        self.reputation > 0.41
    }

    fn status(&self) {
        let trust_label = if self.is_trusted() { "TRUSTED" } else { "ISOLATED" };
        println!(
            "ID: {:10} | Zone: {:6} | Honest: {:5} | Reputation: {:.2} | Status: {}",
            self.id, self.zone, self.is_honest.to_string(), self.reputation, trust_label
        );
    }
}

// ─── Observation ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Observation {
    node_id: String,
    zone: String,
    severity: u8,
}

// ─── Network ─────────────────────────────────────────────────────────────────

struct Network {
    nodes: HashMap<String, Node>,
    observations: Vec<Observation>,
}

impl Network {
    fn new() -> Network {
        Network {
            nodes: HashMap::new(),
            observations: Vec::new(),
        }
    }

    fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    fn connect(&mut self, a: &str, b: &str) {
        if let Some(node_a) = self.nodes.get_mut(a) {
            node_a.add_peer(b);
        }
        if let Some(node_b) = self.nodes.get_mut(b) {
            node_b.add_peer(a);
        }
    }

    fn new_round(&mut self) {
        for node in self.nodes.values_mut() {
            node.seen_messages.clear();
        }
    }

    fn gossip(&mut self, origin_id: &str, message: Message) {
        let mut queue: Vec<(String, Message)> = vec![(origin_id.to_string(), message)];

        while let Some((sender_id, msg)) = queue.pop() {
            let peers_to_forward: Vec<String> = {
                let sender = match self.nodes.get_mut(&sender_id) {
                    Some(n) => n,
                    None => continue,
                };

                if sender.seen_messages.contains(&msg.id) {
                    continue;
                }
                sender.seen_messages.insert(msg.id.clone());

                println!(
                    "  [{}][{}] received msg '{}' | severity: {} | needs_help: {} | note: {}",
                    sender.id,
                    sender.zone,
                    msg.id,
                    msg.signal.severity,
                    msg.signal.needs_help,
                    msg.note.as_deref().unwrap_or("—")
                );

                if !sender.is_trusted() {
                    println!("  [{}] is ISOLATED — message stops here.", sender.id);
                    continue;
                }

                sender.peers.clone()
            };

            // Record observation with zone from the origin node
            let origin_zone = self.nodes.get(&msg.origin)
                .map(|n| n.zone.clone())
                .unwrap_or_default();

            self.observations.push(Observation {
                node_id: msg.origin.clone(),
                zone: origin_zone,
                severity: msg.signal.severity,
            });

            for peer_id in peers_to_forward {
                if let Some(peer) = self.nodes.get(&peer_id) {
                    if !peer.seen_messages.contains(&msg.id) {
                        queue.push((peer_id.clone(), msg.clone()));
                    }
                }
            }
        }
    }

    // ─── Consensus Engine (per zone) ─────────────────────────────────────────

    fn run_consensus(&mut self) {
        if self.observations.is_empty() {
            println!("  No observations to analyse.");
            return;
        }

        // Deduplicate: one observation per origin node
        let mut deduped: HashMap<String, (String, u8)> = HashMap::new(); // node_id → (zone, severity)
        for obs in &self.observations {
            deduped.insert(obs.node_id.clone(), (obs.zone.clone(), obs.severity));
        }

        // Group by zone
        let mut by_zone: HashMap<String, Vec<(String, u8)>> = HashMap::new();
        for (node_id, (zone, severity)) in &deduped {
            by_zone.entry(zone.clone()).or_default().push((node_id.clone(), *severity));
        }

        for (zone, reporters) in &by_zone {
            println!("  [Zone {}] {} reporter(s)", zone, reporters.len());

            if reporters.len() < 3 {
                println!("  [Zone {}] ⚠ Insufficient reporters — consensus skipped.", zone);
                continue;
            }

            let mut severities: Vec<u8> = reporters.iter().map(|(_, s)| *s).collect();
            severities.sort();
            let mid = severities.len() / 2;
            let median = if severities.len() % 2 == 0 {
                (severities[mid - 1] + severities[mid]) / 2
            } else {
                severities[mid]
            };

            println!("  [Zone {}] Median severity: {}", zone, median);

            let mut any_anomaly = false;
            for (node_id, severity) in reporters {
                let deviation = (*severity as i16 - median as i16).unsigned_abs();
                if deviation > 2 {
                    any_anomaly = true;
                    println!(
                        "  [Zone {}] ⚠ ANOMALY: [{}] reported severity {} (deviation {} from median {})",
                        zone, node_id, severity, deviation, median
                    );
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        node.penalize();
                        println!(
                            "  [Zone {}] ✗ [{}] auto-penalised → reputation now {:.2}",
                            zone, node.id, node.reputation
                        );
                    }
                }
            }
            if !any_anomaly {
                println!("  [Zone {}] ✓ All nodes within acceptable deviation.", zone);
            }
        }

        self.observations.clear();
    }

    fn print_all(&self) {
        let mut ids: Vec<&String> = self.nodes.keys().collect();
        ids.sort();
        for id in ids {
            self.nodes[id].status();
        }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let mut net = Network::new();

    // Zone A — crisis area, Byzantine node here
    net.add_node(Node::new("node-001", true,  "zone-a"));
    net.add_node(Node::new("node-002", true,  "zone-a"));
    net.add_node(Node::new("node-003", false, "zone-a")); // Byzantine
    net.add_node(Node::new("node-004", true,  "zone-a"));

    // Zone B — separate safe area, low severity is legitimate here
    net.add_node(Node::new("node-005", true, "zone-b"));
    net.add_node(Node::new("node-006", true, "zone-b"));
    net.add_node(Node::new("node-007", true, "zone-b"));

    net.connect("node-001", "node-002");
    net.connect("node-002", "node-003");
    net.connect("node-003", "node-004");
    net.connect("node-005", "node-006");
    net.connect("node-006", "node-007");

    println!("=== CRCI Network — Initial State ===");
    net.print_all();

    // ── Round 1 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 1 ===");
    // Zone A — crisis, node-003 lies
    net.gossip("node-001", Message::new("msg-001", "node-001", Signal::new(4, true,  false, true), Some("flooding on main road")));
    net.gossip("node-002", Message::new("msg-002", "node-002", Signal::new(4, true,  false, true), None));
    net.gossip("node-003", Message::new("msg-003", "node-003", Signal::new(1, false, true,  false), Some("all clear here")));
    net.gossip("node-004", Message::new("msg-004", "node-004", Signal::new(5, true,  false, true), Some("bridge collapsed")));
    // Zone B — genuinely calm, low severity is honest
    net.gossip("node-005", Message::new("msg-005", "node-005", Signal::new(1, false, true, true), Some("no issues here")));
    net.gossip("node-006", Message::new("msg-006", "node-006", Signal::new(1, false, true, true), None));
    net.gossip("node-007", Message::new("msg-007", "node-007", Signal::new(2, false, true, true), None));

    println!("\n=== Consensus Engine — Round 1 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 1 ===");
    net.print_all();
    net.new_round();

    // ── Round 2 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 2 ===");
    net.gossip("node-001", Message::new("msg-008", "node-001", Signal::new(5, true,  false, true), Some("situation worsening")));
    net.gossip("node-002", Message::new("msg-009", "node-002", Signal::new(5, true,  false, true), None));
    net.gossip("node-003", Message::new("msg-010", "node-003", Signal::new(1, false, true,  false), None));
    net.gossip("node-004", Message::new("msg-011", "node-004", Signal::new(5, true,  false, true), None));
    net.gossip("node-005", Message::new("msg-012", "node-005", Signal::new(1, false, true, true), None));
    net.gossip("node-006", Message::new("msg-013", "node-006", Signal::new(2, false, true, true), None));
    net.gossip("node-007", Message::new("msg-014", "node-007", Signal::new(1, false, true, true), None));

    println!("\n=== Consensus Engine — Round 2 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 2 ===");
    net.print_all();
    net.new_round();

    // ── Round 3 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 3 ===");
    net.gossip("node-001", Message::new("msg-015", "node-001", Signal::new(5, true,  false, true), Some("roads completely blocked")));
    net.gossip("node-002", Message::new("msg-016", "node-002", Signal::new(5, true,  false, true), None));
    net.gossip("node-003", Message::new("msg-017", "node-003", Signal::new(1, false, true,  false), None));
    net.gossip("node-004", Message::new("msg-018", "node-004", Signal::new(5, true,  false, true), None));
    net.gossip("node-005", Message::new("msg-019", "node-005", Signal::new(1, false, true, true), None));
    net.gossip("node-006", Message::new("msg-020", "node-006", Signal::new(1, false, true, true), None));
    net.gossip("node-007", Message::new("msg-021", "node-007", Signal::new(2, false, true, true), None));

    println!("\n=== Consensus Engine — Round 3 ===");
    net.run_consensus();
    println!("\n=== Final Network State ===");
    net.print_all();
}