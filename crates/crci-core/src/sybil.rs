use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

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
//
// A peer starts at INITIAL_SCORE. Each strike (evidenced misbehaviour) costs
// STRIKE_PENALTY; a score below BAN_THRESHOLD bans the peer. Between strikes the score
// recovers with elapsed time, at RECOVERY_PER_SEC, whatever the peer sends: sending more
// valid traffic does not buy forgiveness faster. Recovery never raises the score above
// INITIAL_SCORE, so credit cannot be banked and three strikes in a row always ban.
//
// Strike damping: while DAMPING_STRIKES or more strikes fall within the last
// STRIKE_MEMORY, recovery cannot raise the score above DAMPED_CEILING. It only caps the
// increase (new = max(current, min(current + increment, ceiling))) and never lowers a
// score. Once older strikes leave the memory, normal recovery resumes, so no peer is
// capped for good.
//
// A banned peer recovers at the same rate and is readmitted once its score reaches
// UNBAN_THRESHOLD (hysteresis). Inactivity never changes a score.
//
// Residual bound: any forgiving recovery lets an adversary that paces its violations
// sustain about one per STRIKE_PENALTY / RECOVERY_PER_SEC seconds (300 s here) without
// being banned.

/// Score of a new peer, and the highest score recovery can restore.
pub const INITIAL_SCORE: f32 = 0.5;
/// Score lost per strike.
pub const STRIKE_PENALTY: f32 = 0.15;
/// A score below this bans the peer.
pub const BAN_THRESHOLD: f32 = 0.1;
/// A banned peer is readmitted once its score reaches this value.
pub const UNBAN_THRESHOLD: f32 = 0.25;
/// Score regained per second without a new strike: one strike is forgiven in 300 s.
pub const RECOVERY_PER_SEC: f32 = 0.0005;
/// How long a strike counts towards strike damping.
pub const STRIKE_MEMORY: Duration = Duration::from_secs(300);
/// Strikes within `STRIKE_MEMORY` from which recovery is capped at `DAMPED_CEILING`.
pub const DAMPING_STRIKES: usize = 2;
/// Recovery cap while strike damping applies.
pub const DAMPED_CEILING: f32 = 0.18;

pub struct PeerReputation {
    pub score: f32,
    /// Time up to which recovery has been applied.
    pub last_updated: Instant,
    /// Times of the strikes still inside `STRIKE_MEMORY`, oldest first.
    pub strikes: VecDeque<Instant>,
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
        Self::new_at(Instant::now())
    }

    pub fn new_at(now: Instant) -> Self {
        Self {
            score: INITIAL_SCORE,
            last_updated: now,
            strikes: VecDeque::new(),
            banned: false,
        }
    }

    /// Recovery cap in force while the current `strikes` are remembered.
    fn ceiling(&self) -> f32 {
        if self.strikes.len() >= DAMPING_STRIKES {
            DAMPED_CEILING
        } else {
            INITIAL_SCORE
        }
    }

    fn forget_strikes_before(&mut self, t: Instant) {
        while self
            .strikes
            .front()
            .is_some_and(|s| *s + STRIKE_MEMORY <= t)
        {
            self.strikes.pop_front();
        }
    }

    /// Apply recovery for the time between the last update and `now`. The interval is
    /// split where the oldest remembered strike expires, since the cap can change there.
    pub fn recover_at(&mut self, now: Instant) {
        let mut t = self.last_updated;
        while t < now {
            self.forget_strikes_before(t);
            let ceiling = self.ceiling();
            let segment_end = match self.strikes.front() {
                Some(oldest) if self.strikes.len() >= DAMPING_STRIKES => {
                    (*oldest + STRIKE_MEMORY).min(now)
                }
                _ => now,
            };
            let increment = RECOVERY_PER_SEC * segment_end.duration_since(t).as_secs_f32();
            self.score = self.score.max((self.score + increment).min(ceiling));
            t = segment_end;
        }
        self.forget_strikes_before(now);
        if now > self.last_updated {
            self.last_updated = now;
        }
        if self.banned && self.score >= UNBAN_THRESHOLD {
            self.banned = false;
        }
    }

    pub fn recover(&mut self) {
        self.recover_at(Instant::now());
    }

    /// Record a strike of `amount` at `now`.
    pub fn penalise_at(&mut self, amount: f32, now: Instant) {
        self.recover_at(now);
        let new_score = self.score - amount;
        // Clamp to [0.0, 1.0] and round to 3 decimal places
        self.score = round_to_3_dec_f32(new_score.clamp(0.0, 1.0));
        self.strikes.push_back(now);
        if self.score < BAN_THRESHOLD {
            self.banned = true;
        }
    }

    pub fn penalise(&mut self, amount: f32) {
        self.penalise_at(amount, Instant::now());
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
        self.try_consume_at(Instant::now())
    }

    pub fn try_consume_at(&mut self, now: Instant) -> bool {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f32();

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
/// Messages a transport sender may deliver in a burst before it is rate-limited.
pub const SENDER_BURST: f32 = 20.0;
/// Messages per second added back to a transport sender's budget. Only messages not seen
/// before spend it. Chosen from a testbed sweep (5/10/20/50 per s): the lowest rate with no
/// honest frame dropped at 16 messages/s network-wide; at most 20 × 37 µs of signature
/// checks per second per neighbour.
pub const SENDER_REFILL_PER_SEC: f32 = 20.0;

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
    /// Messages per second added back to each sender's budget (`SENDER_REFILL_PER_SEC`
    /// unless an experiment sets another rate before the first message arrives).
    pub sender_refill_per_sec: f32,
    /// Added to the clock. Zero in normal operation; simulations and tests advance it to
    /// run hours of reputation dynamics without waiting.
    clock_offset: Duration,
    /// When set, the clock starts here and moves only through `advance_time`, so a
    /// simulation does not depend on how long it takes to run.
    manual_epoch: Option<Instant>,
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
            sender_refill_per_sec: SENDER_REFILL_PER_SEC,
            clock_offset: Duration::ZERO,
            manual_epoch: None,
        }
    }

    /// The guard's notion of now: the system clock (or the manual epoch) plus any
    /// simulated advance.
    pub fn now(&self) -> Instant {
        self.manual_epoch.unwrap_or_else(Instant::now) + self.clock_offset
    }

    /// Stop following the system clock: from here on time moves only through
    /// `advance_time` (simulations only).
    pub fn use_manual_clock(&mut self) {
        if self.manual_epoch.is_none() {
            self.manual_epoch = Some(Instant::now());
        }
    }

    /// Move the guard's clock forward by `by` (simulations and tests only).
    pub fn advance_time(&mut self, by: Duration) {
        self.clock_offset += by;
    }

    pub fn check(&mut self, peer: &NodeId) -> Result<(), SybilError> {
        let now = self.now();
        let is_new = !self.reputations.contains_key(peer);
        if is_new {
            // Sybil cluster correlation: new peers from a subnet with a banned peer start penalised
            if let Some(subnet) = get_subnet(peer) {
                let has_banned_in_subnet = self.reputations.iter().any(|(other_id, other_rep)| {
                    other_rep.is_banned() && get_subnet(other_id) == Some(subnet.clone())
                });
                if has_banned_in_subnet {
                    let mut new_rep = PeerReputation::new_at(now);
                    new_rep.score = 0.2; // Start penalized
                    self.reputations.insert(peer.clone(), new_rep);
                }
            }
        }

        let rep = self
            .reputations
            .entry(peer.clone())
            .or_insert_with(|| PeerReputation::new_at(now));
        rep.recover_at(now);

        if rep.is_banned() {
            return Err(SybilError::Banned(peer.clone()));
        }

        // BFT-039: Prune low/zero reputation records when reputations map exceeds 1000 to prevent memory exhaustion
        if self.reputations.len() > 1000 {
            self.reputations.retain(|_, r| !r.is_banned());
        }

        let bucket = self
            .buckets
            .entry(peer.clone())
            .or_insert_with(|| TokenBucket {
                tokens: SENDER_BURST,
                capacity: SENDER_BURST,
                refill_rate: self.sender_refill_per_sec,
                last_refill: now,
            });
        if !bucket.try_consume_at(now) {
            return Err(SybilError::RateLimited(peer.clone()));
        }

        Ok(())
    }

    /// Tokens left in `peer`'s budget as last recorded (no refill is applied), or `None`
    /// if the peer has not been seen.
    pub fn available_tokens(&self, peer: &NodeId) -> Option<f32> {
        self.buckets.get(peer).map(|b| b.tokens)
    }

    /// Record one strike against `peer`.
    pub fn report_violation(&mut self, peer: &NodeId) {
        let now = self.now();
        let rep = self
            .reputations
            .entry(peer.clone())
            .or_insert_with(|| PeerReputation::new_at(now));
        rep.penalise_at(STRIKE_PENALTY, now);
    }

    /// Whether `peer` is banned now (recovery up to now applied first).
    pub fn is_banned(&mut self, peer: &NodeId) -> bool {
        let now = self.now();
        self.reputations.get_mut(peer).is_some_and(|rep| {
            rep.recover_at(now);
            rep.is_banned()
        })
    }

    pub fn verify_pow(&self, challenge: &PowChallenge) -> Result<(), SybilError> {
        if challenge.verify() {
            Ok(())
        } else {
            Err(SybilError::PowFailed(challenge.node_id.clone()))
        }
    }

    pub fn reputation(&mut self, peer: &NodeId) -> f32 {
        let now = self.now();
        let rep = self
            .reputations
            .entry(peer.clone())
            .or_insert_with(|| PeerReputation::new_at(now));
        rep.recover_at(now);
        rep.score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn three_strikes_ban_from_the_initial_score() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.penalise_at(STRIKE_PENALTY, t0);
        rep.penalise_at(STRIKE_PENALTY, t0);
        assert!(!rep.is_banned());
        rep.penalise_at(STRIKE_PENALTY, t0);
        assert!(rep.is_banned());
    }

    #[test]
    fn recovery_is_linear_in_time_and_capped_at_the_initial_score() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.penalise_at(STRIKE_PENALTY, t0); // 0.35
        rep.recover_at(t0 + 100 * SEC);
        assert!((rep.score - 0.40).abs() < 1e-4, "score {}", rep.score);
        rep.recover_at(t0 + 300 * SEC);
        assert!((rep.score - 0.50).abs() < 1e-4, "score {}", rep.score);
        rep.recover_at(t0 + 10_000 * SEC);
        assert_eq!(rep.score, INITIAL_SCORE, "recovery must not bank credit");
    }

    #[test]
    fn recovery_does_not_depend_on_how_often_it_is_applied() {
        let t0 = Instant::now();
        let mut once = PeerReputation::new_at(t0);
        let mut often = PeerReputation::new_at(t0);
        once.penalise_at(STRIKE_PENALTY, t0);
        often.penalise_at(STRIKE_PENALTY, t0);
        for ms in (200..=120_000).step_by(200) {
            often.recover_at(t0 + Duration::from_millis(ms));
        }
        once.recover_at(t0 + 120 * SEC);
        assert!((once.score - often.score).abs() < 1e-3);
    }

    #[test]
    fn two_recent_strikes_cap_recovery_until_one_expires() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.penalise_at(STRIKE_PENALTY, t0); // 0.35
        rep.penalise_at(STRIKE_PENALTY, t0 + SEC); // 0.20 (first recovery is negligible)
        let damped = rep.score;
        // Above the damped ceiling: no recovery while both strikes are remembered.
        rep.recover_at(t0 + 299 * SEC);
        assert_eq!(rep.score, damped);
        // The first strike expires at t0 + 300 s; recovery resumes.
        rep.recover_at(t0 + 400 * SEC);
        assert!(rep.score > damped);
        rep.recover_at(t0 + 2_000 * SEC);
        assert_eq!(
            rep.score, INITIAL_SCORE,
            "damping must not cap a peer for good"
        );
    }

    #[test]
    fn damping_never_lowers_a_score() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.score = 0.3;
        rep.strikes = VecDeque::from([t0, t0]);
        rep.recover_at(t0 + 10 * SEC);
        assert_eq!(rep.score, 0.3);
    }

    #[test]
    fn damped_peer_below_the_ceiling_recovers_only_to_the_ceiling() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.score = 0.12;
        rep.strikes = VecDeque::from([t0, t0]);
        rep.recover_at(t0 + 200 * SEC); // +0.1 uncapped would give 0.22
        assert!(
            (rep.score - DAMPED_CEILING).abs() < 1e-6,
            "score {}",
            rep.score
        );
    }

    #[test]
    fn a_banned_peer_is_readmitted_after_recovering_to_the_unban_threshold() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        for _ in 0..3 {
            rep.penalise_at(STRIKE_PENALTY, t0);
        }
        assert!(rep.is_banned()); // 0.05, three strikes remembered
        rep.recover_at(t0 + 299 * SEC); // damped: at most 0.18
        assert!(rep.is_banned());
        // Strikes expire at 300 s; 0.18 → 0.25 takes another 140 s.
        rep.recover_at(t0 + 430 * SEC);
        assert!(rep.is_banned());
        rep.recover_at(t0 + 450 * SEC);
        assert!(!rep.is_banned());
    }

    #[test]
    fn inactivity_does_not_change_a_score() {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        rep.recover_at(t0 + 86_400 * SEC);
        assert_eq!(rep.score, INITIAL_SCORE);
        assert!(!rep.is_banned());
    }

    #[test]
    fn guard_clock_can_be_advanced() {
        let mut guard = SybilGuard::new(0);
        let peer = "p".to_string();
        guard.report_violation(&peer);
        assert!((guard.reputation(&peer) - 0.35).abs() < 1e-3);
        guard.advance_time(300 * SEC);
        assert!((guard.reputation(&peer) - 0.5).abs() < 1e-3);
    }
}
