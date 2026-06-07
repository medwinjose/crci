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

use crate::runtime::NodeRuntime;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub struct NodeRuntimeHandle {
    _runtime: tokio::runtime::Runtime,
    node: NodeRuntime,
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

        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(_) => return false,
        };

        let inbox: crate::transport::SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let node = NodeRuntime::new(&config.node_id, "zone-ffi", inbox);

        *handle = Some(NodeRuntimeHandle {
            _runtime: rt,
            node,
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
    if let Ok(handle) = get_node_handle().lock() {
        if let Some(runtime_handle) = handle.as_ref() {
            return runtime_handle.node.peers.len() as u32;
        }
    }
    0
}
