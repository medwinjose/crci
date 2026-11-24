// ─── Replay Protection ────────────────────────────────────────────────────────
// Per-origin sliding window over signed sequence numbers.
//
// For each origin we keep the highest accepted sequence number `seq_max` and the
// set of sequence numbers accepted inside the window [seq_max - W, seq_max],
// together with the transport peers that delivered each one.
//
// - seq > seq_max: accepted; the window slides forward.
// - seq < seq_max - W: out of window. Dropped without a strike: an honest peer
//   can deliver an old message after a partition or a reconnect.
// - inside the window, not seen before: accepted (late or reordered delivery).
// - inside the window, already delivered by the same transport peer: replay,
//   the caller strikes that peer.
// - inside the window, first delivered by a different peer: gossip redundancy,
//   dropped without a strike.
//
// W = 0 gives strict-monotonic delivery: every late packet is dropped as out of
// window, and only a repeat of seq_max can be a replay.
//
// The sequence number must be covered by the origin's signature; otherwise a
// relay could move seq_max and push an origin's later messages out of window.
//
// A node forgets origins whole when it tracks too many (`NodeRuntime::forget_origins`). Its
// floor stays: the highest sequence number accepted, kept per origin in a table of
// `MAX_FORGOTTEN_FLOORS` (oldest first out). Every message of that origin at or below the floor
// is then out of window, dropped without a strike, not accepted and so not relayed: a node never
// accepts the same message twice, so an honest relay never repeats one, while it keeps the
// floor. Without it, an adversary that flooded a relay with new origins until it forgot a quiet
// one could resend one of that origin's old messages: the relay accepted it again and forwarded
// it, and a neighbour that still remembered the origin struck the relay for the repeat.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasher;

pub const DEFAULT_WINDOW_SIZE: u64 = 64;
/// Largest window an operator may configure: each peer record holds `W + 1` bits, so a larger
/// value costs memory per peer and `W + 1` overflows at `u64::MAX`.
pub const MAX_WINDOW_SIZE: u64 = 1024;
/// Transport peers remembered per origin: above the listener's connection cap. Deliveries by
/// further peers count as held but cannot be attributed, so they are never a strike.
pub const MAX_PEERS_PER_ORIGIN: usize = 128;
/// Stands for every peer past `MAX_PEERS_PER_ORIGIN`.
const UNATTRIBUTED: u64 = 0;
/// Floors of forgotten origins kept (`ReplayFilter::forget`): four times the tracked origins, 7/8
/// of 4 096 buckets. Under churn a hash table settles at twice that (removed entries leave markers
/// until the next resize), about 0.27 MB for the table and its order together.
pub const MAX_FORGOTTEN_FLOORS: usize = 3_584;

// Which seqs of the window one transport peer delivered: one bit per seq, at `seq % (W + 1)`,
// unique inside a window of W + 1 values. The peer is kept as a keyed 64-bit hash: its identity
// string would cost 70 bytes per copy. The key is random per filter, so a collision (a false
// strike) cannot be aimed at anyone.
struct PeerBits {
    tag: u64,
    bits: Box<[u64]>,
}

struct OriginWindow {
    seq_max: u64,
    peers: Vec<PeerBits>,
    /// One above the floor this origin had when it was last forgotten (0: never forgotten):
    /// everything below it is out of window.
    forgotten_below: u64,
}

impl OriginWindow {
    fn floor(&self, window_size: u64) -> u64 {
        self.seq_max.saturating_sub(window_size)
    }
}

fn slot(seq: u64, window_size: u64) -> (usize, u64) {
    let position = seq % (window_size + 1);
    ((position / 64) as usize, 1u64 << (position % 64))
}

pub struct ReplayFilter {
    windows: HashMap<String, OriginWindow>,
    /// Floor of each forgotten origin, by keyed tag, with the stamp of its entry in `floor_order`.
    floors: HashMap<u64, (u64, u64)>,
    /// Floors in the order they were kept, to drop the oldest; entries whose stamp no longer
    /// matches `floors` are stale.
    floor_order: VecDeque<(u64, u64)>,
    floor_stamp: u64,
    tag_key: RandomState,
    /// Sum of `peers.len()` over all windows: what the filter's memory grows with.
    peer_records: usize,
    pub window_size: u64,
    /// Whether `forget` keeps a floor. A node's control messages (sync requests, chain-head
    /// announcements) are never relayed, so no honest neighbour can repeat one and their filter
    /// forgets whole.
    pub keep_floors: bool,
}

impl Default for ReplayFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayFilter {
    pub fn new() -> ReplayFilter {
        ReplayFilter {
            windows: HashMap::new(),
            floors: HashMap::new(),
            floor_order: VecDeque::new(),
            floor_stamp: 0,
            tag_key: RandomState::new(),
            peer_records: 0,
            window_size: DEFAULT_WINDOW_SIZE,
            keep_floors: true,
        }
    }

    /// A filter whose `forget` keeps no floor (for messages no honest node relays).
    pub fn without_floors() -> ReplayFilter {
        ReplayFilter {
            keep_floors: false,
            ..ReplayFilter::new()
        }
    }

    /// True if `seq` is older than the window for `origin`, or at or below the floor of a
    /// forgotten origin. Does not record anything.
    pub fn is_out_of_window(&self, origin: &str, seq: u64) -> bool {
        match self.windows.get(origin) {
            Some(w) => seq < w.floor(self.window_size) || seq < w.forgotten_below,
            None => self.kept_floor(origin).is_some_and(|below| seq < below),
        }
    }

    fn origin_tag(&self, origin: &str) -> u64 {
        self.tag_key.hash_one(("origin", origin))
    }

    /// One above the floor kept for a forgotten `origin`.
    fn kept_floor(&self, origin: &str) -> Option<u64> {
        self.floors
            .get(&self.origin_tag(origin))
            .map(|(floor, _)| *floor)
    }

    /// Keep `floor`, one above the highest sequence number accepted, for a forgotten `origin`.
    fn keep_floor(&mut self, origin: &str, floor: u64) {
        let tag = self.origin_tag(origin);
        self.floor_stamp += 1;
        let stamp = self.floor_stamp;
        let previous = self.floors.get(&tag).map_or(0, |(f, _)| *f);
        // Make room first, so the table never holds more than the limit.
        while previous == 0 && self.floors.len() >= MAX_FORGOTTEN_FLOORS {
            match self.floor_order.pop_front() {
                Some((old, old_stamp)) => {
                    if self.floors.get(&old).is_some_and(|(_, s)| *s == old_stamp) {
                        self.floors.remove(&old);
                    }
                }
                None => break,
            }
        }
        self.floors.insert(tag, (floor.max(previous), stamp));
        self.floor_order.push_back((tag, stamp));
        if self.floor_order.len() > MAX_FORGOTTEN_FLOORS + MAX_FORGOTTEN_FLOORS / 8 {
            let floors = &self.floors;
            self.floor_order
                .retain(|(t, st)| floors.get(t).is_some_and(|(_, s)| s == st));
        }
    }

    /// Classifies (`origin`, `seq`) delivered by transport peer `sender` and records it
    /// if accepted.
    pub fn check_and_record(&mut self, origin: &str, seq: u64, sender: &str) -> ReplayVerdict {
        let window_size = self.window_size;
        // Never UNATTRIBUTED, which would match the overflow record.
        let tag = self.tag_key.hash_one(sender) | 1;
        if !self.windows.contains_key(origin) {
            let below = self.kept_floor(origin).unwrap_or(0);
            if seq < below {
                return ReplayVerdict::OutOfWindow {
                    received_seq: seq,
                    window_floor: below,
                };
            }
            // The floor moves into the window; the table entry goes.
            let origin_tag = self.origin_tag(origin);
            self.floors.remove(&origin_tag);
            self.windows.insert(
                origin.to_string(),
                OriginWindow {
                    seq_max: seq,
                    peers: Vec::new(),
                    forgotten_below: below,
                },
            );
            self.record(origin, seq, tag);
            return ReplayVerdict::Accept;
        }
        let Some(window) = self.windows.get_mut(origin) else {
            return ReplayVerdict::Accept;
        };

        if seq > window.seq_max {
            let old_floor = window.floor(window_size);
            window.seq_max = seq;
            let new_floor = window.floor(window_size);
            let before = window.peers.len();
            for peer in &mut window.peers {
                if new_floor - old_floor > window_size {
                    peer.bits.fill(0);
                } else {
                    for expired in old_floor..new_floor {
                        let (word, bit) = slot(expired, window_size);
                        peer.bits[word] &= !bit;
                    }
                }
            }
            window.peers.retain(|p| p.bits.iter().any(|w| *w != 0));
            self.peer_records -= before - window.peers.len();
            self.record(origin, seq, tag);
            return ReplayVerdict::Accept;
        }

        let floor = window.floor(window_size);
        if seq < floor || seq < window.forgotten_below {
            return ReplayVerdict::OutOfWindow {
                received_seq: seq,
                window_floor: floor.max(window.forgotten_below),
            };
        }

        let (word, bit) = slot(seq, window_size);
        if window
            .peers
            .iter()
            .any(|p| p.tag == tag && p.bits[word] & bit != 0)
        {
            return ReplayVerdict::Replayed { received_seq: seq };
        }
        let held = window.peers.iter().any(|p| p.bits[word] & bit != 0);
        self.record(origin, seq, tag);
        if held {
            ReplayVerdict::Redundant { received_seq: seq }
        } else {
            ReplayVerdict::Accept
        }
    }

    /// Note that the peer with `tag` delivered `seq`.
    fn record(&mut self, origin: &str, seq: u64, tag: u64) {
        let window_size = self.window_size;
        let Some(window) = self.windows.get_mut(origin) else {
            return;
        };
        let known = window.peers.iter().any(|p| p.tag == tag);
        let tag = if !known && window.peers.len() >= MAX_PEERS_PER_ORIGIN {
            UNATTRIBUTED
        } else {
            tag
        };
        let index = match window.peers.iter().position(|p| p.tag == tag) {
            Some(i) => i,
            None => {
                let words = (window_size + 1).div_ceil(64) as usize;
                // Most origins are delivered by a few peers: no spare capacity per origin.
                window.peers.reserve_exact(1);
                window.peers.push(PeerBits {
                    tag,
                    bits: vec![0; words].into_boxed_slice(),
                });
                self.peer_records += 1;
                window.peers.len() - 1
            }
        };
        let (word, bit) = slot(seq, window_size);
        window.peers[index].bits[word] |= bit;
    }

    /// For each tracked origin (at most `max_origins`, highest `seq_max` first, then by
    /// origin): `seq_max` and the sequence numbers accepted inside the window, ascending.
    pub fn digest(&self, max_origins: usize) -> Vec<(String, u64, Vec<u64>)> {
        let mut heads: Vec<(&String, &OriginWindow)> = self.windows.iter().collect();
        heads.sort_by(|a, b| b.1.seq_max.cmp(&a.1.seq_max).then_with(|| a.0.cmp(b.0)));
        heads.truncate(max_origins);
        heads
            .into_iter()
            .map(|(origin, w)| {
                let held = (w.floor(self.window_size)..=w.seq_max)
                    .filter(|seq| {
                        let (word, bit) = slot(*seq, self.window_size);
                        *seq < w.forgotten_below || w.peers.iter().any(|p| p.bits[word] & bit != 0)
                    })
                    .collect();
                (origin.clone(), w.seq_max, held)
            })
            .collect()
    }

    /// Forget everything about `origin` except its floor (the highest sequence number accepted).
    pub fn forget(&mut self, origin: &str) {
        if let Some(window) = self.windows.remove(origin) {
            self.peer_records -= window.peers.len();
            if !self.keep_floors {
                return;
            }
            self.keep_floor(
                origin,
                window.seq_max.saturating_add(1).max(window.forgotten_below),
            );
        }
    }

    /// The highest sequence number accepted from each tracked origin.
    pub fn heads(&self) -> impl Iterator<Item = (&str, u64)> {
        self.windows.iter().map(|(o, w)| (o.as_str(), w.seq_max))
    }

    /// How many unique origins we're tracking.
    pub fn tracked_origins(&self) -> usize {
        self.windows.len()
    }

    /// Peer records over all origins; with `tracked_origins`, what the memory grows with.
    pub fn peer_records(&self) -> usize {
        self.peer_records
    }

    /// Floors kept for forgotten origins.
    pub fn forgotten_floors(&self) -> usize {
        self.floors.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplayVerdict {
    Accept,
    /// Same (origin, seq) already delivered by this transport peer: strike the peer.
    Replayed {
        received_seq: u64,
    },
    /// Same (origin, seq) already delivered by another peer: drop, no strike.
    Redundant {
        received_seq: u64,
    },
    /// Older than the window: drop, no strike.
    OutOfWindow {
        received_seq: u64,
        window_floor: u64,
    },
}

impl ReplayVerdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, ReplayVerdict::Accept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_message_from_an_origin_is_accepted() {
        let mut f = ReplayFilter::new();
        assert_eq!(f.check_and_record("a", 7, "p"), ReplayVerdict::Accept);
        assert_eq!(f.tracked_origins(), 1);
    }

    #[test]
    fn reordered_packet_inside_window_is_accepted() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 10, "p");
        assert_eq!(f.check_and_record("a", 5, "p"), ReplayVerdict::Accept);
    }

    #[test]
    fn packet_below_window_floor_is_out_of_window() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 100, "p");
        assert!(f.is_out_of_window("a", 35));
        assert_eq!(
            f.check_and_record("a", 35, "p"),
            ReplayVerdict::OutOfWindow {
                received_seq: 35,
                window_floor: 36
            }
        );
        assert!(!f.is_out_of_window("a", 36));
        assert_eq!(f.check_and_record("a", 36, "p"), ReplayVerdict::Accept);
    }

    #[test]
    fn duplicate_from_same_peer_is_replayed() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 10, "p");
        assert_eq!(
            f.check_and_record("a", 10, "p"),
            ReplayVerdict::Replayed { received_seq: 10 }
        );
        f.check_and_record("a", 12, "p");
        f.check_and_record("a", 11, "p");
        assert_eq!(
            f.check_and_record("a", 11, "p"),
            ReplayVerdict::Replayed { received_seq: 11 }
        );
    }

    #[test]
    fn duplicate_from_other_peer_is_redundant() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 10, "p");
        assert_eq!(
            f.check_and_record("a", 10, "q"),
            ReplayVerdict::Redundant { received_seq: 10 }
        );
        // q has now delivered it too, so a second copy from q is a replay
        assert_eq!(
            f.check_and_record("a", 10, "q"),
            ReplayVerdict::Replayed { received_seq: 10 }
        );
    }

    #[test]
    fn a_forgotten_origin_keeps_its_floor() {
        let mut f = ReplayFilter::new();
        for seq in 1..=5 {
            assert!(f.check_and_record("o", seq, "p").is_accept());
        }
        f.forget("o");
        assert_eq!(f.tracked_origins(), 0);
        assert_eq!(f.forgotten_floors(), 1);
        for seq in [1, 3, 5] {
            assert!(f.is_out_of_window("o", seq));
            assert!(matches!(
                f.check_and_record("o", seq, "q"),
                ReplayVerdict::OutOfWindow { .. }
            ));
        }
        assert_eq!(
            f.tracked_origins(),
            0,
            "an old message tracks nothing again"
        );
        // A new message is accepted; the floor moves into the origin's window.
        assert!(f.check_and_record("o", 7, "q").is_accept());
        assert_eq!(f.forgotten_floors(), 0);
        assert!(
            f.check_and_record("o", 6, "q").is_accept(),
            "above the floor"
        );
        assert!(matches!(
            f.check_and_record("o", 5, "q"),
            ReplayVerdict::OutOfWindow { .. }
        ));
        // The digest lists everything up to the floor as held, so no one answers with it.
        let (_, max, held) = f.digest(1).remove(0);
        assert_eq!(max, 7);
        assert_eq!(held, (0..=7).collect::<Vec<_>>());
        // Forgotten again, the floor is the new highest number.
        f.forget("o");
        assert!(f.is_out_of_window("o", 7));
        assert!(!f.is_out_of_window("o", 8));
    }

    #[test]
    fn the_floor_table_never_outgrows_its_buckets_and_can_be_switched_off() {
        let mut f = ReplayFilter::new();
        for i in 0..(3 * MAX_FORGOTTEN_FLOORS) {
            let origin = format!("o{i}");
            assert!(f.check_and_record(&origin, 1, "p").is_accept());
            f.forget(&origin);
        }
        // At most one doubling under churn, however many origins are forgotten.
        assert_eq!(f.forgotten_floors(), MAX_FORGOTTEN_FLOORS);
        assert!(
            f.floors.capacity() <= 2 * MAX_FORGOTTEN_FLOORS,
            "{}",
            f.floors.capacity()
        );
        assert!(
            f.floor_order.capacity() <= 4096,
            "{}",
            f.floor_order.capacity()
        );

        let mut g = ReplayFilter::without_floors();
        assert!(g.check_and_record("o", 3, "p").is_accept());
        g.forget("o");
        assert_eq!(g.forgotten_floors(), 0);
        assert!(g.check_and_record("o", 3, "p").is_accept());
    }

    #[test]
    fn a_floor_covers_sequence_number_zero() {
        let mut f = ReplayFilter::new();
        assert!(f.check_and_record("o", 0, "p").is_accept());
        f.forget("o");
        assert!(!f.check_and_record("o", 0, "p").is_accept());
    }

    #[test]
    fn floors_are_kept_oldest_first_out_up_to_their_limit() {
        let mut f = ReplayFilter::new();
        for i in 0..(MAX_FORGOTTEN_FLOORS + 10) {
            let origin = format!("o{i}");
            assert!(f.check_and_record(&origin, 1, "p").is_accept());
            f.forget(&origin);
        }
        assert_eq!(f.forgotten_floors(), MAX_FORGOTTEN_FLOORS);
        assert!(!f.is_out_of_window("o0", 1), "the oldest floor went first");
        assert!(f.is_out_of_window(&format!("o{}", MAX_FORGOTTEN_FLOORS + 9), 1));
        // Forgetting the same origin again and again does not grow the order without bound.
        for seq in 1..=(3 * MAX_FORGOTTEN_FLOORS as u64) {
            assert!(f.check_and_record("again", seq, "p").is_accept());
            f.forget("again");
        }
        assert!(f.floor_order.len() <= 2 * MAX_FORGOTTEN_FLOORS + 1);
    }

    #[test]
    fn window_slides_and_forgets_old_entries() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 1, "p");
        f.check_and_record("a", 200, "p");
        assert_eq!(
            f.check_and_record("a", 1, "p"),
            ReplayVerdict::OutOfWindow {
                received_seq: 1,
                window_floor: 136
            }
        );
        assert_eq!(f.digest(1), vec![("a".to_string(), 200, vec![200])]);
    }

    #[test]
    fn window_zero_is_strict_monotonic() {
        let mut f = ReplayFilter::new();
        f.window_size = 0;
        f.check_and_record("a", 10, "p");
        assert_eq!(
            f.check_and_record("a", 9, "p"),
            ReplayVerdict::OutOfWindow {
                received_seq: 9,
                window_floor: 10
            }
        );
        assert_eq!(
            f.check_and_record("a", 10, "p"),
            ReplayVerdict::Replayed { received_seq: 10 }
        );
        assert_eq!(f.check_and_record("a", 11, "p"), ReplayVerdict::Accept);
    }

    #[test]
    fn origins_are_independent() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 100, "p");
        assert_eq!(f.check_and_record("b", 1, "p"), ReplayVerdict::Accept);
        assert!(!f.is_out_of_window("b", 1));
    }

    #[test]
    fn slide_clears_exactly_the_seqs_that_left_the_window() {
        let mut f = ReplayFilter::new();
        f.window_size = 3;
        f.check_and_record("a", 10, "p");
        f.check_and_record("a", 9, "q");
        f.check_and_record("a", 12, "p");
        // floor 9: 9 is still q's, 10 still p's
        assert_eq!(f.digest(1)[0].2, vec![9, 10, 12]);
        assert_eq!(
            f.check_and_record("a", 9, "q"),
            ReplayVerdict::Replayed { received_seq: 9 }
        );
        f.check_and_record("a", 13, "p");
        assert_eq!(f.digest(1)[0].2, vec![10, 12, 13]);
        // the same slot as 9 (9 % 4 == 13 % 4), now holding 13: q never delivered it
        assert_eq!(
            f.check_and_record("a", 13, "q"),
            ReplayVerdict::Redundant { received_seq: 13 }
        );
    }

    #[test]
    fn a_window_wider_than_one_word_keeps_every_seq() {
        let mut f = ReplayFilter::new();
        f.window_size = 200;
        for seq in (1..=201).rev() {
            assert_eq!(f.check_and_record("a", seq, "p"), ReplayVerdict::Accept);
        }
        assert_eq!(f.digest(1)[0].2.len(), 201);
        for seq in 1..=201 {
            assert_eq!(
                f.check_and_record("a", seq, "p"),
                ReplayVerdict::Replayed { received_seq: seq }
            );
        }
    }

    #[test]
    fn peer_records_are_bounded_and_counted() {
        let mut f = ReplayFilter::new();
        for i in 0..1000 {
            f.check_and_record("a", 5, &format!("p{i}"));
        }
        assert_eq!(f.peer_records(), MAX_PEERS_PER_ORIGIN + 1);
        // past the limit nobody can be blamed: no strike
        assert_eq!(
            f.check_and_record("a", 5, "p999"),
            ReplayVerdict::Redundant { received_seq: 5 }
        );
        // a peer inside the limit is still recognised
        assert_eq!(
            f.check_and_record("a", 5, "p0"),
            ReplayVerdict::Replayed { received_seq: 5 }
        );
        f.forget("a");
        assert_eq!((f.peer_records(), f.tracked_origins()), (0, 0));
    }

    #[test]
    fn a_peer_with_nothing_left_in_the_window_is_dropped() {
        let mut f = ReplayFilter::new();
        f.check_and_record("a", 1, "old");
        f.check_and_record("a", 1, "older");
        assert_eq!(f.peer_records(), 2);
        f.check_and_record("a", 1000, "p");
        assert_eq!(f.peer_records(), 1);
    }
}
