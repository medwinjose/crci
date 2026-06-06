use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

pub struct TcpTransport {
    pub listen_addr: SocketAddr,
    pub rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, NetworkMessage)>>,
    pub accepted_count: Arc<AtomicU64>,
    pub rejected_count: Arc<AtomicU64>,
    pub node_id: String,
}

impl TcpTransport {
    pub async fn bind(node_id: String, addr: SocketAddr) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (tx, rx) = mpsc::channel(1024);
        let accepted = Arc::new(AtomicU64::new(0));
        let rejected = Arc::new(AtomicU64::new(0));

        let actual_addr = listener.local_addr().unwrap_or(addr);
        let transport = Self {
            listen_addr: actual_addr,
            rx: tokio::sync::Mutex::new(rx),
            accepted_count: accepted.clone(),
            rejected_count: rejected.clone(),
            node_id: node_id.clone(),
        };

        // Spawn a background task to accept incoming connections and push messages to the channel
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _peer_addr)) = listener.accept().await {
                    let tx_clone = tx.clone();
                    let local_node_id = node_id.clone();

                    tokio::spawn(async move {
                        while let Ok(len) = stream.read_u32().await {
                            if len > 1024 * 1024 {
                                break;
                            }

                            let mut buf = vec![0u8; len as usize];
                            if stream.read_exact(&mut buf).await.is_err() {
                                break;
                            }

                            if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&buf) {
                                if msg.origin_node == local_node_id {
                                    continue;
                                }

                                // We assume the peer's NodeId is the SocketAddr for incoming connections,
                                // or we can use msg.origin_node as the NodeId, but for routing, 
                                // we need the actual address if we are to respond.
                                // The prompt doesn't specify, but let's use the origin_node.
                                let sender_id = msg.origin_node.clone();
                                
                                if tx_clone.send((sender_id, msg)).await.is_err() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    });
                }
            }
        });

        Ok(transport)
    }
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    async fn send(&self, peer: &NodeId, message: &NetworkMessage) -> Result<(), TransportError> {
        let addr: SocketAddr = peer
            .parse()
            .map_err(|_| TransportError::ConnectionFailed(format!("Invalid SocketAddr: {}", peer)))?;

        let mut stream = TcpStream::connect(addr)
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        let payload = serde_json::to_vec(message)
            .map_err(|e| TransportError::Serialisation(e.to_string()))?;

        stream
            .write_u32(payload.len() as u32)
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        stream
            .write_all(&payload)
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        Ok(())
    }

    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError> {
        let mut rx = self.rx.lock().await;
        if let Some(msg) = rx.recv().await {
            Ok(msg)
        } else {
            Err(TransportError::ConnectionFailed("Channel closed".into()))
        }
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Tcp
    }

    fn is_available(&self) -> bool {
        true
    }
}
