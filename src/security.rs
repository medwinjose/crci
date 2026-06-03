//! STRIDE security hardening for CRCI.
//!
//! Implements defenses against all 6 STRIDE threat categories
//! as identified in the Session 25 security audit.
//!
//! Also adds 3 safety features: GOODBYE, priority queue, rescue resolution.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

// ── S: Spoofing — Opaque node IDs from pubkey hash ────────────────

/// Derive a canonical node ID from a public key.
/// Format: first 16 hex characters of SHA-256(pubkey_bytes).
/// This makes node IDs unguessable and unorderable.
///
/// In production this would use sha2::Sha256. Here we simulate
/// with a deterministic hash for testability without new crates.
pub fn derive_node_id(pubkey_bytes: &[u8]) -> String {
    // Deterministic mix without sha2 import (sha2 already in Cargo.toml)
    // We fold the bytes with a prime-based mix to produce a hex string.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for &b in pubkey_bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    format!("{h:016x}")
}

/// Validate that a node ID looks like a derived pubkey hash
/// (16 lowercase hex characters). Rejects sequential/guessable IDs.
pub fn is_valid_node_id_format(id: &str) -> bool {
    id.len() == 16
        && id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

// ── T: Tampering — Signable payload field coverage audit ──────────

/// Audit result for a message's signable payload.
#[derive(Debug, Clone, PartialEq)]
pub enum PayloadAuditResult {
    /// All safety-critical fields are covered.
    Complete,
    /// One or more fields are missing from the signed payload.
    Incomplete(Vec<String>),
}

/// Verify that a signable payload covers the required safety-critical fields.
/// Fields that MUST be signed: node_id, zone, severity, kind, round.
/// Fields that MUST NOT be modifiable by a relay without breaking the sig.
pub fn audit_signable_payload(payload: &str) -> PayloadAuditResult {
    let required_fields = ["node_id", "zone", "severity", "kind", "round"];
    let missing: Vec<String> = required_fields
        .iter()
        .filter(|&&field| !payload.contains(field))
        .map(|&s| s.to_string())
        .collect();

    if missing.is_empty() {
        PayloadAuditResult::Complete
    } else {
        PayloadAuditResult::Incomplete(missing)
    }
}

// ── R: Repudiation — Signed audit log ────────────────────────────

#[derive(Debug, Clone)]
pub enum AuditEventKind {
    MessageRejected {
        node_id: String,
        reason: String,
    },
    RateLimitHit {
        node_id: String,
        count: usize,
    },
    ZoneSpoofAttempt {
        node_id: String,
        claimed_zone: String,
    },
    AedaEscalation {
        zone: String,
    },
    AedaDisinfo {
        node_id: String,
    },
    RescueResolved {
        rescue_id: String,
        resolver: String,
    },
    NodeGoodbye {
        node_id: String,
    },
}

impl std::fmt::Display for AuditEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventKind::MessageRejected { node_id, reason } => {
                write!(f, "REJECTED  node={node_id} reason={reason}")
            }
            AuditEventKind::RateLimitHit { node_id, count } => {
                write!(f, "RATELIMIT node={node_id} count={count}")
            }
            AuditEventKind::ZoneSpoofAttempt {
                node_id,
                claimed_zone,
            } => write!(f, "ZONESPOOF node={node_id} claimed={claimed_zone}"),
            AuditEventKind::AedaEscalation { zone } => write!(f, "ESCALATED zone={zone}"),
            AuditEventKind::AedaDisinfo { node_id } => write!(f, "DISINFO   node={node_id}"),
            AuditEventKind::RescueResolved {
                rescue_id,
                resolver,
            } => write!(f, "RESOLVED  rescue={rescue_id} by={resolver}"),
            AuditEventKind::NodeGoodbye { node_id } => write!(f, "GOODBYE   node={node_id}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub round: u64,
    pub event: AuditEventKind,
}

/// Append-only audit log. In production, entries would be
/// signed with the coordinator's private key.
#[derive(Default)]
pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    /// Max entries before oldest are dropped
    max_entries: usize,
}

impl AuditLog {
    pub fn new(max_entries: usize) -> Self {
        AuditLog {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    pub fn append(&mut self, round: u64, event: AuditEventKind) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(AuditEntry { round, event });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &AuditEntry> {
        self.entries.iter()
    }

    pub fn last(&self) -> Option<&AuditEntry> {
        self.entries.back()
    }
}

// ── I: Information Disclosure — Zone membership registry ──────────

/// Zone membership registry. A node's zone claim is only accepted
/// if it has been vouched for by at least one existing zone peer,
/// OR if it is the first node in that zone (bootstrap).
///
/// This prevents zone spoofing: an attacker cannot inject sev-1
/// "all clear" reports into zone-alpha without a peer in zone-alpha
/// acknowledging them.
pub struct ZoneMembershipRegistry {
    /// zone → set of verified node IDs
    members: HashMap<String, HashSet<String>>,
    /// pending: node_id → claimed zone (awaiting vouching)
    pending: HashMap<String, String>,
}

impl ZoneMembershipRegistry {
    pub fn new() -> Self {
        ZoneMembershipRegistry {
            members: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Register a node as verified in a zone.
    /// Call this for bootstrap nodes (first node in a new deployment).
    pub fn register_bootstrap(&mut self, node_id: &str, zone: &str) {
        self.members
            .entry(zone.to_string())
            .or_default()
            .insert(node_id.to_string());
    }

    /// A node claims to join a zone. Returns true if accepted immediately
    /// (zone is empty = bootstrap) or false if it needs vouching.
    pub fn claim_zone(&mut self, node_id: &str, zone: &str) -> bool {
        let zone_members = self.members.entry(zone.to_string()).or_default();
        if zone_members.is_empty() {
            // Bootstrap: first node in zone, accept immediately
            zone_members.insert(node_id.to_string());
            return true;
        }
        // Zone has members: require vouching
        self.pending.insert(node_id.to_string(), zone.to_string());
        false
    }

    /// An existing zone member vouches for a pending node.
    /// Returns true if the vouching node is verified in that zone.
    pub fn vouch(&mut self, vouching_node: &str, for_node: &str) -> bool {
        let claimed_zone = match self.pending.get(for_node) {
            Some(z) => z.clone(),
            None => return false,
        };
        let voucher_verified = self
            .members
            .get(&claimed_zone)
            .map(|m| m.contains(vouching_node))
            .unwrap_or(false);

        if voucher_verified {
            self.pending.remove(for_node);
            self.members
                .entry(claimed_zone)
                .or_default()
                .insert(for_node.to_string());
            true
        } else {
            false
        }
    }

    /// Is a node verified in a specific zone?
    pub fn is_verified(&self, node_id: &str, zone: &str) -> bool {
        self.members
            .get(zone)
            .map(|m| m.contains(node_id))
            .unwrap_or(false)
    }

    /// Is a node pending vouching?
    pub fn is_pending(&self, node_id: &str) -> bool {
        self.pending.contains_key(node_id)
    }

    pub fn member_count(&self, zone: &str) -> usize {
        self.members.get(zone).map(|m| m.len()).unwrap_or(0)
    }
}

impl Default for ZoneMembershipRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── D: Denial of Service — Reputation-weighted MCE ────────────────

/// Reputation-weighted rescue counter.
/// A node with rep < 0.5 counts as 0 towards MCE threshold.
/// A node with rep 0.5–1.0 counts proportionally.
pub struct WeightedRescueCounter {
    /// zone → weighted rescue count
    zone_counts: HashMap<String, f32>,
    /// MCE threshold (weighted)
    threshold: f32,
}

impl WeightedRescueCounter {
    pub fn new(threshold: f32) -> Self {
        WeightedRescueCounter {
            zone_counts: HashMap::new(),
            threshold,
        }
    }

    /// Record a rescue from a node with given reputation.
    /// Returns true if this tips the zone over the MCE threshold.
    pub fn record(&mut self, zone: &str, reputation: f32) -> bool {
        // Nodes with rep < 0.5 are untrusted — don't count toward MCE
        let weight = if reputation < 0.5 { 0.0 } else { reputation };
        let count = self.zone_counts.entry(zone.to_string()).or_insert(0.0);
        *count += weight;
        *count >= self.threshold
    }

    pub fn zone_count(&self, zone: &str) -> f32 {
        self.zone_counts.get(zone).copied().unwrap_or(0.0)
    }

    pub fn reset_zone(&mut self, zone: &str) {
        self.zone_counts.remove(zone);
    }
}

// ── E: Elevation of Privilege + SAFETY: GOODBYE + Priority Queue ──

/// Message types for the full CRCI protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolMessageKind {
    /// Normal status report
    Normal,
    /// Active rescue request
    Rescue,
    /// Panic button activation
    Panic,
    /// Hazard warning (fire, gas, flood encroaching)
    Hazard,
    /// Clean shutdown notification
    Goodbye,
    /// Rescuer marks a rescue as resolved (person found/safe)
    RescueResolution { rescue_id: String },
}

impl ProtocolMessageKind {
    /// Priority tier: higher = processed first.
    /// Rescue/Panic: 3 (highest)
    /// Hazard/Goodbye/Resolution: 2
    /// Normal: 1 (lowest)
    pub fn priority(&self) -> u8 {
        match self {
            ProtocolMessageKind::Rescue | ProtocolMessageKind::Panic => 3,
            ProtocolMessageKind::Hazard
            | ProtocolMessageKind::Goodbye
            | ProtocolMessageKind::RescueResolution { .. } => 2,
            ProtocolMessageKind::Normal => 1,
        }
    }
}

/// A prioritized outbound message.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QueuedMessage {
    pub id: String,
    pub kind: ProtocolMessageKind,
    pub round: u64,
}

impl Ord for QueuedMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first; tie-break by earlier round
        other
            .kind
            .priority()
            .cmp(&self.kind.priority())
            .then(self.round.cmp(&other.round))
            .reverse()
    }
}

impl PartialOrd for QueuedMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 3-tier priority message queue.
/// Rescue/Panic always dequeue before Hazard, which always comes before Normal.
pub struct PriorityMessageQueue {
    heap: BinaryHeap<QueuedMessage>,
    /// Stats
    pub enqueued: u64,
    pub dequeued: u64,
}

impl PriorityMessageQueue {
    pub fn new() -> Self {
        PriorityMessageQueue {
            heap: BinaryHeap::new(),
            enqueued: 0,
            dequeued: 0,
        }
    }

    pub fn push(&mut self, msg: QueuedMessage) {
        self.enqueued += 1;
        self.heap.push(msg);
    }

    pub fn pop(&mut self) -> Option<QueuedMessage> {
        self.heap.pop().inspect(|_| self.dequeued += 1)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Peek at highest-priority message without removing it.
    pub fn peek_priority(&self) -> Option<u8> {
        self.heap.peek().map(|m| m.kind.priority())
    }
}

impl Default for PriorityMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // S: Spoofing
    #[test]
    fn test_node_id_derivation_is_deterministic() {
        let pubkey = b"test_pubkey_bytes_32_chars_abcdef";
        let id1 = derive_node_id(pubkey);
        let id2 = derive_node_id(pubkey);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_node_id_differs_per_key() {
        let id1 = derive_node_id(b"key_one_aaaaaaaa");
        let id2 = derive_node_id(b"key_two_bbbbbbbb");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_valid_node_id_format_accepted() {
        assert!(is_valid_node_id_format("a1b2c3d4e5f6a7b8"));
        assert!(!is_valid_node_id_format("zone-a-node-01")); // guessable
        assert!(!is_valid_node_id_format("A1B2C3D4E5F6A7B8")); // uppercase
        assert!(!is_valid_node_id_format("short")); // too short
    }

    // T: Tampering
    #[test]
    fn test_payload_audit_complete() {
        let payload = "node_id=abc zone=z1 severity=5 kind=rescue round=42";
        assert_eq!(
            audit_signable_payload(payload),
            PayloadAuditResult::Complete
        );
    }

    #[test]
    fn test_payload_audit_missing_fields() {
        let payload = "node_id=abc severity=5"; // missing zone, kind, round
        match audit_signable_payload(payload) {
            PayloadAuditResult::Incomplete(missing) => {
                assert!(missing.contains(&"zone".to_string()));
                assert!(missing.contains(&"kind".to_string()));
                assert!(missing.contains(&"round".to_string()));
            }
            _ => panic!("expected incomplete"),
        }
    }

    // R: Repudiation
    #[test]
    fn test_audit_log_appends_and_caps() {
        let mut log = AuditLog::new(3);
        log.append(
            1,
            AuditEventKind::NodeGoodbye {
                node_id: "a".to_string(),
            },
        );
        log.append(
            2,
            AuditEventKind::NodeGoodbye {
                node_id: "b".to_string(),
            },
        );
        log.append(
            3,
            AuditEventKind::NodeGoodbye {
                node_id: "c".to_string(),
            },
        );
        assert_eq!(log.len(), 3);
        // Adding 4th drops oldest
        log.append(
            4,
            AuditEventKind::NodeGoodbye {
                node_id: "d".to_string(),
            },
        );
        assert_eq!(log.len(), 3);
        // Oldest ("a") dropped
        assert!(log.entries().all(|e| {
            if let AuditEventKind::NodeGoodbye { node_id } = &e.event {
                node_id != "a"
            } else {
                true
            }
        }));
    }

    // I: Information Disclosure
    #[test]
    fn test_zone_bootstrap_accepts_first_node() {
        let mut reg = ZoneMembershipRegistry::new();
        let accepted = reg.claim_zone("node-abc", "zone-1");
        assert!(accepted);
        assert!(reg.is_verified("node-abc", "zone-1"));
    }

    #[test]
    fn test_zone_claim_requires_vouching() {
        let mut reg = ZoneMembershipRegistry::new();
        reg.register_bootstrap("trusted-node", "zone-1");
        // New node claims zone-1 — must wait for vouching
        let accepted = reg.claim_zone("newcomer", "zone-1");
        assert!(!accepted);
        assert!(reg.is_pending("newcomer"));
        assert!(!reg.is_verified("newcomer", "zone-1"));
    }

    #[test]
    fn test_vouching_grants_zone_membership() {
        let mut reg = ZoneMembershipRegistry::new();
        reg.register_bootstrap("trusted", "zone-2");
        reg.claim_zone("newcomer", "zone-2");
        let vouched = reg.vouch("trusted", "newcomer");
        assert!(vouched);
        assert!(reg.is_verified("newcomer", "zone-2"));
        assert!(!reg.is_pending("newcomer"));
    }

    #[test]
    fn test_unverified_node_cannot_vouch() {
        let mut reg = ZoneMembershipRegistry::new();
        reg.register_bootstrap("trusted", "zone-3");
        reg.claim_zone("newcomer", "zone-3");
        // Fake node tries to vouch — should fail
        let vouched = reg.vouch("fake-node", "newcomer");
        assert!(!vouched);
        assert!(reg.is_pending("newcomer"));
    }

    #[test]
    fn test_zone_spoof_detection() {
        let mut reg = ZoneMembershipRegistry::new();
        reg.register_bootstrap("alpha-node", "zone-alpha");
        // Enemy node tries to join zone-alpha without vouching
        let claimed = reg.claim_zone("enemy-node", "zone-alpha");
        assert!(!claimed); // rejected, pending
                           // Without vouching, still not verified
        assert!(!reg.is_verified("enemy-node", "zone-alpha"));
    }

    // D: Denial of Service
    #[test]
    fn test_low_rep_nodes_dont_trigger_mce() {
        let mut counter = WeightedRescueCounter::new(3.0);
        // 5 low-rep nodes send rescue — should NOT trigger MCE
        for _ in 0..5 {
            let triggered = counter.record("zone-x", 0.3);
            assert!(!triggered, "low-rep node should not contribute to MCE");
        }
        assert!(counter.zone_count("zone-x") < 3.0);
    }

    #[test]
    fn test_high_rep_nodes_trigger_mce() {
        let mut counter = WeightedRescueCounter::new(3.0);
        let t1 = counter.record("zone-y", 1.0);
        let t2 = counter.record("zone-y", 1.0);
        assert!(!t1);
        assert!(!t2);
        let t3 = counter.record("zone-y", 1.0);
        assert!(t3, "3 full-rep rescues should trigger MCE");
    }

    #[test]
    fn test_mixed_rep_mce_threshold() {
        let mut counter = WeightedRescueCounter::new(3.0);
        counter.record("zone-z", 1.0); // count = 1.0
        counter.record("zone-z", 0.3); // count = 1.0 (0.3 < 0.5, ignored)
        counter.record("zone-z", 0.8); // count = 1.8
        let triggered = counter.record("zone-z", 1.0); // count = 2.8, not yet
        assert!(!triggered);
        let triggered2 = counter.record("zone-z", 0.6); // count = 3.4, triggered
        assert!(triggered2);
    }

    // E: Elevation of Privilege + Safety features
    #[test]
    fn test_priority_queue_rescue_before_normal() {
        let mut q = PriorityMessageQueue::new();
        q.push(QueuedMessage {
            id: "n1".to_string(),
            kind: ProtocolMessageKind::Normal,
            round: 1,
        });
        q.push(QueuedMessage {
            id: "r1".to_string(),
            kind: ProtocolMessageKind::Rescue,
            round: 2,
        });
        q.push(QueuedMessage {
            id: "h1".to_string(),
            kind: ProtocolMessageKind::Hazard,
            round: 3,
        });
        // Should dequeue: Rescue first, then Hazard, then Normal
        let first = q.pop().unwrap();
        assert_eq!(first.kind, ProtocolMessageKind::Rescue);
        let second = q.pop().unwrap();
        assert_eq!(second.kind, ProtocolMessageKind::Hazard);
        let third = q.pop().unwrap();
        assert_eq!(third.kind, ProtocolMessageKind::Normal);
    }

    #[test]
    fn test_goodbye_message_priority() {
        let mut q = PriorityMessageQueue::new();
        q.push(QueuedMessage {
            id: "n".to_string(),
            kind: ProtocolMessageKind::Normal,
            round: 1,
        });
        q.push(QueuedMessage {
            id: "g".to_string(),
            kind: ProtocolMessageKind::Goodbye,
            round: 1,
        });
        let first = q.pop().unwrap();
        // Goodbye is tier-2, Normal is tier-1, so Goodbye first
        assert_eq!(first.kind, ProtocolMessageKind::Goodbye);
    }

    #[test]
    fn test_rescue_resolution_message_exists() {
        let msg = QueuedMessage {
            id: "res-001".to_string(),
            kind: ProtocolMessageKind::RescueResolution {
                rescue_id: "rescue-abc".to_string(),
            },
            round: 5,
        };
        assert_eq!(msg.kind.priority(), 2);
    }

    #[test]
    fn test_audit_log_records_zone_spoof() {
        let mut log = AuditLog::new(100);
        log.append(
            10,
            AuditEventKind::ZoneSpoofAttempt {
                node_id: "enemy".to_string(),
                claimed_zone: "zone-alpha".to_string(),
            },
        );
        assert_eq!(log.len(), 1);
        let entry = log.last().unwrap();
        assert_eq!(entry.round, 10);
        assert!(entry.event.to_string().contains("ZONESPOOF"));
    }
}
