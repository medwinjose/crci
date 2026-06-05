use crci::merkle::StateSnapshot;
use crci::message::{Message, MessageType, Signal, Visibility};
use crci::runtime::NodeRuntime;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn create_test_inbox() -> crci::transport::SharedInbox {
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
        message_type: MessageType::ChainHeadAnnouncement { head_hash, head_seq },
        origin_active: true,
        signature: None,
        created_at: crci::message::now_ts(),
        ttl_seconds: 30,
        priority: crci::message::MessagePriority::Normal,
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
        message_type: MessageType::ChainHeadAnnouncement { head_hash, head_seq },
        origin_active: true,
        signature: None,
        created_at: crci::message::now_ts(),
        ttl_seconds: 30,
        priority: crci::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };
    
    // Simulate receiving it
    let wire = crci::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();
    
    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox.entry(node.id.clone()).or_default().push(bytes);
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
        message_type: MessageType::ChainHeadAnnouncement { head_hash, head_seq: head_seq + 5 },
        origin_active: true,
        signature: None,
        created_at: crci::message::now_ts(),
        ttl_seconds: 30,
        priority: crci::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };
    
    let wire = crci::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();
    
    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox.entry(node.id.clone()).or_default().push(bytes);
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
        message_type: MessageType::ChainHeadAnnouncement { head_hash, head_seq },
        origin_active: true,
        signature: None,
        created_at: crci::message::now_ts(),
        ttl_seconds: 30,
        priority: crci::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };
    
    // We use a different identity for the peer to pass signature verify if needed, 
    // but MCE bypasses it, and for normal messages we sign it with node-a's identity 
    // which fails pubkey check if origin != node-a unless known_keys matches.
    // Wait, WireMessage::from_message uses node.identity, so pubkey matches.
    // Wait, if origin is "peer-b", verify_signature checks "peer-b".
    // Oh, but we used node.identity to sign it, so the pubkey inside is node.identity.
    // So the origin is "peer-b", pubkey is node.identity.
    // It will pass verify_signature because verify_signature checks payload using origin_pubkey.
    // And it will pass pubkey consistency if known_keys is empty for "peer-b".
    
    let wire = crci::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();
    
    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox.entry(node.id.clone()).or_default().push(bytes);
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
        message_type: MessageType::ChainHeadAnnouncement { head_hash: bad_genesis, head_seq: 0 },
        origin_active: true,
        signature: None,
        created_at: crci::message::now_ts(),
        ttl_seconds: 30,
        priority: crci::message::MessagePriority::Normal,
        hop_count: 0,
        seq: 1,
    };
    
    let wire = crci::runtime::WireMessage::from_message(&msg, &node.identity);
    let bytes = serde_json::to_vec(&wire).unwrap();
    
    {
        let mut inbox = node.inbox.lock().unwrap();
        inbox.entry(node.id.clone()).or_default().push(bytes);
    }
    
    let events_before = node.byzantine_events;
    node.process_inbox();
    
    assert_eq!(node.divergence_log.len(), 1);
    assert_eq!(node.byzantine_events, events_before + 1);
    assert_eq!(node.divergence_log[0].divergence_seq, 0);
}
