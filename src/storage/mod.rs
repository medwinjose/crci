pub mod legacy;
pub mod encrypted;
pub mod record;

pub use legacy::{PersistedRescue, PersistedState};

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn write(&self, id: &str, plaintext: &[u8]) -> Result<(), StorageError>;
    async fn read(&self, id: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
    async fn list_ids(&self) -> Result<Vec<String>, StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("authentication failed — data may be tampered")]
    AuthenticationFailed,
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialisation error: {0}")]
    Serialisation(String),
}
