use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};
use futures::stream::{FuturesUnordered, StreamExt};

pub struct TransportMultiplexer {
    transports: Vec<Box<dyn Transport>>,
}

impl TransportMultiplexer {
    pub fn new(transports: Vec<Box<dyn Transport>>) -> Self {
        Self { transports }
    }
}

#[async_trait::async_trait]
impl Transport for TransportMultiplexer {
    async fn send(&self, peer: &NodeId, message: &NetworkMessage) -> Result<(), TransportError> {
        let mut last_err = None;

        for transport in &self.transports {
            if !transport.is_available() {
                continue;
            }

            match transport.send(peer, message).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!(
                        "Transport {:?} failed to send: {}",
                        transport.transport_type(),
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or(TransportError::NotAvailable))
    }

    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError> {
        let mut futures = FuturesUnordered::new();

        for transport in &self.transports {
            if transport.is_available() {
                futures.push(transport.receive());
            }
        }

        if futures.is_empty() {
            return Err(TransportError::NotAvailable);
        }

        while let Some(res) = futures.next().await {
            match res {
                Ok(msg) => return Ok(msg),
                Err(e) => {
                    log::warn!("Transport receive error: {}", e);
                    // continue to next future
                }
            }
        }

        Err(TransportError::ConnectionFailed(
            "All transports failed to receive".into(),
        ))
    }

    fn transport_type(&self) -> TransportType {
        // Return Tcp as a dummy or proxy?
        // The trait requires returning a type. Since multiplexer aggregates them, we can return the first available.
        for t in &self.transports {
            if t.is_available() {
                return t.transport_type();
            }
        }
        TransportType::Tcp
    }

    fn is_available(&self) -> bool {
        self.transports.iter().any(|t| t.is_available())
    }
}
