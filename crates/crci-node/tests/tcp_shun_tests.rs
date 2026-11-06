//! A banned peer's connection is closed and its frames are no longer read.
//!
//! `TcpTransport::shun_peer` is called by a node when it bans an authenticated identity. From
//! then on the listener closes that identity's connections without reading or verifying their
//! frames, and refuses its new connections, except for one probe per interval: the first frame
//! after the interval is delivered so the node can tell whether the peer was readmitted.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::{TcpTransport, Transport};
use std::sync::atomic::Ordering;
use std::time::Duration;

fn signed(identity: &Identity, id: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        &identity.id,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

async fn listener(node_id: &str) -> TcpTransport {
    TcpTransport::bind(
        node_id.to_string(),
        "127.0.0.1:0".parse().unwrap(),
        Identity::new(node_id).signing_key,
    )
    .await
    .unwrap()
}

async fn next_frame(tcp: &TcpTransport, millis: u64) -> Option<(String, WireMessage)> {
    tokio::time::timeout(Duration::from_millis(millis), tcp.receive())
        .await
        .ok()
        .and_then(|r| r.ok())
}

#[tokio::test]
async fn a_shunned_peer_delivers_nothing_and_cannot_reconnect_its_way_back() {
    let receiver = listener("receiver").await;
    let sender = listener("sender").await;
    let author = Identity::new("author");
    let target = receiver.listen_addr.to_string();

    sender.relay(&target, signed(&author, "m-1", 1)).unwrap();
    let (peer, _) = next_frame(&receiver, 3000)
        .await
        .expect("the first frame is delivered");

    receiver.shun_peer(&peer);
    for seq in 2..=20u64 {
        // A frame sent after the shun is never delivered, on the old connection or a new one.
        let _ = sender.relay(&target, signed(&author, &format!("m-{seq}"), seq));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        next_frame(&receiver, 500).await.is_none(),
        "a shunned peer's frames must not be delivered"
    );
    assert_eq!(
        receiver.auth_failures.load(Ordering::SeqCst),
        0,
        "shunning is not an authentication failure"
    );
}

#[tokio::test]
async fn a_shunned_peer_gets_one_probe_frame_after_the_interval() {
    let receiver = listener("receiver").await;
    let sender = listener("sender").await;
    let author = Identity::new("author");
    let target = receiver.listen_addr.to_string();

    sender.relay(&target, signed(&author, "m-1", 1)).unwrap();
    let (peer, _) = next_frame(&receiver, 3000).await.expect("first frame");

    receiver.shun_peer_for(&peer, Duration::from_millis(1500));
    let _ = sender.relay(&target, signed(&author, "m-2", 2));
    assert!(
        next_frame(&receiver, 600).await.is_none(),
        "no frame inside the interval"
    );

    tokio::time::sleep(Duration::from_millis(1200)).await;
    // The interval has passed: the next connection may deliver (a probe).
    let mut delivered = false;
    for seq in 3..=12u64 {
        let _ = sender.relay(&target, signed(&author, &format!("m-{seq}"), seq));
        if next_frame(&receiver, 400).await.is_some() {
            delivered = true;
            break;
        }
    }
    assert!(delivered, "after the interval a frame must get through");
}
