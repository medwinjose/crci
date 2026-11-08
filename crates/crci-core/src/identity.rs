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
