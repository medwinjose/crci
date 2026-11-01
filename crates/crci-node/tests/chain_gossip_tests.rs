use crci_core::merkle::StateSnapshot;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::NodeRuntime;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn create_test_inbox() -> crci_core::transport::SharedInbox {
    Arc::new(Mutex::new(HashMap::new()))
}

fn create_node(id: &str) -> NodeRuntime {
    NodeRuntime::new(id, "zone-1", create_test_inbox())
}

#[test]
fn test_no_peers_safe_broadcast() {
    let mut node = create_node("node-a");
    // Advance internal state so it has a valid seq counter
    node.messages_handled = 50;
    node.process_inbox(); // Will trigger snapshot append

    // Manually trigger a chain head announcement (mocking the end of a process_inbox with messages)
    let (head_hash, head_seq) = node.chain_head();
    let msg = Message {
        id: "test-head-1".to_string(),
        origin: node.id.clone(),
        signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
        note: None,
        message_type: MessageType::ChainHeadAnnouncement {
            head_hash,
            head_seq,
        },
        origin_active: true,
        signature: None,
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 30,
        priority: crci_core::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 0,
    };
    node.originate(msg); // should not crash
    assert_eq!(node.divergence_log.len(), 0);
}

#[test]
fn test_matching_head_no_divergence() {
    let mut node = create_node("node-a");
    let (head_hash, head_seq) = node.chain_head();

    let msg = Message {
        id: "test-head-2".to_string(),
        origin: "peer-b".to_string(),
        signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
        note: None,
        message_type: MessageType::ChainHeadAnnouncement {
            head_hash,
            head_seq,
        },
        origin_active: true,
        signature: None,
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 30,
        priority: crci_core::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };

    // Simulate receiving it
    let wire = crci_core::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();

    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox
            .entry(node.id.clone())
            .or_default()
            .push(("test-sender".to_string(), bytes));
    }

    node.process_inbox();
    assert_eq!(node.divergence_log.len(), 0);
    assert_eq!(node.byzantine_events, 0);
}

#[test]
fn test_peer_ahead_no_divergence() {
    let mut node = create_node("node-a");
    let (mut head_hash, head_seq) = node.chain_head();
    head_hash[0] ^= 0xFF; // Change hash arbitrarily

    let msg = Message {
        id: "test-head-3".to_string(),
        origin: "peer-b".to_string(),
        signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
        note: None,
        message_type: MessageType::ChainHeadAnnouncement {
            head_hash,
            head_seq: head_seq + 5,
        },
        origin_active: true,
        signature: None,
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 30,
        priority: crci_core::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };

    let wire = crci_core::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();

    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox
            .entry(node.id.clone())
            .or_default()
            .push(("test-sender".to_string(), bytes));
    }

    node.process_inbox();
    assert_eq!(node.divergence_log.len(), 0);
    assert_eq!(node.byzantine_events, 0);
}

#[test]
fn test_mismatched_head_divergence_alert() {
    let mut node = create_node("node-a");
    let (mut head_hash, head_seq) = node.chain_head();
    head_hash[0] ^= 0xFF; // Mismatch the hash

    let msg = Message {
        id: "test-head-4".to_string(),
        origin: "peer-b".to_string(),
        signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
        note: None,
        message_type: MessageType::ChainHeadAnnouncement {
            head_hash,
            head_seq,
        },
        origin_active: true,
        signature: None,
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 30,
        priority: crci_core::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };

    // The message claims origin "peer-b" but is signed with node-a's identity. Signature
    // verification uses the embedded public key, and no key is pinned for "peer-b" yet,
    // so the message passes both the signature and the key-consistency checks.

    let wire = crci_core::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();

    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox
            .entry(node.id.clone())
            .or_default()
            .push(("test-sender".to_string(), bytes));
    }

    let events_before = node.byzantine_events;
    node.process_inbox();

    assert_eq!(node.divergence_log.len(), 1);
    assert_eq!(node.byzantine_events, events_before + 1);

    let alert = &node.divergence_log[0];
    assert_eq!(alert.peer_id, "peer-b");
    assert_eq!(alert.divergence_seq, head_seq);
}

#[test]
fn test_divergence_increments_byzantine_events() {
    let mut node = create_node("node-a");
    node.merkle.append(StateSnapshot {
        peer_count: 1,
        reputation_floor: 1.0,
        messages_handled: 50,
        byzantine_events: 0,
        timestamp_ms: 1000,
    });

    let (_our_hash, our_seq) = node.chain_head();
    assert_eq!(our_seq, 1);

    // Send a message with seq = 0 (which is the genesis block) but with a tampered hash
    let mut bad_genesis = [0u8; 32];
    bad_genesis[0] = 0x42; // arbitrary mismatch

    let msg = Message {
        id: "test-head-5".to_string(),
        origin: "peer-bad".to_string(),
        signal: Signal::new(1, false, false, false, 1, Visibility::Direct),
        note: None,
        message_type: MessageType::ChainHeadAnnouncement {
            head_hash: bad_genesis,
            head_seq: 0,
        },
        origin_active: true,
        signature: None,
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 30,
        priority: crci_core::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };

    let wire = crci_core::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();

    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox
            .entry(node.id.clone())
            .or_default()
            .push(("test-sender".to_string(), bytes));
    }

    let events_before = node.byzantine_events;
    node.process_inbox();

    assert_eq!(node.divergence_log.len(), 1);
    assert_eq!(node.byzantine_events, events_before + 1);
    assert_eq!(node.divergence_log[0].divergence_seq, 0);
}

/// A chain-head announcement does not trigger another one: two flooding neighbours do not
/// echo announcements back and forth (which used to exhaust their rate limits).
#[test]
fn chain_head_announcements_do_not_echo() {
    let inbox = create_test_inbox();
    let mut sim = crci_core::mesh::MeshSimulator::new();
    sim.add_node("a", "z", inbox.clone());
    sim.add_node("b", "z", inbox.clone());
    for node in sim.nodes.values_mut() {
        node.use_router = false;
    }
    sim.connect("a", "b");
    sim.originate(
        "a",
        Message::new(
            "only",
            "a",
            Signal::new(3, false, false, true, 3, Visibility::Direct),
            None,
        ),
    );
    sim.drain(); // ten passes

    // a: its message + b's announcement processed without answering; b: one announcement.
    assert!(
        sim.nodes["a"].seq_counter <= 2,
        "a announced {} times",
        sim.nodes["a"].seq_counter - 1
    );
    assert!(
        sim.nodes["b"].seq_counter <= 1,
        "b announced {} times",
        sim.nodes["b"].seq_counter
    );
    assert_eq!(sim.nodes["a"].sybil_rate_limited_events, 0);
    assert_eq!(sim.nodes["b"].sybil_rate_limited_events, 0);
}
