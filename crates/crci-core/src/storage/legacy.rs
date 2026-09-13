// ─── Encrypted Local Storage ──────────────────────────────────────────────────
// In real life: each phone saves its CRCI state to disk, encrypted with a key
// derived from the node's own private key. If the phone restarts during a
// crisis, it loads its state back — peer list, reputation scores, rescue
// requests — and picks up exactly where it left off.
//
// Nobody else can read the file even if they physically take the phone,
// because the encryption key never leaves the device in plaintext.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Domain-separation label for AES-GCM key derivation from Ed25519 signing key material.
/// Changing this string would invalidate all previously saved NodeStorage files.
const KDF_DOMAIN_LABEL: &[u8] = b"crci-legacy-storage-v1";

/// Derive a 32-byte AES-256-GCM key from Ed25519 signing key bytes using
/// SHA-256 with domain separation.
///
/// SHA-256(KDF_DOMAIN_LABEL || 0x00 || signing_key_bytes) produces a key that:
///   - Is cryptographically independent of the raw signing key bytes
///   - Cannot be reversed to recover Ed25519 key material
///   - Is deterministic: same signing key always produces the same AES key
///   - Is domain-separated: the label prevents cross-context key reuse
///
/// This uses sha2 (already a workspace dependency) — no new crates added.
fn derive_storage_key(signing_key_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KDF_DOMAIN_LABEL);
    hasher.update([0x00]); // separator byte between label and key material
    hasher.update(signing_key_bytes);
    let digest = hasher.finalize();
    // SHA-256 output is exactly 32 bytes — matches AES-256 key size.
    digest.into()
}

// ─── Persisted State ─────────────────────────────────────────────────────────
// Everything a node needs to restore itself after a restart.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PersistedState {
    pub node_id: String,
    pub zone: String,
    pub reputation: f64,
    pub peers: Vec<String>,
    pub rescue_messages: Vec<PersistedRescue>,
    pub mce_zones: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PersistedRescue {
    pub id: String,
    pub origin: String,
    pub note: Option<String>,
    pub origin_active: bool,
}

// ─── NodeStorage ─────────────────────────────────────────────────────────────

pub struct NodeStorage {
    cipher: Aes256Gcm,
    path: String,
}

impl NodeStorage {
    // Create storage for a node. The encryption key is derived from the first
    // 32 bytes of the node's Ed25519 signing key — the phone's own private key
    // becomes the storage encryption key. No separate password needed.
    pub fn new(node_id: &str, signing_key_bytes: &[u8]) -> NodeStorage {
        // Derive the AES-256-GCM key from the Ed25519 signing key using
        // SHA-256 with domain separation rather than using the raw signing key
        // bytes directly. This ensures the AES key is cryptographically
        // independent of the Ed25519 key material (fixes key-separation concern
        // identified in Session 77 pre-audit).
        let key_bytes = derive_storage_key(signing_key_bytes);

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        NodeStorage {
            cipher,
            path: format!("{}_state.enc", node_id),
        }
    }

    // Save state to disk, encrypted
    pub fn save(&self, state: &PersistedState) -> Result<(), String> {
        // Serialise to JSON bytes
        let json = serde_json::to_vec(state).map_err(|e| format!("Serialise failed: {}", e))?;

        // Generate a random 12-byte nonce — different every save
        // (nonce = "number used once" — prevents pattern analysis)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, json.as_ref())
            .map_err(|e| format!("Encrypt failed: {}", e))?;

        // Write: [12 nonce bytes][encrypted data]
        let mut output = nonce_bytes.to_vec();
        output.extend_from_slice(&ciphertext);

        fs::write(&self.path, &output).map_err(|e| format!("Write failed: {}", e))?;

        println!(
            "  💾 [{}] state saved ({} bytes)",
            state.node_id,
            output.len()
        );
        Ok(())
    }

    // Load and decrypt state from disk
    pub fn load(&self) -> Result<PersistedState, String> {
        if !Path::new(&self.path).exists() {
            return Err("No saved state found".to_string());
        }

        let raw = fs::read(&self.path).map_err(|e| format!("Read failed: {}", e))?;

        if raw.len() < 12 {
            return Err("Corrupted file — too short".to_string());
        }

        // Split nonce from ciphertext
        let nonce = Nonce::from_slice(&raw[..12]);
        let ciphertext = &raw[12..];

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Decrypt failed — wrong key or corrupted data".to_string())?;

        // Deserialise
        let state: PersistedState =
            serde_json::from_slice(&plaintext).map_err(|e| format!("Deserialise failed: {}", e))?;

        println!(
            "  📂 [{}] state loaded ({} rescue msgs, {} peers)",
            state.node_id,
            state.rescue_messages.len(),
            state.peers.len()
        );

        Ok(state)
    }

    // Delete the state file — called on clean shutdown or factory reset
    pub fn delete(&self) {
        let _ = fs::remove_file(&self.path);
    }
}
