use crci_core::discovery::{DiscoveryMethod, PeerRecord, PeerTable};
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::{PeerReputation, SybilGuard};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn test_bft_subnet_limit() {
    let mut table = PeerTable::new();

    let p1 = PeerRecord {
        node_id: "192.168.1.1:8080".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };
    let p2 = PeerRecord {
        node_id: "192.168.1.2:8080".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };
    let p3 = PeerRecord {
        node_id: "192.168.1.3:8080".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };

    assert!(table.upsert(p1));
    assert!(table.upsert(p2));
    assert!(table.upsert(p3));

    let p4 = PeerRecord {
        node_id: "192.168.1.4:8080".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };
    assert!(!table.upsert(p4));
}

#[test]
fn test_bft_sub_linear_recovery() {
    let mut rep = PeerReputation::new();
    rep.penalise(0.5);
    let original = rep.score;

    std::thread::sleep(Duration::from_millis(100));
    rep.decay_recover();

    assert!(rep.score > original);
    assert!(rep.score <= 1.0);
}

#[test]
fn test_bft_pruning_banned_records() {
    let mut guard = SybilGuard::new(4);

    for i in 0..1005 {
        let peer = format!("banned_node_{}", i);
        for _ in 0..7 {
            guard.report_violation(&peer);
        }
    }

    let new_peer = "healthy_node".to_string();
    let result = guard.check(&new_peer);
    assert!(result.is_ok());
    assert!(guard.reputation(&new_peer) >= 0.5);
}

#[test]
fn test_bft_signature_verification_rate_limit() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("my-node-id", "zone-alpha", inbox.clone());

    let mut raw_messages = Vec::new();
    for i in 1..=10 {
        let mut msg = Message::new(
            &format!("msg-id-{}", i),
            "peer-attacker",
            Signal::new(3, true, false, true, 3, Visibility::Direct),
            None,
        );
        msg.seq = i; // Incremental sequence numbers to bypass replay check
        let wire = WireMessage::from_message(&msg, &node.identity);
        let payload = serde_json::to_vec(&wire).unwrap();
        raw_messages.push(payload);
    }

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("my-node-id".to_string(), raw_messages);
    }

    node.process_inbox();

    // We expect at least 5 messages to trigger the BFT-008 limit and increment byzantine events.
    assert!(node.byzantine_events >= 5);
}

#[test]
fn test_bft_reputation_bounds_clamping() {
    // BFT-011: Verify reputation is strictly clamped between 0.0 and 1.0
    let mut rep = PeerReputation::new();

    // Penalize repeatedly to force score below 0.0
    for _ in 0..10 {
        rep.penalise(0.2);
    }
    assert_eq!(rep.score, 0.0);

    rep.score = 0.99;
    std::thread::sleep(Duration::from_millis(50));
    rep.decay_recover();
    assert!(rep.score <= 1.0);
}

#[test]
fn test_bft_reputation_fixed_point_rounding() {
    // BFT-016: Verify reputation math uses 3 decimal places fixed-precision rounding
    let mut rep = PeerReputation::new(); // Starts at 0.5

    rep.penalise(0.12345);
    // 0.5 - 0.12345 = 0.37655. Rounded to 3 dec places is 0.377.
    assert_eq!(rep.score, 0.377);
}

#[test]
fn test_bft_signature_invalidation_spoof_defense() {
    // BFT-019: Defend against signature-invalidation attacks aimed at burning a target peer's reputation
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("my-node-id", "zone-alpha", inbox.clone());

    // Attacker spoofing Victim with invalid signature
    let msg = Message::new(
        "msg-id-spoof",
        "victim-node",
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    let mut wire = WireMessage::from_message(&msg, &node.identity);
    wire.signature_bytes = vec![0; 64]; // Invalid signature
    let payload = serde_json::to_vec(&wire).unwrap();

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert("my-node-id".to_string(), vec![payload]);
    }

    node.process_inbox();

    // Victim reputation must NOT be penalized (BFT-019)
    let victim_rep = node.sybil_guard.reputation(&"victim-node".to_string());
    assert!(victim_rep >= 0.5); // Default starting reputation is 0.5, must not be penalized
}

#[test]
fn test_bft_reputation_persistence_clamping() {
    // BFT-020: Verify loaded own reputation is clamped to [0.0, 1.0]
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("my-node-id-persist", "zone-alpha", inbox.clone());

    // Save state with a corrupted high reputation
    let rescues = vec![];
    let state = crci_core::storage::PersistedState {
        node_id: node.id.clone(),
        zone: node.zone.clone(),
        reputation: 99.9, // Corrupted out-of-bounds reputation
        peers: vec![],
        rescue_messages: rescues,
        mce_zones: vec![],
    };
    node.storage.save(&state).unwrap();

    // Load state
    node.load_state();

    // Reputation must be clamped to 1.0 (BFT-011) and rounded
    assert_eq!(node.reputation, 1.0);

    // Clean up state file
    node.storage.delete();
}
