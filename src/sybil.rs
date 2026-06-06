use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Instant;

pub type NodeId = String;

// ── PoW challenge ────────────────────────────────────────────────────────────
pub struct PowChallenge {
    pub node_id: NodeId,
    pub nonce: u64,
    pub difficulty: u8,
}

impl PowChallenge {
    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.node_id.as_bytes());
        hasher.update(self.nonce.to_le_bytes());
        let hash = hasher.finalize();

        if self.difficulty == 0 {
            return true;
        }

        let mut diff_remaining = self.difficulty as usize;
        for &byte in hash.iter() {
            if diff_remaining >= 8 {
                if byte != 0 {
                    return false;
                }
                diff_remaining -= 8;
            } else {
                let shift = 8 - diff_remaining;
                let mask = 0xFF << shift;
                return (byte & mask as u8) == 0;
            }
        }
        true
    }
}

// ── Peer reputation ──────────────────────────────────────────────────────────
pub struct PeerReputation {
    pub score: f32,
    pub last_updated: Instant,
}

impl Default for PeerReputation {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerReputation {
    pub fn new() -> Self {
        Self {
            score: 0.5,
            last_updated: Instant::now(),
        }
    }

    pub fn penalise(&mut self, amount: f32) {
        self.score -= amount;
        if self.score < 0.0 {
            self.score = 0.0;
        }
        self.last_updated = Instant::now();
    }

    pub fn decay_recover(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_updated).as_secs_f32();

        if elapsed > 0.0 {
            let recovery = elapsed * 0.01;
            self.score += recovery;
            if self.score > 1.0 {
                self.score = 1.0;
            }
            self.last_updated = now;
        }
    }

    pub fn is_banned(&self) -> bool {
        self.score < 0.1
    }
}

// ── Token-bucket rate limiter ─────────────────────────────────────────────────
pub struct TokenBucket {
    pub tokens: f32,
    pub capacity: f32,
    pub refill_rate: f32,
    pub last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f32, refill_rate: f32) -> Self {
        Self {
            tokens: capacity, // Starts full
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    pub fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();

        if elapsed > 0.0 {
            let refill_amount = elapsed * self.refill_rate;
            self.tokens += refill_amount;
            if self.tokens > self.capacity {
                self.tokens = self.capacity;
            }
            self.last_refill = now;
        }

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ── SybilGuard ────────────────────────────────────────────────────────────────
pub struct SybilGuard {
    reputations: HashMap<NodeId, PeerReputation>,
    buckets: HashMap<NodeId, TokenBucket>,
    pub difficulty: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum SybilError {
    #[error("peer {0:?} is banned (reputation too low)")]
    Banned(NodeId),
    #[error("peer {0:?} exceeded rate limit")]
    RateLimited(NodeId),
    #[error("PoW challenge failed for peer {0:?}")]
    PowFailed(NodeId),
}

impl SybilGuard {
    pub fn new(difficulty: u8) -> Self {
        Self {
            reputations: HashMap::new(),
            buckets: HashMap::new(),
            difficulty,
        }
    }

    pub fn check(&mut self, peer: &NodeId) -> Result<(), SybilError> {
        let rep = self.reputations.entry(peer.clone()).or_default();
        rep.decay_recover();

        if rep.is_banned() {
            return Err(SybilError::Banned(peer.clone()));
        }

        let bucket = self
            .buckets
            .entry(peer.clone())
            .or_insert_with(|| TokenBucket::new(20.0, 5.0));
        if !bucket.try_consume() {
            return Err(SybilError::RateLimited(peer.clone()));
        }

        Ok(())
    }

    pub fn report_violation(&mut self, peer: &NodeId) {
        let rep = self.reputations.entry(peer.clone()).or_default();
        rep.penalise(0.15);
    }

    pub fn verify_pow(&self, challenge: &PowChallenge) -> Result<(), SybilError> {
        if challenge.verify() {
            Ok(())
        } else {
            Err(SybilError::PowFailed(challenge.node_id.clone()))
        }
    }

    pub fn reputation(&mut self, peer: &NodeId) -> f32 {
        let rep = self.reputations.entry(peer.clone()).or_default();
        rep.decay_recover();
        rep.score
    }
}
