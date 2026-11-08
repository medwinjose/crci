//! A whole `NodeRuntime` driven by signed, tampered, repeated and junk frames from several
//! neighbours while its clock moves. Checked after every step:
//!   - every table stays inside its limit (with a small origin limit, so forgetting runs),
//!   - everything delivered carries a valid origin signature,
//!   - a neighbour that only ever sent each message once, untampered, is never banned.
#![no_main]

use arbitrary::Arbitrary;
use crci_core::identity::Identity;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{
    NodeRuntime, WireMessage, OBSERVATIONS_MAX, PERSISTENT_MAX, SEEN_MAX, SYNC_REQUEST_TYPE,
};
use crci_core::transport::SharedInbox;
use ed25519_dalek::{Signature, SigningKey};
use libfuzzer_sys::fuzz_target;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ORIGINS: u8 = 12;
const SENDERS: u8 = 6;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Kind {
    Normal,
    Rescue,
    Control,
}

#[derive(Arbitrary, Debug)]
enum Op {
    /// A well-formed message from `origin`, relayed by `sender`.
    Send {
        origin: u8,
        seq: u8,
        sender: u8,
        kind: Kind,
    },
    /// The same, with the signature corrupted.
    Tamper {
        origin: u8,
        seq: u8,
        sender: u8,
    },
    /// Junk bytes.
    Raw {
        sender: u8,
        bytes: Vec<u8>,
    },
    Advance(u16),
}

#[derive(Arbitrary, Debug)]
struct Input {
    max_origins: u8,
    ops: Vec<Op>,
}

fn identity(origin: u8) -> Identity {
    let signing_key = SigningKey::from_bytes(&[origin.wrapping_add(1); 32]);
    Identity::from_signing_key(&format!("o{origin}"), signing_key)
}

fn wire(origin: u8, seq: u8, kind: Kind) -> WireMessage {
    let mut m = Message::new(
        &format!("m{origin}-{seq}-{}", kind as u8),
        &format!("o{origin}"),
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    m.seq = seq as u64 + 1;
    if matches!(kind, Kind::Rescue) {
        m.message_type = MessageType::RescueRequest;
    }
    let id = identity(origin);
    let mut w = WireMessage::from_message(&m, &id);
    if matches!(kind, Kind::Control) {
        w.message_type = SYNC_REQUEST_TYPE.to_string();
        w.sign_with(&id);
    }
    w
}

fuzz_target!(|input: Input| {
    let max_origins = 2 + (input.max_origins % 15) as usize;
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.use_router = false;
    node.max_tracked_origins = max_origins;

    // Neighbours that sent something other than distinct, untampered messages.
    let mut misbehaved: HashSet<u8> = HashSet::new();
    let mut sent: HashSet<(u8, u8, u8, u8)> = HashSet::new();

    for op in input.ops.iter().take(2048) {
        let (sender, bytes) = match op {
            Op::Advance(ms) => {
                node.sybil_guard
                    .advance_time(Duration::from_millis(*ms as u64 * 10));
                continue;
            }
            Op::Send {
                origin,
                seq,
                sender,
                kind,
            } => {
                let (origin, sender) = (origin % ORIGINS, sender % SENDERS);
                if !sent.insert((origin, *seq, sender, *kind as u8)) {
                    misbehaved.insert(sender);
                }
                (sender, serde_json::to_vec(&wire(origin, *seq, *kind)))
            }
            Op::Tamper {
                origin,
                seq,
                sender,
            } => {
                let (origin, sender) = (origin % ORIGINS, sender % SENDERS);
                misbehaved.insert(sender);
                let mut w = wire(origin, *seq, Kind::Normal);
                if let Some(b) = w.signature_bytes.first_mut() {
                    *b ^= 0x01;
                }
                (sender, serde_json::to_vec(&w))
            }
            Op::Raw { sender, bytes } => {
                let sender = sender % SENDERS;
                misbehaved.insert(sender);
                (sender, Ok(bytes.clone()))
            }
        };
        let Ok(bytes) = bytes else { continue };
        if let Ok(mut guard) = inbox.lock() {
            guard
                .entry(node.id.clone())
                .or_default()
                .push((format!("s{sender}"), bytes));
        }
        node.sybil_guard.advance_time(Duration::from_millis(250));

        for d in node.process_inbox_delivered() {
            let ok = (|| {
                let key = ed25519_dalek::VerifyingKey::from_bytes(
                    &<[u8; 32]>::try_from(d.origin_pubkey.as_slice()).ok()?,
                )
                .ok()?;
                let sig = Signature::from_bytes(
                    &<[u8; 64]>::try_from(d.signature_bytes.as_slice()).ok()?,
                );
                key.verify_strict(&d.signing_bytes(), &sig).ok()
            })();
            assert!(
                ok.is_some(),
                "delivered a message without a valid signature: {}",
                d.id
            );
        }

        assert!(
            node.seen_messages.len() <= SEEN_MAX,
            "seen cache over its limit"
        );
        assert!(
            node.persistent_messages.len() <= PERSISTENT_MAX,
            "rescue store over its limit"
        );
        assert!(
            node.observations.len() <= OBSERVATIONS_MAX,
            "observations over their limit"
        );
        assert!(
            node.origin_budgets.len() <= max_origins,
            "origin budgets over the origin limit"
        );
        assert!(
            node.replay_filter.tracked_origins() <= max_origins,
            "replay windows over the origin limit"
        );
        for s in 0..SENDERS {
            if !misbehaved.contains(&s) {
                assert!(
                    !node.sybil_guard.is_banned(&format!("s{s}")),
                    "honest neighbour s{s} banned"
                );
            }
        }
    }
});
