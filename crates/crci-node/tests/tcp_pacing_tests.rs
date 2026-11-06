//! A connection's frames are read at a bounded rate.
//!
//! Before any frame is read the listener takes a token from a per-connection bucket
//! (`FRAME_PACING_RATE` frames/s, `FRAME_PACING_BURST` burst by default) and sleeps when it is empty, so a
//! sender cannot make the receiver parse and verify frames faster than that: TCP flow control
//! slows the sender down. Nothing is struck or banned for it.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::{TcpTransport, Transport, FRAME_PACING_BURST, FRAME_PACING_RATE};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn signed(identity: &Identity, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        &format!("m-{seq}"),
        &identity.id,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

/// A listener with the given pacing. The tests use a rate below what an unoptimised build can
/// verify (about 140 frames/s), so that pacing, not the CPU, is what limits the link.
async fn listener_paced(node_id: &str, rate: f64, burst: u32) -> TcpTransport {
    TcpTransport::bind_with_pacing(
        node_id.to_string(),
        "127.0.0.1:0".parse().unwrap(),
        Identity::new(node_id).signing_key,
        rate,
        burst,
    )
    .await
    .unwrap()
}

async fn listener(node_id: &str) -> TcpTransport {
    listener_paced(node_id, FRAME_PACING_RATE, FRAME_PACING_BURST).await
}

/// Frames delivered by `receiver` in `window`, while `sender` sends `total` frames as fast as
/// the connection takes them.
async fn delivered_in(window: Duration, total: u64, rate: f64, burst: u32) -> (u64, u64) {
    let receiver = listener_paced("receiver", rate, burst).await;
    let sender = listener("sender").await;
    let author = Identity::new("author");
    let target = receiver.listen_addr.to_string();
    // `relay` queues the frames (up to 1 024) for one task that writes them back to back, as the
    // node does; `send` waits for the connection's round trip each time and cannot saturate it.
    for seq in 1..=total {
        sender.relay(&target, signed(&author, seq)).expect("queued");
    }
    let start = Instant::now();
    let mut got = 0u64;
    while let Some(left) = window.checked_sub(start.elapsed()) {
        match tokio::time::timeout(left, receiver.receive()).await {
            Ok(Ok(_)) => got += 1,
            _ => break,
        }
        if got == total {
            break;
        }
    }
    (got, receiver.paced_frames.load(Ordering::Relaxed))
}

#[tokio::test]
async fn a_connection_delivers_frames_at_the_paced_rate_not_faster() {
    let (rate, burst) = (50.0, 20);
    let window = Duration::from_millis(2000);
    let (got, paced) = delivered_in(window, 1000, rate, burst).await;
    assert!(paced > 0, "the reader must have waited for tokens");
    let allowed = f64::from(burst) + rate * window.as_secs_f64();
    assert!(
        (got as f64) <= allowed * 1.15,
        "{got} frames in {window:?}; pacing allows about {allowed}"
    );
    // The burst arrives at once whatever the CPU; more than that depends on the runner's speed.
    assert!(
        got >= u64::from(burst),
        "{got} frames in {window:?}: pacing must not starve the link (burst {burst})"
    );
}

#[tokio::test]
async fn an_honest_burst_below_the_default_burst_is_not_delayed() {
    let n = u64::from(FRAME_PACING_BURST / 2);
    let (got, paced) = delivered_in(
        Duration::from_secs(30),
        n,
        FRAME_PACING_RATE,
        FRAME_PACING_BURST,
    )
    .await;
    assert_eq!(got, n, "a burst of {n} frames must be delivered whole");
    // Not a time limit: an unoptimised build verifies a frame in milliseconds, and a slow runner
    // needs seconds for the burst. What matters is that the reader never had to wait for a token.
    assert_eq!(paced, 0, "a burst below the default burst must never wait");
}
