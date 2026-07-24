use super::record::StorageRecord;
use super::{StorageBackend, StorageError};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::Argon2;
use bincode;
use rand::rngs::OsRng;
use rand::RngCore;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

pub struct EncryptedStore {
    path: PathBuf,
    key: [u8; 32],
    // We use a Tokio Mutex because writes and reads will be asynchronous
    file_mutex: Mutex<()>,
}

impl EncryptedStore {
    pub fn new(path: &Path, passphrase: &str) -> Result<Self, StorageError> {
        let mut key = [0u8; 32];

        if !path.exists() {
            let mut salt_bytes = [0u8; 16];
            OsRng.fill_bytes(&mut salt_bytes);

            let argon2 = Argon2::default();
            // We use the salt bytes directly. Argon2id requires salt.
            // We can use hash_password but we need a raw key.
            // Let's use `argon2.hash_password_into(passphrase.as_bytes(), &salt_bytes, &mut key)`
            // Wait, does Argon2 expose hash_password_into?
            // Actually `password-hash` trait `PasswordHasher` doesn't have `hash_password_into` in all versions,
            // but `Argon2::hash_password_into` is inherent.
            argon2
                .hash_password_into(passphrase.as_bytes(), &salt_bytes, &mut key)
                .map_err(|e| StorageError::KeyDerivation(e.to_string()))?;

            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)?;
            f.write_all(&salt_bytes)?;
        } else {
            let mut f = fs::File::open(path)?;
            let mut salt_bytes = [0u8; 16];
            f.read_exact(&mut salt_bytes)?;

            let argon2 = Argon2::default();
            argon2
                .hash_password_into(passphrase.as_bytes(), &salt_bytes, &mut key)
                .map_err(|e| StorageError::KeyDerivation(e.to_string()))?;
        }

        Ok(Self {
            path: path.to_path_buf(),
            key,
            file_mutex: Mutex::new(()),
        })
    }

    fn read_all_frames(&self) -> Result<HashMap<String, StorageRecord>, StorageError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let mut f = fs::File::open(&self.path)?;
        let mut salt = [0u8; 16];
        if f.read_exact(&mut salt).is_err() {
            return Ok(HashMap::new()); // Empty or broken
        }

        let mut map = HashMap::new();
        loop {
            let mut len_buf = [0u8; 8];
            if f.read_exact(&mut len_buf).is_err() {
                break; // EOF
            }
            let frame_len = u64::from_le_bytes(len_buf);
            let mut frame_buf = vec![0u8; frame_len as usize];
            if f.read_exact(&mut frame_buf).is_err() {
                break; // EOF or truncated
            }

            if let Ok(record) = bincode::deserialize::<StorageRecord>(&frame_buf) {
                map.insert(record.id.clone(), record);
            }
        }

        Ok(map)
    }

    fn append_record(&self, record: &StorageRecord) -> Result<(), StorageError> {
        let serialized =
            bincode::serialize(record).map_err(|e| StorageError::Serialisation(e.to_string()))?;
        let len = serialized.len() as u64;

        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        f.write_all(&len.to_le_bytes())?;
        f.write_all(&serialized)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for EncryptedStore {
    async fn write(&self, id: &str, plaintext: &[u8]) -> Result<(), StorageError> {
        let _guard = self.file_mutex.lock().await;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let aes_key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(aes_key);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| StorageError::AuthenticationFailed)?; // If encryption fails (rare)

        let record = StorageRecord {
            id: id.to_string(),
            nonce: nonce_bytes,
            ciphertext,
        };

        self.append_record(&record)?;
        Ok(())
    }

    async fn read(&self, id: &str) -> Result<Vec<u8>, StorageError> {
        let _guard = self.file_mutex.lock().await;

        let frames = self.read_all_frames()?;
        let record = frames
            .get(id)
            .ok_or_else(|| StorageError::NotFound(id.to_string()))?;

        if record.ciphertext.is_empty() && record.nonce == [0; 12] {
            return Err(StorageError::NotFound(id.to_string()));
        }

        let aes_key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(aes_key);
        let nonce = Nonce::from_slice(&record.nonce);

        let plaintext = cipher
            .decrypt(nonce, record.ciphertext.as_ref())
            .map_err(|_| StorageError::AuthenticationFailed)?;

        Ok(plaintext)
    }

    async fn delete(&self, id: &str) -> Result<(), StorageError> {
        let _guard = self.file_mutex.lock().await;

        let tombstone = StorageRecord {
            id: id.to_string(),
            nonce: [0; 12],
            ciphertext: vec![],
        };

        self.append_record(&tombstone)?;
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<String>, StorageError> {
        let _guard = self.file_mutex.lock().await;

        let frames = self.read_all_frames()?;
        let ids: Vec<String> = frames
            .into_iter()
            .filter(|(_, r)| !(r.ciphertext.is_empty() && r.nonce == [0; 12]))
            .map(|(k, _)| k)
            .collect();

        Ok(ids)
    }
}

// ─── Test-only helpers ────────────────────────────────────────────────────────
// These methods are named and documented as test-only. They are not part of
// the public API surface. Do not use them in production code.
#[doc(hidden)]
impl EncryptedStore {
    /// Thin write alias for nonce-collision tests. Do not call from production code.
    pub async fn write_raw_for_nonce_test(
        &self,
        id: &str,
        plaintext: &[u8],
    ) -> Result<(), super::StorageError> {
        use super::StorageBackend;
        self.write(id, plaintext).await
    }

    /// Read every stored frame and return their 12-byte nonces in insertion order.
    /// Used exclusively by test_aes_gcm_nonce_never_repeats_across_writes.
    pub fn read_all_nonces_for_test(&self) -> Result<Vec<[u8; 12]>, super::StorageError> {
        // Re-read the file directly to collect every frame's nonce in order,
        // including nonces from overwritten records (which read_all_frames dedups away).
        use std::io::Read;
        let mut f = std::fs::File::open(&self.path)?;
        let mut salt = [0u8; 16];
        if f.read_exact(&mut salt).is_err() {
            return Ok(vec![]);
        }
        let mut nonces = Vec::new();
        loop {
            let mut len_buf = [0u8; 8];
            if f.read_exact(&mut len_buf).is_err() {
                break;
            }
            let frame_len = u64::from_le_bytes(len_buf);
            let mut frame_buf = vec![0u8; frame_len as usize];
            if f.read_exact(&mut frame_buf).is_err() {
                break;
            }
            if let Ok(record) = bincode::deserialize::<super::record::StorageRecord>(&frame_buf) {
                // Skip tombstones (nonce=[0;12], ciphertext empty)
                if !(record.ciphertext.is_empty() && record.nonce == [0; 12]) {
                    nonces.push(record.nonce);
                }
            }
        }
        Ok(nonces)
    }
}
