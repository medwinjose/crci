use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

// ─── Identity ────────────────────────────────────────────────────────────────
// In real life: every person's phone generates a unique cryptographic identity
// when they first install CRCI. Think of it like a passport that cannot be
// forged — a private key only they hold, and a public key everyone can check.

pub struct Identity {
    #[allow(dead_code)]
    pub id: String,
    pub signing_key: SigningKey,     // private — never leaves this device
    pub verifying_key: VerifyingKey, // public — shared with everyone
}

impl Identity {
    pub fn new(id: &str) -> Identity {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Identity {
            id: id.to_string(),
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
        self.verifying_key.verify(payload, signature).is_ok()
    }
}
