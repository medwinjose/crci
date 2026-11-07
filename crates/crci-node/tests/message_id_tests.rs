//! A message ID is chosen by its signer, so it says nothing about who may use it. An adversary
//! that signs its own message under the ID of an honest one must not make receivers treat the
//! honest message as a copy of it.

use crci_core::identity::Identity;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const NODE: &str = "receiver";
const HONEST: &str = "origin-a";
const ADVERSARY: &str = "origin-z";
const SHARED_ID: &str = "rescue-1";

fn new_node() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let node = NodeRuntime::new(NODE, "zone-a", inbox.clone());
    (node, inbox)
}

fn signed(signer: &Identity, id: &str, origin: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    msg.message_type = MessageType::RescueRequest;
    WireMessage::from_message(&msg, signer)
}

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

fn accepted_seqs(node: &NodeRuntime, origin: &str) -> Vec<u64> {
    node.recent_messages
        .get(origin)
        .map(|m| m.keys().copied().collect())
        .unwrap_or_default()
}

#[test]
fn an_adversary_message_under_the_same_id_does_not_suppress_the_honest_one() {
    let (mut node, inbox) = new_node();
    let honest = Identity::new(HONEST);
    let adversary = Identity::new(ADVERSARY);

    let squat = signed(&adversary, SHARED_ID, ADVERSARY, 1);
    deliver(&mut node, &inbox, "peer-z", &squat);
    let genuine = signed(&honest, SHARED_ID, HONEST, 1);
    deliver(&mut node, &inbox, "peer-b", &genuine);

    assert_eq!(accepted_seqs(&node, HONEST), vec![1], "honest message lost");
    assert_eq!(accepted_seqs(&node, ADVERSARY), vec![1]);
    assert_eq!(node.byzantine_events, 0);
}

#[test]
fn a_frame_rejected_for_its_key_does_not_suppress_the_honest_message() {
    let (mut node, inbox) = new_node();
    let honest = Identity::new(HONEST);
    let adversary = Identity::new(ADVERSARY);

    // The honest origin's key is pinned by its first message.
    deliver(
        &mut node,
        &inbox,
        "peer-b",
        &signed(&honest, "first", HONEST, 1),
    );
    // Validly signed, but with the adversary's key under the honest origin's name.
    let squat = signed(&adversary, SHARED_ID, HONEST, 2);
    deliver(&mut node, &inbox, "peer-z", &squat);
    assert_eq!(accepted_seqs(&node, HONEST), vec![1], "wrong key accepted");

    let genuine = signed(&honest, SHARED_ID, HONEST, 2);
    deliver(&mut node, &inbox, "peer-b", &genuine);
    assert_eq!(
        accepted_seqs(&node, HONEST),
        vec![1, 2],
        "honest message lost"
    );
}

#[test]
fn an_adversary_message_under_the_same_id_does_not_replace_a_stored_rescue() {
    let (mut node, inbox) = new_node();
    let honest = Identity::new(HONEST);
    let adversary = Identity::new(ADVERSARY);

    deliver(
        &mut node,
        &inbox,
        "peer-b",
        &signed(&honest, SHARED_ID, HONEST, 1),
    );
    deliver(
        &mut node,
        &inbox,
        "peer-z",
        &signed(&adversary, SHARED_ID, ADVERSARY, 1),
    );

    assert!(
        node.persistent_messages
            .values()
            .any(|m| m.origin == HONEST),
        "stored rescue replaced"
    );
}

#[test]
fn a_repeat_of_the_same_signed_message_is_still_a_strike() {
    let (mut node, inbox) = new_node();
    let adversary = Identity::new(ADVERSARY);
    let wire = signed(&adversary, SHARED_ID, ADVERSARY, 1);

    deliver(&mut node, &inbox, "peer-z", &wire);
    assert_eq!(node.byzantine_events, 0);
    deliver(&mut node, &inbox, "peer-z", &wire);
    assert_eq!(node.byzantine_events, 1);
}
