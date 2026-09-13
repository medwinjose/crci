use crci_core::ffi::{consensus_rounds, start_node, stop_node, FfiNodeConfig};
use std::time::Duration;

#[test]
fn test_ffi_background_consensus_loop_runs() {
    let config = FfiNodeConfig {
        node_id: "test-ffi-node-81".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 10,
    };

    // Ensure we start from a clean state (in case other tests ran in same process, though tests are mostly isolated)
    stop_node();

    assert_eq!(consensus_rounds(), 0, "Consensus rounds should start at 0");

    let started = start_node(config);
    assert!(started, "Node should start successfully");

    // The loop sleeps for 500ms, then processes inbox and increments consensus_rounds.
    // Let's wait 1.5 seconds to allow it to tick at least twice.
    std::thread::sleep(Duration::from_millis(1500));

    let rounds = consensus_rounds();
    assert!(
        rounds >= 2,
        "Background loop did not tick enough times. Expected >= 2, got {}",
        rounds
    );

    stop_node();
}
