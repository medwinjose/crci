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

// ─── Tokio Async Gossip Transport Shim ─────────────────────────────────────────

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::integration::{GossipPipeline, PipelineMessage, PipelineVerdict};

pub struct AsyncTransport {
    pub node_id: String,
    pub listen_addr: SocketAddr,
    pub peers: Vec<SocketAddr>,
    pub accepted_count: Arc<AtomicU64>,
    pub rejected_count: Arc<AtomicU64>,
    pub rescue_held: Arc<AtomicU64>,
}

impl AsyncTransport {
    pub async fn bind(
        node_id: String,
        addr: SocketAddr,
        peers: Vec<SocketAddr>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            node_id,
            listen_addr: addr,
            peers,
            accepted_count: Arc::new(AtomicU64::new(0)),
            rejected_count: Arc::new(AtomicU64::new(0)),
            rescue_held: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn send_msg(&self, peer: SocketAddr, msg: &PipelineMessage) -> std::io::Result<()> {
        let mut stream = TcpStream::connect(peer).await?;
        let payload = serde_json::to_vec(msg)?;
        stream.write_u32(payload.len() as u32).await?;
        stream.write_all(&payload).await?;
        Ok(())
    }

    pub async fn broadcast(&self, msg: &PipelineMessage) -> Vec<std::io::Result<()>> {
        let mut results = Vec::new();
        for peer in &self.peers {
            let res = self.send_msg(*peer, msg).await;
            results.push(res);
        }
        results
    }

    pub async fn recv_loop(
        self: Arc<Self>,
        pipeline: Arc<Mutex<GossipPipeline>>,
        mut shutdown: broadcast::Receiver<()>,
    ) {
        let listener = match TcpListener::bind(self.listen_addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "[{}] Failed to bind TCP listener on {}: {}",
                    self.node_id, self.listen_addr, e
                );
                return;
            }
        };

        loop {
            tokio::select! {
                Ok((mut stream, _addr)) = listener.accept() => {
                    let pipeline_inner = pipeline.clone();
                    let transport_inner = self.clone();
                    let local_node_id = self.node_id.clone();

                    tokio::spawn(async move {
                        while let Ok(len) = stream.read_u32().await {
                            // Prevent allocating huge memory on garbage length
                            if len > 1024 * 1024 {
                                break;
                            }

                            let mut buf = vec![0u8; len as usize];
                            if stream.read_exact(&mut buf).await.is_err() {
                                break;
                            }

                            if let Ok(msg) = serde_json::from_slice::<PipelineMessage>(&buf) {
                                // Skip messages that originated from ourselves to prevent loops
                                if msg.origin_node == local_node_id {
                                    continue;
                                }

                                let verdict = {
                                    let mut pipeline_guard = pipeline_inner.lock().unwrap();
                                    pipeline_guard.process(&msg)
                                };

                                match verdict {
                                    PipelineVerdict::Accept => {
                                        transport_inner.accepted_count.fetch_add(1, Ordering::SeqCst);
                                        // Forward the accepted message to all our peers
                                        let _ = transport_inner.broadcast(&msg).await;
                                    }
                                    PipelineVerdict::Reject(_) | PipelineVerdict::Throttle(_) => {
                                        transport_inner.rejected_count.fetch_add(1, Ordering::SeqCst);
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                    });
                }
                _ = shutdown.recv() => {
                    break;
                }
            }
        }
    }
}
