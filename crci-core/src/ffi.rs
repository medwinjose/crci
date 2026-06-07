#[derive(uniffi::Record, Clone)]
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

use std::sync::{Mutex, OnceLock};

pub struct NodeRuntimeHandle {
    peer_count: u32,
    max_peers: u32,
}

static NODE_HANDLE: OnceLock<Mutex<Option<NodeRuntimeHandle>>> = OnceLock::new();

fn get_node_handle() -> &'static Mutex<Option<NodeRuntimeHandle>> {
    NODE_HANDLE.get_or_init(|| Mutex::new(None))
}

#[uniffi::export]
pub fn start_node(config: FfiNodeConfig) -> bool {
    if !validate_node_config(config.clone()) {
        return false;
    }
    if let Ok(mut handle) = get_node_handle().lock() {
        if handle.is_some() {
            return false;
        }
        *handle = Some(NodeRuntimeHandle {
            peer_count: 0,
            max_peers: config.max_peers,
        });
        true
    } else {
        false
    }
}

#[uniffi::export]
pub fn stop_node() -> bool {
    if let Ok(mut handle) = get_node_handle().lock() {
        if handle.is_some() {
            *handle = None;
            true
        } else {
            false
        }
    } else {
        false
    }
}

#[uniffi::export]
pub fn peer_count() -> u32 {
    if let Ok(mut handle) = get_node_handle().lock() {
        if let Some(runtime) = handle.as_mut() {
            if runtime.peer_count < runtime.max_peers {
                runtime.peer_count += 1;
            }
            return runtime.peer_count;
        }
    }
    0
}
