//! Replay-window semantics on the signed message path (`NodeRuntime::process_inbox`).
//!
//! Rule under test, with W = window size and `seq_max` the highest sequence number
//! accepted from an origin:
//! - `seq < seq_max - W` (out of window): dropped silently, no strike on the relay.
//! - inside the window, first delivery: accepted, even if it arrives out of order.
//! - inside the window, the same message again from the same transport peer: strike.
//! - inside the window, the same message from a different transport peer: dropped
//!   silently (normal gossip redundancy).
//! - the sequence number is covered by the origin's signature, so a relay cannot
//!   rewrite it.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const NODE: &str = "receiver";
const ORIGIN: &str = "origin-a";

fn new_node() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let node = NodeRuntime::new(NODE, "zone-a", inbox.clone());
    (node, inbox)
}

fn signed(origin: &Identity, msg_id: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        msg_id,
        ORIGIN,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, origin)
}

/// Queues `wire` as if `relay` had sent it, then runs one inbox pass.
fn deliver(node: &mut NodeRuntime, inbox: &SharedInbox, relay: &str, wire: &WireMessage) {
    let bytes = serde_json::to_vec(wire).unwrap();
    inbox
        .lock()
        .unwrap()
        .entry(NODE.to_string())
        .or_default()
        .push((relay.to_string(), bytes));
    node.process_inbox();
}

fn relay_score(node: &mut NodeRuntime, relay: &str) -> f32 {
    node.sybil_guard.reputation(&relay.to_string())
}

/// No strike: a fresh peer starts at 0.5 and one strike costs 0.15.
fn assert_not_struck(node: &mut NodeRuntime, relay: &str) {
    let score = relay_score(node, relay);
    assert!(score > 0.45, "{relay} was struck: score {score}");
}

#[test]
fn reordered_packet_inside_window_is_accepted_without_strike() {
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);

    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m5", 5));
    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m3", 3));

    assert_eq!(
        node.messages_handled, 2,
        "late packet inside the window must be accepted"
    );
    assert_eq!(node.byzantine_events, 0);
    assert_not_struck(&mut node, "relay-b");
}

#[test]
fn out_of_window_packet_is_dropped_without_strike() {
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);

    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m100", 100));
    // 100 - 64 = 36, so seq 35 is out of window and seq 36 is the oldest in-window value.
    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m35", 35));

    assert_eq!(
        node.messages_handled, 1,
        "out-of-window packet must not be accepted"
    );
    assert_eq!(node.byzantine_events, 0);
    assert_not_struck(&mut node, "relay-b");

    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m36", 36));
    assert_eq!(
        node.messages_handled, 2,
        "seq_max - W is still inside the window"
    );
}

#[test]
fn stale_packets_never_ban_the_relay() {
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);

    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m100", 100));
    for seq in 1..=3 {
        deliver(
            &mut node,
            &inbox,
            "relay-b",
            &signed(&origin, &format!("m{seq}"), seq),
        );
    }

    assert_eq!(node.messages_handled, 1);
    assert_not_struck(&mut node, "relay-b");
}

#[test]
fn resending_an_out_of_window_message_is_not_a_strike() {
    // An honest peer that reconnects may resend a message it already delivered;
    // once that message has left the window it is dropped as stale, not struck.
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);
    let old = signed(&origin, "m1", 1);

    deliver(&mut node, &inbox, "relay-b", &old);
    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m100", 100));
    deliver(&mut node, &inbox, "relay-b", &old);

    assert_eq!(node.messages_handled, 2);
    assert_eq!(node.byzantine_events, 0);
    assert_not_struck(&mut node, "relay-b");
}

#[test]
fn duplicate_inside_window_from_same_peer_is_struck_and_banned_after_three() {
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);
    let wire = signed(&origin, "m5", 5);

    deliver(&mut node, &inbox, "relay-evil", &wire);
    deliver(&mut node, &inbox, "relay-evil", &wire);
    assert!(
        relay_score(&mut node, "relay-evil") < 0.4,
        "one in-window duplicate from the same peer must cost one strike"
    );

    deliver(&mut node, &inbox, "relay-evil", &wire);
    deliver(&mut node, &inbox, "relay-evil", &wire);
    assert!(
        node.sybil_guard.check(&"relay-evil".to_string()).is_err(),
        "three strikes must ban the peer"
    );
    assert_eq!(node.messages_handled, 1);
}

#[test]
fn duplicate_inside_window_from_another_peer_is_not_struck() {
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);
    let wire = signed(&origin, "m5", 5);

    deliver(&mut node, &inbox, "relay-b", &wire);
    deliver(&mut node, &inbox, "relay-c", &wire);

    assert_eq!(node.messages_handled, 1);
    assert_eq!(node.byzantine_events, 0);
    assert_not_struck(&mut node, "relay-b");
    assert_not_struck(&mut node, "relay-c");
}

#[test]
fn relay_cannot_rewrite_the_sequence_number() {
    // If `seq` were not signed, a relay could push an origin's seq_max far ahead and
    // make every later honest message from that origin look like a replay.
    let (mut node, inbox) = new_node();
    let origin = Identity::new(ORIGIN);

    let mut forged = signed(&origin, "m1", 1);
    forged.seq = 1_000_000;
    deliver(&mut node, &inbox, "relay-evil", &forged);
    assert_eq!(
        node.messages_handled, 0,
        "a rewritten seq must fail signature verification"
    );

    deliver(&mut node, &inbox, "relay-b", &signed(&origin, "m2", 2));
    assert_eq!(
        node.messages_handled, 1,
        "the origin's window must not have moved"
    );
    assert_not_struck(&mut node, "relay-b");
}
