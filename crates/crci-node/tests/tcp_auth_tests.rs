//! Transport-peer authentication on TCP.
//!
//! Every connection starts with a mutual Ed25519 challenge-response; the listener
//! attributes every frame to the dialer's authenticated identity (declared node ID plus
//! the key it proved), never to a claimed ID or to a message's origin. Frames are signed
//! by the dialer for the session, so they cannot be injected, altered or replayed.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::tcp::{
    handshake_transcript, peer_identity, read_json, send_authenticated, session_id, sign_frame,
    sign_handshake, write_json, ClientAuth, HandshakeRole, ServerAuth, ServerHello, TcpFrame,
    NONCE_LEN,
};
use crci_core::transport::{SharedInbox, TcpTransport, Transport, TransportError};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

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

async fn listener(node_id: &str) -> TcpTransport {
    TcpTransport::bind(
        node_id.to_string(),
        "127.0.0.1:0".parse().unwrap(),
        Identity::new(node_id).signing_key,
    )
    .await
    .unwrap()
}

/// Receive `n` messages, failing if any of them takes longer than two seconds.
async fn receive_n(tcp: &TcpTransport, n: usize) -> Vec<(String, WireMessage)> {
    let mut out = Vec::new();
    for _ in 0..n {
        let item = tokio::time::timeout(Duration::from_secs(2), tcp.receive())
            .await
            .expect("timed out waiting for a frame")
            .expect("receive failed");
        out.push(item);
    }
    out
}

/// Assert that nothing is delivered within half a second.
async fn assert_nothing_delivered(tcp: &TcpTransport) {
    let res = tokio::time::timeout(Duration::from_millis(500), tcp.receive()).await;
    assert!(
        res.is_err(),
        "a frame was delivered: {:?}",
        res.map(|r| r.ok())
    );
}

/// Feed `(sender, message)` pairs into a fresh `NodeRuntime`, one inbox pass each.
fn run_through_runtime(deliveries: &[(String, WireMessage)]) -> NodeRuntime {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone-a", inbox.clone());
    for (sender, wire) in deliveries {
        inbox
            .lock()
            .unwrap()
            .entry("receiver".to_string())
            .or_default()
            .push((sender.clone(), serde_json::to_vec(wire).unwrap()));
        node.process_inbox();
    }
    node
}

/// Dialer side of a valid handshake done by hand. Returns the open stream and the
/// session ID, so a test can then write frames of its choosing.
async fn manual_handshake(
    addr: std::net::SocketAddr,
    dialer: &Identity,
    node_id: &str,
) -> (TcpStream, [u8; 32]) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let hello: ServerHello = read_json(&mut stream, 4096).await.unwrap();
    let mut auth = ClientAuth {
        node_id: node_id.to_string(),
        public_key: dialer.verifying_key.to_bytes().to_vec(),
        nonce: vec![7u8; NONCE_LEN],
        signature: Vec::new(),
    };
    let transcript = handshake_transcript(&hello, &auth);
    auth.signature = sign_handshake(&dialer.signing_key, HandshakeRole::Dialer, &transcript);
    write_json(&mut stream, &auth).await.unwrap();
    let _reply: ServerAuth = read_json(&mut stream, 4096).await.unwrap();
    (stream, session_id(&transcript))
}

#[tokio::test]
async fn adversary_claiming_an_honest_id_cannot_get_it_penalised() {
    let receiver = listener("receiver").await;
    let addr = receiver.listen_addr.to_string();
    let honest = Identity::new("node-h");
    let adversary = Identity::new("node-h"); // claims the same node ID with its own key

    // The honest node sends one message of its own.
    let honest_msg = signed(&honest, "h-1", "node-h", 1);
    send_authenticated(&addr, "node-h", &honest.signing_key, &[honest_msg])
        .await
        .unwrap();
    // The adversary, declaring "node-h", replays one of its own messages four times.
    let replayed = signed(&adversary, "a-1", "adversary-origin", 1);
    for _ in 0..4 {
        send_authenticated(
            &addr,
            "node-h",
            &adversary.signing_key,
            std::slice::from_ref(&replayed),
        )
        .await
        .unwrap();
    }

    let deliveries = receive_n(&receiver, 5).await;
    let honest_id = peer_identity("node-h", &honest.verifying_key);
    let adversary_id = peer_identity("node-h", &adversary.verifying_key);
    assert_ne!(honest_id, adversary_id);
    assert_eq!(deliveries[0].0, honest_id);
    assert!(deliveries[1..].iter().all(|(s, _)| *s == adversary_id));

    let mut node = run_through_runtime(&deliveries);
    // Three repeats: three strikes on the adversary's identity, which bans it.
    assert!(node.sybil_guard.check(&adversary_id).is_err());
    assert!(
        node.sybil_guard.reputation(&honest_id) >= 0.5,
        "the honest identity must not be struck"
    );
    assert!(node.sybil_guard.check(&honest_id).is_ok());
}

#[tokio::test]
async fn bad_signature_handshake_is_rejected_before_any_frame() {
    let receiver = listener("receiver").await;
    let attacker = Identity::new("attacker");

    let mut stream = TcpStream::connect(receiver.listen_addr).await.unwrap();
    let hello: ServerHello = read_json(&mut stream, 4096).await.unwrap();
    let mut auth = ClientAuth {
        node_id: "attacker".to_string(),
        public_key: attacker.verifying_key.to_bytes().to_vec(),
        nonce: vec![1u8; NONCE_LEN],
        signature: Vec::new(),
    };
    let transcript = handshake_transcript(&hello, &auth);
    auth.signature = sign_handshake(&attacker.signing_key, HandshakeRole::Dialer, &transcript);
    auth.signature[0] ^= 0xFF;
    write_json(&mut stream, &auth).await.unwrap();
    // A correctly signed frame follows; it must never be processed.
    let frame = sign_frame(
        &attacker.signing_key,
        &session_id(&transcript),
        0,
        &signed(&attacker, "x-1", "attacker", 1),
    );
    let _ = write_json(&mut stream, &frame).await;

    assert_nothing_delivered(&receiver).await;
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn handshake_from_another_connection_is_rejected() {
    let receiver = listener("receiver").await;
    let attacker = Identity::new("attacker");

    // A valid dialer signature for one connection's listener nonce…
    let mut first = TcpStream::connect(receiver.listen_addr).await.unwrap();
    let hello_1: ServerHello = read_json(&mut first, 4096).await.unwrap();
    let mut auth = ClientAuth {
        node_id: "attacker".to_string(),
        public_key: attacker.verifying_key.to_bytes().to_vec(),
        nonce: vec![2u8; NONCE_LEN],
        signature: Vec::new(),
    };
    auth.signature = sign_handshake(
        &attacker.signing_key,
        HandshakeRole::Dialer,
        &handshake_transcript(&hello_1, &auth),
    );
    drop(first);

    // …presented on a second connection, which has a fresh listener nonce.
    let mut second = TcpStream::connect(receiver.listen_addr).await.unwrap();
    let hello_2: ServerHello = read_json(&mut second, 4096).await.unwrap();
    assert_ne!(hello_1.nonce, hello_2.nonce);
    write_json(&mut second, &auth).await.unwrap();
    let reply = read_json::<ServerAuth>(&mut second, 4096).await;
    assert!(reply.is_err(), "the listener must close the connection");

    assert_nothing_delivered(&receiver).await;
    // Both connections failed: the first closed before authenticating, the second was
    // rejected.
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 2);
}

/// A party on the path that alters a message after the dialer signed the frame cannot
/// make the dialer appear to have sent an invalid signature.
#[tokio::test]
async fn altered_frame_is_rejected_and_never_attributed_to_the_dialer() {
    let receiver = listener("receiver").await;
    let dialer = Identity::new("dialer");
    let (mut stream, session) = manual_handshake(receiver.listen_addr, &dialer, "dialer").await;

    let mut frame = sign_frame(
        &dialer.signing_key,
        &session,
        0,
        &signed(&dialer, "d-1", "dialer", 1),
    );
    frame.message.signature_bytes[0] ^= 0xFF;
    write_json(&mut stream, &frame).await.unwrap();

    assert_nothing_delivered(&receiver).await;
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn replayed_frame_counter_closes_the_connection() {
    let receiver = listener("receiver").await;
    let dialer = Identity::new("dialer");
    let (mut stream, session) = manual_handshake(receiver.listen_addr, &dialer, "dialer").await;

    let frame: TcpFrame = sign_frame(
        &dialer.signing_key,
        &session,
        0,
        &signed(&dialer, "d-1", "dialer", 1),
    );
    write_json(&mut stream, &frame).await.unwrap();
    write_json(&mut stream, &frame).await.unwrap();

    let delivered = receive_n(&receiver, 1).await;
    assert_eq!(delivered[0].1.id, "d-1");
    assert_nothing_delivered(&receiver).await;
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn frame_signed_for_another_session_is_rejected() {
    let receiver = listener("receiver").await;
    let dialer = Identity::new("dialer");
    let (mut stream, _session) = manual_handshake(receiver.listen_addr, &dialer, "dialer").await;

    let frame = sign_frame(
        &dialer.signing_key,
        &[0u8; 32],
        0,
        &signed(&dialer, "d-1", "dialer", 1),
    );
    write_json(&mut stream, &frame).await.unwrap();

    assert_nothing_delivered(&receiver).await;
    assert_eq!(receiver.auth_failures.load(Ordering::SeqCst), 1);
}

/// The dialer checks the listener's proof of its key and sends nothing if it is wrong.
#[tokio::test]
async fn dialer_rejects_a_listener_that_cannot_prove_its_key() {
    let fake = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = fake.local_addr().unwrap();
    let claimed = Identity::new("listener");
    let server = tokio::spawn(async move {
        let (mut stream, _) = fake.accept().await.unwrap();
        let hello = ServerHello {
            node_id: "listener".to_string(),
            public_key: claimed.verifying_key.to_bytes().to_vec(),
            nonce: vec![3u8; NONCE_LEN],
        };
        write_json(&mut stream, &hello).await.unwrap();
        let _auth: ClientAuth = read_json(&mut stream, 4096).await.unwrap();
        // Signed with a key other than the one announced.
        let wrong = Identity::new("someone-else");
        let reply = ServerAuth {
            signature: sign_handshake(&wrong.signing_key, HandshakeRole::Listener, b"anything"),
        };
        write_json(&mut stream, &reply).await.unwrap();
        // Anything the dialer sends next would be read here.
        read_json::<TcpFrame>(&mut stream, 1 << 20).await.is_ok()
    });

    let dialer = Identity::new("dialer");
    let res = send_authenticated(
        &addr.to_string(),
        "dialer",
        &dialer.signing_key,
        &[signed(&dialer, "d-1", "dialer", 1)],
    )
    .await;
    assert!(matches!(res, Err(TransportError::Authentication(_))));
    assert!(
        !server.await.unwrap(),
        "no frame may follow a failed handshake"
    );
}

#[tokio::test]
async fn transport_refuses_to_dial_its_own_key() {
    let key = Identity::new("self").signing_key;
    let tcp = TcpTransport::bind("self".to_string(), "127.0.0.1:0".parse().unwrap(), key)
        .await
        .unwrap();
    let id = Identity::new("self");
    let res = tcp
        .send(&tcp.listen_addr.to_string(), &signed(&id, "s-1", "self", 1))
        .await;
    assert!(matches!(res, Err(TransportError::Authentication(_))));
    assert_nothing_delivered(&tcp).await;
}

/// The first key seen for an origin ID is pinned. If an adversary announced that ID
/// first, an honest relay that later forwards the real node's message must not be struck:
/// the receiver cannot tell which key is genuine.
#[test]
fn origin_key_conflict_is_dropped_without_striking_the_relay() {
    let squatter = Identity::new("node-a");
    let real = Identity::new("node-a");
    let deliveries = vec![
        (
            "squatter-relay".to_string(),
            signed(&squatter, "s-1", "node-a", 1),
        ),
        (
            "honest-relay".to_string(),
            signed(&real, "r-1", "node-a", 2),
        ),
    ];
    let mut node = run_through_runtime(&deliveries);

    assert_eq!(node.metrics.messages_accepted.load(Ordering::Relaxed), 1);
    assert!(
        node.sybil_guard.reputation(&"honest-relay".to_string()) >= 0.5,
        "an honest relay must not be struck for a key conflict"
    );
}
