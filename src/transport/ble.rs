use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};

pub struct BleConfig {
    pub device_name: String,
    pub service_uuid: String,
    pub mtu: u16,
}

pub struct BleTransport {
    pub config: BleConfig,
}

impl BleTransport {
    pub fn new(config: BleConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl Transport for BleTransport {
    async fn send(&self, _peer: &NodeId, _message: &NetworkMessage) -> Result<(), TransportError> {
        Err(TransportError::NotAvailable)
    }

    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError> {
        Err(TransportError::NotAvailable)
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Ble
    }

    fn is_available(&self) -> bool {
        false
    }
}
