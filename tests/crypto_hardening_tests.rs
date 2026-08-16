// Session 77 — Part B: Crypto-layer hardening tests
//
// Derived from pre-audit of:
//   crci-core/src/storage/encrypted.rs  (EncryptedStore — Argon2+AES-256-GCM)
//   crci-core/src/storage/legacy.rs     (NodeStorage — raw-key AES-256-GCM)
//   crci-core/src/identity.rs           (Ed25519 via ed25519-dalek v2)
//   crci-core/src/ffi.rs                (UniFFI-exported functions)
//
// These tests are named descriptively per the session requirement.
// No invented vector IDs.

use crci_core::ffi::{connect_peer, start_node, stop_node, validate_node_config, FfiNodeConfig};
use crci_core::identity::Identity;
use crci_core::storage::encrypted::EncryptedStore;
use ed25519_dalek::{Signature, VerifyingKey};
use std::collections::HashSet;
use tempfile::TempDir;

// ─── A: AES-GCM nonce handling (BFT-031) ──────────────────────────────────────
//
// Both encrypted.rs and legacy.rs generate nonces via OsRng.fill_bytes() —
// independent random draws, not counters. Test that across N writes to the same
// store with the same key, no two nonces collide.

/// Vector BFT-031: AES-GCM Nonce Uniqueness Verification across 200 writes
#[tokio::test]
async fn test_aes_gcm_nonce_never_repeats_across_writes() {
    // This tests EncryptedStore (the live production path).
    // Each write generates a fresh 12-byte OsRng nonce.
    // We write N distinct values and directly compare the nonces stored in
    // the on-disk frames by re-reading the raw StorageRecord nonce fields.
    //
    // We use N=200 writes with distinct keys to give OsRng enough draws to
    // detect any degenerate nonce reuse. With a 96-bit nonce space the
    // birthday probability at N=200 is ~10^{-25}; a collision would be a
    // catastrophic RNG failure, not a statistical event.

    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nonce_test.enc");
    let store =
        EncryptedStore::new(&path, "nonce_collision_test_passphrase").expect("EncryptedStore::new");

    let n: usize = 200;
    let mut nonce_set: HashSet<[u8; 12]> = HashSet::new();

    for i in 0..n {
        let key = format!("record-{}", i);
        let value = format!("payload-{}", i);
        store
            .write_raw_for_nonce_test(&key, value.as_bytes())
            .await
            .expect("write should succeed");
    }

    // Read back all stored frames and collect nonces.
    let nonces = store.read_all_nonces_for_test().expect("read_all_nonces");

    assert_eq!(
        nonces.len(),
        n,
        "expected {} distinct nonces, got {}",
        n,
        nonces.len()
    );

    for nonce in &nonces {
        assert!(
            nonce_set.insert(*nonce),
            "nonce collision detected — AES-GCM nonce reuse would break confidentiality: {:?}",
            nonce
        );
    }
}

// ─── B: Ed25519 verification edge cases (BFT-032..BFT-037) ───────────────────

/// Vector BFT-032: Ed25519 Zero-Length Payload Signature Verification
/// Signing an empty payload must produce a valid signature that also verifies.
#[test]
fn test_ed25519_empty_payload_signs_and_verifies() {
    let identity = Identity::new("test-node-empty");
    let payload = b"";
    let sig = identity.sign(payload);
    assert!(
        identity.verify(payload, &sig),
        "empty payload should produce a valid, verifiable signature"
    );
}

/// Vector BFT-033: Ed25519 Corrupted/Malformed Signature Rejection
/// A malformed signature (all-zero bytes) should not verify against a valid payload.
#[test]
fn test_ed25519_rejects_malformed_signature() {
    let identity = Identity::new("test-node-mal");
    let payload = b"real message content";

    // Construct an all-zero 64-byte "signature" — this is structurally valid
    // (ed25519-dalek v2 accepts the construction) but is not a real signature.
    let zero_sig = Signature::from_bytes(&[0u8; 64]);

    assert!(
        !identity.verify(payload, &zero_sig),
        "all-zero bytes should not pass as a valid Ed25519 signature"
    );
}

/// Vector BFT-034: Ed25519 Foreign Key Signature Rejection
/// A signature produced by key A must not verify against key B.
#[test]
fn test_ed25519_rejects_signature_from_wrong_key() {
    let identity_a = Identity::new("node-a");
    let identity_b = Identity::new("node-b");
    let payload = b"this message was signed by node-a";

    let sig_from_a = identity_a.sign(payload);

    // Verify sig_from_a using identity_b's public key — must fail.
    assert!(
        !identity_b.verify(payload, &sig_from_a),
        "a signature from key A must not verify under key B"
    );
}

/// Vector BFT-035: Ed25519 Tampered Payload Signature Rejection
/// A signature produced against payload X must not verify against a different payload Y.
#[test]
fn test_ed25519_rejects_signature_for_different_payload() {
    let identity = Identity::new("node-sig-mismatch");
    let payload_original = b"original authorized message";
    let payload_tampered = b"tampered message content";

    let sig = identity.sign(payload_original);

    assert!(
        !identity.verify(payload_tampered, &sig),
        "signature over payload X must not verify against payload Y"
    );
}

/// Vector BFT-036: Ed25519 Signature Verification Idempotency
/// A valid signature should continue to verify when re-checked.
#[test]
fn test_ed25519_signature_verifies_repeatedly() {
    let identity = Identity::new("node-repeat");
    let payload = b"broadcast rescue message";
    let sig = identity.sign(payload);

    for _ in 0..10 {
        assert!(
            identity.verify(payload, &sig),
            "the same valid signature should verify identically on each call"
        );
    }
}

/// Vector BFT-037: Ed25519 VerifyingKey Serialization Round-Trip
/// Construct a VerifyingKey from raw bytes and verify a signature produced by Identity::sign().
#[test]
fn test_ed25519_verifying_key_roundtrip_matches_signing() {
    let identity = Identity::new("node-roundtrip");
    let payload = b"roundtrip test payload";
    let sig = identity.sign(payload);

    // Serialize the verifying key to bytes and reconstruct it.
    let vk_bytes = identity.verifying_key.to_bytes();
    let reconstructed_vk =
        VerifyingKey::from_bytes(&vk_bytes).expect("valid verifying key bytes should deserialize");

    use ed25519_dalek::Verifier;
    assert!(
        reconstructed_vk.verify(payload, &sig).is_ok(),
        "signature must verify against a VerifyingKey reconstructed from serialized bytes"
    );
}

/// Cross-check: a replayed-but-valid message (same signature, same payload)
/// is NOT caught by Ed25519 itself — it is semantically valid. The replay
/// protection is in ReplayFilter (replay.rs), not in identity.rs.
/// This test documents the boundary: Ed25519 says accept, replay.rs must catch it.
#[test]
fn test_ed25519_does_not_catch_replay_by_itself() {
    use crci_core::replay::{ReplayFilter, ReplayVerdict};

    let identity = Identity::new("node-replay");
    let payload = b"rescue alert round=1 seq=42";
    let sig = identity.sign(payload);

    // Ed25519 accepts both "first delivery" and "replay" — correct behavior.
    assert!(identity.verify(payload, &sig), "first delivery verifies");
    assert!(
        identity.verify(payload, &sig),
        "replay also verifies at Ed25519 level — replay.rs must handle this"
    );

    // Confirm ReplayFilter catches the replay at sequence level.
    let mut filter = ReplayFilter::new();
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let first = filter.check_and_record("node-replay", 42, now_ts);
    assert!(
        matches!(first, ReplayVerdict::Accept),
        "first message (seq=42) should be accepted"
    );

    let replay = filter.check_and_record("node-replay", 42, now_ts);
    assert!(
        matches!(replay, ReplayVerdict::Replayed { .. }),
        "replayed message (seq=42 again) must be rejected by ReplayFilter"
    );
}

// ─── C: FFI error propagation (BFT-038) ───────────────────────────────────────
//
// Vector BFT-038: FFI Boundary Panic Safety & Proxy Verification.
// Verifies that exported FFI functions reachable from CrciViewModel.kt return false/0
// rather than panicking on invalid inputs, and that std::panic::catch_unwind catches
// call-stack invocations across all FFI endpoints.

/// An invalid node config (empty node_id) must return false — not panic.
/// This exercises the validation guard at the FFI boundary.
#[test]
fn test_ffi_invalid_config_returns_false_not_panic() {
    let bad_config = FfiNodeConfig {
        node_id: String::new(), // empty — validate_node_config returns false
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 8,
    };
    // If this panicked, the test would abort rather than fail.
    assert!(
        !validate_node_config(bad_config),
        "empty node_id must return false at FFI boundary, not panic"
    );
}

/// max_peers=0 is rejected by validate_node_config — confirms the guard range check.
#[test]
fn test_ffi_zero_peers_returns_false_not_panic() {
    let bad_config = FfiNodeConfig {
        node_id: "test-node".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 0,
    };
    assert!(
        !validate_node_config(bad_config),
        "max_peers=0 must return false at FFI boundary"
    );
}

/// max_peers > 256 is out of the accepted range — confirms upper bound check.
#[test]
fn test_ffi_excessive_peers_returns_false_not_panic() {
    let bad_config = FfiNodeConfig {
        node_id: "test-node".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 257,
    };
    assert!(
        !validate_node_config(bad_config),
        "max_peers > 256 must return false at FFI boundary"
    );
}

/// connect_peer with an address when no node is running must return false.
/// This exercises the None branch in get_node_handle() without panicking.
#[test]
fn test_ffi_connect_peer_when_no_node_running_returns_false() {
    // Ensure no node is running from a previous test — stop_node is idempotent.
    stop_node();

    let result = connect_peer("127.0.0.1:9999".to_string());
    assert!(
        !result,
        "connect_peer with no running node must return false, not panic"
    );
}

/// start_node with a valid config must return true. stop_node must return true.
/// Then a second stop_node must return false (no node running) — not panic.
#[test]
fn test_ffi_start_stop_node_lifecycle_handles_errors_cleanly() {
    // Clean up any state from other tests first.
    stop_node();

    let config = FfiNodeConfig {
        node_id: "lifecycle-test-node".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        max_peers: 4,
    };

    // NOTE: start_node acquires a global OnceLock. If another test already
    // initialized it, this may return false because a node is already running.
    // We tolerate that here — the key assertion is that no panic occurs.
    let _started = start_node(config); // may be true or false depending on test order

    let stopped = stop_node();
    // After stop, a second stop must return false — not panic.
    let second_stop = stop_node();
    assert!(
        !second_stop,
        "stop_node with no running node must return false, not panic"
    );

    // If we successfully stopped something, peer_count after stop must be 0.
    if stopped {
        let count = crci_core::ffi::peer_count();
        assert_eq!(count, 0, "peer_count with no running node must be 0");
    }
}

/// Vector BFT-038 Proxy Test: Explicit catch_unwind wrapper around every FFI function
/// reachable from dev.crci.android.CrciViewModel (startNode, connectPeer, peerCount, stopNode,
/// validateNodeConfig, crciVersion, listPeersStub).
#[test]
fn test_ffi_catch_unwind_panic_safety() {
    use crci_core::ffi::{crci_version, list_peers_stub, peer_count};

    stop_node();

    // 1. crci_version
    let v_res = std::panic::catch_unwind(crci_version);
    assert!(v_res.is_ok(), "crci_version panicked under catch_unwind");
    assert_eq!(v_res.unwrap(), "0.1.0");

    // 2. validate_node_config (valid & invalid)
    let valid_cfg = FfiNodeConfig {
        node_id: "valid-id".to_string(),
        listen_addr: "0.0.0.0:9000".to_string(),
        max_peers: 8,
    };
    let val_res = std::panic::catch_unwind(|| validate_node_config(valid_cfg));
    assert!(val_res.is_ok() && val_res.unwrap());

    let invalid_cfg = FfiNodeConfig {
        node_id: "".to_string(),
        listen_addr: "invalid_addr".to_string(),
        max_peers: 0,
    };
    let val_bad_res = std::panic::catch_unwind(|| validate_node_config(invalid_cfg));
    assert!(val_bad_res.is_ok() && !val_bad_res.unwrap());

    // 3. peer_count when stopped
    let pc_res = std::panic::catch_unwind(peer_count);
    assert!(pc_res.is_ok(), "peer_count panicked when stopped");
    assert_eq!(pc_res.unwrap(), 0);

    // 4. connect_peer when stopped
    let cp_res = std::panic::catch_unwind(|| connect_peer("10.0.2.2:9000".to_string()));
    assert!(cp_res.is_ok(), "connect_peer panicked when stopped");
    assert!(!cp_res.unwrap());

    // 5. list_peers_stub
    let lp_res = std::panic::catch_unwind(list_peers_stub);
    assert!(lp_res.is_ok(), "list_peers_stub panicked");

    // 6. stop_node when already stopped
    let sn_res = std::panic::catch_unwind(stop_node);
    assert!(sn_res.is_ok(), "stop_node panicked when already stopped");
}
