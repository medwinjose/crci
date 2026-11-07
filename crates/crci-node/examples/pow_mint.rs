//! Cost of minting a proof-of-work identity on one CPU core.
//!
//! The candidate scheme: a node mints `nonce` such that SHA-256("crci-pow-v1" || public key || nonce)
//! has at least `d` leading zero bits. Expected work is 2^d hashes, so the measured time per
//! difficulty is a geometric random variable: each row reports min / median / mean / max over
//! `trials` fresh keys, and the single-core hash rate. This measures the cost only; the protocol
//! does not use the scheme.
//!
//!   cargo run --release -p crci-node --example pow_mint

use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::Instant;

const DOMAIN: &[u8] = b"crci-pow-v1";

fn leading_zero_bits(h: &[u8]) -> u32 {
    let mut n = 0;
    for b in h {
        if *b == 0 {
            n += 8;
        } else {
            n += b.leading_zeros();
            break;
        }
    }
    n
}

/// Returns (nonce, hashes tried).
fn mint(key: &[u8; 32], d: u32) -> (u64, u64) {
    let mut base = Sha256::new();
    base.update(DOMAIN);
    base.update(key);
    let mut nonce = 0u64;
    loop {
        let mut h = base.clone();
        h.update(nonce.to_le_bytes());
        if leading_zero_bits(&h.finalize()) >= d {
            return (nonce, nonce + 1);
        }
        nonce += 1;
    }
}

fn verify(key: &[u8; 32], nonce: u64, d: u32) -> bool {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update(key);
    h.update(nonce.to_le_bytes());
    leading_zero_bits(&h.finalize()) >= d
}

fn main() {
    let plan: &[(u32, usize)] = &[
        (8, 200),
        (12, 200),
        (14, 200),
        (16, 100),
        (18, 50),
        (20, 20),
        (22, 10),
        (24, 5),
        (26, 3),
    ];
    let mut rng = rand::thread_rng();
    println!("difficulty_bits,trials,expected_hashes,min_ms,median_ms,mean_ms,max_ms,mean_hashes,hashes_per_s");
    for &(d, trials) in plan {
        let mut ms = Vec::new();
        let mut hashes = 0u64;
        for _ in 0..trials {
            let mut key = [0u8; 32];
            rng.fill_bytes(&mut key);
            let t = Instant::now();
            let (nonce, n) = mint(&key, d);
            ms.push(t.elapsed().as_secs_f64() * 1e3);
            hashes += n;
            assert!(verify(&key, nonce, d), "minted nonce must verify");
        }
        ms.sort_by(|a, b| a.total_cmp(b));
        let total_ms: f64 = ms.iter().sum();
        println!(
            "{d},{trials},{},{:.4},{:.4},{:.4},{:.4},{:.0},{:.0}",
            1u64 << d,
            ms[0],
            ms[ms.len() / 2],
            total_ms / trials as f64,
            ms[ms.len() - 1],
            hashes as f64 / trials as f64,
            hashes as f64 / (total_ms / 1e3)
        );
    }
}
