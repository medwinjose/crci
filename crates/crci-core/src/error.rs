use thiserror::Error;

/// Comprehensive error type covering all CRCI failure modes.
#[derive(Error, Debug)]
pub enum CrciError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Network transport error: {0}")]
    Transport(String),

    #[error("Cryptographic signature or identity invalid")]
    InvalidSignature,

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Zone not recognized: {0}")]
    ZoneNotFound(String),

    #[error("Storage engine error: {0}")]
    Storage(String),

    #[error("API interaction error: {0}")]
    Api(String),

    #[error("Rate limit exceeded for peer: {0}")]
    RateLimited(String),

    #[error("Byzantine actor detected and isolated: {0}")]
    ByzantineIsolation(String),

    #[error("State divergence detected at sequence: {0}")]
    Divergence(u64),

    #[error("Hardware abstraction layer error: {0}")]
    Hardware(String),

    #[error("Lock was poisoned during concurrent access")]
    LockPoisoned,

    #[error("Internal system error: {0}")]
    Internal(String),
}
