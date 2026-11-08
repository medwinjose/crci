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
/// Shortest interval over which recovery is applied, except right before a strike. The score is
/// an `f32`, and adding a tiny increment rounds: applied on every frame of a peer that sends one
/// per millisecond, recovery ran 1.3 % fast. In steps of a second the error is below 0.01 %.
pub const RECOVERY_STEP: Duration = Duration::from_secs(1);

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

    /// Apply recovery for the time between the last update and `now`, once at least
    /// `RECOVERY_STEP` has passed (a ban can therefore be lifted up to that much late).
    pub fn recover_at(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_updated) >= RECOVERY_STEP {
            self.recover_to(now);
        }
    }

    /// Apply recovery up to `now`. The interval is split where the oldest remembered strike
    /// expires, since the cap can change there.
    fn recover_to(&mut self, now: Instant) {
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
        self.recover_to(now);
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
/// New messages one origin may have accepted at a node in a burst. Every honest node relays
/// what it accepts, so without this one origin flooding fresh, validly signed messages takes
/// the whole network's capacity, whatever the per-neighbour budget.
pub const ORIGIN_BURST: f32 = 20.0;
/// Messages per second added back to an origin's budget: above any single origin's rate in
/// the testbed's honest load (4/s), a quarter of the per-neighbour rate.
pub const ORIGIN_REFILL_PER_SEC: f32 = 5.0;
/// Transport senders with a reputation record. Past this, `make_room` forgets a batch: idle peers
/// with a clean record first, banned peers last, because forgetting a ban readmits the peer.
pub const MAX_TRACKED_PEERS: usize = 1024;

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
    /// Score lost per strike: `STRIKE_PENALTY` unless an experiment changes it
    /// (`set_strikes_to_ban`).
    pub strike_penalty: f32,
    /// When false, strikes lower a peer's score but never ban it (baselines).
    pub bans_enabled: bool,
    /// When false, `check` creates the per-sender budget but never refuses a message
    /// (the vanilla baseline).
    pub rate_limit_enabled: bool,
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
            strike_penalty: STRIKE_PENALTY,
            bans_enabled: true,
            rate_limit_enabled: true,
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

    /// Forget 1/8 of the records if `MAX_TRACKED_PEERS` is reached: a peer that has done nothing
    /// wrong loses nothing by being forgotten, and new identities are free anyway.
    fn make_room(&mut self) {
        if self.reputations.len() < MAX_TRACKED_PEERS {
            return;
        }
        let mut ranked: Vec<(bool, bool, Instant, &NodeId)> = self
            .reputations
            .iter()
            .map(|(id, r)| (r.banned, !r.strikes.is_empty(), r.last_updated, id))
            .collect();
        ranked.sort_unstable();
        let victims: Vec<NodeId> = ranked
            .into_iter()
            .take(MAX_TRACKED_PEERS / 8)
            .map(|(_, _, _, id)| id.clone())
            .collect();
        for id in &victims {
            self.reputations.remove(id);
            self.buckets.remove(id);
        }
    }

    pub fn check(&mut self, peer: &NodeId) -> Result<(), SybilError> {
        let now = self.now();
        let is_new = !self.reputations.contains_key(peer);
        if is_new {
            self.make_room();
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

        let bucket = self
            .buckets
            .entry(peer.clone())
            .or_insert_with(|| TokenBucket {
                tokens: SENDER_BURST,
                capacity: SENDER_BURST,
                refill_rate: self.sender_refill_per_sec,
                last_refill: now,
            });
        if !bucket.try_consume_at(now) && self.rate_limit_enabled {
            return Err(SybilError::RateLimited(peer.clone()));
        }

        Ok(())
    }

    /// Number of transport senders with a reputation record.
    pub fn tracked_peers(&self) -> usize {
        self.reputations.len()
    }

    /// Tokens left in `peer`'s budget as last recorded (no refill is applied), or `None`
    /// if the peer has not been seen.
    pub fn available_tokens(&self, peer: &NodeId) -> Option<f32> {
        self.buckets.get(peer).map(|b| b.tokens)
    }

    /// Record one strike against `peer`.
    pub fn report_violation(&mut self, peer: &NodeId) {
        if !self.reputations.contains_key(peer) {
            self.make_room();
        }
        let now = self.now();
        let rep = self
            .reputations
            .entry(peer.clone())
            .or_insert_with(|| PeerReputation::new_at(now));
        rep.penalise_at(self.strike_penalty, now);
        if !self.bans_enabled {
            rep.banned = false;
        }
    }

    /// Make the `k`-th strike in a row ban a peer that starts at `INITIAL_SCORE`: the penalty
    /// is `(INITIAL_SCORE - 0.05) / k`, so `k` strikes leave 0.05, below `BAN_THRESHOLD`, and
    /// `k - 1` leave at least 0.275. `k = 3` is the shipped `STRIKE_PENALTY` (experiments only).
    pub fn set_strikes_to_ban(&mut self, k: u32) {
        self.strike_penalty = (INITIAL_SCORE - 0.05) / k.max(1) as f32;
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
        if !self.reputations.contains_key(peer) {
            self.make_room();
        }
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

    // Checks of the analytical bounds (docs/formal-verification.md, "Analytical bounds") against
    // the code. The theorems are stated in exact arithmetic; the code rounds a score to 0.001
    // after each strike, which moves a score by at most 0.0005, the recovery of one second.

    /// Time `secs` after `t0`.
    fn at(t0: Instant, secs: f64) -> Instant {
        t0 + Duration::from_secs_f64(secs)
    }

    /// Deterministic pseudo-random numbers in [0, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Theorem 1: k strikes in an interval of L seconds, none of which bans, satisfy
    /// k (penalty - 0.0005) <= score before the first - 0.1 + 0.0005 L.
    #[test]
    fn no_strike_schedule_beats_the_strike_budget() {
        let t0 = Instant::now();
        let mut rng = Lcg(27);
        let mut windows = 0u64;
        for schedule in 0..3_000 {
            let mut rep = PeerReputation::new_at(t0);
            let poll = if schedule % 10 == 0 {
                0.003
            } else {
                0.5 + 20.0 * rng.next()
            };
            let scale = [5.0, 60.0, 200.0, 400.0][schedule % 4];
            let (mut t, mut last_poll) = (0.0, 0.0);
            // (time, score just before, banned after) of each strike
            let mut strikes: Vec<(f64, f32, bool)> = Vec::new();
            for _ in 0..(3 + schedule % 30) {
                t += scale * rng.next();
                while last_poll + poll < t {
                    last_poll += poll;
                    rep.recover_at(at(t0, last_poll));
                }
                // what the strike starts from: all recovery up to t
                rep.recover_to(at(t0, t));
                let before = rep.score;
                let was_banned = rep.is_banned();
                rep.penalise_at(STRIKE_PENALTY, at(t0, t));
                strikes.push((t, before, was_banned || rep.is_banned()));
            }
            for i in 0..strikes.len() {
                for j in i..strikes.len() {
                    if strikes[i..=j].iter().any(|s| s.2) {
                        break;
                    }
                    let k = (j - i + 1) as f64;
                    let budget = strikes[i].1 as f64 - BAN_THRESHOLD as f64
                        + RECOVERY_PER_SEC as f64 * (strikes[j].0 - strikes[i].0);
                    assert!(
                        k * (STRIKE_PENALTY as f64 - 0.0005) <= budget + 1e-5,
                        "schedule {schedule}: strikes {i}..={j} beat the budget"
                    );
                    windows += 1;
                }
            }
        }
        assert!(windows > 10_000, "only {windows} windows checked");
    }

    /// Strike number at which a peer struck every `period` seconds is first banned (0: never
    /// within `max` strikes), with recovery polled every `poll` seconds (0: only at strikes).
    fn banned_at_strike(period: f64, max: u64, poll: f64) -> u64 {
        let t0 = Instant::now();
        let mut rep = PeerReputation::new_at(t0);
        let mut last_poll = 0.0;
        for n in 1..=max {
            let t = n as f64 * period;
            while poll > 0.0 && last_poll + poll < t {
                last_poll += poll;
                rep.recover_at(at(t0, last_poll));
            }
            rep.penalise_at(STRIKE_PENALTY, at(t0, t));
            if rep.is_banned() {
                return n;
            }
        }
        0
    }

    /// Theorem 2 in the code: a strike every P >= 299 s never bans (300 s in exact arithmetic;
    /// the rounding gives back the one second); a faster pace bans by the strike the theorem bounds.
    #[test]
    fn periodic_strikes_ban_exactly_when_faster_than_the_forgiveness_period() {
        for p in 1..=400u64 {
            let pf = p as f64;
            let got = banned_at_strike(pf, 400, 0.0);
            if p >= 299 {
                assert_eq!(got, 0, "a strike every {p} s was banned at strike {got}");
                continue;
            }
            let bound = if p < 150 {
                4
            } else {
                // a_2 <= 0.2 + rho P (+ 0.001 rounding); each period then loses
                // at least rho (300 - P), less 0.0005 for rounding.
                let rho = RECOVERY_PER_SEC as f64;
                3 + ((0.101 + rho * pf) / (rho * (299.0 - pf))).floor() as u64
            };
            assert!(
                got >= 3 && got <= bound,
                "P = {p}: banned at strike {got}, bound {bound}"
            );
            if p < 97 {
                assert_eq!(got, 3, "P = {p}");
            }
        }
        // The values TLC and recovery_bench give for 120 s and 290 s (480 s and 10 150 s).
        assert_eq!(banned_at_strike(120.0, 400, 0.0), 4);
        assert_eq!(banned_at_strike(290.0, 400, 0.0), 35);
        // How often recovery is applied in between changes nothing.
        for (p, poll) in [(120.0, 0.004), (250.0, 0.5), (290.0, 7.3)] {
            assert_eq!(
                banned_at_strike(p, 400, poll),
                banned_at_strike(p, 400, 0.0)
            );
        }
        assert_eq!(banned_at_strike(299.0, 3_000, 0.0), 0);
        assert_eq!(banned_at_strike(300.0, 3_000, 5.0), 0);
    }

    /// Theorem 3: with no strike after the one that bans, a peer is readmitted between 300 s and
    /// 500 s after it (plus the one-second recovery step).
    #[test]
    fn a_ban_lasts_between_300_and_500_seconds() {
        let t0 = Instant::now();
        let mut rng = Lcg(3);
        let (mut shortest, mut longest) = (f64::MAX, 0.0f64);
        for case in 0..2_000 {
            let mut rep = PeerReputation::new_at(t0);
            let mut t = 0.0;
            let gap = [0.0, 10.0, 60.0, 150.0][case % 4];
            while !rep.is_banned() {
                t += gap * rng.next();
                rep.penalise_at(STRIKE_PENALTY, at(t0, t));
            }
            let banned_at = t;
            let mut step = 0.0;
            while rep.is_banned() {
                step += 1.0;
                rep.recover_at(at(t0, banned_at + step));
            }
            shortest = shortest.min(step);
            longest = longest.max(step);
            assert!(
                (300.0..=501.0).contains(&step),
                "case {case}: ban lasted {step} s"
            );
        }
        println!("ban lengths {shortest}..{longest} s");
    }
}
