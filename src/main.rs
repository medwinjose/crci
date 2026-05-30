use std::collections::{HashMap, HashSet};

// ─── Message Type ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum MessageType {
    Normal,
    RescueRequest,
    MassCasualtyEvent,
}

// ─── Visibility ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Visibility {
    Direct,    // I can see it with my own eyes
    Indirect,  // I can hear/smell/feel it
    Unknown,   // I am indoors or cannot observe
}

// ─── Signal (Layer 1) ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Signal {
    severity: u8,
    needs_help: bool,
    can_help_others: bool,
    location_confirmed: bool,
    confidence: u8,       // 1–5: how sure is the reporter?
    visibility: Visibility,
}

impl Signal {
    fn new(
        severity: u8,
        needs_help: bool,
        can_help_others: bool,
        location_confirmed: bool,
        confidence: u8,
        visibility: Visibility,
    ) -> Signal {
        assert!(severity >= 1 && severity <= 5, "Severity must be 1–5");
        assert!(confidence >= 1 && confidence <= 5, "Confidence must be 1–5");
        Signal { severity, needs_help, can_help_others, location_confirmed, confidence, visibility }
    }

    // Panic button — one call, maximum emergency, no choices needed
    fn panic() -> Signal {
        Signal {
            severity: 5,
            needs_help: true,
            can_help_others: false,
            location_confirmed: true,
            confidence: 5,
            visibility: Visibility::Direct,
        }
    }
}

// ─── Message ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Message {
    id: String,
    origin: String,
    signal: Signal,
    note: Option<String>,
    message_type: MessageType,
    origin_active: bool, // false = origin device went silent
}

impl Message {
    fn new(id: &str, origin: &str, signal: Signal, note: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            origin: origin.to_string(),
            signal,
            note: note.map(|s| s.to_string()),
            message_type: MessageType::Normal,
            origin_active: true,
        }
    }

    fn rescue(id: &str, origin: &str, note: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            origin: origin.to_string(),
            signal: Signal::panic(),
            note: note.map(|s| s.to_string()),
            message_type: MessageType::RescueRequest,
            origin_active: true,
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
    persistent_messages: HashMap<String, Message>, // never wiped
    is_online: bool, // false = phone died or person unconscious
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
            persistent_messages: HashMap::new(),
            is_online: true,
        }
    }

    fn add_peer(&mut self, peer_id: &str) {
        if !self.peers.contains(&peer_id.to_string()) {
            self.peers.push(peer_id.to_string());
        }
    }

    fn penalize(&mut self) {
        self.reputation -= 0.2;
        if self.reputation < 0.0 { self.reputation = 0.0; }
    }

    fn is_trusted(&self) -> bool {
        self.reputation > 0.41
    }

    fn go_offline(&mut self) {
        self.is_online = false;
        println!("  ⚡ [{}] device went offline.", self.id);
    }

    fn status(&self) {
        let trust_label = if self.is_trusted() { "TRUSTED" } else { "ISOLATED" };
        let online_label = if self.is_online { "ONLINE" } else { "OFFLINE" };
        let rescue_count = self.persistent_messages
            .values()
            .filter(|m| m.message_type == MessageType::RescueRequest)
            .count();
        println!(
            "ID: {:10} | Zone: {:6} | Honest: {:5} | Rep: {:.2} | {} | {} | Rescue msgs held: {}",
            self.id, self.zone, self.is_honest.to_string(),
            self.reputation, trust_label, online_label, rescue_count
        );
    }
}

// ─── Observation ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Observation {
    node_id: String,
    zone: String,
    severity: u8,
    confidence: u8,
    is_unknown_visibility: bool,
}

// ─── Network ─────────────────────────────────────────────────────────────────

struct Network {
    nodes: HashMap<String, Node>,
    observations: Vec<Observation>,
}

impl Network {
    fn new() -> Network {
        Network { nodes: HashMap::new(), observations: Vec::new() }
    }

    fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    fn connect(&mut self, a: &str, b: &str) {
        if let Some(n) = self.nodes.get_mut(a) { n.add_peer(b); }
        if let Some(n) = self.nodes.get_mut(b) { n.add_peer(a); }
    }

    fn new_round(&mut self) {
        for node in self.nodes.values_mut() {
            node.seen_messages.clear();
        }
        // Re-gossip persistent rescue requests from offline nodes
        // (adoption: nearby nodes keep the request alive)
        let adoptable: Vec<(String, Message)> = self.nodes.values()
            .flat_map(|n| n.persistent_messages.values().cloned().map(move |m| (n.id.clone(), m)))
            .collect();

        for (holder_id, mut msg) in adoptable {
            let origin_online = self.nodes.get(&msg.origin)
                .map(|n| n.is_online)
                .unwrap_or(false);
            if !origin_online {
                msg.origin_active = false;
                println!(
                    "  📡 [{}] re-broadcasting rescue request '{}' — origin OFFLINE",
                    holder_id, msg.id
                );
            }
        }
    }

    fn gossip(&mut self, origin_id: &str, message: Message) {
        // Offline nodes cannot originate new messages
        if let Some(origin) = self.nodes.get(origin_id) {
            if !origin.is_online {
                println!("  [{}] is OFFLINE — cannot send.", origin_id);
                return;
            }
        }

        let is_persistent = message.message_type == MessageType::RescueRequest
            || message.message_type == MessageType::MassCasualtyEvent;

        let mut queue: Vec<(String, Message)> = vec![(origin_id.to_string(), message)];

        while let Some((sender_id, msg)) = queue.pop() {
            let peers_to_forward: Vec<String> = {
                let sender = match self.nodes.get_mut(&sender_id) {
                    Some(n) => n,
                    None => continue,
                };

                if sender.seen_messages.contains(&msg.id) { continue; }
                sender.seen_messages.insert(msg.id.clone());

                // Store persistent messages permanently
                if is_persistent {
                    sender.persistent_messages.insert(msg.id.clone(), msg.clone());
                }

                let type_label = match msg.message_type {
                    MessageType::Normal => "normal",
                    MessageType::RescueRequest => "🆘 RESCUE",
                    MessageType::MassCasualtyEvent => "🚨 MASS CASUALTY",
                };

                let vis_label = match msg.signal.visibility {
                    Visibility::Direct => "direct",
                    Visibility::Indirect => "indirect",
                    Visibility::Unknown => "unknown",
                };

                println!(
                    "  [{}][{}] {} | sev:{} conf:{} vis:{} | note: {}",
                    sender.id, sender.zone, type_label,
                    msg.signal.severity, msg.signal.confidence, vis_label,
                    msg.note.as_deref().unwrap_or("—")
                );

                if !sender.is_trusted() {
                    println!("  [{}] ISOLATED — stops here.", sender.id);
                    continue;
                }

                sender.peers.clone()
            };

            // Record observation
            let (origin_zone, origin_conf, origin_vis_unknown) = self.nodes
                .get(&msg.origin)
                .map(|n| (
                    n.zone.clone(),
                    msg.signal.confidence,
                    matches!(msg.signal.visibility, Visibility::Unknown),
                ))
                .unwrap_or_default();

            self.observations.push(Observation {
                node_id: msg.origin.clone(),
                zone: origin_zone,
                severity: msg.signal.severity,
                confidence: origin_conf,
                is_unknown_visibility: origin_vis_unknown,
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

    // ─── Consensus Engine ────────────────────────────────────────────────────

    fn run_consensus(&mut self) {
        if self.observations.is_empty() {
            println!("  No observations to analyse.");
            return;
        }

        // Deduplicate by origin node
        let mut deduped: HashMap<String, Observation> = HashMap::new();
        for obs in self.observations.drain(..) {
            deduped.insert(obs.node_id.clone(), obs);
        }

        // Group by zone
        let mut by_zone: HashMap<String, Vec<(String, u8, u8, bool)>> = HashMap::new();
        for (node_id, obs) in &deduped {
            by_zone.entry(obs.zone.clone())
                .or_default()
                .push((node_id.clone(), obs.severity, obs.confidence, obs.is_unknown_visibility));
        }

        for (zone, reporters) in &by_zone {
            println!("  [Zone {}] {} reporter(s)", zone, reporters.len());

            if reporters.len() < 3 {
                println!("  [Zone {}] ⚠ Insufficient reporters — skipped.", zone);
                continue;
            }

            // Only use high-confidence, non-unknown-visibility reports for median
            let confident_severities: Vec<u8> = reporters.iter()
                .filter(|(_, _, conf, unknown_vis)| *conf >= 3 && !unknown_vis)
                .map(|(_, sev, _, _)| *sev)
                .collect();

            if confident_severities.len() < 2 {
                println!("  [Zone {}] ⚠ Not enough confident observers — skipped.", zone);
                continue;
            }

            let mut sorted = confident_severities.clone();
            sorted.sort();
            let mid = sorted.len() / 2;
            let median = if sorted.len() % 2 == 0 {
                (sorted[mid - 1] + sorted[mid]) / 2
            } else {
                sorted[mid]
            };

            println!("  [Zone {}] Median severity (confident reporters): {}", zone, median);

            let mut rescue_count = 0u32;
            let mut any_anomaly = false;

            for (node_id, severity, confidence, unknown_vis) in reporters {
                // Unknown visibility = unverified, not penalised
                if *unknown_vis {
                    println!(
                        "  [Zone {}] ℹ [{}] reported sev {} — visibility unknown, not penalised",
                        zone, node_id, severity
                    );
                    continue;
                }

                // Low confidence = not penalised either
                if *confidence < 3 {
                    println!(
                        "  [Zone {}] ℹ [{}] reported sev {} — low confidence ({}), not penalised",
                        zone, node_id, severity, confidence
                    );
                    continue;
                }

                let deviation = (*severity as i16 - median as i16).unsigned_abs();
                if deviation > 2 {
                    any_anomaly = true;
                    println!(
                        "  [Zone {}] ⚠ ANOMALY: [{}] sev {} (deviation {} from median {})",
                        zone, node_id, severity, deviation, median
                    );
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        node.penalize();
                        println!(
                            "  [Zone {}] ✗ [{}] penalised → rep {:.2}",
                            zone, node.id, node.reputation
                        );
                    }
                }

                // Count rescue requests in this zone
                let node_rescue_count = self.nodes.get(node_id)
                    .map(|n| n.persistent_messages.values()
                        .filter(|m| m.message_type == MessageType::RescueRequest)
                        .count() as u32)
                    .unwrap_or(0);
                rescue_count += node_rescue_count;
            }

            if !any_anomaly {
                println!("  [Zone {}] ✓ No anomalies.", zone);
            }

            // MCE declaration: 3+ rescue requests in same zone same round
            if rescue_count >= 3 {
                println!(
                    "  [Zone {}] 🚨 MASS CASUALTY EVENT DECLARED — {} rescue requests clustered",
                    zone, rescue_count
                );
                // Broadcast MCE to all nodes in this zone
                let zone_nodes: Vec<String> = self.nodes.values()
                    .filter(|n| n.zone == *zone)
                    .map(|n| n.id.clone())
                    .collect();
                for node_id in &zone_nodes {
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        let mce_msg = Message {
                            id: format!("mce-{}", zone),
                            origin: "system".to_string(),
                            signal: Signal::panic(),
                            note: Some(format!("MASS CASUALTY EVENT in {} — {} casualties", zone, rescue_count)),
                            message_type: MessageType::MassCasualtyEvent,
                            origin_active: true,
                        };
                        node.persistent_messages.insert(mce_msg.id.clone(), mce_msg);
                    }
                }
            }
        }
    }

    fn print_all(&self) {
        let mut ids: Vec<&String> = self.nodes.keys().collect();
        ids.sort();
        for id in ids { self.nodes[id].status(); }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let mut net = Network::new();

    // Zone A — crisis, Byzantine node + injured person + indoor person
    net.add_node(Node::new("node-001", true,  "zone-a")); // outdoor, direct observer
    net.add_node(Node::new("node-002", true,  "zone-a")); // outdoor, direct observer
    net.add_node(Node::new("node-003", false, "zone-a")); // Byzantine liar
    net.add_node(Node::new("node-004", true,  "zone-a")); // INJURED — will use panic button
    net.add_node(Node::new("node-005", true,  "zone-a")); // INDOOR — unknown visibility
    net.add_node(Node::new("node-006", true,  "zone-a")); // outdoor, direct observer

    // Zone B — safe area
    net.add_node(Node::new("node-007", true, "zone-b"));
    net.add_node(Node::new("node-008", true, "zone-b"));
    net.add_node(Node::new("node-009", true, "zone-b"));

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

    // Normal outdoor reporters
    net.gossip("node-001", Message::new("msg-001", "node-001",
        Signal::new(4, true, false, true, 4, Visibility::Direct),
        Some("flooding on main road")));

    net.gossip("node-002", Message::new("msg-002", "node-002",
        Signal::new(4, true, false, true, 5, Visibility::Direct), None));

    // Byzantine liar — confident but wrong
    net.gossip("node-003", Message::new("msg-003", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct),
        Some("all clear here")));

    // Injured person — panic button, one tap
    net.gossip("node-004", Message::rescue("msg-004", "node-004",
        Some("help trapped under debris")));

    // Indoor person — unknown visibility, low confidence, NOT penalised
    net.gossip("node-005", Message::new("msg-005", "node-005",
        Signal::new(1, false, true, false, 1, Visibility::Unknown),
        Some("indoors, cannot see outside")));

    // Another outdoor honest reporter
    net.gossip("node-006", Message::new("msg-006", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("bridge collapsed, people in water")));

    // Zone B calm reports
    net.gossip("node-007", Message::new("msg-007", "node-007",
        Signal::new(1, false, true, true, 5, Visibility::Direct), None));
    net.gossip("node-008", Message::new("msg-008", "node-008",
        Signal::new(2, false, true, true, 4, Visibility::Direct), None));
    net.gossip("node-009", Message::new("msg-009", "node-009",
        Signal::new(1, false, true, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 1 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 1 ===");
    net.print_all();
    net.new_round();

    // ── Round 2 — node-004 phone dies ────────────────────────────────────────
    println!("\n=== Round 2 — node-004 goes offline (battery died) ===");
    if let Some(n) = net.nodes.get_mut("node-004") { n.go_offline(); }

    net.gossip("node-001", Message::new("msg-010", "node-001",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("situation critical")));
    net.gossip("node-002", Message::new("msg-011", "node-002",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));
    net.gossip("node-003", Message::new("msg-012", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct), None));
    net.gossip("node-006", Message::new("msg-013", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 2 ===");
    net.run_consensus();
    println!("\n=== Network State After Round 2 ===");
    net.print_all();
    net.new_round();

    // ── Round 3 ──────────────────────────────────────────────────────────────
    println!("\n=== Gossip Round 3 ===");
    net.gossip("node-001", Message::new("msg-014", "node-001",
        Signal::new(5, true, false, true, 5, Visibility::Direct),
        Some("roads completely blocked")));
    net.gossip("node-002", Message::new("msg-015", "node-002",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));
    net.gossip("node-003", Message::new("msg-016", "node-003",
        Signal::new(1, false, true, false, 5, Visibility::Direct), None));
    net.gossip("node-006", Message::new("msg-017", "node-006",
        Signal::new(5, true, false, true, 5, Visibility::Direct), None));

    println!("\n=== Consensus Engine — Round 3 ===");
    net.run_consensus();
    println!("\n=== Final Network State ===");
    net.print_all();
}