//! Strike-damped, time-based reputation recovery on the signed message path.
//!
//! Each test drives a `NodeRuntime` with simulated time (`SybilGuard::use_manual_clock`,
//! `advance_time`). A "replay" is an exact duplicate of the peer's latest message sent by
//! the same transport peer inside the window: one strike.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::{INITIAL_SCORE, RECOVERY_PER_SEC, STRIKE_PENALTY};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Sim {
    node: NodeRuntime,
    inbox: SharedInbox,
    keys: HashMap<String, Identity>,
    seqs: HashMap<String, u64>,
    latest: HashMap<String, WireMessage>,
}

impl Sim {
    fn new() -> Sim {
        let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut node = NodeRuntime::new("rx", "zone-a", inbox.clone());
        node.sybil_guard.use_manual_clock();
        Sim {
            node,
            inbox,
            keys: HashMap::new(),
            seqs: HashMap::new(),
            latest: HashMap::new(),
        }
    }

    fn deliver(&mut self, peer: &str, wire: &WireMessage) {
        self.inbox
            .lock()
            .unwrap()
            .entry("rx".to_string())
            .or_default()
            .push((peer.to_string(), serde_json::to_vec(wire).unwrap()));
        self.node.process_inbox();
    }

    /// `peer` sends a fresh message it authored.
    fn send(&mut self, peer: &str) {
        let key = self
            .keys
            .entry(peer.to_string())
            .or_insert_with(|| Identity::new(peer));
        let seq = self.seqs.entry(peer.to_string()).or_insert(0);
        *seq += 1;
        let mut msg = Message::new(
            &format!("{peer}-{seq}"),
            peer,
            Signal::new(3, false, false, true, 3, Visibility::Direct),
            None,
        );
        msg.seq = *seq;
        let wire = WireMessage::from_message(&msg, key);
        self.deliver(peer, &wire);
        self.latest.insert(peer.to_string(), wire);
    }

    /// `peer` resends its latest message unchanged.
    fn replay(&mut self, peer: &str) {
        let wire = self.latest.get(peer).cloned().expect("send before replay");
        self.deliver(peer, &wire);
    }

    fn advance(&mut self, secs: u64) {
        self.node
            .sybil_guard
            .advance_time(Duration::from_secs(secs));
    }

    fn score(&mut self, peer: &str) -> f32 {
        self.node.sybil_guard.reputation(&peer.to_string())
    }

    fn banned(&mut self, peer: &str) -> bool {
        self.node.sybil_guard.is_banned(&peer.to_string())
    }
}

#[test]
fn three_rapid_replays_ban() {
    let mut sim = Sim::new();
    sim.send("atk");
    for _ in 0..3 {
        sim.replay("atk");
    }
    assert!(sim.banned("atk"));
}

/// A peer that behaves for an hour cannot bank credit for a later burst: three replays
/// still ban it.
#[test]
fn good_behaviour_cannot_be_banked() {
    let mut sim = Sim::new();
    for _ in 0..360 {
        sim.send("atk");
        sim.advance(10);
    }
    assert_eq!(sim.score("atk"), INITIAL_SCORE);
    for _ in 0..3 {
        sim.replay("atk");
    }
    assert!(sim.banned("atk"));
}

/// Forgiveness depends on elapsed time only: a struck peer that keeps sending valid
/// traffic recovers no faster than one that stays silent.
#[test]
fn valid_traffic_does_not_speed_up_recovery() {
    let mut sim = Sim::new();
    sim.send("busy");
    sim.send("quiet");
    sim.replay("busy");
    sim.replay("quiet");
    for _ in 0..100 {
        sim.send("busy"); // 1 message per second, within the token bucket
        sim.advance(1);
    }
    let busy = sim.score("busy");
    let quiet = sim.score("quiet");
    assert!((busy - quiet).abs() < 1e-3, "busy {busy}, quiet {quiet}");
    let expected = INITIAL_SCORE - STRIKE_PENALTY + 100.0 * RECOVERY_PER_SEC;
    assert!(
        (busy - expected).abs() < 1e-3,
        "busy {busy}, expected {expected}"
    );
}

/// An honest peer with an occasional fault (one strike every 20 minutes for 4 hours) is
/// never banned and is back at its initial score before each next fault.
#[test]
fn honest_peer_with_occasional_faults_recovers() {
    let mut sim = Sim::new();
    sim.send("honest");
    for _ in 0..12 {
        sim.replay("honest");
        assert!(!sim.banned("honest"));
        for _ in 0..120 {
            sim.advance(10);
            sim.send("honest");
        }
        assert!(
            (sim.score("honest") - INITIAL_SCORE).abs() < 1e-3,
            "score {} before the next fault",
            sim.score("honest")
        );
    }
}

/// Two faults close together cap recovery (strike damping) until the older one leaves
/// the strike memory; the peer then recovers fully and is never capped for good.
#[test]
fn damping_is_temporary() {
    let mut sim = Sim::new();
    sim.send("p");
    sim.replay("p");
    sim.advance(10);
    sim.replay("p");
    let damped = sim.score("p");
    sim.advance(280); // older strike still remembered
    assert_eq!(sim.score("p"), damped);
    sim.advance(1_000);
    assert_eq!(sim.score("p"), INITIAL_SCORE);
}

/// Below the forgiveness period (300 s) a paced attacker is banned.
#[test]
fn attacker_pacing_faster_than_forgiveness_is_banned() {
    let mut sim = Sim::new();
    let mut banned_at = None;
    sim.send("atk");
    for t in 1..=3_600u64 {
        sim.advance(1);
        if t % 10 == 0 {
            sim.send("atk");
        }
        if t % 120 == 0 {
            sim.replay("atk");
        }
        if banned_at.is_none() && sim.banned("atk") {
            banned_at = Some(t);
        }
    }
    assert!(
        banned_at.is_some(),
        "a replay every 120 s must lead to a ban"
    );
}

/// Residual bound: an attacker pacing at the forgiveness period stays admitted, but lands
/// at most one strike per 300 s.
#[test]
fn attacker_pacing_at_forgiveness_period_is_bounded() {
    let mut sim = Sim::new();
    let mut strikes = 0;
    sim.send("atk");
    for t in 1..=3_600u64 {
        sim.advance(1);
        if t % 10 == 0 {
            sim.send("atk");
        }
        if t % 300 == 0 {
            let before = sim.node.byzantine_events;
            sim.replay("atk");
            strikes += sim.node.byzantine_events - before;
        }
        assert!(!sim.banned("atk"));
    }
    assert_eq!(strikes, 12);
}
