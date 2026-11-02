//! Signature-check budget.
//!
//! Signature verifications are bounded per transport sender by the SybilGuard token
//! bucket, which refills over time. An honest origin must never run out of budget, and
//! messages that merely *claim* an origin must not spend that origin's budget.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::{SybilError, SENDER_BURST, SENDER_REFILL_PER_SEC};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RECEIVER: &str = "receiver";

fn signed(identity: &Identity, id: &str, origin: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

fn forged(identity: &Identity, id: &str, origin: &str, seq: u64) -> WireMessage {
    let mut wire = signed(identity, id, origin, seq);
    wire.signature_bytes[0] ^= 0xFF;
    wire
}

fn deliver(inbox: &SharedInbox, sender: &str, wires: &[WireMessage]) {
    let mut guard = inbox.lock().unwrap();
    let queue = guard.entry(RECEIVER.to_string()).or_default();
    for wire in wires {
        queue.push((sender.to_string(), serde_json::to_vec(wire).unwrap()));
    }
}

fn receiver() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    (
        NodeRuntime::new(RECEIVER, "zone-alpha", inbox.clone()),
        inbox,
    )
}

#[test]
fn honest_origin_sending_twenty_messages_over_time_has_all_accepted() {
    let (mut node, inbox) = receiver();
    let alice = Identity::new("alice");

    for seq in 1..=20u64 {
        deliver(
            &inbox,
            "alice",
            &[signed(&alice, &format!("alice-{}", seq), "alice", seq)],
        );
        node.process_inbox();
        std::thread::sleep(Duration::from_millis(50));
    }

    assert_eq!(node.metrics.messages_accepted.load(Ordering::Relaxed), 20);
    assert_eq!(node.byzantine_events, 0);
    assert!(node.sybil_guard.check(&"alice".to_string()).is_ok());
}

#[test]
fn forged_messages_claiming_an_origin_do_not_spend_its_budget() {
    let (mut node, inbox) = receiver();
    let alice = Identity::new("alice");

    // Two attackers each send three bad-signature messages that claim to come from alice.
    for attacker in ["mallory-1", "mallory-2"] {
        let key = Identity::new(attacker);
        let wires: Vec<WireMessage> = (1..=3u64)
            .map(|i| forged(&key, &format!("{}-{}", attacker, i), "alice", i))
            .collect();
        deliver(&inbox, attacker, &wires);
    }
    node.process_inbox();

    deliver(
        &inbox,
        "alice",
        &[signed(&alice, "alice-real", "alice", 10)],
    );
    node.process_inbox();

    assert_eq!(node.metrics.messages_accepted.load(Ordering::Relaxed), 1);

    // Every bad signature struck the attacker that delivered it, so both are banned.
    for attacker in ["mallory-1", "mallory-2"] {
        assert!(matches!(
            node.sybil_guard.check(&attacker.to_string()),
            Err(SybilError::Banned(_))
        ));
    }
}

#[test]
fn invalid_signature_flood_is_bounded_by_strikes() {
    let (mut node, inbox) = receiver();
    let flooder = Identity::new("flooder");

    // Every message claims a different origin, so a per-origin cap would not bound it.
    let wires: Vec<WireMessage> = (1..=100u64)
        .map(|i| {
            forged(
                &flooder,
                &format!("flood-{}", i),
                &format!("origin-{}", i),
                1,
            )
        })
        .collect();
    deliver(&inbox, "flooder", &wires);
    node.process_inbox();

    // Three strikes ban the sender; no further signatures are checked.
    assert_eq!(
        node.metrics
            .sig_verifications_performed
            .load(Ordering::Relaxed),
        3
    );
    assert!(matches!(
        node.sybil_guard.check(&"flooder".to_string()),
        Err(SybilError::Banned(_))
    ));
}

#[test]
fn valid_signature_burst_is_bounded_by_sender_budget() {
    let (mut node, inbox) = receiver();
    let sender = Identity::new("chatty");

    let wires: Vec<WireMessage> = (1..=100u64)
        .map(|i| {
            signed(
                &sender,
                &format!("burst-{}", i),
                &format!("origin-{}", i),
                1,
            )
        })
        .collect();
    deliver(&inbox, "chatty", &wires);

    let start = Instant::now();
    node.process_inbox();
    let elapsed = start.elapsed().as_secs_f32();

    let checks = node
        .metrics
        .sig_verifications_performed
        .load(Ordering::Relaxed);
    let bound = (SENDER_BURST + SENDER_REFILL_PER_SEC * elapsed).floor() as u64;
    assert!(
        checks <= bound,
        "{} signature checks exceed the sender budget of {} ({} s)",
        checks,
        bound,
        elapsed
    );
    assert!(node.sybil_rate_limited_events >= 100 - bound);
}

/// Every link carries every message, so most frames a node receives are copies of messages
/// it already has from another neighbour. Those copies are dropped before admission: they
/// spend neither the sender's budget nor a signature check, so the budget limits only new
/// messages and the network-wide rate is not capped by one link's budget.
#[test]
fn copies_already_received_from_another_neighbour_spend_no_budget() {
    let (mut node, inbox) = receiver();
    node.sybil_guard.use_manual_clock();
    let alice = Identity::new("alice");
    let total = 2 * SENDER_BURST as u64;
    let wires: Vec<WireMessage> = (1..=total)
        .map(|seq| signed(&alice, &format!("m-{seq}"), "alice", seq))
        .collect();
    for chunk in wires.chunks(SENDER_BURST as usize / 2) {
        deliver(&inbox, "first-path", chunk);
        deliver(&inbox, "second-path", chunk);
        node.process_inbox();
        node.sybil_guard.advance_time(Duration::from_secs(2));
    }
    let verified = node
        .metrics
        .sig_verifications_performed
        .load(Ordering::Relaxed);
    assert_eq!(verified, total, "one signature check per new message");
    assert_eq!(node.sybil_rate_limited_events, 0);
    assert_eq!(
        node.sybil_guard
            .available_tokens(&"second-path".to_string()),
        None,
        "the second path never touched its budget"
    );
}

/// A sender that keeps its budget empty cannot hide replays behind the rate limit: a
/// repeat of a message it already delivered is judged before admission and struck.
#[test]
fn a_replay_over_budget_is_still_struck() {
    let (mut node, inbox) = receiver();
    node.sybil_guard.use_manual_clock();
    let mallory = Identity::new("mallory");
    let wires: Vec<WireMessage> = (1..=SENDER_BURST as u64)
        .map(|seq| signed(&mallory, &format!("m-{seq}"), "mallory", seq))
        .collect();
    deliver(&inbox, "mallory", &wires); // spends the whole burst
    node.process_inbox();
    deliver(&inbox, "mallory", &wires[..3]); // replays with no budget left
    node.process_inbox();
    assert_eq!(node.byzantine_events, 3);
    assert!(node.sybil_guard.is_banned(&"mallory".to_string()));
}
