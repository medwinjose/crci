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

use std::collections::{HashMap, HashSet};

pub const DEFAULT_WINDOW_SIZE: u64 = 64;

#[derive(Default)]
struct OriginWindow {
    seq_max: u64,
    // seq -> transport peers that delivered it; only seqs inside the window are kept
    delivered_by: HashMap<u64, HashSet<String>>,
}

impl OriginWindow {
    fn floor(&self, window_size: u64) -> u64 {
        self.seq_max.saturating_sub(window_size)
    }
}

pub struct ReplayFilter {
    windows: HashMap<String, OriginWindow>,
    pub window_size: u64,
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
            window_size: DEFAULT_WINDOW_SIZE,
        }
    }

    /// True if `seq` is older than the window for `origin`. Does not record anything.
    pub fn is_out_of_window(&self, origin: &str, seq: u64) -> bool {
        self.windows
            .get(origin)
            .is_some_and(|w| seq < w.floor(self.window_size))
    }

    /// Classifies (`origin`, `seq`) delivered by transport peer `sender` and records it
    /// if accepted.
    pub fn check_and_record(&mut self, origin: &str, seq: u64, sender: &str) -> ReplayVerdict {
        let window_size = self.window_size;
        let Some(window) = self.windows.get_mut(origin) else {
            let mut window = OriginWindow {
                seq_max: seq,
                ..Default::default()
            };
            window
                .delivered_by
                .insert(seq, HashSet::from([sender.to_string()]));
            self.windows.insert(origin.to_string(), window);
            return ReplayVerdict::Accept;
        };

        if seq > window.seq_max {
            window.seq_max = seq;
            let floor = window.floor(window_size);
            window.delivered_by.retain(|&s, _| s >= floor);
            window
                .delivered_by
                .insert(seq, HashSet::from([sender.to_string()]));
            return ReplayVerdict::Accept;
        }

        let floor = window.floor(window_size);
        if seq < floor {
            return ReplayVerdict::OutOfWindow {
                received_seq: seq,
                window_floor: floor,
            };
        }

        match window.delivered_by.get_mut(&seq) {
            None => {
                window
                    .delivered_by
                    .insert(seq, HashSet::from([sender.to_string()]));
                ReplayVerdict::Accept
            }
            Some(peers) if peers.contains(sender) => ReplayVerdict::Replayed { received_seq: seq },
            Some(peers) => {
                peers.insert(sender.to_string());
                ReplayVerdict::Redundant { received_seq: seq }
            }
        }
    }

    /// For each tracked origin (at most `max_origins`, highest `seq_max` first, then by
    /// origin): `seq_max` and the sequence numbers accepted inside the window, ascending.
    pub fn digest(&self, max_origins: usize) -> Vec<(String, u64, Vec<u64>)> {
        let mut out: Vec<(String, u64, Vec<u64>)> = self
            .windows
            .iter()
            .map(|(origin, w)| {
                let mut held: Vec<u64> = w.delivered_by.keys().copied().collect();
                held.sort_unstable();
                (origin.clone(), w.seq_max, held)
            })
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.truncate(max_origins);
        out
    }

    // How many unique origins we're tracking
    pub fn tracked_origins(&self) -> usize {
        self.windows.len()
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
        let kept = f.windows.get("a").map(|w| w.delivered_by.len());
        assert_eq!(kept, Some(1));
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
}
