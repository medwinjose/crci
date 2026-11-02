//! Partition heal with anti-entropy (digest exchange).
//!
//! After a partition, each side asks the other for what it missed with a signed digest.
//! Answers never strike an honest node, including an answer that crosses with the same
//! message relayed as ordinary gossip; unsolicited duplicates still strike.

use crci_core::identity::Identity;
use crci_core::mesh::MeshSimulator;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage, SYNC_MAX_BATCH, SYNC_REQUEST_TYPE};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn msg(id: &str, origin: &str) -> Message {
    Message::new(
        id,
        origin,
        Signal::new(3, false, false, true, 3, Visibility::Direct),
        None,
    )
}

/// Has `node` received (or authored) message `id`?
fn holds(node: &NodeRuntime, id: &str) -> bool {
    node.seen_messages.contains_key(id)
        || node
            .recent_messages
            .values()
            .any(|m| m.values().any(|w| w.id == id))
}

fn assert_no_strikes(sim: &mut MeshSimulator) {
    let ids: Vec<String> = sim.nodes.keys().cloned().collect();
    for id in &ids {
        let node = sim.nodes.get_mut(id).unwrap();
        assert_eq!(node.byzantine_events, 0, "{id} recorded a violation");
        for peer in &ids {
            if peer != id {
                let rep = node.sybil_guard.reputation(peer);
                assert!(rep >= 0.5, "{id} struck {peer} (reputation {rep})");
            }
        }
    }
}

/// Two groups {a1, a2} and {b1, b2}, bridged by a2–b1. The bridge fails, both groups keep
/// sending, the bridge comes back and the two ends exchange digests. Every node ends up
/// with every message and no honest node is struck.
#[test]
fn partitioned_groups_converge_after_heal_without_strikes() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();
    for (id, zone) in [("a1", "za"), ("a2", "za"), ("b1", "zb"), ("b2", "zb")] {
        sim.add_node(id, zone, inbox.clone());
    }
    // Flood over links, as the TCP node does: the temporal router picks next hops among
    // every origin it has seen, so it would bypass a cut link.
    for node in sim.nodes.values_mut() {
        node.use_router = false;
    }
    sim.connect("a1", "a2");
    sim.connect("b1", "b2");
    sim.connect("a2", "b1");

    sim.originate("a1", msg("before-a1", "a1"));
    sim.originate("b2", msg("before-b2", "b2"));
    sim.drain();

    sim.disconnect("a2", "b1");
    let mut during = Vec::new();
    for i in 0..3 {
        for origin in ["a1", "a2", "b1", "b2"] {
            let id = format!("during-{origin}-{i}");
            sim.originate(origin, msg(&id, origin));
            during.push(id);
        }
        sim.drain();
    }
    // The partition is real: nothing crossed it.
    assert!(!holds(&sim.nodes["b2"], "during-a1-0"));
    assert!(!holds(&sim.nodes["a1"], "during-b2-0"));

    sim.heal("a2", "b1");
    sim.drain();

    for node_id in ["a1", "a2", "b1", "b2"] {
        for id in during
            .iter()
            .chain(["before-a1".to_string(), "before-b2".to_string()].iter())
        {
            assert!(holds(&sim.nodes[node_id], id), "{node_id} is missing {id}");
        }
    }
    assert_no_strikes(&mut sim);
}

/// Without the digest exchange the same heal leaves both sides without the messages sent
/// during the partition: the exchange is what delivers them.
#[test]
fn heal_without_anti_entropy_leaves_messages_undelivered() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();
    sim.add_node("a", "za", inbox.clone());
    sim.add_node("b", "zb", inbox.clone());
    for node in sim.nodes.values_mut() {
        node.use_router = false;
    }
    sim.connect("a", "b");
    sim.disconnect("a", "b");
    sim.originate("a", msg("during-a", "a"));
    sim.drain();
    sim.connect("a", "b"); // link back, no digest exchange
    sim.drain();
    assert!(!holds(&sim.nodes["b"], "during-a"));
}

// ── Two-node harness with explicit control over what each side receives ───────────

struct Pair {
    a: NodeRuntime,
    b: NodeRuntime,
    inbox: SharedInbox,
}

impl Pair {
    fn new() -> Pair {
        let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut a = NodeRuntime::new("a", "z", inbox.clone());
        let mut b = NodeRuntime::new("b", "z", inbox.clone());
        a.sybil_guard.use_manual_clock();
        b.sybil_guard.use_manual_clock();
        Pair { a, b, inbox }
    }

    /// Put raw `wire` in `to`'s inbox as sent by `from`.
    fn inject(&self, from: &str, to: &str, wire: &WireMessage) {
        self.inbox
            .lock()
            .unwrap()
            .entry(to.to_string())
            .or_default()
            .push((from.to_string(), serde_json::to_vec(wire).unwrap()));
    }

    /// Messages waiting for `to`, removed from the inbox.
    fn take(&self, to: &str) -> Vec<(String, WireMessage)> {
        self.inbox
            .lock()
            .unwrap()
            .remove(to)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(from, bytes)| {
                serde_json::from_slice::<WireMessage>(&bytes)
                    .ok()
                    .map(|w| (from, w))
            })
            .collect()
    }

    fn requeue(&self, to: &str, items: &[(String, WireMessage)]) {
        for (from, wire) in items {
            self.inject(from, to, wire);
        }
    }
}

/// `a` authors messages 1..=n while `b` is cut off; returns them as signed by `a`.
fn authored_by_a(pair: &mut Pair, n: u64) -> Vec<WireMessage> {
    for i in 1..=n {
        pair.a.originate(msg(&format!("a-{i}"), "a"));
    }
    pair.a
        .recent_messages
        .get("a")
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default()
}

/// The heal race: `a` relays message 1 to `b` as ordinary gossip while `b`'s sync request
/// (which did not list message 1) is on its way; `a`'s answer then repeats message 1.
/// The repeat is excused once and nothing is struck.
#[test]
fn answer_crossing_with_gossip_is_not_struck() {
    let mut pair = Pair::new();
    let sent = authored_by_a(&mut pair, 3);

    pair.b.request_sync("a"); // digest lists nothing from a
    pair.inject("a", "b", &sent[0]); // gossip crosses the request
    pair.b.process_inbox();
    pair.a.process_inbox(); // answers with messages 1..3
    let answer: Vec<_> = pair
        .take("b")
        .into_iter()
        .filter(|(_, w)| w.message_type != SYNC_REQUEST_TYPE && w.origin == "a")
        .collect();
    assert_eq!(answer.len(), 3, "the answer carries every message b lacked");
    pair.requeue("b", &answer);
    pair.b.process_inbox();

    assert_eq!(pair.b.byzantine_events, 0);
    assert_eq!(pair.b.sync_excused_duplicates, 1);
    assert!(pair.b.sybil_guard.reputation(&"a".to_string()) >= 0.5);
    for i in 1..=3 {
        assert!(holds(&pair.b, &format!("a-{i}")));
    }
}

/// Unsolicited duplicates during a heal still strike: a repeat of a message the digest
/// listed as held, a second repeat of an excused message, and a repeat from a peer that
/// was not asked.
#[test]
fn unsolicited_duplicates_during_heal_still_strike() {
    let mut pair = Pair::new();
    let sent = authored_by_a(&mut pair, 2);

    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox(); // b holds message 1 from a
    pair.b.request_sync("a"); // digest lists message 1

    // Message 1 was listed as held: repeating it is a replay.
    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox();
    assert_eq!(pair.b.byzantine_events, 1);

    // Message 2: first copy accepted, the crossing repeat excused once, the next struck.
    for _ in 0..3 {
        pair.inject("a", "b", &sent[1]);
        pair.b.process_inbox();
    }
    assert_eq!(pair.b.sync_excused_duplicates, 1);
    assert_eq!(pair.b.byzantine_events, 2);

    // A peer that was not asked gets no excuse.
    pair.inject("c", "b", &sent[1]);
    pair.b.process_inbox();
    pair.inject("c", "b", &sent[1]);
    pair.b.process_inbox();
    assert_eq!(pair.b.byzantine_events, 3);
}

/// The excuse ends with the session.
#[test]
fn excuse_expires_with_the_session() {
    let mut pair = Pair::new();
    let sent = authored_by_a(&mut pair, 1);
    pair.b.request_sync("a");
    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox();
    pair.b.sybil_guard.advance_time(Duration::from_secs(31));
    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox();
    assert_eq!(pair.b.sync_excused_duplicates, 0);
    assert_eq!(pair.b.byzantine_events, 1);
}

/// A session excuses at most one answer batch: a peer cannot replay every message it relays
/// once for free while sessions are renewed by periodic sync requests.
#[test]
fn a_session_excuses_at_most_one_answer_batch() {
    let mut pair = Pair::new();
    let sent = authored_by_a(&mut pair, 20);
    pair.b.request_sync("a"); // digest lists nothing from a

    // Every message arrives twice from a, inside the session; the clock advances so the
    // token bucket never drops a frame.
    for wire in &sent {
        pair.inject("a", "b", wire);
        pair.inject("a", "b", wire);
        pair.b.process_inbox();
        pair.b.sybil_guard.advance_time(Duration::from_millis(400));
    }

    assert_eq!(pair.b.sync_excused_duplicates, SYNC_MAX_BATCH as u64);
    assert_eq!(
        pair.b.byzantine_events, 3,
        "repeats after the batch are strikes"
    );
    assert!(pair.b.sybil_guard.is_banned(&"a".to_string()));
}

/// The excuse covers only the time an answer needs to arrive, not the whole sync interval.
#[test]
fn excuse_window_is_shorter_than_the_sync_interval() {
    let mut pair = Pair::new();
    let sent = authored_by_a(&mut pair, 1);
    pair.b.request_sync("a");
    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox();
    pair.b.sybil_guard.advance_time(Duration::from_secs(11));
    pair.inject("a", "b", &sent[0]);
    pair.b.process_inbox();
    assert_eq!(pair.b.sync_excused_duplicates, 0);
    assert_eq!(pair.b.byzantine_events, 1);
}

/// A request is answered only when it comes from its author; a copy relayed by another
/// peer is ignored, so a captured request cannot be used to flood its author.
#[test]
fn relayed_sync_request_is_not_answered() {
    let mut pair = Pair::new();
    authored_by_a(&mut pair, 3);
    let request = pair.b.build_sync_request(None);
    pair.inject("mallory", "a", &request);
    pair.a.process_inbox();
    assert!(pair.take("b").is_empty());
    assert!(pair.take("mallory").is_empty());
}

/// One answer per requester per cooldown, and at most `SYNC_MAX_BATCH` messages each.
#[test]
fn answers_are_rate_limited_and_batched() {
    let mut pair = Pair::new();
    authored_by_a(&mut pair, 40);
    pair.b.request_sync("a");
    pair.a.process_inbox();
    let first = pair.take("b").len();
    assert_eq!(first, crci_core::runtime::SYNC_MAX_BATCH);

    pair.b.request_sync("a"); // too soon
    pair.a.process_inbox();
    assert!(pair.take("b").is_empty());

    pair.a.sybil_guard.advance_time(Duration::from_secs(6));
    pair.b.request_sync("a");
    pair.a.process_inbox();
    assert_eq!(pair.take("b").len(), crci_core::runtime::SYNC_MAX_BATCH);
}

/// The digest is covered by the requester's signature: a relay cannot alter it.
#[test]
fn sync_digest_is_signed() {
    let mut pair = Pair::new();
    let mut request = pair.b.build_sync_request(Some("10.0.0.2:7000".to_string()));
    assert!(request.verify_signature());
    if let Some(digest) = request.sync.as_mut() {
        digest.reply_to = Some("10.0.0.66:7000".to_string());
    }
    assert!(!request.verify_signature());
    let _ = Identity::new("unused");
}

/// Answers are also limited in total, whoever asks: a requester can name any `reply_to`,
/// so many identities must not be able to make one node send without limit.
#[test]
fn answers_are_limited_across_requesters() {
    let mut pair = Pair::new();
    authored_by_a(&mut pair, 3);
    let mut answered = 0;
    for i in 0..10 {
        let id = format!("asker-{i}");
        let inbox = pair.inbox.clone();
        let mut asker = NodeRuntime::new(&id, "z", inbox);
        let request = asker.build_sync_request(None);
        pair.inject(&id, "a", &request);
        pair.a.process_inbox();
        if !pair.take(&id).is_empty() {
            answered += 1;
        }
    }
    assert_eq!(answered, crci_core::runtime::SYNC_ANSWER_BURST as usize);
}
