pub mod ble;
pub mod lora;
pub mod multiplexer;
pub mod sim;
pub mod tcp;

pub type NodeId = String;
pub type NetworkMessage = crate::integration::PipelineMessage;

#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    Tcp,
    Ble,
    Lora,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("transport not available on this platform")]
    NotAvailable,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("serialisation error: {0}")]
    Serialisation(String),
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, peer: &NodeId, message: &NetworkMessage) -> Result<(), TransportError>;
    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError>;
    fn transport_type(&self) -> TransportType;
    fn is_available(&self) -> bool;
}

// Re-exports
#[allow(unused_imports)]
pub use ble::{BleConfig, BleTransport};
#[allow(unused_imports)]
pub use lora::{LoraConfig, LoraTransport};
#[allow(unused_imports)]
pub use multiplexer::TransportMultiplexer;
#[allow(unused_imports)]
pub use sim::{LegacyTransport, SharedInbox, SimTransport};
#[allow(unused_imports)]
pub use tcp::TcpTransport;
