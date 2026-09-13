use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRecord {
    pub id: String,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}
