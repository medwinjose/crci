//! Ed25519 cost of the operations the protocol performs per message: key generation,
//! signing, and verification of a valid and an invalid signature.
//!
//! The message-level benchmarks use the same `WireMessage` signing and verification
//! paths as the runtime (domain-separated canonical bytes, `verify_strict`).
use crci_core::identity::Identity;
use crci_core::message::{Message, Signal};
use crci_core::runtime::WireMessage;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};

fn signed_wire(identity: &Identity) -> WireMessage {
    let mut msg = Message::new("msg-bench", "node-bench", Signal::panic(), None);
    msg.seq = 1;
    WireMessage::from_message(&msg, identity)
}

fn bench_crypto(c: &mut Criterion) {
    let identity = Identity::new("node-bench");
    let valid = signed_wire(&identity);

    // Same message with the lowest bit of S flipped (byte 32): R still decompresses and S stays
    // canonical, so verification runs in full and fails at the final check. Flipping a bit of R
    // (bytes 0-31) instead fails point decompression in about half of all keys, about 5x cheaper.
    let mut invalid = valid.clone();
    if let Some(byte) = invalid.signature_bytes.get_mut(32) {
        *byte ^= 0x01;
    }
    assert!(
        !invalid.verify_signature(),
        "the altered signature must not verify"
    );

    let mut group = c.benchmark_group("ed25519");
    group.throughput(Throughput::Elements(1));

    group.bench_function("keygen", |b| {
        b.iter(|| black_box(Identity::new("node-bench")))
    });

    group.bench_function("sign_wire_message", |b| {
        b.iter_batched(
            || valid.clone(),
            |mut wire| {
                wire.sign_with(&identity);
                black_box(wire)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("verify_valid", |b| {
        b.iter(|| black_box(black_box(&valid).verify_signature()))
    });

    group.bench_function("verify_invalid", |b| {
        b.iter(|| black_box(black_box(&invalid).verify_signature()))
    });

    // Cost of building the signed bytes alone, to separate encoding from curve work.
    group.bench_function("signing_bytes_only", |b| {
        b.iter(|| black_box(black_box(&valid).signing_bytes()))
    });

    group.finish();
}

criterion_group!(benches, bench_crypto);
criterion_main!(benches);
