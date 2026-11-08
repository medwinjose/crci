//! Arbitrary bytes into every decoder a remote peer reaches, then into a node's inbox.
#![no_main]

use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::tcp::{
    frame_signing_bytes, handshake_transcript, ClientAuth, ServerAuth, ServerHello, TcpFrame,
};
use crci_core::transport::SharedInbox;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fuzz_target!(|data: &[u8]| {
    if let (Ok(hello), Ok(auth)) = (
        serde_json::from_slice::<ServerHello>(data),
        serde_json::from_slice::<ClientAuth>(data),
    ) {
        let _ = handshake_transcript(&hello, &auth);
    }
    let _ = serde_json::from_slice::<ServerAuth>(data);
    if let Ok(frame) = serde_json::from_slice::<TcpFrame>(data) {
        let _ = frame_signing_bytes(&[0u8; 32], frame.counter, &frame.message);
    }

    let Ok(wire) = serde_json::from_slice::<WireMessage>(data) else {
        return;
    };
    // The signed bytes must not depend on a round trip through the wire format.
    let before = wire.signing_bytes();
    if let Ok(again) = serde_json::to_vec(&wire) {
        if let Ok(back) = serde_json::from_slice::<WireMessage>(&again) {
            assert_eq!(
                before,
                back.signing_bytes(),
                "signing bytes changed on re-encode"
            );
        }
    }

    // Whatever decodes goes through the real receive path as a neighbour's frame.
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone", inbox.clone());
    node.use_router = false;
    if let Ok(mut guard) = inbox.lock() {
        guard
            .entry(node.id.clone())
            .or_default()
            .push(("peer#00".to_string(), data.to_vec()));
    }
    let _ = node.process_inbox_delivered();
});
