use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

// ─── Identity ────────────────────────────────────────────────────────────────
// Every node holds an Ed25519 key pair. Its node ID is derived from the public key, so
// an ID cannot be claimed by anyone who does not hold the matching private key. The
// human-readable label is for logs only and is never authenticated.

/// Domain tag for node-ID derivation.
pub const NODE_ID_DOMAIN: &[u8] = b"CRCI-NODE-ID-v1\0";
/// Bytes of the SHA-256 digest kept in a node ID (128 bits: claiming an existing ID needs a
/// second preimage, about 2^128 key generations).
pub const NODE_ID_BYTES: usize = 16;

/// The node ID of a public key: the first 16 bytes of SHA-256(domain tag ‖ key), as 32
/// lowercase hex characters.
pub fn derive_node_id(public_key: &[u8]) -> String {
    let digest = Sha256::new()
        .chain_update(NODE_ID_DOMAIN)
        .chain_update(public_key)
        .finalize();
    digest[..NODE_ID_BYTES]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub struct Identity {
    /// Node ID, derived from `verifying_key`.
    pub id: String,
    /// Human-readable name for logs; not authenticated.
    pub label: String,
    pub signing_key: SigningKey,     // private — never leaves this device
    pub verifying_key: VerifyingKey, // public — shared with everyone
}

impl Identity {
    /// A fresh key pair, labelled `label`.
    pub fn new(label: &str) -> Identity {
        Identity::from_signing_key(label, SigningKey::generate(&mut OsRng))
    }

    /// The identity of an existing key (for example one loaded by `load_or_create`).
    pub fn from_signing_key(label: &str, signing_key: SigningKey) -> Identity {
        let verifying_key = signing_key.verifying_key();
        Identity {
            id: derive_node_id(verifying_key.as_bytes()),
            label: label.to_string(),
            signing_key,
            verifying_key,
        }
    }

    // Sign a message payload — proves this identity created it
    pub fn sign(&self, payload: &[u8]) -> Signature {
        self.signing_key.sign(payload)
    }

    // Verify a signature against this identity's public key
    #[allow(dead_code)]
    pub fn verify(&self, payload: &[u8], signature: &Signature) -> bool {
        self.verifying_key.verify_strict(payload, signature).is_ok()
    }
}

// ─── Persistent node key ─────────────────────────────────────────────────────

const KEY_RECORD: &str = "node-signing-key";
const SEQ_RECORD: &str = "node-seq-floor";
/// Sequence numbers reserved per start. A restarted node continues above everything the
/// previous run could have used, so its neighbours never see a sequence number twice from it
/// (they would drop the new message, or strike it as a replay).
pub const SEQ_BLOCK: u64 = 1 << 32;

/// Load the node key kept in the encrypted store at `path` (AES-256-GCM, key from
/// `passphrase` with Argon2id), or create and store one if the store has none. Returns the key
/// and the first sequence number this run may use; the next run's floor is stored before
/// returning. A wrong passphrase or a damaged store is an error, never a new key.
pub async fn load_or_create(
    path: &std::path::Path,
    passphrase: &str,
) -> Result<(SigningKey, u64), crate::storage::StorageError> {
    use crate::storage::{StorageBackend, StorageError};
    if passphrase.is_empty() {
        return Err(StorageError::KeyDerivation(
            "empty key passphrase".to_string(),
        ));
    }
    let store = crate::storage::encrypted::EncryptedStore::new(path, passphrase)?;
    let key = match store.read(KEY_RECORD).await {
        Ok(bytes) => {
            let arr: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::AuthenticationFailed)?;
            SigningKey::from_bytes(&arr)
        }
        Err(StorageError::NotFound(_)) => {
            if !store.list_ids().await?.is_empty() {
                // Records exist but not the key: a damaged store, not a new node.
                return Err(StorageError::NotFound(KEY_RECORD.to_string()));
            }
            let key = SigningKey::generate(&mut OsRng);
            store.write(KEY_RECORD, &key.to_bytes()).await?;
            key
        }
        Err(e) => return Err(e),
    };
    let floor = match store.read(SEQ_RECORD).await {
        Ok(bytes) => u64::from_be_bytes(
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::AuthenticationFailed)?,
        ),
        Err(StorageError::NotFound(_)) => 0,
        Err(e) => return Err(e),
    };
    // shortcut: a run that sends more than SEQ_BLOCK messages (about 27 years at the
    // per-origin limit of 5/s) would overlap the next run's block; reserve more if that matters.
    let next = floor
        .checked_add(SEQ_BLOCK)
        .ok_or_else(|| StorageError::KeyDerivation("sequence numbers exhausted".to_string()))?;
    store.write(SEQ_RECORD, &next.to_be_bytes()).await?;
    Ok((key, floor))
}

// ─── What an earlier run relayed ─────────────────────────────────────────────

const RELAY_RECORD: &str = "relay-marks";
/// How often a node saves its relay marks while they change (a new origin is saved at once).
pub const RELAY_MARKS_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
/// Messages one origin can have accepted between two saves: its burst and what its budget refills.
/// After a run that did not stop cleanly this many above each saved mark may have been relayed.
pub const RELAY_MARKS_MARGIN: u64 = 45;

/// Keeps, next to the node key, the highest sequence number this node accepted per origin, so a
/// restarted node does not relay again what its neighbours hold from it already: they would strike
/// the repeats as replays (`NodeRuntime::relayed_before`).
pub struct RelayMarkStore(crate::storage::encrypted::EncryptedStore);

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RelayMarks {
    /// Saved when the run stopped, so nothing was relayed after it.
    clean: bool,
    marks: std::collections::HashMap<String, u64>,
}

impl RelayMarkStore {
    /// Open the store belonging to the key file `key_file`, with the marks of the previous run and
    /// the margin to add to them. A file that cannot be read yields no marks and an error to
    /// report: the node can still run, at the risk of being struck for old messages.
    pub async fn open(
        key_file: &std::path::Path,
        passphrase: &str,
    ) -> Result<
        (
            Self,
            std::collections::HashMap<String, u64>,
            u64,
            Option<crate::storage::StorageError>,
        ),
        crate::storage::StorageError,
    > {
        use crate::storage::{StorageBackend, StorageError};
        let mut path = key_file.as_os_str().to_os_string();
        path.push(".relayed");
        let path = std::path::PathBuf::from(path);
        // A power loss while the file was first made can leave it without its salt.
        let store = match crate::storage::encrypted::EncryptedStore::new(&path, passphrase) {
            Ok(store) => store,
            Err(_) => {
                std::fs::remove_file(&path)?;
                crate::storage::encrypted::EncryptedStore::new(&path, passphrase)?
            }
        };
        let (saved, error) = match store.read(RELAY_RECORD).await {
            Ok(bytes) => match serde_json::from_slice::<RelayMarks>(&bytes) {
                Ok(saved) => (saved, None),
                Err(e) => (
                    RelayMarks::default(),
                    Some(StorageError::Serialisation(e.to_string())),
                ),
            },
            Err(StorageError::NotFound(_)) => (RelayMarks::default(), None),
            Err(e) => (RelayMarks::default(), Some(e)),
        };
        let margin = if saved.clean { 0 } else { RELAY_MARKS_MARGIN };
        let this = Self(store);
        // From here on the saved marks may fall behind.
        this.save(&saved.marks, false).await?;
        Ok((this, saved.marks, margin, error))
    }

    /// Replace the saved marks; `clean` if the node relays nothing after this.
    pub async fn save(
        &self,
        marks: &std::collections::HashMap<String, u64>,
        clean: bool,
    ) -> Result<(), crate::storage::StorageError> {
        let bytes = serde_json::to_vec(&RelayMarks {
            clean,
            marks: marks.clone(),
        })
        .map_err(|e| crate::storage::StorageError::Serialisation(e.to_string()))?;
        self.0.replace_all(RELAY_RECORD, &bytes).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_matches_an_independent_computation() {
        // python: hashlib.sha256(b'CRCI-NODE-ID-v1\0' + bytes(32)).hexdigest()[:32]
        assert_eq!(
            derive_node_id(&[0u8; 32]),
            "08db4998917da5a71217722da4e78693"
        );
    }

    #[test]
    fn an_identity_id_is_derived_from_its_key() {
        let id = Identity::new("alice");
        assert_eq!(id.id, derive_node_id(id.verifying_key.as_bytes()));
        assert_eq!(id.label, "alice");
        assert_ne!(Identity::new("alice").id, id.id);
    }
}
