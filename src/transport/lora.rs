use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};

pub struct LoraConfig {
    pub frequency_hz: u32,
    pub spreading_factor: u8,
    pub bandwidth_khz: u32,
    pub coding_rate: u8,
}

pub struct LoraTransport {
    pub config: LoraConfig,
}

impl LoraTransport {
    pub fn new(config: LoraConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl Transport for LoraTransport {
    async fn send(&self, _peer: &NodeId, _message: &NetworkMessage) -> Result<(), TransportError> {
        Err(TransportError::NotAvailable)
    }

    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError> {
        Err(TransportError::NotAvailable)
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Lora
    }

    fn is_available(&self) -> bool {
        false
    }
}
