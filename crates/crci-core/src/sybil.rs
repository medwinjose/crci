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
    pub last_activity: Instant,
    pub banned: bool,
}

impl Default for PeerReputation {
    fn default() -> Self {
        Self::new()
    }
}

fn round_to_3_dec_f32(val: f32) -> f32 {
    (val * 1000.0).round() / 1000.0
}

impl PeerReputation {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            score: 0.5,
            last_updated: now,
            last_activity: now,
            banned: false,
        }
    }

    pub fn penalise(&mut self, amount: f32) {
        let new_score = self.score - amount;
        // BFT-011: clamp to [0.0, 1.0] and BFT-016: fixed precision rounding
        self.score = round_to_3_dec_f32(new_score.clamp(0.0, 1.0));
        self.last_updated = Instant::now();
        if self.score < 0.1 {
            self.banned = true;
        }
    }

    pub fn record_active_behavior(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_updated).as_secs_f32();
        if elapsed >= 0.01 {
            // BFT-013: Sub-linear recovery curve
            let recovery = elapsed.sqrt() * 0.005;
            let new_score = self.score + recovery;
            self.score = round_to_3_dec_f32(new_score.clamp(0.0, 1.0));
            self.last_updated = now;
        }
        self.last_activity = now;

        // BFT-020: Hysteresis unbanning threshold
        if self.banned && self.score >= 0.25 {
            self.banned = false;
        }
    }

    pub fn decay_recover(&mut self) {
        let now = Instant::now();
        let inactive_dur = now.duration_since(self.last_activity).as_secs_f32();

        if inactive_dur > 2.0 {
            // BFT-011: Idle decay curve
            let idle_time_to_decay = now.duration_since(self.last_updated).as_secs_f32();
            if idle_time_to_decay >= 0.01 {
                let decay = idle_time_to_decay * 0.05;
                let new_score = self.score - decay;
                self.score = round_to_3_dec_f32(new_score.clamp(0.0, 1.0));
                self.last_updated = now;

                if self.score < 0.1 {
                    self.banned = true;
                }
            }
        } else {
            let elapsed = now.duration_since(self.last_updated).as_secs_f32();
            if elapsed >= 0.01 {
                let recovery = elapsed.sqrt() * 0.005;
                let new_score = self.score + recovery;
                self.score = round_to_3_dec_f32(new_score.clamp(0.0, 1.0));
                self.last_updated = now;

                if self.banned && self.score >= 0.25 {
                    self.banned = false;
                }
            }
        }
    }

    pub fn is_banned(&self) -> bool {
        self.banned
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
fn get_subnet(id: &str) -> Option<String> {
    if let Ok(addr) = id.parse::<std::net::SocketAddr>() {
        match addr.ip() {
            std::net::IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                Some(format!("{}.{}.{}", octets[0], octets[1], octets[2]))
            }
            std::net::IpAddr::V6(ipv6) => {
                let segments = ipv6.segments();
                Some(format!("{:x}:{:x}", segments[0], segments[1]))
            }
        }
    } else {
        None
    }
}

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
        let is_new = !self.reputations.contains_key(peer);
        if is_new {
            // BFT-019: Sybil cluster reputation correlation
            if let Some(subnet) = get_subnet(peer) {
                let has_banned_in_subnet = self.reputations.iter().any(|(other_id, other_rep)| {
                    other_rep.is_banned() && get_subnet(other_id) == Some(subnet.clone())
                });
                if has_banned_in_subnet {
                    let mut new_rep = PeerReputation::new();
                    new_rep.score = 0.2; // Start penalized
                    self.reputations.insert(peer.clone(), new_rep);
                }
            }
        }

        let rep = self.reputations.entry(peer.clone()).or_default();
        rep.decay_recover();

        if rep.is_banned() {
            return Err(SybilError::Banned(peer.clone()));
        }

        rep.record_active_behavior();

        // BFT-039: Prune low/zero reputation records when reputations map exceeds 1000 to prevent memory exhaustion
        if self.reputations.len() > 1000 {
            self.reputations.retain(|_, r| !r.is_banned());
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
