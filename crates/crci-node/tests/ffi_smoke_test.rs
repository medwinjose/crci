use crci_core::ffi::{crci_version, list_peers_stub, validate_node_config, FfiNodeConfig};

#[test]
fn test_crci_version_non_empty() {
    let version = crci_version();
    assert!(!version.is_empty());
}

#[test]
fn test_validate_node_config_valid() {
    let config = FfiNodeConfig {
        node_id: "node-1".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 8,
    };
    assert!(validate_node_config(config));
}

#[test]
fn test_validate_node_config_rejects_empty_id() {
    let config = FfiNodeConfig {
        node_id: "".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 8,
    };
    assert!(!validate_node_config(config));
}

#[test]
fn test_validate_node_config_rejects_bad_addr() {
    let config = FfiNodeConfig {
        node_id: "node-1".to_string(),
        listen_addr: "not-an-addr".to_string(),
        max_peers: 8,
    };
    assert!(!validate_node_config(config));
}

#[test]
fn test_validate_node_config_rejects_zero_peers() {
    let config = FfiNodeConfig {
        node_id: "node-1".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 0,
    };
    assert!(!validate_node_config(config));
}

#[test]
fn test_list_peers_stub_returns_empty() {
    let peers = list_peers_stub();
    assert_eq!(peers.len(), 0);
}
