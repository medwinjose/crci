//! Switchable defences, for baselines and ablations.
//!
//! `Full` is the protocol as shipped. `StaticBucket` keeps only the per-sender token bucket: no
//! strike ever bans and there is no per-origin budget. `Vanilla` keeps nothing but signature
//! checks and the same-ID dedupe: no admission limit, no bans, no origin budget. Strikes are
//! still counted in every mode, so a run can report what a mode would have punished.
//! `strikes_to_ban(k)` makes the k-th strike ban, for the strike-threshold ablation.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{DefenceMode, NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const NODE: &str = "receiver";

fn new_node(mode: DefenceMode) -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new(NODE, "zone-a", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.set_defence_mode(mode);
    (node, inbox)
}

fn signed(origin: &Identity, msg_id: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        msg_id,
        &origin.id,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, origin)
}

fn deliver(node: &mut NodeRuntime, inbox: &SharedInbox, relay: &str, wire: &WireMessage) {
    let bytes = serde_json::to_vec(wire).unwrap();
    inbox
        .lock()
        .unwrap()
        .entry(node.id.clone())
        .or_default()
        .push((relay.to_string(), bytes));
    node.process_inbox();
}

/// `copies` deliveries of one message by one relay: the first is accepted, each repeat is a
/// strike.
fn replay(node: &mut NodeRuntime, inbox: &SharedInbox, relay: &str, copies: usize) {
    let origin = Identity::new("origin-a");
    let wire = signed(&origin, "replayed", 1);
    for _ in 0..copies {
        deliver(node, inbox, relay, &wire);
    }
}

#[test]
fn full_bans_a_replayer_after_three_strikes() {
    let (mut node, inbox) = new_node(DefenceMode::Full);
    replay(&mut node, &inbox, "relay", 4);
    assert!(node.sybil_guard.is_banned(&"relay".to_string()));
    assert_eq!(node.byzantine_events, 3);
}

#[test]
fn the_baselines_count_strikes_but_never_ban() {
    for mode in [DefenceMode::StaticBucket, DefenceMode::Vanilla] {
        let (mut node, inbox) = new_node(mode);
        replay(&mut node, &inbox, "relay", 20);
        assert!(
            !node.sybil_guard.is_banned(&"relay".to_string()),
            "{mode:?} must never ban"
        );
        assert_eq!(
            node.byzantine_events, 19,
            "{mode:?} still counts every repeat"
        );
        assert!(
            node.sybil_guard.reputation(&"relay".to_string()) < 0.1,
            "{mode:?}: the score still records the strikes"
        );
    }
}

/// `count` fresh messages from distinct origins delivered by one relay at one instant.
fn flood_from_one_relay(node: &mut NodeRuntime, inbox: &SharedInbox, count: usize) {
    for i in 0..count {
        let origin = Identity::new(&format!("origin-{i}"));
        deliver(node, inbox, "relay", &signed(&origin, &format!("m-{i}"), 1));
    }
}

#[test]
fn the_per_sender_bucket_limits_full_and_static_bucket_but_not_vanilla() {
    for (mode, limited) in [
        (DefenceMode::Full, true),
        (DefenceMode::StaticBucket, true),
        (DefenceMode::Vanilla, false),
    ] {
        let (mut node, inbox) = new_node(mode);
        flood_from_one_relay(&mut node, &inbox, 100);
        if limited {
            assert!(
                node.sybil_rate_limited_events >= 70,
                "{mode:?}: expected the 20-message burst to be exceeded, got {} drops",
                node.sybil_rate_limited_events
            );
        } else {
            assert_eq!(
                node.sybil_rate_limited_events, 0,
                "{mode:?} has no admission limit"
            );
            assert_eq!(node.messages_handled, 100);
        }
    }
}

/// One origin, many messages, each relayed by a different neighbour (so no sender bucket
/// is exhausted): only the per-origin budget can stop it.
fn flood_from_one_origin(node: &mut NodeRuntime, inbox: &SharedInbox, count: usize) {
    let origin = Identity::new("flooder");
    for i in 0..count {
        deliver(
            node,
            inbox,
            &format!("relay-{i}"),
            &signed(&origin, &format!("f-{i}"), i as u64 + 1),
        );
    }
}

#[test]
fn only_full_has_a_per_origin_budget() {
    for (mode, budgeted) in [
        (DefenceMode::Full, true),
        (DefenceMode::StaticBucket, false),
        (DefenceMode::Vanilla, false),
    ] {
        let (mut node, inbox) = new_node(mode);
        flood_from_one_origin(&mut node, &inbox, 100);
        if budgeted {
            assert!(
                node.origin_rate_limited_events >= 70,
                "{mode:?}: the origin budget (20) must stop most of 100, got {}",
                node.origin_rate_limited_events
            );
        } else {
            assert_eq!(
                node.origin_rate_limited_events, 0,
                "{mode:?} has no origin budget"
            );
            assert_eq!(node.messages_handled, 100);
        }
    }
}

#[test]
fn strikes_to_ban_bans_exactly_at_the_kth_strike() {
    for k in 1..=5u32 {
        let (mut node, inbox) = new_node(DefenceMode::Full);
        node.sybil_guard.set_strikes_to_ban(k);
        let relay = "relay".to_string();
        let wire = signed(&Identity::new("origin-a"), "replayed", 1);
        // The first copy is accepted; each further copy is one strike.
        for _ in 0..k {
            deliver(&mut node, &inbox, "relay", &wire);
        }
        assert!(
            !node.sybil_guard.is_banned(&relay),
            "k = {k}: {} strikes must not ban",
            k - 1
        );
        deliver(&mut node, &inbox, "relay", &wire);
        assert!(
            node.sybil_guard.is_banned(&relay),
            "k = {k}: the {k}th strike must ban"
        );
    }
}

#[test]
fn k_equal_three_is_the_shipped_penalty() {
    let (mut node, _inbox) = new_node(DefenceMode::Full);
    let shipped = node.sybil_guard.strike_penalty;
    node.sybil_guard.set_strikes_to_ban(3);
    assert!(
        (node.sybil_guard.strike_penalty - shipped).abs() < 1e-6,
        "k = 3 must keep the shipped penalty {shipped}, got {}",
        node.sybil_guard.strike_penalty
    );
}
