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

struct FfiMockStorage;

#[async_trait::async_trait]
impl crate::storage::StorageBackend for FfiMockStorage {
    async fn write(&self, _key: &str, _value: &[u8]) -> Result<(), crate::storage::StorageError> {
        Ok(())
    }
    async fn read(&self, _key: &str) -> Result<Vec<u8>, crate::storage::StorageError> {
        Ok(Vec::new())
    }
    async fn delete(&self, _id: &str) -> Result<(), crate::storage::StorageError> {
        Ok(())
    }
    async fn list_ids(&self) -> Result<Vec<String>, crate::storage::StorageError> {
        Ok(Vec::new())
    }
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
        let mut node = NodeRuntime::new(&config.node_id, "zone-ffi", inbox);
        node.storage_backend = Some(Arc::new(FfiMockStorage));

        *handle = Some(NodeRuntimeHandle { _runtime: rt, node });
    } else {
        return false;
    };

    // Spawn the background consensus loop using the static node handle
    if let Ok(handle_guard) = get_node_handle().lock() {
        if let Some(h) = handle_guard.as_ref() {
            h._runtime.spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if let Ok(mut lock) = get_node_handle().lock() {
                        if let Some(runtime_handle) = lock.as_mut() {
                            runtime_handle.node.process_inbox();
                            runtime_handle.node.run_consensus();
                        } else {
                            break; // Stop loop if node is stopped
                        }
                    } else {
                        break; // Stop loop if mutex is poisoned
                    }
                }
            });
        }
    }

    true
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

#[uniffi::export]
pub fn connect_peer(addr: String) -> bool {
    let mut guard = match get_node_handle().lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    match guard.as_mut() {
        Some(h) => {
            h.node.add_peer(&addr);
            true
        }
        None => false,
    }
}

#[uniffi::export]
pub fn consensus_rounds() -> u64 {
    if let Ok(handle) = get_node_handle().lock() {
        if let Some(runtime_handle) = handle.as_ref() {
            return runtime_handle.node.consensus_rounds;
        }
    }
    0
}
