//! Per-node state stays bounded whatever identities and messages peers send, and what the
//! limits forget is the least active state.

use crci_core::identity::Identity;
use crci_core::integration::{GossipPipeline, MessageKind, PipelineMessage, PipelineVerdict};
use crci_core::merkle::{MerkleChain, StateSnapshot, MERKLE_KEEP_BLOCKS};
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{
    message_key, NodeRuntime, WireMessage, DIVERGENCE_LOG_MAX, OBSERVATIONS_MAX, PERSISTENT_MAX,
    RECENT_MAX_MESSAGES,
};
use crci_core::sybil::{SybilGuard, MAX_TRACKED_PEERS};
use crci_core::transport::tcp::{TcpTransport, MAX_RELAY_TARGETS};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const LIMIT: usize = 32;

fn signed(identity: &Identity, origin: &str, seq: u64, kind: MessageType) -> WireMessage {
    let mut msg = Message::new(
        &format!("{origin}-{seq}"),
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.message_type = kind;
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

fn receiver() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.use_router = false;
    node.max_tracked_origins = LIMIT;
    (node, inbox)
}

/// Deliver one frame after enough simulated time for every budget to refill.
fn deliver(node: &mut NodeRuntime, inbox: &SharedInbox, sender: &str, wire: &WireMessage) {
    node.sybil_guard.advance_time(Duration::from_secs(1));
    inbox
        .lock()
        .unwrap()
        .entry(node.id.clone())
        .or_default()
        .push((sender.to_string(), serde_json::to_vec(wire).unwrap()));
    node.process_inbox();
}

fn flood_new_origins(node: &mut NodeRuntime, inbox: &SharedInbox, tag: &str, count: usize) {
    for i in 0..count {
        let origin = format!("sybil-{tag}-{i}");
        let wire = signed(&Identity::new(&origin), &origin, 1, MessageType::Normal);
        deliver(node, inbox, "flooder", &wire);
    }
}

#[test]
fn a_flood_of_new_origins_leaves_every_origin_table_at_its_limit() {
    let (mut node, inbox) = receiver();
    flood_new_origins(&mut node, &inbox, "a", 4 * LIMIT);

    assert!(node.origins_forgotten > 0);
    for (name, size) in [
        ("activity", node.origin_activity.len()),
        ("budgets", node.origin_budgets.len()),
        ("windows", node.replay_filter.tracked_origins()),
        ("recent", node.recent_messages.len()),
    ] {
        assert!(size <= LIMIT + 1, "{name}: {size} entries");
    }
    // the tables forget an origin together
    assert_eq!(
        node.replay_filter.tracked_origins(),
        node.origin_activity.len()
    );
}

#[test]
fn a_flood_of_one_message_origins_does_not_displace_a_busy_origin() {
    let (mut node, inbox) = receiver();
    let busy = Identity::new("busy");
    let first = signed(&busy, "busy", 1, MessageType::Normal);
    deliver(&mut node, &inbox, "honest", &first);
    for seq in 2..=5 {
        let wire = signed(&busy, "busy", seq, MessageType::Normal);
        deliver(&mut node, &inbox, "honest", &wire);
    }

    flood_new_origins(&mut node, &inbox, "a", 8 * LIMIT);
    assert!(node.origins_forgotten > 0);
    assert!(node.origin_activity.contains_key(&busy.id));

    // Its window survived: an old message is still recognised once the seen cache has gone.
    node.seen_messages.clear();
    let before = node.byzantine_events;
    deliver(&mut node, &inbox, "honest", &first);
    assert_eq!(
        node.byzantine_events,
        before + 1,
        "a replay is still struck"
    );
}

#[test]
fn a_forgotten_origin_is_accepted_again_and_this_is_the_documented_limit() {
    let (mut node, inbox) = receiver();
    let quiet = Identity::new("quiet");
    let old = signed(&quiet, "quiet", 1, MessageType::Normal);
    deliver(&mut node, &inbox, "honest", &old);

    flood_new_origins(&mut node, &inbox, "a", 8 * LIMIT);
    assert!(!node.origin_activity.contains_key(&quiet.id));

    node.seen_messages.clear();
    let accepted = node.messages_handled;
    deliver(&mut node, &inbox, "honest", &old);
    assert_eq!(node.messages_handled, accepted + 1);
}

/// The price of forgetting, in the real code: an adversary floods `relay` until it forgets a quiet
/// origin, then replays an old message of that origin to it. `relay` accepts and forwards it, and
/// a neighbour that still remembers the origin strikes the honest relay. The model checker finds
/// the same behaviour (`origin-forgotten-framing`). A busy origin is not exposed this way.
#[test]
fn a_replay_after_forgetting_gets_an_honest_relay_struck_by_a_neighbour_that_remembers() {
    let (mut relay, relay_inbox) = receiver();
    let (mut neighbour, neighbour_inbox) = receiver();
    neighbour.max_tracked_origins = usize::MAX;
    let quiet = Identity::new("quiet");
    let old = signed(&quiet, "quiet", 1, MessageType::Normal);

    deliver(&mut relay, &relay_inbox, "someone", &old);
    deliver(&mut neighbour, &neighbour_inbox, "relay", &old);

    flood_new_origins(&mut relay, &relay_inbox, "a", 8 * LIMIT);
    assert!(!relay.origin_activity.contains_key(&quiet.id));

    relay.seen_messages.clear();
    let accepted = relay.messages_handled;
    deliver(&mut relay, &relay_inbox, "adversary", &old);
    assert_eq!(
        relay.messages_handled,
        accepted + 1,
        "accepted and forwarded"
    );

    let strikes = neighbour.byzantine_events;
    deliver(&mut neighbour, &neighbour_inbox, "relay", &old);
    assert_eq!(
        neighbour.byzantine_events,
        strikes + 1,
        "the relay is struck"
    );
}

#[test]
fn invalid_signatures_create_no_origin_state() {
    let (mut node, inbox) = receiver();
    for i in 0..LIMIT as u64 {
        let origin = format!("forged-{i}");
        let mut wire = signed(&Identity::new(&origin), &origin, 1, MessageType::Normal);
        wire.signature_bytes[0] ^= 0xFF;
        deliver(&mut node, &inbox, &format!("relay-{i}"), &wire);
    }
    assert_eq!(node.origin_activity.len(), 0);
    assert_eq!(node.origin_budgets.len(), 0);
    assert_eq!(node.replay_filter.tracked_origins(), 0);
    assert_eq!(node.seen_messages.len(), 0);
}

#[test]
fn stores_of_messages_keep_the_newest_up_to_their_limits() {
    let (mut node, inbox) = receiver();
    node.max_tracked_origins = usize::MAX;
    let alice = Identity::new("alice");
    let first = signed(&alice, "alice", 1, MessageType::RescueRequest);
    let total = RECENT_MAX_MESSAGES as u64 + OBSERVATIONS_MAX as u64 / 2;
    deliver(&mut node, &inbox, "honest", &first);
    let mut last = first.clone();
    for seq in 2..=PERSISTENT_MAX as u64 + 1 {
        last = signed(&alice, "alice", seq, MessageType::RescueRequest);
        deliver(&mut node, &inbox, "honest", &last);
    }
    assert_eq!(node.persistent_messages.len(), PERSISTENT_MAX);
    assert!(!node.persistent_messages.contains_key(&message_key(&first)));
    assert!(node.persistent_messages.contains_key(&message_key(&last)));

    // One message each from many origins: the recent store is limited over all origins.
    let mut newest = String::new();
    for i in 0..total {
        let origin = Identity::new(&format!("o{i}"));
        let wire = signed(&origin, "", 1, MessageType::Normal);
        deliver(&mut node, &inbox, "honest", &wire);
        newest = origin.id;
    }
    let kept: usize = node.recent_messages.values().map(|m| m.len()).sum();
    assert!(kept <= RECENT_MAX_MESSAGES, "{kept} recent messages");
    assert!(node.recent_messages.contains_key(&newest));
    assert!(node.observations.len() <= OBSERVATIONS_MAX);
}

#[test]
fn the_divergence_log_keeps_its_limit() {
    let (mut node, inbox) = receiver();
    let liar = Identity::new("liar");
    for seq in 1..=DIVERGENCE_LOG_MAX as u64 + 5 {
        let kind = MessageType::ChainHeadAnnouncement {
            head_hash: [seq as u8; 32],
            head_seq: 0,
        };
        let wire = signed(&liar, "liar", seq, kind);
        deliver(&mut node, &inbox, "liar", &wire);
    }
    assert_eq!(node.divergence_log.len(), DIVERGENCE_LOG_MAX);
}

#[test]
fn a_flood_of_new_senders_is_bounded_and_keeps_the_bans() {
    let mut guard = SybilGuard::new(0);
    guard.use_manual_clock();
    let banned = "banned-peer".to_string();
    for _ in 0..3 {
        guard.report_violation(&banned);
    }
    assert!(guard.is_banned(&banned));

    for i in 0..4 * MAX_TRACKED_PEERS {
        guard.advance_time(Duration::from_millis(1));
        let _ = guard.check(&format!("peer-{i}"));
    }
    assert!(guard.tracked_peers() <= MAX_TRACKED_PEERS);
    assert!(
        guard.is_banned(&banned),
        "a ban survives a flood of identities"
    );
}

#[test]
fn rescues_from_new_identities_cannot_grow_the_ttl_store() {
    let mut pipeline = GossipPipeline::new();
    for i in 0..3000 {
        pipeline.register_bootstrap("zone", &format!("o{i}"));
        let verdict = pipeline.process(&PipelineMessage {
            id: format!("m{i}"),
            origin_node: format!("o{i}"),
            zone: "zone".to_string(),
            severity: 3,
            kind: MessageKind::Rescue,
            payload_bytes: 0,
            reputation: 0.5,
            round: 0,
            seq: 1,
        });
        assert_eq!(verdict, PipelineVerdict::Accept);
    }
    assert!(pipeline.ttl_store.active_count() <= 200);
    assert!(pipeline.ttl_store.messages.contains_key("m2999"));
    assert!(pipeline.rescue_ack_counts.len() <= 1024);
    assert!(pipeline.peer_severities.len() <= 1024);
    assert!(pipeline.zone_registry.member_count("zone") <= 1024);
    assert!(pipeline.aeda.decisions().len() <= 1025);
}

#[tokio::test]
async fn relay_targets_chosen_by_peers_are_bounded() {
    let tcp = TcpTransport::bind(
        "127.0.0.1:0".parse().unwrap(),
        Identity::new("n").signing_key,
    )
    .await
    .unwrap();
    let alice = Identity::new("alice");
    let wire = signed(&alice, "alice", 1, MessageType::Normal);
    let busy = "127.0.0.1:1";
    for i in 0..MAX_RELAY_TARGETS + 40 {
        tcp.relay(busy, wire.clone()).unwrap();
        tcp.relay(&format!("127.0.0.1:{}", 2 + i), wire.clone())
            .unwrap();
    }
    assert!(tcp.relay_targets() <= MAX_RELAY_TARGETS);
}

#[test]
fn a_chain_keeps_only_its_newest_blocks_and_still_verifies() {
    let mut chain = MerkleChain::new();
    let snapshot = |n| StateSnapshot {
        peer_count: 1,
        reputation_floor: 0.5,
        messages_handled: n,
        byzantine_events: 0,
        timestamp_ms: n,
    };
    let old_head = {
        chain.append(snapshot(0));
        (chain.head_hash(), chain.head_sequence())
    };
    let wrong = [7u8; 32];
    assert!(chain.diverges_from(wrong, old_head.1));

    for n in 1..5 * MERKLE_KEEP_BLOCKS as u64 {
        chain.append(snapshot(n));
    }
    assert!(chain.len() < 2 * MERKLE_KEEP_BLOCKS);
    assert!(chain.verify_integrity());
    assert_eq!(chain.head_sequence(), 5 * MERKLE_KEEP_BLOCKS as u64);
    // A head older than the blocks kept cannot be compared: no alert either way.
    assert!(!chain.diverges_from(wrong, old_head.1));
    assert!(chain.diverges_from(wrong, chain.head_sequence()));
    assert!(!chain.diverges_from(chain.head_hash(), chain.head_sequence()));
}

#[test]
fn per_message_lines_are_bounded_per_node_and_a_light_sender_keeps_every_line() {
    use crci_core::runtime::{FairLogGate, FAIR_LOG_WINDOW};
    let now = std::time::Instant::now();

    // Below the soft limit nothing is held back.
    let mut gate = FairLogGate::new(1000, 20);
    for i in 0..999 {
        assert_eq!(gate.pass(&format!("s{}", i % 5), now), Some(String::new()));
    }

    // One window: 90 senders at 200 lines each, plus an honest sender with a burst of 15.
    let mut gate = FairLogGate::new(1000, 20);
    let mut printed = 0;
    let mut honest_printed = 0;
    for round in 0..200 {
        for s in 0..90 {
            printed += gate.pass(&format!("flood-{s}"), now).is_some() as u32;
        }
        if (150..165).contains(&round) {
            honest_printed += gate.pass("honest", now).is_some() as u32;
        }
    }
    assert_eq!(
        honest_printed, 15,
        "a sender under its share never loses a line"
    );
    assert!(printed <= 2 * 1000 + 20 * 91, "{printed} lines printed");
    assert!(printed < 90 * 200 / 4, "{printed} lines printed");

    // The next window, a held-back sender's line says how many were not printed.
    let later = now + FAIR_LOG_WINDOW;
    let line = gate.pass("flood-0", later).unwrap_or_default();
    assert!(
        line.starts_with(" (+") && line.ends_with(" not printed)"),
        "{line}"
    );
}

/// A signer chooses its message IDs, so an adversary origin can sign a second message under the
/// ID of its first with a new sequence number. A relay whose seen cache has been cleared since
/// the first accepts the second as new and forwards it; a neighbour whose cache still holds the
/// ID must not take that for a replay by the relay: the relay never sent the same message twice.
#[test]
fn a_reused_message_id_with_a_new_sequence_number_does_not_strike_the_relay() {
    let (mut relay, relay_inbox) = receiver();
    let (mut neighbour, neighbour_inbox) = receiver();
    let byz = Identity::new("byz");
    let reused = |seq: u64| {
        let mut msg = Message::new(
            "x",
            "byz",
            Signal::new(3, true, false, true, 3, Visibility::Direct),
            None,
        );
        msg.seq = seq;
        WireMessage::from_message(&msg, &byz)
    };
    let (first, second) = (reused(1), reused(2));

    deliver(&mut relay, &relay_inbox, "byz", &first);
    deliver(&mut neighbour, &neighbour_inbox, "relay", &first);

    relay.seen_messages.clear();
    let accepted = relay.messages_handled;
    deliver(&mut relay, &relay_inbox, "byz", &second);
    assert_eq!(relay.messages_handled, accepted + 1, "new to the relay");

    let strikes = neighbour.byzantine_events;
    deliver(&mut neighbour, &neighbour_inbox, "relay", &second);
    assert_eq!(
        neighbour.byzantine_events, strikes,
        "the relay is not struck"
    );
}
