#[derive(uniffi::Record)]
pub struct FfiNodeConfig {
    pub node_id: String,
    pub listen_addr: String,
    pub max_peers: u32,
}

#[derive(uniffi::Record)]
pub struct FfiPeerInfo {
    pub peer_id: String,
    pub last_seen_secs: u64,
    pub byzantine_score: f32,
}

#[uniffi::export]
pub fn crci_version() -> String {
    "0.1.0".to_string()
}

#[uniffi::export]
pub fn validate_node_config(config: FfiNodeConfig) -> bool {
    if config.node_id.is_empty() {
        return false;
    }
    if config.max_peers == 0 || config.max_peers > 256 {
        return false;
    }
    // Very basic check for host:port
    let parts: Vec<&str> = config.listen_addr.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    true
}

#[uniffi::export]
pub fn list_peers_stub() -> Vec<FfiPeerInfo> {
    Vec::new()
}
