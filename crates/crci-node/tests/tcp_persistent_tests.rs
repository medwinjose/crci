//! Persistent authenticated TCP connections.
//!
//! A node keeps one authenticated connection per peer and reuses it for every frame, so a
//! relayed message costs one frame, not a handshake. Sends to different peers do not block
//! each other, and a connection that dropped is re-established before the next frame.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::{TcpTransport, Transport};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;

fn signed(identity: &Identity, id: &str, origin: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

async fn bind_at(node_id: &str, addr: SocketAddr) -> TcpTransport {
    TcpTransport::bind(
        node_id.to_string(),
        addr,
        Identity::new(node_id).signing_key,
    )
    .await
    .unwrap()
}

async fn listener(node_id: &str) -> TcpTransport {
    bind_at(node_id, "127.0.0.1:0".parse().unwrap()).await
}

async fn receive_within(tcp: &TcpTransport, secs: u64) -> (String, WireMessage) {
    tokio::time::timeout(Duration::from_secs(secs), tcp.receive())
        .await
        .expect("timed out waiting for a frame")
        .expect("receive failed")
}

/// An address nothing listens on.
fn unused_addr() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

#[tokio::test]
async fn many_messages_to_one_peer_use_one_handshake() {
    let receiver = listener("receiver").await;
    let sender = listener("sender").await;
    let id = Identity::new("author");
    let target = receiver.listen_addr.to_string();

    for seq in 1..=10u64 {
        sender
            .send(&target, &signed(&id, &format!("m-{seq}"), "author", seq))
            .await
            .unwrap();
    }
    for _ in 0..10 {
        receive_within(&receiver, 2).await;
    }

    assert_eq!(
        receiver.accepted_count.load(Ordering::SeqCst),
        1,
        "ten frames must share one authenticated connection"
    );
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_stalled_peer_does_not_delay_delivery_to_another_peer() {
    // Accepts TCP connections and never speaks: a handshake with it waits for the timeout.
    let stalled = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stalled_addr = stalled.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = stalled.accept().await {
            held.push(stream);
        }
    });

    let good = listener("good").await;
    let sender = listener("sender").await;
    let id = Identity::new("author");

    sender
        .relay(&stalled_addr, signed(&id, "to-stalled", "author", 1))
        .unwrap();
    sender
        .relay(
            &good.listen_addr.to_string(),
            signed(&id, "to-good", "author", 2),
        )
        .unwrap();

    let (_, delivered) = receive_within(&good, 1).await;
    assert_eq!(delivered.id, "to-good");
}

#[tokio::test]
async fn frames_to_one_peer_stay_in_order_when_relayed() {
    let receiver = listener("receiver").await;
    let sender = listener("sender").await;
    let id = Identity::new("author");
    let target = receiver.listen_addr.to_string();

    for seq in 1..=20u64 {
        sender
            .relay(&target, signed(&id, &format!("m-{seq}"), "author", seq))
            .unwrap();
    }
    for seq in 1..=20u64 {
        let (_, wire) = receive_within(&receiver, 2).await;
        assert_eq!(wire.id, format!("m-{seq}"));
    }
    assert_eq!(receiver.accepted_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_peer_that_starts_later_receives_later_frames() {
    let addr = unused_addr();
    let sender = listener("sender").await;
    let id = Identity::new("author");

    // Nothing listens yet: the frame cannot be delivered.
    let early_msg = signed(&id, "early", "author", 1);
    assert!(sender.send(&addr.to_string(), &early_msg).await.is_err());

    let receiver = bind_at("receiver", addr).await;
    sender
        .send(&addr.to_string(), &signed(&id, "late", "author", 2))
        .await
        .unwrap();
    let (_, wire) = receive_within(&receiver, 2).await;
    assert_eq!(wire.id, "late");
}

#[tokio::test]
async fn reconnects_after_the_listener_restarts() {
    let first = listener("receiver").await;
    let addr = first.listen_addr;
    let sender = listener("sender").await;
    let id = Identity::new("author");

    sender
        .send(&addr.to_string(), &signed(&id, "before", "author", 1))
        .await
        .unwrap();
    assert_eq!(receive_within(&first, 2).await.1.id, "before");

    drop(first);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let second = bind_at("receiver", addr).await;

    sender
        .send(&addr.to_string(), &signed(&id, "after", "author", 2))
        .await
        .unwrap();
    assert_eq!(receive_within(&second, 2).await.1.id, "after");
}

#[tokio::test]
async fn an_idle_connection_outlives_the_read_timeout() {
    let receiver = listener("receiver").await;
    let sender = listener("sender").await;
    let id = Identity::new("author");
    let target = receiver.listen_addr.to_string();

    sender
        .send(&target, &signed(&id, "one", "author", 1))
        .await
        .unwrap();
    receive_within(&receiver, 2).await;

    // Longer than the 5 s limit for reading a frame that has started.
    tokio::time::sleep(Duration::from_secs(7)).await;

    sender
        .send(&target, &signed(&id, "two", "author", 2))
        .await
        .unwrap();
    receive_within(&receiver, 2).await;
    assert_eq!(
        receiver.accepted_count.load(Ordering::SeqCst),
        1,
        "the idle connection must not have been closed and re-dialled"
    );
}
