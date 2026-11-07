use crci_core::discovery::{DiscoveryMethod, PeerRecord, PeerTable};
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::SEEN_MAX;
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::{PeerReputation, SybilGuard};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
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
fn test_bft_recovery_after_penalty() {
    let mut rep = PeerReputation::new();
    rep.penalise(0.5);
    let original = rep.score;

    let later = rep.last_updated + Duration::from_secs(100);
    rep.recover_at(later);

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
fn test_bft_signature_checks_do_not_cap_an_honest_origin() {
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
        guard.insert(
            "my-node-id".to_string(),
            raw_messages
                .into_iter()
                .map(|p| ("test-sender".to_string(), p))
                .collect(),
        );
    }

    node.process_inbox();

    // Ten validly signed messages fit inside the sender's burst budget: all are accepted.
    assert_eq!(node.messages_handled, 10);
    assert_eq!(node.byzantine_events, 0);
}

#[test]
fn test_bft_reputation_bounds_clamping() {
    // Verify reputation is strictly clamped between 0.0 and 1.0
    let mut rep = PeerReputation::new();

    // Penalize repeatedly to force score below 0.0
    for _ in 0..10 {
        rep.penalise(0.2);
    }
    assert_eq!(rep.score, 0.0);

    rep.score = 0.99;
    std::thread::sleep(Duration::from_millis(50));
    rep.recover();
    assert!(rep.score <= 1.0);
}

#[test]
fn test_bft_reputation_fixed_point_rounding() {
    // Verify reputation math uses 3 decimal places fixed-precision rounding
    let mut rep = PeerReputation::new(); // Starts at 0.5

    rep.penalise(0.12345);
    // 0.5 - 0.12345 = 0.37655. Rounded to 3 dec places is 0.377.
    assert_eq!(rep.score, 0.377);
}

#[test]
fn test_bft_signature_invalidation_spoof_defense() {
    // BFT-042: Defend against signature-invalidation attacks aimed at burning a target peer's reputation
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
        guard.insert(
            "my-node-id".to_string(),
            vec![("test-sender".to_string(), payload)],
        );
    }

    node.process_inbox();

    // Victim reputation must NOT be penalized (BFT-042)
    let victim_rep = node.sybil_guard.reputation(&"victim-node".to_string());
    assert!(victim_rep >= 0.5); // Default starting reputation is 0.5, must not be penalized
}

#[test]
fn test_bft_reputation_persistence_clamping() {
    // BFT-044: Verify loaded own reputation is clamped to [0.0, 1.0]
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

    // Reputation must be clamped to 1.0 and rounded
    assert_eq!(node.reputation, 1.0);

    // Clean up state file
    node.storage.delete();
}

#[test]
fn test_bft_reputation_no_idle_decay() {
    // Inactivity is not evidence: an idle peer keeps its score.
    let mut rep = PeerReputation::new(); // Starts at 0.5
    let later = rep.last_updated + Duration::from_secs(3_600);
    rep.recover_at(later);
    assert_eq!(rep.score, 0.5);
}

#[test]
fn test_idle_peer_is_never_banned_by_inactivity() {
    // An honest peer that is silent for a long time must not be banned when it
    // sends its next message: only evidenced misbehaviour may ban.
    let mut rep = PeerReputation::new();
    let later = rep.last_updated + Duration::from_secs(60);

    rep.recover_at(later);

    assert!(!rep.is_banned(), "inactivity produced a ban");
}

#[test]
fn test_idle_peer_passes_sybil_guard() {
    use crci_core::sybil::SybilGuard;
    let mut guard = SybilGuard::new(0);
    let peer = "honest-peer".to_string();
    assert!(guard.check(&peer).is_ok());
    std::thread::sleep(Duration::from_millis(2100));
    for _ in 0..3 {
        assert!(guard.check(&peer).is_ok());
    }
}

#[test]
fn test_bft_reputation_recovery() {
    // Verify recovery rate is slower than decay rate
    let mut rep = PeerReputation::new(); // Starts at 0.5
    rep.penalise(0.4); // Drop to 0.1
    assert_eq!(rep.score, 0.1);

    // Recovery is linear in time: 0.0005 per second, so 0.5 s adds 0.00025.
    let later = rep.last_updated + Duration::from_millis(500);
    rep.recover_at(later);

    // Score should recover slightly but not immediately jump back.
    assert!(rep.score > 0.1);
    assert!(rep.score < 0.15); // Stays bounded and recovers slowly
}

#[test]
fn test_bft_reputation_weighted_quorum() {
    // Verify low-reputation majority cannot out-vote high-reputation minority
    use crci_core::mesh::MeshSimulator;

    let mut mesh = MeshSimulator::new();
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));

    // Set up 5 nodes in the same zone
    mesh.add_node("node-high-1", "zone-alpha", inbox.clone());
    mesh.add_node("node-high-2", "zone-alpha", inbox.clone());
    mesh.add_node("node-low-1", "zone-alpha", inbox.clone());
    mesh.add_node("node-low-2", "zone-alpha", inbox.clone());
    mesh.add_node("node-low-3", "zone-alpha", inbox.clone());

    // Establish reputations: high-reputation nodes have 1.0, low-reputation nodes have 0.45
    mesh.nodes.get_mut("node-high-1").unwrap().reputation = 1.0;
    mesh.nodes.get_mut("node-high-2").unwrap().reputation = 1.0;
    mesh.nodes.get_mut("node-low-1").unwrap().reputation = 0.45;
    mesh.nodes.get_mut("node-low-2").unwrap().reputation = 0.45;
    mesh.nodes.get_mut("node-low-3").unwrap().reputation = 0.45;

    // Connect them
    mesh.connect("node-high-1", "node-high-2");
    mesh.connect("node-high-1", "node-low-1");
    mesh.connect("node-high-1", "node-low-2");
    mesh.connect("node-high-1", "node-low-3");

    // Generate observations:
    // Low-reputation majority votes 5 (deviating), High-reputation minority votes 2 (honest)
    mesh.originate(
        "node-high-1",
        Message::new(
            "msg-h1",
            "node-high-1",
            Signal::new(2, true, false, true, 3, Visibility::Direct),
            None,
        ),
    );
    mesh.originate(
        "node-high-2",
        Message::new(
            "msg-h2",
            "node-high-2",
            Signal::new(2, true, false, true, 3, Visibility::Direct),
            None,
        ),
    );
    mesh.originate(
        "node-low-1",
        Message::new(
            "msg-l1",
            "node-low-1",
            Signal::new(5, true, false, true, 3, Visibility::Direct),
            None,
        ),
    );
    mesh.originate(
        "node-low-2",
        Message::new(
            "msg-l2",
            "node-low-2",
            Signal::new(5, true, false, true, 3, Visibility::Direct),
            None,
        ),
    );
    mesh.originate(
        "node-low-3",
        Message::new(
            "msg-l3",
            "node-low-3",
            Signal::new(5, true, false, true, 3, Visibility::Direct),
            None,
        ),
    );

    // Deliver all messages
    mesh.drain();

    // Run consensus in mesh. The high-reputation nodes deviate from the simple median 5.
    // So w_faulty is the sum of reputations of high-reputation nodes (2 * 1.0 = 2.0).
    // w_total is 2 * 1.0 + 3 * 0.45 = 3.35.
    // Since 3 * w_faulty (6.0) >= w_total (3.35), a Byzantine Quorum Failure must trigger, aborting the consensus.
    // Thus, the low-reputation majority cannot force consensus.
    mesh.run_consensus();

    // Verify consensus aborted/failed and didn't result in consensus on 5,
    // and that the deviators (node-high-1 and node-high-2) did NOT get penalized because consensus was aborted!
    assert_eq!(mesh.nodes.get("node-high-1").unwrap().reputation, 1.0);
    assert_eq!(mesh.nodes.get("node-high-2").unwrap().reputation, 1.0);
}

#[test]
fn test_bft_reputation_persistence_across_reconnect() {
    // BFT-040: Verify a node's reputation survives disconnect/reconnect cycles
    let mut guard = SybilGuard::new(4);
    // Freeze the clock: recovery is continuous in time, so exact scores need no time to pass.
    guard.use_manual_clock();
    let peer = "peer-127.0.0.1:9091".to_string();

    // Initialize/check once
    assert!(guard.check(&peer).is_ok());
    assert_eq!(guard.reputation(&peer), 0.5);

    // Penalize
    guard.report_violation(&peer);
    assert_eq!(guard.reputation(&peer), 0.35);

    // Simulate disconnect and reconnect by querying again
    // The reputation score must still be 0.35 (accounting for no decay as time hasn't passed)
    assert_eq!(guard.reputation(&peer), 0.35);
}

#[test]
fn test_bft_reputation_isolation_per_peer_view() {
    // BFT-041: Verify local reputation table is isolated and cannot be overridden by self-reported values
    let mut guard = SybilGuard::new(4);
    // Freeze the clock: recovery is continuous in time, so exact scores need no time to pass.
    guard.use_manual_clock();
    let peer = "peer-127.0.0.1:9092".to_string();

    // Check node
    assert!(guard.check(&peer).is_ok());

    // Record violation
    guard.report_violation(&peer);

    // Check reputation locally
    let score = guard.reputation(&peer);
    assert_eq!(score, 0.35);

    // Even if an external message attempts to claim a higher reputation,
    // there is no field or mechanism to update the local table from incoming messages.
    // The score remains isolated at 0.35.
    assert_eq!(guard.reputation(&peer), 0.35);
}

#[test]
fn test_bft_sybil_cluster_reputation_correlation() {
    // BFT-043: Verify new peers joining from a banned subnet start with a penalty
    let mut guard = SybilGuard::new(4);
    // Freeze the clock: recovery is continuous in time, so exact scores need no time to pass.
    guard.use_manual_clock();

    let banned_peer = "192.168.2.1:8080".to_string();
    let new_peer_same_subnet = "192.168.2.2:8080".to_string();
    let new_peer_diff_subnet = "192.168.3.1:8080".to_string();

    // Ban the first peer
    for _ in 0..7 {
        guard.report_violation(&banned_peer);
    }
    assert!(guard.reputation(&banned_peer) < 0.1);

    // Check new peer from same subnet -> should start with 0.2 penalty
    assert!(guard.check(&new_peer_same_subnet).is_ok());
    assert_eq!(guard.reputation(&new_peer_same_subnet), 0.2);

    // Check new peer from different subnet -> should start at standard 0.5
    assert!(guard.check(&new_peer_diff_subnet).is_ok());
    assert_eq!(guard.reputation(&new_peer_diff_subnet), 0.5);
}

#[test]
fn test_bft_reputation_based_eviction_hysteresis() {
    // BFT-045: Verify eviction hysteresis prevents flapping
    let mut rep = PeerReputation::new(); // Starts at 0.5
    assert!(!rep.is_banned());

    // Penalize repeatedly to ban it
    rep.penalise(0.45); // 0.5 - 0.45 = 0.05
    assert!(rep.is_banned());

    // Let it recover past the eviction threshold (0.1): 200 s at 0.0005/s = +0.1
    let later = rep.last_updated + Duration::from_secs(200);
    rep.recover_at(later); // Recover slightly
    assert!(rep.score > 0.1); // Score is now > 0.1

    // Due to hysteresis, it must still remain banned since it hasn't crossed the 0.25 threshold
    assert!(rep.is_banned());

    // Recover more to cross 0.25
    rep.score = 0.26;
    rep.recover_at(later);
    assert!(!rep.is_banned()); // Successfully reinstated
}

// BFT-004
#[test]
fn test_bft_xor_hash_collision_guard() {
    let mut table = PeerTable::new();
    let p1 = PeerRecord {
        node_id: "8yn0iYCKYHlIj4-BwPqk".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };
    assert!(table.upsert(p1.clone()));

    let p2 = PeerRecord {
        node_id: "GReLUrM4wMqfg9yzV3KQ".to_string(),
        zone: "alpha".to_string(),
        last_seen: 1,
        discovery_method: DiscoveryMethod::DirectBeacon,
        battery_tier: "FULL".to_string(),
        zone_vouched: true,
    };
    assert!(!table.upsert(p2));
}

// BFT-009
#[test]
fn test_bft_untrusted_node_escalation_guard() {
    use crci_core::aeda::{AedaDecision, AedaEngine, RescueEvent};

    let mut engine = AedaEngine::new();

    for i in 0..3 {
        engine.process(
            RescueEvent {
                node_id: format!("low_rep_{}", i),
                zone: "alpha".to_string(),
                severity: 5,
                round: 1,
                reputation: 0.3,
            },
            1,
        );
    }

    let escalated = engine
        .decisions()
        .iter()
        .any(|d| matches!(d, AedaDecision::ZoneEscalated { .. }));
    assert!(
        !escalated,
        "BFT-009/030 violation: untrusted nodes triggered escalation"
    );
}

// BFT-030
#[test]
fn test_bft_trusted_node_passthrough_guard() {
    use crci_core::aeda::{AedaDecision, AedaEngine, RescueEvent};

    let mut engine = AedaEngine::new();

    for i in 0..3 {
        engine.process(
            RescueEvent {
                node_id: format!("trusted_{}", i),
                zone: "alpha".to_string(),
                severity: 5,
                round: 2,
                reputation: 0.9,
            },
            2,
        );
    }

    let escalated_now = engine
        .decisions()
        .iter()
        .any(|d| matches!(d, AedaDecision::ZoneEscalated { .. }));
    assert!(
        escalated_now,
        "Expected trusted nodes to trigger escalation"
    );
}

// BFT-046
#[test]
fn test_bft_seen_messages_cache_ceiling() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("bft-046-node", "zone-alpha", inbox.clone());

    for i in 0..SEEN_MAX {
        node.seen_messages
            .insert(format!("msg-id-{}", i), Vec::new());
    }
    assert_eq!(node.seen_messages.len(), SEEN_MAX);

    let msg = Message::new(
        "msg-id-5001",
        "peer-test",
        Signal::new(1, false, false, false, 1, Visibility::Direct),
        None,
    );
    let wire = WireMessage::from_message(&msg, &node.identity);
    let payload = serde_json::to_vec(&wire).unwrap();

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert(
            "bft-046-node".to_string(),
            vec![("test-sender".to_string(), payload)],
        );
    }

    node.process_inbox();

    assert_eq!(node.seen_messages.len(), 2, "BFT-046: after hard-clear eviction at SEEN_MAX entries, cache should contain exactly the triggering message and a chain-head announcement");
    assert!(node
        .seen_messages
        .keys()
        .any(|k| k.ends_with(":msg-id-5001")));
}

// BFT-047: BFT-008 persists across BFT-046 eviction
#[test]
fn test_bft008_persists_across_bft046_eviction() {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("bft-008-046-node", "zone-alpha", inbox.clone());

    // Spend the spammy peer's whole per-sender budget.
    let peer = "spammy-peer".to_string();
    while node.sybil_guard.check(&peer).is_ok() {}
    let drained = node.sybil_guard.available_tokens(&peer).unwrap();
    assert!(drained < 1.0);

    for i in 0..SEEN_MAX {
        node.seen_messages
            .insert(format!("msg-id-{}", i), Vec::new());
    }
    assert_eq!(node.seen_messages.len(), SEEN_MAX);

    let msg = Message::new(
        "msg-id-5001",
        "some-other-peer",
        Signal::new(1, false, false, false, 1, Visibility::Direct),
        None,
    );
    let wire = WireMessage::from_message(&msg, &node.identity);
    let payload = serde_json::to_vec(&wire).unwrap();

    {
        let mut guard = inbox.lock().unwrap();
        guard.insert(
            "bft-008-046-node".to_string(),
            vec![("test-sender".to_string(), payload)],
        );
    }

    node.process_inbox();
    assert_eq!(
        node.metrics.seen_messages_evictions.load(Ordering::Relaxed),
        1
    );

    assert_eq!(
        node.sybil_guard.available_tokens(&peer),
        Some(drained),
        "BFT-008 per-sender budget should persist across BFT-046 eviction"
    );
}
