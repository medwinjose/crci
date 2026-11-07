//! Serialized size of the protocol's real messages, as they go on the TCP wire today
//! (length-prefixed JSON) and in the compact canonical form that is signed.
//!
//! Keys and signatures are byte vectors, which JSON writes as arrays of decimal numbers, so the
//! JSON size depends on the byte values: each row is measured over `TRIALS` fresh identities
//! and reports min / mean / max. The canonical (signed) size does not vary and is asserted equal
//! across trials.
//!
//!   cargo run --release -p crci-node --example envelope_size

use crci_core::identity::Identity;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{OriginDigest, SyncDigest, WireMessage};
use crci_core::transport::tcp::sign_frame;

const TRIALS: usize = 1000;

fn gossip(note: Option<&str>) -> WireMessage {
    let mut msg = Message::new(
        "m1",
        "n1",
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        note,
    );
    msg.message_type = MessageType::RescueRequest;
    msg.seq = 1;
    WireMessage::from_message(&msg, &Identity::new("n1"))
}

fn sync_request(origins: usize, held: usize) -> WireMessage {
    let mut wire = gossip(None);
    wire.message_type = "sync_request".to_string();
    wire.sync = Some(SyncDigest {
        reply_to: Some("10.0.0.1:7001".to_string()),
        origins: (0..origins)
            .map(|i| OriginDigest {
                origin: format!("node{i:03}"),
                seq_max: 100,
                held: (0..held as u64).map(|s| 100 - s).collect(),
            })
            .collect(),
    });
    wire.sign_with(&Identity::new("n1"));
    wire
}

struct Stat(usize, usize, usize);

impl Stat {
    fn new() -> Self {
        Stat(usize::MAX, 0, 0)
    }
    fn add(&mut self, v: usize) {
        self.0 = self.0.min(v);
        self.1 += v;
        self.2 = self.2.max(v);
    }
    fn csv(&self) -> String {
        format!("{},{:.1},{}", self.0, self.1 as f64 / TRIALS as f64, self.2)
    }
}

fn row(name: &str, make: impl Fn() -> WireMessage) {
    let (mut json, mut frame) = (Stat::new(), Stat::new());
    let mut signing = None;
    let relay = Identity::new("relay");
    for _ in 0..TRIALS {
        let wire = make();
        let j = serde_json::to_vec(&wire).map(|v| v.len()).unwrap_or(0);
        let f = sign_frame(&relay.signing_key, &[7u8; 32], 0, &wire);
        // + 4: the length prefix the TCP transport writes before every JSON value.
        let f = serde_json::to_vec(&f).map(|v| v.len()).unwrap_or(0) + 4;
        assert!(f > j, "{name}: frame not larger than message");
        json.add(j);
        frame.add(f);
        let s = wire.signing_bytes().len();
        assert_eq!(
            *signing.get_or_insert(s),
            s,
            "{name}: canonical size varies"
        );
    }
    let s = signing.unwrap_or(0);
    println!("{name},{},{},{s},{}", json.csv(), frame.csv(), s + 64);
}

fn main() {
    println!(
        "variant,json_min,json_mean,json_max,frame_min,frame_mean,frame_max,canonical,canonical_plus_sig"
    );
    row("gossip_no_note", || gossip(None));
    row("gossip_note_20", || gossip(Some(&"x".repeat(20))));
    row("gossip_note_100", || gossip(Some(&"x".repeat(100))));
    row("sync_request_1_origin_0_held", || sync_request(1, 0));
    row("sync_request_1_origin_8_held", || sync_request(1, 8));
    row("sync_request_5_origins_16_held", || sync_request(5, 16));
}
