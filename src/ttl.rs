//! Message TTL enforcement and storage pruning for CRCI.
//!
//! Every message has a TTL (time-to-live) in rounds. When a message
//! expires, it is removed from active storage but a tombstone is kept
//! to prevent re-delivery. Rescue messages have an extended TTL and
//! are NEVER pruned while a rescue is still active.
//!
//! Storage is bounded: once MAX_STORED_MESSAGES is reached, oldest
//! non-rescue messages are evicted first.

use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────

/// Default TTL for normal messages (rounds).
const DEFAULT_TTL_ROUNDS: u64 = 50;

/// TTL for rescue messages (rounds) — much longer.
const RESCUE_TTL_ROUNDS: u64 = 500;

/// Max messages in active storage per node.
const MAX_STORED_MESSAGES: usize = 200;

/// Max tombstones kept (for deduplication after expiry).
const MAX_TOMBSTONES: usize = 1000;

// ── Message record ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub created_round: u64,
    pub ttl_rounds: u64,
    pub is_rescue: bool,
    pub rescue_resolved: bool,
}

impl StoredMessage {
    pub fn new_normal(id: &str, round: u64) -> Self {
        StoredMessage {
            id: id.to_string(),
            created_round: round,
            ttl_rounds: DEFAULT_TTL_ROUNDS,
            is_rescue: false,
            rescue_resolved: false,
        }
    }

    pub fn new_rescue(id: &str, round: u64) -> Self {
        StoredMessage {
            id: id.to_string(),
            created_round: round,
            ttl_rounds: RESCUE_TTL_ROUNDS,
            is_rescue: true,
            rescue_resolved: false,
        }
    }

    /// Is this message expired at the given round?
    pub fn is_expired(&self, current_round: u64) -> bool {
        // Rescue messages that are unresolved never expire
        if self.is_rescue && !self.rescue_resolved {
            return false;
        }
        current_round.saturating_sub(self.created_round) >= self.ttl_rounds
    }

    pub fn age_rounds(&self, current_round: u64) -> u64 {
        current_round.saturating_sub(self.created_round)
    }
}

// ── TTL store ─────────────────────────────────────────────────────

/// Per-node message store with TTL enforcement.
#[derive(Default)]
pub struct TtlStore {
    /// Active messages
    messages: HashMap<String, StoredMessage>,
    /// Tombstones: message IDs that have been pruned (for dedup)
    tombstones: Vec<String>,
    /// Total messages ever stored
    pub total_stored: u64,
    /// Total messages pruned by TTL
    pub total_pruned: u64,
    /// Total messages evicted due to storage cap
    pub total_evicted: u64,
}

impl TtlStore {
    pub fn new() -> Self {
        TtlStore::default()
    }

    /// Store a message. Returns false if already present or tombstoned.
    pub fn store(&mut self, msg: StoredMessage) -> bool {
        if self.messages.contains_key(&msg.id) {
            return false;
        }
        if self.tombstones.contains(&msg.id) {
            return false; // previously pruned — do not re-admit
        }
        // Enforce storage cap: evict oldest non-rescue if full
        if self.messages.len() >= MAX_STORED_MESSAGES {
            self.evict_oldest_normal();
        }
        self.total_stored += 1;
        self.messages.insert(msg.id.clone(), msg);
        true
    }

    /// Mark a rescue as resolved — it will now expire normally.
    pub fn resolve_rescue(&mut self, id: &str) {
        if let Some(msg) = self.messages.get_mut(id) {
            msg.rescue_resolved = true;
        }
    }

    /// Prune expired messages. Returns count pruned this pass.
    pub fn prune(&mut self, current_round: u64) -> usize {
        let expired: Vec<String> = self
            .messages
            .values()
            .filter(|m| m.is_expired(current_round))
            .map(|m| m.id.clone())
            .collect();

        let count = expired.len();
        for id in &expired {
            self.messages.remove(id);
            // Add tombstone
            if self.tombstones.len() >= MAX_TOMBSTONES {
                self.tombstones.remove(0); // drop oldest tombstone
            }
            self.tombstones.push(id.clone());
        }
        self.total_pruned += count as u64;
        count
    }

    /// Is a message ID known (active or tombstoned)?
    pub fn is_known(&self, id: &str) -> bool {
        self.messages.contains_key(id) || self.tombstones.contains(&id.to_string())
    }

    pub fn active_count(&self) -> usize {
        self.messages.len()
    }

    pub fn rescue_count(&self) -> usize {
        self.messages
            .values()
            .filter(|m| m.is_rescue && !m.rescue_resolved)
            .count()
    }

    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }

    fn evict_oldest_normal(&mut self) {
        // Find oldest non-rescue message
        let oldest_id = self
            .messages
            .values()
            .filter(|m| !m.is_rescue)
            .min_by_key(|m| m.created_round)
            .map(|m| m.id.clone());

        if let Some(id) = oldest_id {
            self.messages.remove(&id);
            self.total_evicted += 1;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_message_expires() {
        let msg = StoredMessage::new_normal("msg-1", 0);
        assert!(!msg.is_expired(49));
        assert!(msg.is_expired(50));
    }

    #[test]
    fn test_rescue_never_expires_while_active() {
        let msg = StoredMessage::new_rescue("rescue-1", 0);
        // Even after 1000 rounds — unresolved rescue survives
        assert!(!msg.is_expired(1000));
    }

    #[test]
    fn test_rescue_expires_after_resolved() {
        let mut store = TtlStore::new();
        store.store(StoredMessage::new_rescue("rescue-2", 0));
        store.resolve_rescue("rescue-2");
        let pruned = store.prune(500);
        assert_eq!(pruned, 1);
        assert_eq!(store.rescue_count(), 0);
    }

    #[test]
    fn test_prune_removes_expired_normal() {
        let mut store = TtlStore::new();
        store.store(StoredMessage::new_normal("old-msg", 0));
        store.store(StoredMessage::new_normal("new-msg", 100));
        let pruned = store.prune(60); // old-msg expires (age 60 >= 50)
        assert_eq!(pruned, 1);
        assert_eq!(store.active_count(), 1);
    }

    #[test]
    fn test_tombstone_prevents_readmission() {
        let mut store = TtlStore::new();
        store.store(StoredMessage::new_normal("ephemeral", 0));
        store.prune(100); // expires and tombstoned
        let readmitted = store.store(StoredMessage::new_normal("ephemeral", 200));
        assert!(!readmitted); // tombstone blocks re-admission
    }

    #[test]
    fn test_storage_cap_evicts_normal_before_rescue() {
        let mut store = TtlStore::new();
        // Fill to cap with normal messages
        for i in 0..MAX_STORED_MESSAGES {
            store.store(StoredMessage::new_normal(&format!("n-{i}"), i as u64));
        }
        // Add a rescue — should evict oldest normal, not rescue
        store.store(StoredMessage::new_rescue("rescue-cap", 999));
        assert!(store.is_known("rescue-cap"));
        assert_eq!(store.total_evicted, 1);
    }

    #[test]
    fn test_duplicate_not_stored_twice() {
        let mut store = TtlStore::new();
        let r1 = store.store(StoredMessage::new_normal("dup", 0));
        let r2 = store.store(StoredMessage::new_normal("dup", 0));
        assert!(r1);
        assert!(!r2);
        assert_eq!(store.active_count(), 1);
    }
}
