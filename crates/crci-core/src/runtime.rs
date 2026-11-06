use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::identity::Identity;
use crate::message::{Message, MessageType, Signal, Visibility};
use crate::metrics::CrciMetrics;
use crate::storage::{PersistedRescue, PersistedState};
use crate::transport::{LegacyTransport, SharedInbox, SimTransport};

pub const MIN_TRUSTED_REP: f64 = 0.41;

// ─── Anti-entropy (partition-heal re-sync) ───────────────────────────────────
//
// After a partition a node asks a neighbour for what it missed by sending a signed
// `sync_request` whose digest lists, per origin, the highest sequence number it accepted
// and every sequence number it holds inside the window. The neighbour answers with the
// messages it keeps (the last W per origin) that the digest lacks, oldest first.
//
// Every answer names the request it answers (`answers_sync`, a per-hop tag covered by the
// TCP frame signature, not by the origin's). An answer often repeats a message the same
// neighbour already relayed as gossip: the digest is a snapshot, and under packet loss the
// request can arrive seconds late. The requester therefore keeps a sync session per
// request and drops such a repeat without a strike if it is tagged with a request this
// node sent to that neighbour, the request's digest did *not* list the message as held,
// and the message was not excused under that request before; at most `SYNC_MAX_BATCH`
// per request (an answer cannot carry more). A repeat without the tag is never excused,
// so an open session is no licence to replay gossip. Sessions last `SYNC_SESSION_TTL`.
// Other duplicates are judged by the normal rules. A request is answered only when it
// comes straight from its author (a relayed copy is ignored), at most once per
// `SYNC_COOLDOWN` per requester.
//
// Sync requests and chain-head announcements are control messages: they go one hop, nobody keeps
// them, and a node sends them whether or not it has anything to say (a request per neighbour per
// round). They take their sequence numbers from a counter and a replay window of their own, so
// they never push the origin's messages out of its window of W.

/// Most messages one sync answer carries; below the token-bucket burst (20).
pub const SYNC_MAX_BATCH: usize = 16;
/// A requester answered less than this long ago gets no new answer.
pub const SYNC_COOLDOWN: Duration = Duration::from_secs(5);
/// Gap a requester leaves after its last request before asking again for a resumed link. The
/// cooldown runs from the arrival, which TCP can delay: this allows 3 s of retransmissions.
/// A refused request is not reported, and the next chance is the next anti-entropy round.
pub const SYNC_RESUME_SPACING: Duration = Duration::from_secs(8);
/// How long a request's answers are excused. Only frames tagged as answers benefit, so this
/// covers a request delayed by TCP retransmissions under heavy loss, not just a round trip.
pub const SYNC_SESSION_TTL: Duration = Duration::from_secs(120);
/// Open sessions kept per asked peer (the newest); bounds the digests held in memory.
pub const SYNC_SESSIONS_PER_PEER: usize = 4;
/// Most origins one digest lists.
pub const SYNC_MAX_ORIGINS: usize = 256;
/// Most origins whose recent messages a node keeps for answering sync requests.
pub const RECENT_MAX_ORIGINS: usize = 1024;
/// Answers a node sends in a burst, across all requesters. A request names where its
/// answer goes (`reply_to`), so this caps how much a node can be made to send to a third
/// party by requesters with many identities.
pub const SYNC_ANSWER_BURST: f32 = 4.0;
/// Answers per second added back to that budget.
pub const SYNC_ANSWERS_PER_SEC: f32 = 1.0;
/// Requesters remembered for the per-requester cooldown before old entries are pruned.
const SYNC_ANSWERED_MAX: usize = 1024;

/// A sync request this node sent and is waiting on.
pub struct SyncSession {
    /// The peer that was asked; its answers arrive from this identity.
    pub peer: String,
    pub opened_at: Instant,
    /// (origin, seq) pairs the request declared as already held.
    pub held: HashSet<(String, u64)>,
    /// (origin, seq) pairs already excused once in this session.
    pub exempted: HashSet<(String, u64)>,
}

/// True if `sender` is the request's author: the same node ID in-process, or the
/// authenticated TCP identity (`node_id#key`) of the signing key.
fn sender_is_author(sender: &str, wire: &WireMessage) -> bool {
    if sender == wire.origin {
        return true;
    }
    let key_hex: String = wire
        .origin_pubkey
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    sender == format!("{}#{}", wire.origin, key_hex)
}

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
    /// Present only on a `sync_request` (anti-entropy): what the requester already holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncDigest>,
    /// Set only by the neighbour that sends this copy as an answer to a sync request:
    /// the request's ID. Per hop, so not covered by the origin's signature (on TCP the
    /// frame signature covers it); removed on receipt, never stored or relayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers_sync: Option<String>,
}

/// Message type of an anti-entropy request. It is answered by its direct recipient and
/// never delivered, stored or forwarded.
pub const SYNC_REQUEST_TYPE: &str = "sync_request";

/// One-hop messages that are never kept: their sequence numbers are separate from the origin's
/// messages (see "Anti-entropy" above).
fn is_control(wire: &WireMessage) -> bool {
    wire.message_type == SYNC_REQUEST_TYPE || wire.message_type.contains("ChainHeadAnnouncement")
}

/// What a requester holds of one origin's stream: the highest accepted sequence number
/// and every sequence number accepted inside the window.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OriginDigest {
    pub origin: String,
    pub seq_max: u64,
    pub held: Vec<u64>,
}

/// Body of a `sync_request`, covered by the requester's signature.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SyncDigest {
    /// Where to send the answer when the requester's transport identity is not an
    /// address (TCP); `None` means "reply to the sender".
    pub reply_to: Option<String>,
    pub origins: Vec<OriginDigest>,
}

/// Domain-separation tag at the start of the bytes an origin signs for a `WireMessage`.
/// A signature made for any other purpose (with another tag) never verifies as a message.
pub const WIRE_SIGNING_DOMAIN: &[u8] = b"CRCI-WIRE-MESSAGE-v1\0";

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn put_bool(out: &mut Vec<u8>, value: bool) {
    out.push(u8::from(value));
}

impl WireMessage {
    /// Bytes covered by the origin's Ed25519 signature: the domain tag, then every field
    /// except the signature itself, in declaration order. Variable-length fields are
    /// prefixed with their length (u64, big-endian), lists with their element count, and
    /// the optional `note`, `sync` and `sync.reply_to` with a presence byte, so no two
    /// distinct messages encode to the same bytes. A relay cannot change any field
    /// without the signature failing; `seq` in particular cannot be moved, so a relay
    /// cannot shift the origin's replay window.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            WIRE_SIGNING_DOMAIN.len()
                + 64
                + self.id.len()
                + self.origin.len()
                + self.origin_pubkey.len()
                + self.visibility.len()
                + self.note.as_ref().map_or(0, String::len)
                + self.message_type.len(),
        );
        out.extend_from_slice(WIRE_SIGNING_DOMAIN);
        put_bytes(&mut out, self.id.as_bytes());
        put_bytes(&mut out, self.origin.as_bytes());
        put_bytes(&mut out, &self.origin_pubkey);
        out.push(self.severity);
        out.push(self.confidence);
        put_bool(&mut out, self.needs_help);
        put_bool(&mut out, self.can_help_others);
        put_bool(&mut out, self.location_confirmed);
        put_bytes(&mut out, self.visibility.as_bytes());
        match &self.note {
            None => out.push(0),
            Some(note) => {
                out.push(1);
                put_bytes(&mut out, note.as_bytes());
            }
        }
        put_bytes(&mut out, self.message_type.as_bytes());
        put_bool(&mut out, self.origin_active);
        out.extend_from_slice(&self.seq.to_be_bytes());
        match &self.sync {
            None => out.push(0),
            Some(digest) => {
                out.push(1);
                match &digest.reply_to {
                    None => out.push(0),
                    Some(addr) => {
                        out.push(1);
                        put_bytes(&mut out, addr.as_bytes());
                    }
                }
                out.extend_from_slice(&(digest.origins.len() as u64).to_be_bytes());
                for o in &digest.origins {
                    put_bytes(&mut out, o.origin.as_bytes());
                    out.extend_from_slice(&o.seq_max.to_be_bytes());
                    out.extend_from_slice(&(o.held.len() as u64).to_be_bytes());
                    for seq in &o.held {
                        out.extend_from_slice(&seq.to_be_bytes());
                    }
                }
            }
        }
        out
    }

    /// Replaces `signature_bytes` with `identity`'s signature over `signing_bytes()`.
    pub fn sign_with(&mut self, identity: &Identity) {
        self.signature_bytes = identity.sign(&self.signing_bytes()).to_bytes().to_vec();
    }

    pub fn from_message(msg: &Message, identity: &Identity) -> WireMessage {
        let mut wire = WireMessage {
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
            signature_bytes: Vec::new(),
            seq: msg.seq,
            sync: None,
            answers_sync: None,
        };
        wire.sign_with(identity);
        wire
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
        // Strict verification rejects small-order keys and non-canonical signatures, for
        // which one signature can verify for many messages.
        verifying_key
            .verify_strict(&self.signing_bytes(), &signature)
            .is_ok()
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
    pub seen_messages: HashMap<String, HashSet<String>>,
    pub persistent_messages: HashMap<String, WireMessage>,
    pub transport: SimTransport,
    pub inbox: SharedInbox,
    pub storage: crate::storage::legacy::NodeStorage,
    pub storage_backend: Option<std::sync::Arc<dyn crate::storage::StorageBackend>>,
    pub observations: Vec<(String, u8, u8, bool)>,
    pub known_keys: HashMap<String, Vec<u8>>,
    pub replay_filter: crate::replay::ReplayFilter,
    /// Replay window of received control messages (sync requests, chain-head announcements).
    pub control_replay_filter: crate::replay::ReplayFilter,
    pub seq_counter: u64,
    /// Sequence numbers of the control messages this node sends.
    pub control_seq_counter: u64,
    pub messages_handled: u64,
    pub byzantine_events: u64,
    pub consensus_rounds: u64,
    pub merkle: crate::merkle::MerkleChain,
    pub divergence_log: Vec<crate::merkle::DivergenceAlert>,
    pub divergence_tx: Option<tokio::sync::broadcast::Sender<crate::merkle::DivergenceAlert>>,
    pub router: crate::routing::TemporalRouter,
    /// When false, every outgoing message goes to all of `peers` and the router is not
    /// consulted. The router keys next hops by node ID, which a TCP node cannot address.
    pub use_router: bool,
    pub sybil_guard: crate::sybil::SybilGuard,
    pub sybil_banned_events: u64,
    pub sybil_rate_limited_events: u64,
    pub sybil_pow_failed_events: u64,
    /// New messages accepted per origin (`ORIGIN_BURST`, `ORIGIN_REFILL_PER_SEC`).
    /// shortcut: one entry per authenticated origin, like `known_keys`; bounded with the rest
    /// of the per-node state when that is capped.
    pub origin_budgets: HashMap<String, crate::sybil::TokenBucket>,
    /// When false, an origin may have any number of new messages accepted (baselines).
    pub origin_budget_enabled: bool,
    /// Messages dropped because their origin was over its budget.
    pub origin_rate_limited_events: u64,
    pub metrics: CrciMetrics,
    /// The last `window_size` accepted messages per origin, kept to answer sync requests.
    pub recent_messages: HashMap<String, BTreeMap<u64, WireMessage>>,
    /// Open sync sessions, keyed by request ID.
    pub sync_sessions: HashMap<String, SyncSession>,
    /// When each requester was last answered.
    pub sync_answered: HashMap<String, Instant>,
    /// Budget for answers across all requesters (`SYNC_ANSWER_BURST`, `SYNC_ANSWERS_PER_SEC`).
    pub sync_answer_budget: crate::sybil::TokenBucket,
    /// Same-sender duplicates excused as sync answers.
    pub sync_excused_duplicates: u64,
    /// The chain head last announced to peers; an unchanged head is not announced again.
    pub last_announced_head: Option<([u8; 32], u64)>,
}

/// Which defences a node applies. `Full` is the protocol as shipped; the others are the baselines
/// of the evaluation. Signature checks and the same-ID dedupe stay in every mode: they are
/// authentication and gossip's seen-cache, not reputation. Strikes are counted in every mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DefenceMode {
    /// Per-sender token bucket, per-origin budget, strikes and bans.
    #[default]
    Full,
    /// Per-sender token bucket only: no ban, no per-origin budget.
    StaticBucket,
    /// No admission limit, no ban, no per-origin budget.
    Vanilla,
}

impl std::str::FromStr for DefenceMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "full" => Ok(Self::Full),
            "static-bucket" => Ok(Self::StaticBucket),
            "vanilla" => Ok(Self::Vanilla),
            other => Err(format!(
                "unknown defence mode '{other}' (full, static-bucket, vanilla)"
            )),
        }
    }
}

impl NodeRuntime {
    /// Switch the defences (experiments only): see `DefenceMode`. Call before the first message.
    pub fn set_defence_mode(&mut self, mode: DefenceMode) {
        self.sybil_guard.bans_enabled = mode == DefenceMode::Full;
        self.sybil_guard.rate_limit_enabled = mode != DefenceMode::Vanilla;
        self.origin_budget_enabled = mode == DefenceMode::Full;
    }

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
            seen_messages: HashMap::new(),
            persistent_messages: HashMap::new(),
            transport,
            inbox,
            observations: Vec::new(),
            storage,
            storage_backend: None,
            known_keys: HashMap::new(),
            replay_filter: crate::replay::ReplayFilter::new(),
            control_replay_filter: crate::replay::ReplayFilter::new(),
            seq_counter: 0,
            control_seq_counter: 0,
            messages_handled: 0,
            byzantine_events: 0,
            consensus_rounds: 0,
            merkle: crate::merkle::MerkleChain::new(),
            divergence_log: Vec::new(),
            divergence_tx: None,
            router: crate::routing::TemporalRouter::new(),
            use_router: true,
            sybil_guard: crate::sybil::SybilGuard::new(4),
            sybil_banned_events: 0,
            sybil_rate_limited_events: 0,
            sybil_pow_failed_events: 0,
            origin_budgets: HashMap::new(),
            origin_budget_enabled: true,
            origin_rate_limited_events: 0,
            metrics: CrciMetrics::new(),
            recent_messages: HashMap::new(),
            sync_sessions: HashMap::new(),
            sync_answered: HashMap::new(),
            sync_answer_budget: crate::sybil::TokenBucket::new(
                SYNC_ANSWER_BURST,
                SYNC_ANSWERS_PER_SEC,
            ),
            sync_excused_duplicates: 0,
            last_announced_head: None,
        }
    }

    /// Keep `wire` among the recent messages of its origin (for answering sync requests).
    fn remember_recent(&mut self, wire: &WireMessage) {
        if is_control(wire) {
            return;
        }
        if !self.recent_messages.contains_key(&wire.origin)
            && self.recent_messages.len() >= RECENT_MAX_ORIGINS
        {
            // Bounded store: drop the origin whose newest message is oldest.
            let oldest = self
                .recent_messages
                .iter()
                .min_by_key(|(_, m)| m.keys().next_back().copied().unwrap_or(0))
                .map(|(o, _)| o.clone());
            if let Some(origin) = oldest {
                self.recent_messages.remove(&origin);
            }
        }
        let window = self.replay_filter.window_size;
        let per_origin = self.recent_messages.entry(wire.origin.clone()).or_default();
        per_origin.insert(wire.seq, wire.clone());
        let newest = per_origin.keys().next_back().copied().unwrap_or(0);
        let floor = newest.saturating_sub(window);
        per_origin.retain(|seq, _| *seq >= floor);
    }

    /// Build a signed sync request listing what this node holds. `reply_to` is the address
    /// to answer at when this node's transport identity is not an address (TCP).
    pub fn build_sync_request(&mut self, reply_to: Option<String>) -> WireMessage {
        self.control_seq_counter += 1;
        let mut msg = Message::new(
            &format!("sync-{}-{}", self.id, self.control_seq_counter),
            &self.id,
            Signal::new(1, false, false, false, 1, Visibility::Direct),
            None,
        );
        msg.seq = self.control_seq_counter;
        let mut wire = WireMessage::from_message(&msg, &self.identity);
        wire.message_type = SYNC_REQUEST_TYPE.to_string();
        wire.sync = Some(SyncDigest {
            reply_to,
            origins: self
                .replay_filter
                .digest(SYNC_MAX_ORIGINS)
                .into_iter()
                .map(|(origin, seq_max, held)| OriginDigest {
                    origin,
                    seq_max,
                    held,
                })
                .collect(),
        });
        wire.sign_with(&self.identity);
        wire
    }

    /// Record that `request` was sent to `peer` (the identity its answers will arrive from).
    pub fn open_sync_session(&mut self, peer: &str, request: &WireMessage) {
        let held = request
            .sync
            .iter()
            .flat_map(|d| d.origins.iter())
            .flat_map(|o| o.held.iter().map(move |seq| (o.origin.clone(), *seq)))
            .collect();
        let now = self.sybil_guard.now();
        self.sync_sessions
            .retain(|_, s| now.saturating_duration_since(s.opened_at) <= SYNC_SESSION_TTL);
        let mut to_peer: Vec<(Instant, String)> = self
            .sync_sessions
            .iter()
            .filter(|(_, s)| s.peer == peer)
            .map(|(id, s)| (s.opened_at, id.clone()))
            .collect();
        to_peer.sort();
        let excess = (to_peer.len() + 1).saturating_sub(SYNC_SESSIONS_PER_PEER);
        for (_, id) in to_peer.into_iter().take(excess) {
            self.sync_sessions.remove(&id);
        }
        self.sync_sessions.insert(
            request.id.clone(),
            SyncSession {
                peer: peer.to_string(),
                opened_at: now,
                held,
                exempted: HashSet::new(),
            },
        );
    }

    /// Ask in-process peer `peer` for the messages this node is missing. Returns the
    /// request's ID.
    pub fn request_sync(&mut self, peer: &str) -> String {
        let request = self.build_sync_request(None);
        if let Ok(payload) = serde_json::to_vec(&request) {
            self.transport.send(peer, &payload);
        }
        self.open_sync_session(peer, &request);
        request.id
    }

    /// Whether a same-sender duplicate of (`origin`, `seq`) from `sender`, tagged as an
    /// answer to request `answers`, is excused (at most once per message, and at most
    /// `SYNC_MAX_BATCH` messages, per request).
    fn excused_as_sync_answer(
        &mut self,
        sender: &str,
        answers: Option<&str>,
        origin: &str,
        seq: u64,
    ) -> bool {
        let now = self.sybil_guard.now();
        let Some(request_id) = answers else {
            return false;
        };
        let Some(session) = self.sync_sessions.get_mut(request_id) else {
            return false;
        };
        if session.peer != sender
            || now.saturating_duration_since(session.opened_at) > SYNC_SESSION_TTL
        {
            return false;
        }
        let key = (origin.to_string(), seq);
        if session.held.contains(&key)
            || session.exempted.len() >= SYNC_MAX_BATCH
            || !session.exempted.insert(key)
        {
            return false;
        }
        self.sync_excused_duplicates += 1;
        true
    }

    /// Answer a sync request from `sender` with up to `SYNC_MAX_BATCH` recent messages its
    /// digest lacks, oldest first.
    fn answer_sync(&mut self, sender: &str, request: &WireMessage) {
        let Some(digest) = &request.sync else {
            return;
        };
        if !sender_is_author(sender, request) {
            println!(
                "  [{}] sync request by '{}' relayed by '{}' — ignored",
                self.id, request.origin, sender
            );
            return;
        }
        let window = self.replay_filter.window_size;
        if digest.origins.len() > SYNC_MAX_ORIGINS
            || digest
                .origins
                .iter()
                .any(|o| o.held.len() as u64 > window + 1)
        {
            println!(
                "  [{}] oversized sync digest from '{}' — ignored",
                self.id, sender
            );
            return;
        }
        let now = self.sybil_guard.now();
        if let Some(last) = self.sync_answered.get(sender) {
            if now.saturating_duration_since(*last) < SYNC_COOLDOWN {
                return;
            }
        }
        if !self.sync_answer_budget.try_consume_at(now) {
            println!(
                "  [{}] sync answer budget exhausted — request from '{}' not answered",
                self.id, sender
            );
            return;
        }
        if self.sync_answered.len() >= SYNC_ANSWERED_MAX {
            self.sync_answered
                .retain(|_, t| now.saturating_duration_since(*t) < SYNC_COOLDOWN);
        }
        self.sync_answered.insert(sender.to_string(), now);

        let theirs: HashMap<&str, (u64, HashSet<u64>)> = digest
            .origins
            .iter()
            .map(|o| {
                (
                    o.origin.as_str(),
                    (o.seq_max, o.held.iter().copied().collect()),
                )
            })
            .collect();
        let mut origins: Vec<&String> = self.recent_messages.keys().collect();
        origins.sort();
        let mut batch: Vec<Vec<u8>> = Vec::new();
        'origins: for origin in origins {
            if *origin == request.origin {
                continue;
            }
            for (seq, wire) in &self.recent_messages[origin] {
                let wanted = match theirs.get(origin.as_str()) {
                    None => true,
                    Some((seq_max, held)) => *seq + window >= *seq_max && !held.contains(seq),
                };
                if wanted {
                    let mut answer = wire.clone();
                    answer.answers_sync = Some(request.id.clone());
                    if let Ok(payload) = serde_json::to_vec(&answer) {
                        batch.push(payload);
                    }
                    if batch.len() >= SYNC_MAX_BATCH {
                        break 'origins;
                    }
                }
            }
        }
        let dest = digest
            .reply_to
            .clone()
            .unwrap_or_else(|| sender.to_string());
        println!(
            "  🔄 [{}] answering sync request from '{}' with {} message(s)",
            self.id,
            sender,
            batch.len()
        );
        for payload in batch {
            self.transport.send(&dest, &payload);
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
                // Clamp to [0.0, 1.0] and round to 3 decimal places
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
        // Non-zero trust floor enforced via MIN_TRUSTED_REP
        self.reputation > MIN_TRUSTED_REP
    }

    pub fn penalize(&mut self) {
        let new_rep = self.reputation - 0.2;
        // Clamp to [0.0, 1.0] and round to 3 decimal places
        self.reputation = round_to_3_dec(new_rep.clamp(0.0, 1.0));
    }

    pub fn go_offline(&mut self) {
        self.is_online = false;
        println!("  ⚡ [{}] went OFFLINE", self.id);
    }

    pub fn originate(&mut self, mut msg: Message) {
        if matches!(msg.message_type, MessageType::ChainHeadAnnouncement { .. }) {
            self.control_seq_counter += 1;
            msg.seq = self.control_seq_counter;
        } else {
            self.seq_counter += 1;
            msg.seq = self.seq_counter;
        }
        if !self.is_online {
            println!("  [{}] OFFLINE — cannot originate", self.id);
            return;
        }
        let wire = WireMessage::from_message(&msg, &self.identity);
        self.remember_recent(&wire);
        let is_persistent = matches!(
            msg.message_type,
            MessageType::RescueRequest | MessageType::MassCasualtyEvent
        ) || msg.signal.can_help_others;
        if is_persistent {
            self.persistent_messages
                .insert(wire.id.clone(), wire.clone());
        }
        let mut senders = HashSet::new();
        senders.insert(self.id.clone());
        self.seen_messages.insert(wire.id.clone(), senders);
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

        let next_hop = if self.use_router {
            self.router.best_next_hop(&[], now_ms)
        } else {
            None
        };
        if let Some(best) = next_hop {
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
        let _ = self.process_inbox_delivered();
    }

    /// Process the inbox like `process_inbox` and return the messages that passed every
    /// check and were accepted, in arrival order (chain-head announcements excluded).
    pub fn process_inbox_delivered(&mut self) -> Vec<WireMessage> {
        let mut delivered = Vec::new();
        if !self.is_online {
            return delivered;
        }
        let messages: Vec<(String, Vec<u8>)> = {
            let mut inbox = match self.inbox.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            inbox.remove(&self.id).unwrap_or_default()
        };
        for (sender_id, raw) in messages {
            let mut wire: WireMessage = match serde_json::from_slice(&raw) {
                Ok(w) => w,
                Err(_) => {
                    println!("  [{}] received malformed bytes — dropped", self.id);
                    continue;
                }
            };
            // The answer tag belongs to this hop only: it is never stored or relayed.
            let answers = wire.answers_sync.take();

            // A banned sender's frames are dropped by the admission check below.
            if !self.sybil_guard.is_banned(&sender_id) {
                // BFT-046: Hard ceiling on observed message ID cache.
                // This eviction must not touch the per-sender budget (BFT-008): otherwise a peer
                // could refill its own budget by flooding enough IDs to trip the ceiling (BFT-047).
                if self.seen_messages.len() >= 5000 {
                    self.seen_messages.clear();
                    self.metrics
                        .seen_messages_evictions
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                // Copies of messages already received are judged by ID first, before the sender's
                // budget and before any signature check: every link carries every message, so these
                // copies are most of the traffic. A copy from another path is dropped for free; a
                // repeat from the same sender is a replay whatever its budget, so a sender that
                // keeps its budget empty cannot hide replays behind the rate limit. An honest node
                // sends each message ID to a neighbour once (sync answers are tagged).
                let repeat_from_sender = self
                    .seen_messages
                    .get(&wire.id)
                    .map(|senders| senders.contains(&sender_id));
                match repeat_from_sender {
                    Some(true) => {
                        // This specific peer has already sent us this exact message.
                        // Once the message is older than the origin's replay window it is
                        // stale (an honest peer may resend after a reconnect): drop, no strike.
                        let window = if is_control(&wire) {
                            &self.control_replay_filter
                        } else {
                            &self.replay_filter
                        };
                        if window.is_out_of_window(&wire.origin, wire.seq) {
                            println!(
                                "  ⏰ [{}] STALE resend of '{}' from '{}' — out of window, dropped",
                                self.id, wire.id, sender_id
                            );
                            continue;
                        }
                        // A message this node asked the sender for may cross with the same
                        // message relayed as ordinary gossip: drop, no strike (once).
                        if self.excused_as_sync_answer(
                            &sender_id,
                            answers.as_deref(),
                            &wire.origin,
                            wire.seq,
                        ) {
                            println!(
                                "  🔄 [{}] repeat of '{}' from '{}' answers a sync request — dropped, no strike",
                                self.id, wire.id, sender_id
                            );
                            continue;
                        }
                        // Inside the window this is a direct replay, not gossip redundancy.
                        println!(
                            "  ⚠ [{}] MALICIOUS REPLAY from '{}' — same message '{}' sent repeatedly!",
                            self.id, sender_id, wire.id
                        );
                        self.byzantine_events += 1;
                        self.sybil_guard.report_violation(&sender_id);
                        continue;
                    }
                    Some(false) => {
                        // Normal gossip duplicate from a DIFFERENT path. Silently drop without penalty.
                        if let Some(senders) = self.seen_messages.get_mut(&wire.id) {
                            senders.insert(sender_id.clone());
                        }
                        continue;
                    }
                    None => {}
                }
            }

            // Sybil admission check (new messages only)
            match self.sybil_guard.check(&sender_id) {
                Ok(()) => { /* proceed */ }
                Err(crate::sybil::SybilError::Banned(peer)) => {
                    self.sybil_banned_events += 1;
                    self.metrics
                        .sybil_banned_total
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} is banned",
                        self.id, peer
                    );
                    continue;
                }
                Err(crate::sybil::SybilError::RateLimited(peer)) => {
                    self.sybil_rate_limited_events += 1;
                    self.metrics
                        .sybil_rate_limited_total
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} exceeded rate limit",
                        self.id, peer
                    );
                    continue;
                }
                Err(crate::sybil::SybilError::PowFailed(peer)) => {
                    self.sybil_pow_failed_events += 1;
                    self.metrics
                        .sybil_pow_failed_total
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!(
                        "  ⚠ [{}] SYBIL GUARD DROP: peer {} PoW failed",
                        self.id, peer
                    );
                    continue;
                }
            }

            // BFT-008: signature checks of new messages are bounded per transport sender by the
            // SybilGuard token bucket above (it refills over time), and each invalid signature strikes
            // that sender. The claimed origin is not used: it is unauthenticated, so budgeting
            // by it would let any peer spend an honest origin's budget with forgeries.
            self.metrics
                .sig_verifications_performed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Step 1: verify cryptographic signature. Every message type is checked,
            // mass-casualty events included.
            if !wire.verify_signature() {
                println!(
                    "  ⚠ [{}] INVALID SIGNATURE on '{}' — dropped!",
                    self.id, wire.id
                );
                self.byzantine_events += 1;
                self.metrics
                    .byzantine_detected
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.metrics
                    .messages_rejected
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // Defend against signature-invalidation attacks:
                // Penalise the RELAYING NEIGHBOR for forwarding cryptographically invalid data.
                self.sybil_guard.report_violation(&sender_id);
                continue;
            }

            // First time seeing this message. Record the sender.
            let mut new_senders = HashSet::new();
            new_senders.insert(sender_id.clone());
            self.seen_messages.insert(wire.id.clone(), new_senders);

            // This node's own message, echoed by a neighbour after the seen-ID limit above cleared
            // the cache. Accepting it would relay it again and get this node struck for the repeat.
            if wire.origin == self.id {
                continue;
            }

            // Step 2: pubkey consistency check. The first key seen for an origin ID is pinned;
            // a validly signed message claiming that origin with another key is dropped.
            // It is not a strike: the receiver cannot tell which key is the genuine one, so
            // the relay is not at fault. Striking it would let an adversary that announces an
            // honest node's ID first get every honest relay of that node's messages banned.
            if let Some(known) = self.known_keys.get(&wire.origin) {
                if *known != wire.origin_pubkey {
                    println!(
                        "  ⚠ [{}] PUBKEY MISMATCH on '{}' (origin '{}' is pinned to another key) — dropped, no strike",
                        self.id, wire.id, wire.origin
                    );
                    self.byzantine_events += 1;
                    self.metrics
                        .byzantine_detected
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.metrics
                        .messages_rejected
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    continue;
                }
            } else {
                self.known_keys
                    .insert(wire.origin.clone(), wire.origin_pubkey.clone());
            }

            // Per-origin budget, charged only now that signature and pinned key authenticate
            // the origin. Over budget: dropped and forgotten, no strike (the relay is not at
            // fault), so a later copy or sync answer is accepted once the budget refills.
            let now = self.sybil_guard.now();
            let origin_budget = self
                .origin_budgets
                .entry(wire.origin.clone())
                .or_insert_with(|| crate::sybil::TokenBucket {
                    tokens: crate::sybil::ORIGIN_BURST,
                    capacity: crate::sybil::ORIGIN_BURST,
                    refill_rate: crate::sybil::ORIGIN_REFILL_PER_SEC,
                    last_refill: now,
                });
            if !origin_budget.try_consume_at(now) && self.origin_budget_enabled {
                self.seen_messages.remove(&wire.id);
                self.origin_rate_limited_events += 1;
                println!(
                    "  ⚠ [{}] ORIGIN BUDGET DROP: '{}' from origin '{}' via '{}'",
                    self.id, wire.id, wire.origin, sender_id
                );
                continue;
            }

            let window = if is_control(&wire) {
                &mut self.control_replay_filter
            } else {
                &mut self.replay_filter
            };
            let verdict = window.check_and_record(&wire.origin, wire.seq, &sender_id);
            if !verdict.is_accept() {
                self.metrics
                    .messages_rejected
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                match &verdict {
                    // Only a repeat from the same transport peer inside the window is a strike.
                    crate::replay::ReplayVerdict::Replayed { received_seq } => {
                        if self.excused_as_sync_answer(
                            &sender_id,
                            answers.as_deref(),
                            &wire.origin,
                            wire.seq,
                        ) {
                            println!(
                                "  🔄 [{}] seq {} of '{}' again from '{}' answers a sync request — dropped, no strike",
                                self.id, received_seq, wire.origin, sender_id
                            );
                        } else {
                            println!(
                                "  ⛔ [{}] REPLAY dropped from '{}' (claimed '{}') — seq {} already delivered by this peer",
                                self.id, sender_id, wire.origin, received_seq
                            );
                            self.byzantine_events += 1;
                            self.sybil_guard.report_violation(&sender_id);
                        }
                    }
                    crate::replay::ReplayVerdict::Redundant { received_seq } => println!(
                        "  [{}] duplicate seq {} from '{}' via '{}' — dropped",
                        self.id, received_seq, wire.origin, sender_id
                    ),
                    // Honest peers can deliver old messages after a partition: no strike.
                    crate::replay::ReplayVerdict::OutOfWindow {
                        received_seq,
                        window_floor,
                    } => println!(
                        "  ⏰ [{}] STALE seq {} from '{}' (window floor {}) — dropped",
                        self.id, received_seq, wire.origin, window_floor
                    ),
                    crate::replay::ReplayVerdict::Accept => {}
                }
                continue;
            }

            // Anti-entropy requests are answered, never delivered, stored or forwarded.
            if wire.message_type == SYNC_REQUEST_TYPE {
                self.answer_sync(&sender_id, &wire);
                continue;
            }

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
            self.remember_recent(&wire);

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
            self.metrics
                .messages_accepted
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            delivered.push(wire.clone());

            let payload = match serde_json::to_vec(&wire) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            self.router.evict_stale(now_ms);

            let next_hop = if self.use_router {
                self.router.best_next_hop(&[&wire.origin], now_ms)
            } else {
                None
            };
            if let Some(best) = next_hop {
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

        // Announce the chain head after a pass that delivered at least one message, if it
        // changed since the last announcement (it changes every 50 delivered messages). A
        // pass that only received announcements (or sync traffic) does not announce:
        // otherwise two neighbours would answer each other's announcements forever. An
        // unchanged head is not repeated: it carries no information and would double the
        // frames on every link, halving the message rate the per-neighbour budget allows.
        let head = self.chain_head();
        if !delivered.is_empty() && self.last_announced_head != Some(head) {
            self.last_announced_head = Some(head);
            let (head_hash, head_seq) = head;
            let announcement = Message {
                id: format!("head-{}-{}", self.id, self.control_seq_counter + 1),
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
        delivered
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
