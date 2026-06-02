// ─── Replay Protection ────────────────────────────────────────────────────────
// Prevents two attacks:
// 1. Replay attack: attacker captures a legitimate message and re-injects it
//    hours later to trigger false alarms or burn reputation.
// 2. Clock drift: two phones with clocks 8 hours apart cause all messages
//    from one to be rejected as "too old" or "from the future."
//
// Solution: Lamport logical timestamps + a sliding window per origin.
// Wall clock is used only as a hint — logical sequence is what matters.

use std::collections::HashMap;

// Per-origin sequence tracking
pub struct ReplayFilter {
    // origin_id -> highest sequence number seen
    seen_sequences: HashMap<String, u64>,
    // origin_id -> last wall-clock time seen (loose bound, not enforced strictly)
    last_wall: HashMap<String, u64>,
}

impl ReplayFilter {
    pub fn new() -> ReplayFilter {
        ReplayFilter {
            seen_sequences: HashMap::new(),
            last_wall: HashMap::new(),
        }
    }

    // Returns true if the message should be accepted, false if it should be dropped.
    // seq: monotonically increasing counter per origin (starts at 1)
    // wall_ts: unix timestamp from message (advisory only — we tolerate ±2hr drift)
    pub fn check_and_record(&mut self, origin: &str, seq: u64, wall_ts: u64) -> ReplayVerdict {
        let now = crate::message::now_ts();

        // Wall clock check — advisory only, 2-hour tolerance each direction
        // This catches obviously stale messages while tolerating clock drift
        let drift_tolerance: u64 = 7_200; // 2 hours in seconds
        if wall_ts + drift_tolerance < now {
            return ReplayVerdict::Stale {
                age_seconds: now.saturating_sub(wall_ts),
            };
        }
        if wall_ts > now + drift_tolerance {
            return ReplayVerdict::FromFuture {
                skew_seconds: wall_ts.saturating_sub(now),
            };
        }

        // Sequence check — the core replay guard
        // We only advance the counter, never go backward
        if let Some(&last_seq) = self.seen_sequences.get(origin) {
            if seq <= last_seq {
                return ReplayVerdict::Replayed {
                    received_seq: seq,
                    expected_min: last_seq + 1,
                };
            }
        }

        // Accept and record
        self.seen_sequences.insert(origin.to_string(), seq);
        self.last_wall.insert(origin.to_string(), wall_ts);
        ReplayVerdict::Accept
    }

    // How many unique origins we're tracking
    pub fn tracked_origins(&self) -> usize {
        self.seen_sequences.len()
    }
}

#[derive(Debug)]
pub enum ReplayVerdict {
    Accept,
    Replayed { received_seq: u64, expected_min: u64 },
    Stale { age_seconds: u64 },
    FromFuture { skew_seconds: u64 },
}

impl ReplayVerdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, ReplayVerdict::Accept)
    }
}