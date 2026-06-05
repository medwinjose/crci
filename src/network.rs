use crate::message::{Message, MessageType, Signal, Visibility};
use crate::node::Node;
use std::collections::{HashMap, HashSet};

#[allow(dead_code)]
pub struct Observation {
    pub node_id: String,
    pub zone: String,
    pub severity: u8,
    pub confidence: u8,
    pub is_unknown_visibility: bool,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Network {
    pub nodes: HashMap<String, Node>,
    pub observations: Vec<Observation>,
    pub declared_mce_zones: HashSet<String>,
}

#[allow(dead_code)]
impl Network {
    pub fn new() -> Network {
        Network::default()
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    // Exchange public keys between two nodes when they connect.
    // In real life: when two phones first see each other over Bluetooth,
    // they swap public keys. From then on, each can verify the other's messages.
    pub fn connect(&mut self, a: &str, b: &str) {
        let key_a = self
            .nodes
            .get(a)
            .map(|n| (n.id.clone(), n.identity.verifying_key));
        let key_b = self
            .nodes
            .get(b)
            .map(|n| (n.id.clone(), n.identity.verifying_key));

        if let (Some((id_a, vk_a)), Some((id_b, vk_b))) = (key_a, key_b) {
            if let Some(node_b) = self.nodes.get_mut(&id_b) {
                node_b.add_peer(&id_a);
                node_b.known_keys.insert(id_a.clone(), vk_a);
            }
            if let Some(node_a) = self.nodes.get_mut(&id_a) {
                node_a.add_peer(&id_b);
                node_a.known_keys.insert(id_b.clone(), vk_b);
            }
        }
    }

    pub fn new_round(&mut self) {
        for node in self.nodes.values_mut() {
            node.seen_messages.clear();
        }

        let adoptable: Vec<(String, Message)> = self
            .nodes
            .values()
            .flat_map(|n| {
                n.persistent_messages
                    .values()
                    .cloned()
                    .map(move |m| (n.id.clone(), m))
            })
            .collect();

        for (holder_id, msg) in adoptable {
            let origin_online = self
                .nodes
                .get(&msg.origin)
                .map(|n| n.is_online)
                .unwrap_or(false);
            if !origin_online && msg.message_type == MessageType::RescueRequest {
                println!(
                    "  📡 [{}] keeping rescue '{}' alive — origin OFFLINE",
                    holder_id, msg.id
                );
            }
        }
    }

    // Sign a message on behalf of its origin node before gossiping
    // In real life: the moment someone taps send, their phone signs the
    // message with their private key. Nobody else can produce this signature.
    pub fn sign_message(&self, origin_id: &str, message: &mut Message) {
        if let Some(node) = self.nodes.get(origin_id) {
            let payload = message.signable_payload();
            message.signature = Some(node.identity.sign(&payload));
        }
    }

    // Verify a message's signature when a node receives it
    // In real life: before a phone accepts and forwards a message, it checks
    // the signature against the sender's known public key. A message claiming
    // to be from node-001 but signed by a different key is immediately dropped.
    pub fn verify_message(&self, receiver_id: &str, message: &Message) -> bool {
        let signature = match &message.signature {
            Some(s) => s,
            None => {
                // System messages (MCE) have no signature — always accept
                return message.origin == "system";
            }
        };

        let receiver = match self.nodes.get(receiver_id) {
            Some(n) => n,
            None => return false,
        };

        // Check if the receiver knows the origin's public key
        if let Some(verifying_key) = receiver.known_keys.get(&message.origin) {
            let payload = message.signable_payload();
            verifying_key.verify_strict(&payload, signature).is_ok()
        } else {
            // Origin public key not yet known — in real mesh network this
            // would trigger a key exchange. In simulation we accept it.
            true
        }
    }

    pub fn gossip(&mut self, origin_id: &str, mut message: Message) {
        if let Some(origin) = self.nodes.get(origin_id) {
            if !origin.is_online {
                println!("  [{}] is OFFLINE — cannot send.", origin_id);
                return;
            }
        }

        // Sign the message before it enters the network
        self.sign_message(origin_id, &mut message);

        let sig_preview = message
            .signature
            .as_ref()
            .map(|s| {
                let b = s.to_bytes();
                format!("{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3])
            })
            .unwrap_or_else(|| "unsigned".to_string());

        println!(
            "  ✍  [{}] signed msg '{}' → sig: {}...",
            origin_id, message.id, sig_preview
        );

        let is_persistent = message.message_type == MessageType::RescueRequest
            || message.message_type == MessageType::MassCasualtyEvent;

        let mut queue: Vec<(String, Message)> = vec![(origin_id.to_string(), message)];

        while let Some((sender_id, msg)) = queue.pop() {
            let peers_to_forward: Vec<String> = {
                let sender = match self.nodes.get_mut(&sender_id) {
                    Some(n) => n,
                    None => continue,
                };

                if !sender.is_online {
                    continue;
                }
                if sender.seen_messages.contains(&msg.id) {
                    continue;
                }
                sender.seen_messages.insert(msg.id.clone());

                if is_persistent {
                    sender
                        .persistent_messages
                        .insert(msg.id.clone(), msg.clone());
                }

                let type_label = match msg.message_type {
                    MessageType::Normal => "normal",
                    MessageType::RescueRequest => "🆘 RESCUE",
                    MessageType::MassCasualtyEvent => "🚨 MCE",
                    MessageType::Panic => "🆘 PANIC",
                    MessageType::ChainHeadAnnouncement { .. } => "🔗 CHAIN_HEAD",
                };

                let vis_label = match msg.signal.visibility {
                    Visibility::Direct => "direct",
                    Visibility::Indirect => "indirect",
                    Visibility::Unknown => "unknown",
                };

                println!(
                    "  [{}][{}] {} | sev:{} conf:{} vis:{} | note: {}",
                    sender.id,
                    sender.zone,
                    type_label,
                    msg.signal.severity,
                    msg.signal.confidence,
                    vis_label,
                    msg.note.as_deref().unwrap_or("—")
                );

                if !sender.is_trusted() {
                    println!("  [{}] ISOLATED — stops here.", sender.id);
                    continue;
                }

                sender.peers.clone()
            };

            // Verify signature at each hop before forwarding
            // In real life: every phone that receives a message checks the
            // signature before passing it on. Tampered messages die here.
            for peer_id in &peers_to_forward {
                if !self.verify_message(peer_id, &msg) {
                    println!(
                        "  ⚠ [{}] rejected msg '{}' — invalid signature!",
                        peer_id, msg.id
                    );
                    continue;
                }
                if let Some(peer) = self.nodes.get(peer_id) {
                    if !peer.seen_messages.contains(&msg.id) {
                        queue.push((peer_id.clone(), msg.clone()));
                    }
                }
            }

            let (origin_zone, origin_conf, origin_vis_unknown) = self
                .nodes
                .get(&msg.origin)
                .map(|n| {
                    (
                        n.zone.clone(),
                        msg.signal.confidence,
                        matches!(msg.signal.visibility, Visibility::Unknown),
                    )
                })
                .unwrap_or_default();

            self.observations.push(Observation {
                node_id: msg.origin.clone(),
                zone: origin_zone,
                severity: msg.signal.severity,
                confidence: origin_conf,
                is_unknown_visibility: origin_vis_unknown,
            });
        }
    }

    pub fn run_consensus(&mut self) {
        if self.observations.is_empty() {
            println!("  No observations to analyse.");
            return;
        }

        let mut deduped: HashMap<String, Observation> = HashMap::new();
        for obs in self.observations.drain(..) {
            deduped.insert(obs.node_id.clone(), obs);
        }

        let mut by_zone: HashMap<String, Vec<(String, u8, u8, bool)>> = HashMap::new();
        for (node_id, obs) in &deduped {
            by_zone.entry(obs.zone.clone()).or_default().push((
                node_id.clone(),
                obs.severity,
                obs.confidence,
                obs.is_unknown_visibility,
            ));
        }

        for (zone, reporters) in &by_zone {
            println!("  [Zone {}] {} reporter(s)", zone, reporters.len());

            if reporters.len() < 3 {
                println!("  [Zone {}] ⚠ Insufficient reporters — skipped.", zone);
                continue;
            }

            let confident_severities: Vec<u8> = reporters
                .iter()
                .filter(|(_, _, conf, unknown_vis)| *conf >= 3 && !unknown_vis)
                .map(|(_, sev, _, _)| *sev)
                .collect();

            if confident_severities.len() < 2 {
                println!(
                    "  [Zone {}] ⚠ Not enough confident observers — skipped.",
                    zone
                );
                continue;
            }

            let mut sorted = confident_severities.clone();
            sorted.sort();
            let mid = sorted.len() / 2;
            let median = if sorted.len().is_multiple_of(2) {
                (sorted[mid - 1] + sorted[mid]) / 2
            } else {
                sorted[mid]
            };

            println!("  [Zone {}] Median severity: {}", zone, median);

            let mut rescue_count = 0u32;
            let mut any_anomaly = false;

            for (node_id, severity, confidence, unknown_vis) in reporters {
                if *unknown_vis {
                    println!(
                        "  [Zone {}] ℹ [{}] sev {} — unknown visibility, not penalised",
                        zone, node_id, severity
                    );
                    continue;
                }
                if *confidence < 3 {
                    println!(
                        "  [Zone {}] ℹ [{}] sev {} — low confidence, not penalised",
                        zone, node_id, severity
                    );
                    continue;
                }

                let deviation = (*severity as i16 - median as i16).unsigned_abs();
                if deviation > 2 {
                    any_anomaly = true;
                    println!(
                        "  [Zone {}] ⚠ ANOMALY: [{}] sev {} (dev {} from median {})",
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

                rescue_count += self
                    .nodes
                    .get(node_id)
                    .map(|n| {
                        n.persistent_messages
                            .values()
                            .filter(|m| m.message_type == MessageType::RescueRequest)
                            .count() as u32
                    })
                    .unwrap_or(0);
            }

            if !any_anomaly {
                println!("  [Zone {}] ✓ No anomalies.", zone);
            }

            if rescue_count >= 3 && !self.declared_mce_zones.contains(zone) {
                self.declared_mce_zones.insert(zone.clone());
                println!(
                    "  [Zone {}] 🚨 MASS CASUALTY EVENT DECLARED — {} rescue requests",
                    zone, rescue_count
                );

                let zone_nodes: Vec<String> = self
                    .nodes
                    .values()
                    .filter(|n| n.zone == *zone)
                    .map(|n| n.id.clone())
                    .collect();

                for node_id in &zone_nodes {
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        let mce = Message {
                            id: format!("mce-{}", zone),
                            origin: "system".to_string(),
                            signal: Signal::panic(),
                            note: Some(format!(
                                "MCE declared in {} — {} casualties",
                                zone, rescue_count
                            )),
                            message_type: MessageType::MassCasualtyEvent,
                            origin_active: true,
                            signature: None,
                            created_at: crate::message::now_ts(),
                            ttl_seconds: 0,
                            priority: crate::message::MessagePriority::Critical,
                            hop_count: 0,
                            seq: 0,
                        };
                        node.persistent_messages.insert(mce.id.clone(), mce);
                    }
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
