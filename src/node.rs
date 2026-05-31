use std::collections::{HashMap, HashSet};
use crate::message::{Message, MessageType};
use crate::identity::Identity;

// ─── Node ────────────────────────────────────────────────────────────────────
// In real life: one person with one phone. Now each person has a cryptographic
// identity — a keypair generated on their device that cannot be faked.

pub struct Node {
    pub id: String,
    pub identity: Identity,
    pub is_honest: bool,
    pub reputation: f64,
    pub zone: String,
    pub peers: Vec<String>,
    pub seen_messages: HashSet<String>,
    pub persistent_messages: HashMap<String, Message>,
    pub is_online: bool,
    // Known public keys of peers — used to verify incoming signatures
    pub known_keys: HashMap<String, ed25519_dalek::VerifyingKey>,
}

impl Node {
    pub fn new(id: &str, is_honest: bool, zone: &str) -> Node {
        let identity = Identity::new(id);
        Node {
            id: id.to_string(),
            identity,
            is_honest,
            reputation: 1.0,
            zone: zone.to_string(),
            peers: Vec::new(),
            seen_messages: HashSet::new(),
            persistent_messages: HashMap::new(),
            is_online: true,
            known_keys: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_id: &str) {
        if !self.peers.contains(&peer_id.to_string()) {
            self.peers.push(peer_id.to_string());
        }
    }

    pub fn penalize(&mut self) {
        self.reputation -= 0.2;
        if self.reputation < 0.0 { self.reputation = 0.0; }
    }

    pub fn is_trusted(&self) -> bool {
        self.reputation > 0.41
    }

    pub fn go_offline(&mut self) {
        self.is_online = false;
        println!("  ⚡ [{}] device went offline.", self.id);
    }

    pub fn status(&self) {
        let trust_label = if self.is_trusted() { "TRUSTED" } else { "ISOLATED" };
        let online_label = if self.is_online { "ONLINE" } else { "OFFLINE" };
        let rescue_count = self.persistent_messages
            .values()
            .filter(|m| m.message_type == MessageType::RescueRequest)
            .count();
        let key_hex: String = self.identity.verifying_key
            .as_bytes()
            .iter()
            .take(6)
            .map(|b| format!("{:02x}", b))
            .collect();
        println!(
            "ID: {:10} | Zone: {:6} | Rep: {:.2} | {} | {} | Rescue: {} | PubKey: {}...",
            self.id, self.zone, self.reputation,
            trust_label, online_label, rescue_count, key_hex
        );
    }
}