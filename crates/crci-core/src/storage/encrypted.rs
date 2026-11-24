use super::record::StorageRecord;
use super::{StorageBackend, StorageError};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use bincode;
use rand::rngs::OsRng;
use rand::RngCore;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

/// Argon2id cost, written out so that a library upgrade that changed its defaults could not
/// silently make every existing key file undecryptable. These are the `argon2` 0.5 defaults
/// (19 MiB of memory, 2 passes, 1 lane); a test checks they still equal them.
const KDF_MEMORY_KIB: u32 = 19 * 1024;
const KDF_PASSES: u32 = 2;
const KDF_LANES: u32 = 1;

fn kdf() -> Result<Argon2<'static>, StorageError> {
    let params = Params::new(KDF_MEMORY_KIB, KDF_PASSES, KDF_LANES, Some(32))
        .map_err(|e| StorageError::KeyDerivation(e.to_string()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Domain tag of the associated data that ties each record's ciphertext to its ID, so a
/// record cannot be passed off under another ID.
const RECORD_AAD_DOMAIN: &[u8] = b"CRCI-STORE-RECORD-v1\0";

fn record_aad(id: &str) -> Vec<u8> {
    let mut aad = RECORD_AAD_DOMAIN.to_vec();
    aad.extend_from_slice(id.as_bytes());
    aad
}

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

            let argon2 = kdf()?;
            // Derive the raw 32-byte key with Argon2id and the random salt.
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

            let argon2 = kdf()?;
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
        // On disk before the caller acts on it (a reserved sequence-number floor must survive a
        // power loss, or the next start would reuse numbers already sent).
        f.sync_all()?;
        Ok(())
    }

    fn seal(&self, id: &str, plaintext: &[u8]) -> Result<StorageRecord, StorageError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let aes_key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(aes_key);

        let aad = record_aad(id);
        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| StorageError::AuthenticationFailed)?; // If encryption fails (rare)
        Ok(StorageRecord {
            id: id.to_string(),
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Make `id` the only record of the file, replacing the file in one step. For a record that is
    /// rewritten again and again, which would grow the append-only file without limit.
    pub async fn replace_all(&self, id: &str, plaintext: &[u8]) -> Result<(), StorageError> {
        let _guard = self.file_mutex.lock().await;
        let serialized = bincode::serialize(&self.seal(id, plaintext)?)
            .map_err(|e| StorageError::Serialisation(e.to_string()))?;
        let mut salt = [0u8; 16];
        fs::File::open(&self.path)?.read_exact(&mut salt)?;
        let mut temporary = self.path.clone().into_os_string();
        temporary.push(".new");
        let mut f = fs::File::create(&temporary)?;
        f.write_all(&salt)?;
        f.write_all(&(serialized.len() as u64).to_le_bytes())?;
        f.write_all(&serialized)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for EncryptedStore {
    async fn write(&self, id: &str, plaintext: &[u8]) -> Result<(), StorageError> {
        let _guard = self.file_mutex.lock().await;
        self.append_record(&self.seal(id, plaintext)?)
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

        let aad = record_aad(id);
        // A record written before the ID was bound into the encryption has no associated data;
        // it is still read, and the next write of that ID replaces it.
        let plaintext = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: record.ciphertext.as_ref(),
                    aad: &aad,
                },
            )
            .or_else(|_| cipher.decrypt(nonce, record.ciphertext.as_ref()))
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
// Gated behind the "test-utils" feature. This feature is activated only via
// the crci-core dev-dependency self-reference in crci-core/Cargo.toml, so
// these helpers are never compiled into production or release builds.
// Verified with: cargo build --release (helpers absent) vs
//                cargo test --all    (helpers present via test-utils feature).
#[cfg(feature = "test-utils")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_out_cost_is_the_library_default() {
        // If a library upgrade changes its defaults, these constants keep old key files readable;
        // this test makes the difference visible.
        let (mut ours, mut theirs) = ([0u8; 32], [0u8; 32]);
        kdf()
            .and_then(|a| {
                a.hash_password_into(b"pw", &[7u8; 16], &mut ours)
                    .map_err(|e| StorageError::KeyDerivation(e.to_string()))
            })
            .unwrap_or_else(|e| panic!("{e}"));
        Argon2::default()
            .hash_password_into(b"pw", &[7u8; 16], &mut theirs)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ours, theirs);
    }

    fn temp_file(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("crci-store-{name}-{}", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    #[tokio::test]
    async fn a_record_cannot_be_passed_off_under_another_id() {
        let path = temp_file("swap");
        let store = EncryptedStore::new(&path, "pw").unwrap_or_else(|e| panic!("{e}"));
        store
            .write("a", b"secret a")
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        store
            .write("b", b"secret b")
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let frames = store.read_all_frames().unwrap_or_else(|e| panic!("{e}"));
        let stolen = frames.get("a").cloned().unwrap_or_else(|| panic!("no a"));
        store
            .append_record(&StorageRecord {
                id: "b".to_string(),
                ..stolen
            })
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(
            store.read("b").await,
            Err(StorageError::AuthenticationFailed)
        ));
        let _ = fs::remove_file(&path);
    }

    #[tokio::test]
    async fn a_record_written_without_the_id_bound_is_still_read() {
        let path = temp_file("legacy");
        let store = EncryptedStore::new(&path, "pw").unwrap_or_else(|e| panic!("{e}"));
        let nonce_bytes = [9u8; 12];
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&store.key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), b"old".as_ref())
            .unwrap_or_else(|_| panic!("encrypt"));
        store
            .append_record(&StorageRecord {
                id: "old".to_string(),
                nonce: nonce_bytes,
                ciphertext,
            })
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            store.read("old").await.unwrap_or_else(|e| panic!("{e}")),
            b"old"
        );
        let _ = fs::remove_file(&path);
    }
}
