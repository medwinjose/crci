//! Ties the TLA+ reputation model to the code: every transition in the state graph TLC explored
//! (`docs/tla/reputation-transitions.csv`, written by `docs/tla/check.py --export-transitions`)
//! is replayed through `PeerReputation`, which must reach the same state. The graph's grid is
//! 1 unit = 0.01 of score and one Tick = 20 s.

use crci_core::sybil::{PeerReputation, STRIKE_PENALTY};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_secs(20);
const TRANSITIONS: &str = include_str!("../../../docs/tla/reputation-transitions.csv");

struct State {
    score: f32,
    ages: Vec<u32>,
    banned: bool,
}

fn state(score: &str, ages: &str, banned: &str) -> State {
    State {
        score: score.parse::<f32>().unwrap() / 100.0,
        ages: ages.split(' ').flat_map(str::parse).collect(),
        banned: banned == "true",
    }
}

#[test]
fn every_transition_of_the_model_is_what_the_code_does() {
    // Far enough from the epoch for any remembered strike to lie in the past.
    let now = Instant::now() + 100 * TICK;
    let mut rows = 0;
    for line in TRANSITIONS.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let (before, action, after) = (state(f[0], f[1], f[2]), f[3], state(f[4], f[5], f[6]));

        let mut peer = PeerReputation::new_at(now);
        peer.score = before.score;
        peer.banned = before.banned;
        peer.strikes = before.ages.iter().map(|a| now - *a * TICK).collect();

        let end = match action {
            "Strike" => {
                peer.penalise_at(STRIKE_PENALTY, now);
                now
            }
            "Tick" => {
                peer.recover_at(now + TICK);
                now + TICK
            }
            other => panic!("unknown action {other}"),
        };

        let ages: VecDeque<u32> = peer
            .strikes
            .iter()
            .map(|s| (end.duration_since(*s).as_secs() / TICK.as_secs()) as u32)
            .collect();
        assert!(
            (peer.score - after.score).abs() < 1e-3,
            "{line}: score {}",
            peer.score
        );
        assert_eq!(ages, after.ages, "{line}: strikes");
        assert_eq!(
            peer.banned, after.banned,
            "{line}: banned at {}",
            peer.score
        );
        rows += 1;
    }
    assert!(rows > 1000, "only {rows} transitions: stale export?");
}
