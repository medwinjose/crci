use crci_core::sybil::{PowChallenge, SybilError, SybilGuard, TokenBucket};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[test]
fn test_sybil_new_peer_starts_at_half_reputation() {
    let mut guard = SybilGuard::new(4);
    let peer = "node_a".to_string();

    assert!(guard.check(&peer).is_ok());
    assert!((guard.reputation(&peer) - 0.5).abs() < 1e-6);
}

#[test]
fn test_sybil_ban_after_repeated_violations() {
    let mut guard = SybilGuard::new(4);
    let peer = "node_b".to_string();

    for _ in 0..7 {
        guard.report_violation(&peer);
    }

    assert!(guard.reputation(&peer) < 0.1);

    let result = guard.check(&peer);
    assert!(matches!(result, Err(SybilError::Banned(_))));
}

#[test]
fn test_sybil_rate_limit_triggers_on_burst() {
    let mut bucket = TokenBucket::new(3.0, 1.0);

    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(!bucket.try_consume());
}

#[test]
fn test_sybil_token_bucket_refills_over_time() {
    let mut bucket = TokenBucket::new(2.0, 10.0);

    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(!bucket.try_consume());

    std::thread::sleep(Duration::from_millis(200)); // Should refill ~2.0 tokens

    assert!(bucket.try_consume());
}

fn brute_force_nonce(node_id: &str, difficulty: u8) -> u64 {
    let mut nonce: u64 = 0;
    loop {
        let mut hasher = Sha256::new();
        hasher.update(node_id.as_bytes());
        hasher.update(nonce.to_le_bytes());
        let hash = hasher.finalize();

        let mut diff_remaining = difficulty as usize;
        let mut valid = true;
        for &byte in hash.iter() {
            if diff_remaining >= 8 {
                if byte != 0 {
                    valid = false;
                    break;
                }
                diff_remaining -= 8;
            } else if diff_remaining > 0 {
                let shift = 8 - diff_remaining;
                let mask = 0xFF << shift;
                if (byte & mask as u8) != 0 {
                    valid = false;
                    break;
                }
                diff_remaining = 0;
            } else {
                break;
            }
        }

        if valid {
            return nonce;
        }
        nonce += 1;
    }
}

#[test]
fn test_sybil_pow_verify_correct_nonce() {
    let peer = "node_pow".to_string();
    let difficulty = 4;
    let nonce = brute_force_nonce(&peer, difficulty);

    let guard = SybilGuard::new(difficulty);
    let challenge = PowChallenge {
        node_id: peer,
        nonce,
        difficulty,
    };

    assert!(guard.verify_pow(&challenge).is_ok());
}

#[test]
fn test_sybil_pow_verify_wrong_nonce_fails() {
    let peer = "node_pow_fail".to_string();
    let difficulty = 4;

    // Nonce 0 is highly unlikely to satisfy diff 4 (1 in 16 chance).
    // We can just find one that explicitly fails to be sure.
    let mut nonce: u64 = 0;
    loop {
        let challenge = PowChallenge {
            node_id: peer.clone(),
            nonce,
            difficulty,
        };
        if !challenge.verify() {
            break;
        }
        nonce += 1;
    }

    let guard = SybilGuard::new(difficulty);
    let challenge = PowChallenge {
        node_id: peer,
        nonce,
        difficulty,
    };

    assert!(matches!(
        guard.verify_pow(&challenge),
        Err(SybilError::PowFailed(_))
    ));
}

#[test]
fn test_sybil_report_violation_then_check_propagates_penalty() {
    let mut guard = SybilGuard::new(4);
    let peer = "node_c".to_string();

    assert!(guard.check(&peer).is_ok());

    for _ in 0..6 {
        guard.report_violation(&peer);
    }

    // 0.5 - (6 * 0.15) = -0.4 -> clamped to 0.0 -> < 0.1 -> banned
    assert!(matches!(guard.check(&peer), Err(SybilError::Banned(_))));
}
