//! Metrics integration tests (Session 153, Step 10).
//!
//! Verifies that CrciMetrics counters actually increment when real events
//! occur in NodeRuntime — not just that they exist and are zero.

use crci_core::message::{Message, MessagePriority, MessageType, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Helper: create a signed WireMessage from a NodeRuntime identity.
fn make_wire(node: &NodeRuntime, msg_id: &str, seq: u64) -> Vec<u8> {
    let msg = Message {
        id: msg_id.to_string(),
        origin: "test-peer".to_string(),
        created_at: crci_core::message::now_ts(),
        ttl_seconds: 300,
        priority: MessagePriority::Normal,
        hop_count: 0,
        seq,
        signal: Signal::new(3, true, false, true, 3, Visibility::Direct),
        note: None,
        message_type: MessageType::Normal,
        origin_active: true,
        signature: None,
    };
    let wire = WireMessage::from_message(&msg, &node.identity);
    serde_json::to_vec(&wire).unwrap()
}

#[test]
fn test_metrics_messages_accepted_increments() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("metrics-test-node", "zone-alpha", inbox.clone());

    // Baseline: zero
    assert_eq!(
        node.metrics.messages_accepted.load(Ordering::Relaxed),
        0,
        "messages_accepted should start at 0"
    );

    // Send 3 valid messages
    let mut payloads = Vec::new();
    for i in 1..=3u64 {
        payloads.push(make_wire(&node, &format!("metrics-msg-{}", i), i));
    }

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("metrics-test-node".to_string(), payloads.into_iter().map(|p| ("test-sender".to_string(), p)).collect());
    }

    node.process_inbox();

    let accepted = node.metrics.messages_accepted.load(Ordering::Relaxed);
    assert!(
        accepted >= 3,
        "Expected messages_accepted >= 3 after processing 3 valid messages, got {}",
        accepted
    );
}

#[test]
fn test_metrics_byzantine_detected_on_bft008_throttle() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("bft008-metrics-node", "zone-alpha", inbox.clone());

    // Send 10 messages from the same peer — BFT-008 limit is 5,
    // so messages 6-10 should trigger byzantine_detected increments
    let mut payloads = Vec::new();
    for i in 1..=10u64 {
        let msg = Message {
            id: format!("bft008-msg-{}", i),
            origin: "attacker-peer".to_string(),
            created_at: crci_core::message::now_ts(),
            ttl_seconds: 300,
            priority: MessagePriority::Normal,
            hop_count: 0,
            seq: i,
            signal: Signal::new(3, true, false, true, 3, Visibility::Direct),
            note: None,
            message_type: MessageType::Normal,
            origin_active: true,
            signature: None,
        };
        let wire = WireMessage::from_message(&msg, &node.identity);
        payloads.push(serde_json::to_vec(&wire).unwrap());
    }

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("bft008-metrics-node".to_string(), payloads.into_iter().map(|p| ("test-sender".to_string(), p)).collect());
    }

    node.process_inbox();

    let byzantine = node.metrics.byzantine_detected.load(Ordering::Relaxed);
    assert!(
        byzantine >= 5,
        "Expected byzantine_detected >= 5 after BFT-008 throttle, got {}",
        byzantine
    );

    let sig_verifs = node
        .metrics
        .sig_verifications_performed
        .load(Ordering::Relaxed);
    assert!(
        sig_verifs >= 5,
        "Expected sig_verifications_performed >= 5 (the first 5 messages pass), got {}",
        sig_verifs
    );

    let rejected = node.metrics.messages_rejected.load(Ordering::Relaxed);
    assert!(
        rejected >= 5,
        "Expected messages_rejected >= 5 after BFT-008 throttle, got {}",
        rejected
    );
}

#[test]
fn test_metrics_seen_messages_evictions_on_bft046() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("bft046-metrics-node", "zone-alpha", inbox.clone());

    // Pre-fill seen_messages to just below threshold
    for i in 0..5000 {
        node.seen_messages.insert(format!("prefill-{}", i));
    }

    // Send one more message to trigger eviction
    let payload = make_wire(&node, "trigger-eviction", 1);
    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("bft046-metrics-node".to_string(), vec![("test-sender".to_string(), payload)]);
    }

    node.process_inbox();

    let evictions = node.metrics.seen_messages_evictions.load(Ordering::Relaxed);
    assert_eq!(
        evictions, 1,
        "Expected exactly 1 seen_messages_eviction after hitting 5000 ceiling, got {}",
        evictions
    );
}

#[test]
fn test_metrics_to_prometheus_output_well_formed() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("prom-test-node", "zone-alpha", inbox.clone());

    // Process some messages to get real counter values
    let mut payloads = Vec::new();
    for i in 1..=2u64 {
        payloads.push(make_wire(&node, &format!("prom-msg-{}", i), i));
    }
    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("prom-test-node".to_string(), payloads.into_iter().map(|p| ("test-sender".to_string(), p)).collect());
    }
    node.process_inbox();

    let output = node.metrics.to_prometheus();

    // Verify the output contains real values (not just zeros)
    assert!(
        output.contains("crci_messages_accepted"),
        "Prometheus output must contain crci_messages_accepted"
    );
    assert!(
        output.contains("crci_sig_verifications_performed"),
        "Prometheus output must contain crci_sig_verifications_performed"
    );
    assert!(
        output.contains("crci_seen_messages_evictions"),
        "Prometheus output must contain crci_seen_messages_evictions"
    );

    // Verify TYPE annotations are present
    assert!(
        output.contains("# TYPE crci_messages_accepted counter"),
        "Must have TYPE annotation for messages_accepted"
    );
    assert!(
        output.contains("# TYPE crci_sig_verifications_performed counter"),
        "Must have TYPE annotation for sig_verifications_performed"
    );
    assert!(
        output.contains("# TYPE crci_seen_messages_evictions counter"),
        "Must have TYPE annotation for seen_messages_evictions"
    );

    // Verify messages_accepted is > 0 (real increment, not stub)
    let accepted = node.metrics.messages_accepted.load(Ordering::Relaxed);
    assert!(
        accepted > 0,
        "After processing real messages, messages_accepted must be > 0, got {}",
        accepted
    );
    assert!(
        output.contains(&format!("crci_messages_accepted {}", accepted)),
        "Prometheus output must show the real messages_accepted count"
    );
}
