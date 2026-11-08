//! Origin IDs bound to keys.
//!
//! A message is authentic only if it is signed and its origin is the node ID derived from the
//! key that signed it. No one can claim another node's ID, so an impostor that announces an
//! honest node's ID first cannot make receivers drop that node's messages, and a neighbour that
//! forwards a message claiming someone else's ID is struck like one that forwards a forgery.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A message signed by `signer` that claims `origin`, with the signature over that claim.
fn claim(signer: &Identity, origin: &str, id: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    let mut wire = WireMessage::from_message(&msg, signer);
    wire.origin = origin.to_string();
    wire.sign_with(signer);
    wire
}

fn receiver() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let node = NodeRuntime::new("receiver", "zone-a", inbox.clone());
    (node, inbox)
}

fn deliver(
    node: &mut NodeRuntime,
    inbox: &SharedInbox,
    sender: &str,
    wire: &WireMessage,
) -> Vec<WireMessage> {
    inbox
        .lock()
        .unwrap()
        .entry(node.id.clone())
        .or_default()
        .push((sender.to_string(), serde_json::to_vec(wire).unwrap()));
    node.process_inbox_delivered()
}

#[test]
fn an_impostor_that_announces_an_honest_id_first_does_not_silence_it() {
    let honest = Identity::new("honest");
    let impostor = Identity::new("impostor");
    let (mut node, inbox) = receiver();

    let fake = claim(&impostor, &honest.id, "fake-1", 1);
    assert!(deliver(&mut node, &inbox, "relay-x", &fake).is_empty());

    let real = claim(&honest, &honest.id, "real-1", 2);
    let got = deliver(&mut node, &inbox, "relay-y", &real);
    assert_eq!(
        got.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(),
        vec!["real-1"],
        "the honest node's message must be accepted whatever arrived first"
    );
}

#[test]
fn forwarding_a_message_that_claims_another_nodes_id_is_a_strike() {
    let victim = Identity::new("victim");
    let impostor = Identity::new("impostor");
    let (mut node, inbox) = receiver();

    let fake = claim(&impostor, &victim.id, "fake-1", 1);
    assert!(deliver(&mut node, &inbox, "relay-x", &fake).is_empty());
    assert!(
        node.sybil_guard.reputation(&"relay-x".to_string()) < 0.5,
        "a message whose origin is not its signer's ID is inauthentic: its sender is struck"
    );
}

// ── Persistent keys ──────────────────────────────────────────────────────────────

use crci_core::identity::{load_or_create, SEQ_BLOCK};

#[tokio::test]
async fn a_stored_key_is_reloaded_and_each_start_reserves_new_sequence_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node.key");
    let (first, floor_1) = load_or_create(&path, "correct horse").await.unwrap();
    let (again, floor_2) = load_or_create(&path, "correct horse").await.unwrap();
    assert_eq!(first.to_bytes(), again.to_bytes());
    assert_eq!((floor_1, floor_2), (0, SEQ_BLOCK));
    let id = Identity::from_signing_key("restarted", again);
    assert_eq!(id.id, Identity::from_signing_key("first", first).id);
}

#[tokio::test]
async fn a_wrong_or_empty_passphrase_is_refused_and_no_new_key_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node.key");
    let (key, _) = load_or_create(&path, "correct horse").await.unwrap();
    assert!(load_or_create(&path, "wrong").await.is_err());
    assert!(load_or_create(&path, "").await.is_err());
    let (still, _) = load_or_create(&path, "correct horse").await.unwrap();
    assert_eq!(key.to_bytes(), still.to_bytes());
}

/// A restarted node keeps its ID, and its new messages are accepted by a neighbour that saw the
/// old ones: they are numbered above anything the previous run used. Without the stored floor
/// its first new message would reuse a sequence number the neighbour already holds from it, and
/// the neighbour would strike it as a replay.
#[tokio::test]
async fn a_restarted_node_keeps_its_id_and_its_new_messages_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node.key");
    let (mut neighbour, inbox) = receiver();
    // Control: a second neighbour that sees only the first run, then a restart without the floor.
    let (mut control, control_inbox) = receiver();

    let start = |key, floor| {
        let node_inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut node =
            NodeRuntime::with_identity(Identity::from_signing_key("a", key), "zone-a", node_inbox);
        node.seq_counter = floor;
        node.control_seq_counter = floor;
        node
    };
    let send = |node: &mut NodeRuntime, id: &str| {
        let mut msg = Message::new(
            id,
            "a",
            Signal::new(3, true, false, true, 3, Visibility::Direct),
            None,
        );
        node.seq_counter += 1;
        msg.seq = node.seq_counter;
        WireMessage::from_message(&msg, &node.identity)
    };

    let (key, floor) = load_or_create(&path, "pw").await.unwrap();
    let mut first_run = start(key, floor);
    for i in 1..=3 {
        let wire = send(&mut first_run, &format!("before-{i}"));
        assert_eq!(deliver(&mut neighbour, &inbox, "a-link", &wire).len(), 1);
        assert_eq!(
            deliver(&mut control, &control_inbox, "a-link", &wire).len(),
            1
        );
    }

    let (key, floor) = load_or_create(&path, "pw").await.unwrap();
    let mut second_run = start(key, floor);
    assert_eq!(second_run.id, first_run.id, "the ID survives a restart");
    let wire = send(&mut second_run, "after-1");
    assert_eq!(deliver(&mut neighbour, &inbox, "a-link", &wire).len(), 1);
    assert_eq!(neighbour.byzantine_events, 0);
    assert!(neighbour.sybil_guard.reputation(&"a-link".to_string()) >= 0.5);

    // The same restart without the floor: the first new message reuses sequence number 1.
    let mut no_floor = start(load_or_create(&path, "pw").await.unwrap().0, 0);
    let wire = send(&mut no_floor, "after-no-floor");
    assert!(deliver(&mut control, &control_inbox, "a-link", &wire).is_empty());
    assert_eq!(
        control.byzantine_events, 1,
        "a reused sequence number is a strike"
    );
}
