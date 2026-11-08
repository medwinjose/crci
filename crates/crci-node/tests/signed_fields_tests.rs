//! Signature coverage of `WireMessage`.
//!
//! The origin's Ed25519 signature must cover every field a receiver acts on, in an
//! unambiguous encoding, so that a relay cannot change any of them without the message
//! failing verification. Each test below rewrites one field of a validly signed message,
//! as a relay would, and checks that the signature no longer verifies.

use crci_core::identity::Identity;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

fn signed_with(identity: &Identity, id: &str, origin: &str, note: Option<&str>) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        note,
    );
    msg.message_type = MessageType::RescueRequest;
    msg.seq = 7;
    WireMessage::from_message(&msg, identity)
}

fn signed() -> WireMessage {
    let alice = Identity::new("alice");
    signed_with(
        &alice,
        "msg-1",
        "alice",
        Some("trapped on the second floor"),
    )
}

/// Asserts that the untouched message verifies and the rewritten one does not.
fn assert_rewrite_breaks_signature(field: &str, rewrite: impl FnOnce(&mut WireMessage)) {
    let original = signed();
    assert!(
        original.verify_signature(),
        "the untouched message must verify"
    );
    let mut tampered = original.clone();
    rewrite(&mut tampered);
    assert!(
        !tampered.verify_signature(),
        "rewriting `{field}` must break the origin's signature"
    );
}

#[test]
fn rewriting_id_breaks_signature() {
    assert_rewrite_breaks_signature("id", |w| w.id = "msg-2".to_string());
}

#[test]
fn rewriting_origin_breaks_signature() {
    assert_rewrite_breaks_signature("origin", |w| w.origin = "bob".to_string());
}

#[test]
fn rewriting_severity_breaks_signature() {
    assert_rewrite_breaks_signature("severity", |w| w.severity = 1);
}

#[test]
fn rewriting_confidence_breaks_signature() {
    assert_rewrite_breaks_signature("confidence", |w| w.confidence = 5);
}

#[test]
fn rewriting_needs_help_breaks_signature() {
    assert_rewrite_breaks_signature("needs_help", |w| w.needs_help = !w.needs_help);
}

#[test]
fn rewriting_can_help_others_breaks_signature() {
    assert_rewrite_breaks_signature("can_help_others", |w| {
        w.can_help_others = !w.can_help_others
    });
}

#[test]
fn rewriting_location_confirmed_breaks_signature() {
    assert_rewrite_breaks_signature("location_confirmed", |w| {
        w.location_confirmed = !w.location_confirmed
    });
}

#[test]
fn rewriting_visibility_breaks_signature() {
    assert_rewrite_breaks_signature("visibility", |w| w.visibility = "unknown".to_string());
}

#[test]
fn rewriting_note_breaks_signature() {
    assert_rewrite_breaks_signature("note", |w| w.note = Some("all clear".to_string()));
}

#[test]
fn removing_note_breaks_signature() {
    assert_rewrite_breaks_signature("note", |w| w.note = None);
}

#[test]
fn rewriting_message_type_breaks_signature() {
    assert_rewrite_breaks_signature("message_type", |w| w.message_type = "mce".to_string());
}

#[test]
fn rewriting_origin_active_breaks_signature() {
    assert_rewrite_breaks_signature("origin_active", |w| w.origin_active = !w.origin_active);
}

#[test]
fn rewriting_seq_breaks_signature() {
    assert_rewrite_breaks_signature("seq", |w| w.seq = 1_000_000);
}

#[test]
fn rewriting_origin_pubkey_breaks_signature() {
    let other = Identity::new("mallory");
    assert_rewrite_breaks_signature("origin_pubkey", move |w| {
        w.origin_pubkey = other.verifying_key.to_bytes().to_vec()
    });
}

#[test]
fn absent_note_and_empty_note_are_signed_differently() {
    let alice = Identity::new("alice");
    let mut wire = signed_with(&alice, "msg-1", "alice", None);
    assert!(wire.verify_signature());
    wire.note = Some(String::new());
    assert!(
        !wire.verify_signature(),
        "a relay must not be able to turn an absent note into an empty one"
    );
}

/// With a separator-joined payload, (`id` = "a:b", `origin` = "c") and (`id` = "a",
/// `origin` = "b:c") sign the same bytes, so a signature for one verifies on the other.
#[test]
fn field_boundaries_are_unambiguous() {
    let key = Identity::new("shared-key");
    let first = signed_with(&key, "a:b", "c", None);
    assert!(first.verify_signature());

    let mut shifted = first.clone();
    shifted.id = "a".to_string();
    shifted.origin = "b:c".to_string();
    assert!(
        !shifted.verify_signature(),
        "moving bytes between `id` and `origin` must break the signature"
    );
}

/// A relay that rewrites a signed message's type to a mass-casualty event is rejected
/// on the signed path and the relay is struck.
#[test]
fn relay_rewriting_message_type_is_rejected_and_struck() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone-a", inbox.clone());

    let mut tampered = signed();
    tampered.message_type = "mce".to_string();
    tampered.note = Some("evacuate the whole district".to_string());
    inbox
        .lock()
        .unwrap()
        .entry(node.id.clone())
        .or_default()
        .push(("relay".to_string(), serde_json::to_vec(&tampered).unwrap()));
    node.process_inbox();

    assert!(node.persistent_messages.is_empty());
    assert_eq!(node.metrics.messages_accepted.load(Ordering::Relaxed), 0);
    assert_eq!(node.metrics.byzantine_detected.load(Ordering::Relaxed), 1);
    assert!(
        node.sybil_guard.reputation(&"relay".to_string()) < 0.5,
        "the relaying neighbour must take a strike"
    );
}

/// The signed bytes start with a domain tag, so a signature the origin made over the same
/// field encoding for another purpose does not verify as a message signature.
#[test]
fn signature_over_another_domain_does_not_verify() {
    use crci_core::runtime::WIRE_SIGNING_DOMAIN;

    let alice = Identity::new("alice");
    let mut wire = signed_with(&alice, "msg-1", "alice", None);
    let bytes = wire.signing_bytes();
    assert!(bytes.starts_with(WIRE_SIGNING_DOMAIN));

    let mut other_domain = b"CRCI-OTHER-PURPOSE-v1\0".to_vec();
    other_domain.extend_from_slice(&bytes[WIRE_SIGNING_DOMAIN.len()..]);
    wire.signature_bytes = alice.sign(&other_domain).to_bytes().to_vec();
    assert!(!wire.verify_signature());
}

/// A small-order public key (here the identity point) lets one signature verify for every
/// message under non-strict verification: R = identity, s = 0. Such a key must be rejected,
/// or a relay could rewrite any field of that origin's messages.
#[test]
fn small_order_key_does_not_verify_any_message() {
    let mut wire = signed();
    let mut identity_point = [0u8; 32];
    identity_point[0] = 1;
    wire.origin_pubkey = identity_point.to_vec();
    let mut forged = [0u8; 64];
    forged[0] = 1; // R = identity point, s = 0
    wire.signature_bytes = forged.to_vec();
    assert!(!wire.verify_signature());
    wire.note = Some("rewritten by a relay".to_string());
    assert!(!wire.verify_signature());
}
