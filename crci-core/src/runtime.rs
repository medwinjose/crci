use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::identity::Identity;
use crate::message::{Message, MessageType, Signal, Visibility};
use crate::storage::{PersistedRescue, PersistedState};
use crate::transport::{LegacyTransport, SharedInbox, SimTransport};

pub const MIN_TRUSTED_REP: f64 = 0.41;

fn round_to_3_dec(val: f64) -> f64 {
    (val * 1000.0).round() / 1000.0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WireMessage {
    pub id: String,
    pub origin: String,
    pub origin_pubkey: Vec<u8>,
    pub severity: u8,
    pub confidence: u8,
    pub needs_help: bool,
    pub can_help_others: bool,
    pub location_confirmed: bool,
    pub visibility: String,
    pub note: Option<String>,
    pub message_type: String,
    pub origin_active: bool,
    pub signature_bytes: Vec<u8>,
    pub seq: u64,
}

impl WireMessage {
    pub fn from_message(msg: &Message, identity: &Identity) -> WireMessage {
        let payload = format!("{}:{}:{}", msg.id, msg.origin, msg.signal.severity);
        let sig = identity.sign(payload.as_bytes());
        WireMessage {
            id: msg.id.clone(),
            origin: msg.origin.clone(),
            origin_pubkey: identity.verifying_key.to_bytes().to_vec(),
            severity: msg.signal.severity,
            confidence: msg.signal.confidence,
            needs_help: msg.signal.needs_help,
            can_help_others: msg.signal.can_help_others,
            location_confirmed: msg.signal.location_confirmed,
            visibility: match msg.signal.visibility {
                Visibility::Direct => "direct".to_string(),
                Visibility::Indirect => "indirect".to_string(),
                Visibility::Unknown => "unknown".to_string(),
            },
            note: msg.note.clone(),
            message_type: match &msg.message_type {
                MessageType::Normal => "normal".to_string(),
                MessageType::RescueRequest => "rescue".to_string(),
                MessageType::MassCasualtyEvent => "mce".to_string(),
                MessageType::Panic => "panic".to_string(),
                MessageType::ChainHeadAnnouncement { .. } => {
                    serde_json::to_string(&msg.message_type).unwrap_or_default()
                }
            },
            origin_active: msg.origin_active,
            signature_bytes: sig.to_bytes().to_vec(),
            seq: msg.seq,
        }
    }

    pub fn verify_signature(&self) -> bool {
        let key_bytes: [u8; 32] = match self.origin_pubkey.as_slice().try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig_bytes: [u8; 64] = match self.signature_bytes.as_slice().try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signature = Signature::from_bytes(&sig_bytes);
        let payload = format!("{}:{}:{}", self.id, self.origin, self.severity);
        verifying_key.verify(payload.as_bytes(), &signature).is_ok()
    }

    #[allow(dead_code)]
    pub fn to_message(&self) -> Message {
        Message {
            id: self.id.clone(),
            origin: self.origin.clone(),
            created_at: crate::message::now_ts(),
            ttl_seconds: 0,
            priority: crate::message::MessagePriority::Normal,
            hop_count: 0,
            seq: self.seq,
            signal: Signal {
                gps: None,
                resource_type: None,
                severity: self.severity,
                confidence: self.confidence,
                needs_help: self.needs_help,
                can_help_others: self.can_help_others,
                location_confirmed: self.location_confirmed,
                visibility: match self.visibility.as_str() {
                    "direct" => Visibility::Direct,
                    "indirect" => Visibility::Indirect,
                    _ => Visibility::Unknown,
                },
            },
            note: self.note.clone(),
            message_type: match self.message_type.as_str() {
                "rescue" => MessageType::RescueRequest,
                "mce" => MessageType::MassCasualtyEvent,
                "panic" => MessageType::Panic,
                "normal" => MessageType::Normal,
                other => serde_json::from_str(other).unwrap_or(MessageType::Normal),
            },
            origin_active: self.origin_active,
            signature: None,
        }
    }
}

pub struct NodeRuntime {
    pub id: String,
    pub identity: Identity,
    pub reputation: f64,
    pub zone: String,
    pub peers: Vec<String>,
    pub is_online: bool,
    pub seen_messages: HashSet<String>,
    pub persistent_messages: HashMap<String, WireMessage>,
    pub transport: SimTransport,
    pub inbox: SharedInbox,
    pub storage: crate::storage::legacy::NodeStorage,
    pub storage_backend: Option<std::sync::Arc<dyn crate::storage::StorageBackend>>,
    pub observations: Vec<(String, u8, u8, bool)>,
    pub known_keys: HashMap<String, Vec<u8>>,
    pub replay_filter: crate::replay::ReplayFilter,
    pub seq_counter: u64,
    pub messages_handled: u64,
    pub byzantine_events: u64,
    pub consensus_rounds: u64,
    pub merkle: crate::merkle::MerkleChain,
    pub divergence_log: Vec<crate::merkle::DivergenceAlert>,
    pub divergence_tx: Option<tokio::sync::broadcast::Sender<crate::merkle::DivergenceAlert>>,
    pub router: crate::routing::TemporalRouter,
    pub sybil_guard: crate::sybil::SybilGuard,
    pub sybil_banned_events: u64,
    pub sybil_rate_limited_events: u64,
    pub sybil_pow_failed_events: u64,
    pub sig_verifications_count: HashMap<String, usize>,
}

impl NodeRuntime {
    pub fn new(id: &str, zone: &str, inbox: SharedInbox) -> NodeRuntime {
        let transport = SimTransport::new(id, inbox.clone());
        let identity = Identity::new(id);
        let key_bytes = identity.signing_key.to_bytes();
        let storage = crate::storage::legacy::NodeStorage::new(id, &key_bytes);
        NodeRuntime {
            id: id.to_string(),
            identity,
            reputation: round_to_3_dec(1.0),
            zone: zone.to_string(),
            peers: Vec::new(),
            is_online: true,
            seen_messages: HashSet::new(),
            persistent_messages: HashMap::new(),
            transport,
            inbox,
            observations: Vec::new(),
            storage,
            storage_backend: None,
            known_keys: HashMap::new(),
            replay_filter: crate::replay::ReplayFilter::new(),
            seq_counter: 0,
            messages_handled: 0,
            byzantine_events: 0,
            consensus_rounds: 0,
            merkle: crate::merkle::MerkleChain::new(),
            divergence_log: Vec::new(),
            divergence_tx: None,
            router: crate::routing::TemporalRouter::new(),
            sybil_guard: crate::sybil::SybilGuard::new(4),
            sybil_banned_events: 0,
            sybil_rate_limited_events: 0,
            sybil_pow_failed_events: 0,
            sig_verifications_count: HashMap::new(),
        }
    }

    pub fn set_divergence_tx(
        &mut self,
        tx: tokio::sync::broadcast::Sender<crate::merkle::DivergenceAlert>,
    ) {
        self.divergence_tx = Some(tx);
    }

    pub fn chain_head(&self) -> ([u8; 32], u64) {
        (self.merkle.head_hash(), self.merkle.head_sequence())
    }

    pub fn reputation_floor(&self) -> f64 {
        self.reputation // For now, just use node's own reputation or a simple aggregate
    }

    pub fn save_state(&self) {
        let rescues: Vec<PersistedRescue> = self
            .persistent_messages
            .values()
            .filter(|m| m.message_type == "rescue")
            .map(|m| PersistedRescue {
                id: m.id.clone(),
                origin: m.origin.clone(),
                note: m.note.clone(),
                origin_active: m.origin_active,
            })
            .collect();
        let state = PersistedState {
            node_id: self.id.clone(),
            zone: self.zone.clone(),
            reputation: self.reputation,
            peers: self.peers.clone(),
            rescue_messages: rescues,
            mce_zones: vec![],
        };
        if let Err(e) = self.storage.save(&state) {
            println!("  ⚠ [{}] save failed: {}", self.id, e);
        }
    }

    pub fn load_state(&mut self) {
        match self.storage.load() {
            Ok(state) => {
                // BFT-011: clamp to [0.0, 1.0] and BFT-016: fixed precision rounding
                self.reputation = round_to_3_dec(state.reputation.clamp(0.0, 1.0));
                self.peers = state.peers;
                println!(
                    "  ✅ [{}] restored: rep={:.2}, {} peers, {} rescues",
                    self.id,
                    self.reputation,
                    self.peers.len(),
                    state.rescue_messages.len()
                );
            }
            Err(e) => {
                println!("  ℹ [{}] no prior state ({})", self.id, e);
            }
        }
    }

    pub fn add_peer(&mut self, peer_id: &str) {
        if !self.peers.contains(&peer_id.to_string()) {
            self.peers.push(peer_id.to_string());
        }
    }

    pub fn is_trusted(&self) -> bool {
        // BFT-014: non-zero MinTrustedRep floor enforced via MIN_TRUSTED_REP
        self.reputation > MIN_TRUSTED_REP
    }

    pub fn penalize(&mut self) {
        let new_rep = self.reputation - 0.2;
        // BFT-011: clamp to [0.0, 1.0] and BFT-016: fixed precision rounding
        self.reputation = round_to_3_dec(new_rep.clamp(0.0, 1.0));
    }

    pub fn go_offline(&mut self) {
        self.is_online = false;
        println!("  ⚡ [{}] went OFFLINE", self.id);
    }

    pub fn originate(&mut self, mut msg: Message) {
        self.seq_counter += 1;
        msg.seq = self.seq_counter;
        if !self.is_online {
            println!("  [{}] OFFLINE — cannot originate", self.id);
            return;
        }
        let wire = WireMessage::from_message(&msg, &self.identity);
        let is_persistent = matches!(
            msg.message_type,
            MessageType::RescueRequest | MessageType::MassCasualtyEvent
        ) || msg.signal.can_help_others;
        if is_persistent {
            self.persistent_messages
                .insert(wire.id.clone(), wire.clone());
        }
        self.seen_messages.insert(wire.id.clone());
        let payload = match serde_json::to_vec(&wire) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!(
                    "  ⚠ [{}] failed to serialize origination message: {}",
                    self.id, e
                );
                return;
            }
        };
        let sig_preview = wire
            .signature_bytes
            .iter()
            .take(4)
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        println!(
            "  ✍  [{}] originating '{}' → sig: {}...",
            self.id, wire.id, sig_preview
        );

        let now_ms = crate::message::now_ts();
        self.router.evict_stale(now_ms);

        if let Some(best) = self.router.best_next_hop(&[], now_ms) {
            self.transport.send(&best.peer_id, &payload);
        } else {
            for peer in &self.peers.clone() {
                self.transport.send(peer, &payload);
            }
        }

        if !wire.message_type.contains("ChainHeadAnnouncement") {
            self.observations.push((
                self.id.clone(),
                wire.severity,
                wire.confidence,
                wire.visibility == "unknown",
            ));
        }
    }

    pub fn process_inbox(&mut self) {
        if !self.is_online {
            return;
        }
        let messages: Vec<Vec<u8>> = {
            let mut inbox = match self.inbox.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            inbox.remove(&self.id).unwrap_or_default()
        };
        let has_messages = !messages.is_empty();
        for raw in messages {
            let wire: WireMessage = match serde_json::from_slice(&raw) {
                Ok(w) => w,
                Err(_) => {
                    println!("  [{}] received malformed bytes — dropped", self.id);
                    continue;
                }
            };

            // Sybil admission check
            match self.sybil_guard.check(&wire.origin) {
                Ok(()) => { /* proceed */ }
                Err(crate::sybil::SybilError::Banned(peer)) => {
                    self.sybil_banned_events += 1;
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} is banned",
                        self.id, peer
                    );
                    continue;
                }
                Err(crate::sybil::SybilError::RateLimited(peer)) => {
                    self.sybil_rate_limited_events += 1;
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} exceeded rate limit",
                        self.id, peer
                    );
                    continue;
                }
                Err(crate::sybil::SybilError::PowFailed(peer)) => {
                    self.sybil_pow_failed_events += 1;
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} PoW failed",
                        self.id, peer
                    );
                    continue;
                }
            }

            // BFT-008: Bounded local signature verification count per peer to prevent signature check DoS
            if wire.message_type != "mce" {
                let sig_checks = self
                    .sig_verifications_count
                    .entry(wire.origin.clone())
                    .or_insert(0);
                if *sig_checks >= 5 {
                    println!(
                        "  ⚠ [{}] BFT-008 Signature verification limit exceeded for {}, dropping message '{}'",
                        self.id, wire.origin, wire.id
                    );
                    self.byzantine_events += 1;
                    continue;
                }
                *sig_checks += 1;
            }

            // Step 1: verify cryptographic signature
            if wire.message_type != "mce" && !wire.verify_signature() {
                println!(
                    "  ⚠ [{}] INVALID SIGNATURE on '{}' — dropped!",
                    self.id, wire.id
                );
                self.byzantine_events += 1;
                // BFT-019: Defend against signature-invalidation attacks.
                // Do NOT penalise the claimed origin node when signature check fails.
                continue;
            }

            // Step 2: pubkey consistency check — reject impersonation
            if wire.message_type != "mce" {
                if let Some(known) = self.known_keys.get(&wire.origin) {
                    if *known != wire.origin_pubkey {
                        println!(
                            "  ⚠ [{}] PUBKEY MISMATCH on '{}' — impersonation dropped!",
                            self.id, wire.id
                        );
                        self.byzantine_events += 1;
                        // BFT-019: Defend against signature-invalidation attacks.
                        // Do NOT penalise the claimed origin node on pubkey mismatch.
                        continue;
                    }
                } else {
                    self.known_keys
                        .insert(wire.origin.clone(), wire.origin_pubkey.clone());
                }
            }

            let verdict = self.replay_filter.check_and_record(
                &wire.origin,
                wire.seq,
                crate::message::now_ts(),
            );
            if !verdict.is_accept() {
                self.byzantine_events += 1;
                self.sybil_guard.report_violation(&wire.origin);
                match &verdict {
                    crate::replay::ReplayVerdict::Replayed {
                        received_seq,
                        expected_min,
                    } => println!(
                        "  ⛔ [{}] REPLAY dropped from '{}' — seq {} already seen (min {})",
                        self.id, wire.origin, received_seq, expected_min
                    ),
                    crate::replay::ReplayVerdict::Stale { age_seconds } => println!(
                        "  ⏰ [{}] STALE msg from '{}' — {}s old, dropped",
                        self.id, wire.origin, age_seconds
                    ),
                    crate::replay::ReplayVerdict::FromFuture { skew_seconds } => println!(
                        "  ⚠ [{}] FUTURE msg from '{}' — {}s ahead, dropped",
                        self.id, wire.origin, skew_seconds
                    ),
                    _ => {}
                }
                continue;
            }

            if self.seen_messages.contains(&wire.id) {
                continue;
            }
            // BFT-046: Hard ceiling on observed message ID cache
            if self.seen_messages.len() >= 5000 {
                self.seen_messages.clear();
                self.sig_verifications_count.clear(); // Reset verification counters at cache eviction boundary
            }
            self.seen_messages.insert(wire.id.clone());

            let mut is_chain_head = false;
            let type_label = match wire.message_type.as_str() {
                "rescue" => "🆘 RESCUE",
                "mce" => "🚨 MCE",
                "panic" => "🚨 PANIC",
                "normal" => "normal",
                _ => {
                    if wire.message_type.contains("ChainHeadAnnouncement") {
                        is_chain_head = true;
                        "🔗 CHAIN_HEAD"
                    } else {
                        "normal"
                    }
                }
            };
            println!(
                "  [{}][{}] {} | sev:{} conf:{} vis:{} | note: {}",
                self.id,
                self.zone,
                type_label,
                wire.severity,
                wire.confidence,
                wire.visibility,
                wire.note.as_deref().unwrap_or("—")
            );

            if is_chain_head {
                if let Ok(MessageType::ChainHeadAnnouncement {
                    head_hash,
                    head_seq,
                }) = serde_json::from_str(&wire.message_type)
                {
                    if let Some(alert) =
                        self.merkle
                            .build_divergence_alert(&wire.origin, head_hash, head_seq)
                    {
                        println!(
                            "  ⚠ [{}] DIVERGENCE detected with peer {} at seq {}",
                            self.id, wire.origin, alert.divergence_seq
                        );
                        self.divergence_log.push(alert.clone());
                        if let Some(tx) = &self.divergence_tx {
                            let _ = tx.send(alert);
                        }
                        self.byzantine_events += 1;
                    }
                }
                continue; // Do not append to persistent_messages, do not re-gossip
            }

            if wire.message_type == "rescue" || wire.message_type == "mce" {
                self.persistent_messages
                    .insert(wire.id.clone(), wire.clone());
            }

            if !self.is_trusted() {
                println!("  [{}] ISOLATED — not forwarding", self.id);
                continue;
            }

            self.observations.push((
                wire.origin.clone(),
                wire.severity,
                wire.confidence,
                wire.visibility == "unknown",
            ));

            let now_ms = crate::message::now_ts();
            self.router.upsert(crate::routing::RoutingEntry {
                peer_id: wire.origin.clone(), // assume direct connection for hop_count 1
                last_seen_ms: now_ms,
                hop_count: 1,
                freshness_ms: 30_000,
            });

            self.messages_handled += 1;

            let payload = match serde_json::to_vec(&wire) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            self.router.evict_stale(now_ms);

            if let Some(best) = self.router.best_next_hop(&[&wire.origin], now_ms) {
                self.transport.send(&best.peer_id, &payload);
            } else {
                for peer in &self.peers.clone() {
                    if *peer != wire.origin {
                        self.transport.send(peer, &payload);
                    }
                }
            }
            if let Some(backend) = &self.storage_backend {
                let key = format!("msg:{}", wire.id);
                if let Ok(value) = serde_json::to_vec(&wire) {
                    let backend_clone = backend.clone();
                    // Spawn the write task and immediately wrap it with panic detection.
                    // tokio::spawn returns a JoinHandle; if the spawned future panics,
                    // JoinHandle::await returns Err(JoinError) containing the panic payload
                    // rather than propagating the panic to this thread. We spawn a lightweight
                    // watcher task so the caller remains fire-and-forget while panics are
                    // observable (logged) instead of silently killing a Tokio worker thread.
                    let write_handle = tokio::spawn(async move {
                        let _ = backend_clone.write(&key, &value).await;
                    });
                    tokio::spawn(async move {
                        if let Err(join_err) = write_handle.await {
                            if join_err.is_panic() {
                                log::error!(
                                    "[panic-isolation] storage write task panicked: {:?}. \
                                     Tokio worker thread survived.",
                                    join_err
                                );
                            }
                            // is_cancelled() is ignored — cancellation is expected during shutdown.
                        }
                    });
                }
            }
        }

        let current_handled = self.messages_handled;
        if current_handled > 0 && current_handled.is_multiple_of(50) {
            self.merkle.append(crate::merkle::StateSnapshot {
                peer_count: self.peers.len(),
                reputation_floor: self.reputation_floor(),
                messages_handled: self.messages_handled,
                byzantine_events: self.byzantine_events,
                timestamp_ms: crate::message::now_ts(),
            });
        }

        // Periodic broadcast of chain head
        if has_messages {
            let (head_hash, head_seq) = self.chain_head();
            let announcement = Message {
                id: format!("head-{}-{}", self.id, self.seq_counter + 1),
                origin: self.id.clone(),
                signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
                note: None,
                message_type: MessageType::ChainHeadAnnouncement {
                    head_hash,
                    head_seq,
                },
                origin_active: true,
                signature: None,
                created_at: crate::message::now_ts(),
                ttl_seconds: 30,
                priority: crate::message::MessagePriority::Normal,
                hop_count: 0,
                seq: 0,
            };
            self.originate(announcement);
        }
    }

    pub fn status(&self) {
        let trust = if self.is_trusted() {
            "TRUSTED"
        } else {
            "ISOLATED"
        };
        let online = if self.is_online { "ONLINE" } else { "OFFLINE" };
        let rescue = self
            .persistent_messages
            .values()
            .filter(|m| m.message_type == "rescue")
            .count();
        let key_hex: String = self
            .identity
            .verifying_key
            .as_bytes()
            .iter()
            .take(6)
            .map(|b| format!("{:02x}", b))
            .collect();
        println!(
            "ID: {:10} | Zone: {:6} | Rep: {:.2} | {} | {} | Rescue: {} | PubKey: {}...",
            self.id, self.zone, self.reputation, trust, online, rescue, key_hex
        );
    }

    pub fn run_consensus(&mut self) {
        self.consensus_rounds += 1;
        let reporters = &self.observations;
        let n = reporters.len();
        if n == 0 {
            return;
        }

        let confident: Vec<u8> = reporters
            .iter()
            .filter(|(_, _, conf, unknown)| *conf >= 3 && !unknown)
            .map(|(_, sev, _, _)| *sev)
            .collect();

        if confident.len() < 2 {
            return;
        }

        let mut sorted = confident.clone();
        sorted.sort();
        let mid = sorted.len() / 2;
        let median = if sorted.len().is_multiple_of(2) {
            (sorted[mid - 1] + sorted[mid]) / 2
        } else {
            sorted[mid]
        };

        let mut w_total = 0.0f64;
        let mut w_faulty = 0.0f64;

        for (node_id, severity, confidence, unknown_vis) in reporters {
            let rep = self.sybil_guard.reputation(node_id) as f64;
            w_total += rep;

            if *unknown_vis || *confidence < 3 {
                w_faulty += rep;
                continue;
            }

            let deviation = (*severity as i16 - median as i16).abs();
            if deviation > 2 {
                w_faulty += rep;
            }
        }

        let w_total = (w_total * 1000.0).round() / 1000.0;
        let w_faulty = (w_faulty * 1000.0).round() / 1000.0;

        if w_total > 0.0 && 3.0 * w_faulty >= w_total {
            self.byzantine_events += 1;
        }
    }
}
