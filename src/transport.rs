// ─── Transport Trait ─────────────────────────────────────────────────────────
// In real life: this is the radio inside the phone. It could be Bluetooth,
// WiFi Direct, or LoRa. The node logic never changes regardless of which
// radio is underneath — it just calls send() and the trait handles the rest.
//
// Technical: a trait in Rust is like an interface in other languages.
// Any type that implements Transport must provide these two functions.
// This lets us swap the real radio for a simulation without changing
// any node logic — the node just sees "something I can send bytes through."

#[allow(dead_code)]
pub trait Transport {
    // Send raw bytes to a specific peer by their ID
    fn send(&self, to: &str, data: &[u8]);

    // Return this transport's own node ID
    fn node_id(&self) -> &str;
}

// ─── Simulated Transport ─────────────────────────────────────────────────────
// In real life: this doesn't exist. In simulation: it's a fake radio that
// delivers bytes directly in memory, letting us test mesh behaviour without
// actual Bluetooth hardware.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// A shared inbox — bytes waiting to be processed by each node
// Arc = shared ownership across threads
// Mutex = only one thread can read/write at a time (prevents corruption)
pub type SharedInbox = Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>>;

pub struct SimTransport {
    #[allow(dead_code)]
    pub node_id: String,
    pub inbox: SharedInbox, // shared across all nodes in the simulation
}

impl SimTransport {
    pub fn new(node_id: &str, inbox: SharedInbox) -> SimTransport {
        SimTransport {
            node_id: node_id.to_string(),
            inbox,
        }
    }
}

impl Transport for SimTransport {
    fn send(&self, to: &str, data: &[u8]) {
        // Drop bytes into the recipient's inbox
        let mut inbox = match self.inbox.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inbox.entry(to.to_string()).or_default().push(data.to_vec());
    }

    #[allow(dead_code)]
    fn node_id(&self) -> &str {
        &self.node_id
    }
}

// ─── TCP Gossip Transport Shim ───────────────────────────────────────────────

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

pub struct TcpGossipNode {
    pub node_id: String,
    pub zone: String,
    pub peers: Vec<String>,
    pub pipeline: Arc<Mutex<GossipPipeline>>,
    pub received: Arc<Mutex<Vec<PipelineMessage>>>,
}

use crate::integration::{GossipPipeline, PipelineMessage, PipelineVerdict};

impl TcpGossipNode {
    pub fn new(node_id: &str, zone: &str, peers: Vec<String>) -> Self {
        TcpGossipNode {
            node_id: node_id.to_string(),
            zone: zone.to_string(),
            peers,
            pipeline: Arc::new(Mutex::new(GossipPipeline::new())),
            received: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Starts a TCP listener on the configured port.
    /// Spawns a background thread to accept incoming connections.
    pub fn start_listener(&self, port: u16) -> thread::JoinHandle<()> {
        let pipeline = self.pipeline.clone();
        let peers = self.peers.clone();
        let local_node_id = self.node_id.clone();
        let received = self.received.clone();

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let listener = TcpListener::bind(&addr)
                .unwrap_or_else(|e| panic!("Failed to bind TCP listener on {}: {}", addr, e));

            for mut stream in listener.incoming().flatten() {
                let pipeline_inner = pipeline.clone();
                let peers_inner = peers.clone();
                let local_node_id_inner = local_node_id.clone();
                let received_inner = received.clone();
                thread::spawn(move || {
                    let mut buf = Vec::new();
                    // Read message until EOF (connection closed by sender)
                    if stream.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                        if let Ok(msg) = serde_json::from_slice::<PipelineMessage>(&buf) {
                            // Skip messages that originated from ourselves to prevent loops
                            if msg.origin_node == local_node_id_inner {
                                return;
                            }

                            let verdict = {
                                let mut pipeline_guard = pipeline_inner.lock().unwrap();
                                pipeline_guard.process(&msg)
                            };

                            if verdict == PipelineVerdict::Accept {
                                {
                                    let mut rec = received_inner.lock().unwrap();
                                    rec.push(msg.clone());
                                }

                                // Gossip/forward the message to all other peers
                                for peer in peers_inner {
                                    let msg_clone = msg.clone();
                                    thread::spawn(move || {
                                        if let Ok(mut out_stream) = TcpStream::connect(&peer) {
                                            if let Ok(serialized) = serde_json::to_vec(&msg_clone) {
                                                let _ = out_stream.write_all(&serialized);
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                });
            }
        })
    }

    /// Originate and broadcast a message from this node.
    pub fn broadcast(&self, msg: PipelineMessage) {
        // Log/process locally first
        {
            let mut pipeline_guard = self.pipeline.lock().unwrap();
            let _ = pipeline_guard.process(&msg);
        }
        {
            let mut rec = self.received.lock().unwrap();
            rec.push(msg.clone());
        }

        // Send to all peers
        for peer in &self.peers {
            let msg_clone = msg.clone();
            let peer_addr = peer.clone();
            thread::spawn(move || {
                if let Ok(mut out_stream) = TcpStream::connect(&peer_addr) {
                    if let Ok(serialized) = serde_json::to_vec(&msg_clone) {
                        let _ = out_stream.write_all(&serialized);
                    }
                }
            });
        }
    }
}
