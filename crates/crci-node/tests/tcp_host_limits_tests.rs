//! Listener limits charged per host, not per identity.
//!
//! Keys are free, so one host can open connections and sign messages with as many identities
//! as it likes. The listener charges connections, handshakes and frames to the source host of
//! each connection, and a node charges new messages to it too. These tests appear as several
//! hosts by binding dialers to different loopback addresses (127.0.0.0/8).

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::tcp::DialedConnection;
use crci_core::transport::{ListenerLimits, TcpTransport, Transport};
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

fn ip(last: u8) -> Option<IpAddr> {
    Some(IpAddr::from([127, 0, 0, last]))
}

async fn listener(limits: ListenerLimits) -> TcpTransport {
    TcpTransport::bind_with_limits(
        "127.0.0.1:0".parse().unwrap(),
        Identity::new("receiver").signing_key,
        limits,
    )
    .await
    .unwrap()
}

/// Hold every slot of the listener with authenticated, idle connections from thirteen hosts,
/// then send one frame from a fourteenth; true if the listener delivers it.
async fn honest_frame_gets_through_a_full_listener(limits: ListenerLimits) -> bool {
    let receiver = listener(limits).await;
    let target = receiver.listen_addr.to_string();
    let mut held = Vec::new();
    for host in 0..13u8 {
        for _ in 0..8 {
            let key = Identity::new("adversary").signing_key;
            if let Ok(conn) = DialedConnection::connect_from(&target, &key, ip(10 + host)).await {
                held.push(conn);
            }
        }
    }
    let honest = Identity::new("honest");
    let author = Identity::new("author");
    let delivered = match DialedConnection::connect_from(&target, &honest.signing_key, ip(2)).await
    {
        Ok(mut conn) => {
            let _ = conn.send(&signed(&author, "m-1", 1)).await;
            tokio::time::timeout(Duration::from_secs(3), receiver.receive())
                .await
                .is_ok_and(|r| r.is_ok())
        }
        Err(_) => false,
    };
    drop(held);
    delivered
}

#[tokio::test]
async fn idle_connections_from_other_hosts_cannot_lock_an_honest_neighbour_out() {
    assert!(
        honest_frame_gets_through_a_full_listener(ListenerLimits::default()).await,
        "an honest host with no connection must be admitted"
    );
}

/// The same attack against a listener that counts connections only (the behaviour before
/// per-host limits): the honest neighbour is refused.
#[tokio::test]
async fn without_per_host_limits_idle_connections_lock_an_honest_neighbour_out() {
    let limits = ListenerLimits {
        per_host_limits: false,
        ..ListenerLimits::default()
    };
    assert!(!honest_frame_gets_through_a_full_listener(limits).await);
}

#[tokio::test]
async fn one_host_with_many_identities_holds_only_its_share_of_connections() {
    let receiver = listener(ListenerLimits::default()).await;
    let target = receiver.listen_addr.to_string();
    let mut held = Vec::new();
    for _ in 0..100 {
        let key = Identity::new("adversary").signing_key;
        if let Ok(conn) = DialedConnection::connect_from(&target, &key, ip(30)).await {
            held.push(conn);
        }
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let active = receiver.active_connections.load(Ordering::SeqCst);
    let admitted = receiver.accepted_count.load(Ordering::SeqCst);
    let refused = receiver.rejected_count.load(Ordering::SeqCst);
    let evicted = receiver.evicted_connections.load(Ordering::SeqCst);
    assert_eq!(active, 8, "connections from one host left open");
    // 16 at once, then 2 per second for as long as the 100 attempts take.
    assert!(
        refused >= 70,
        "only {refused} connections refused by the handshake rate"
    );
    assert_eq!(
        evicted,
        admitted - 8,
        "every connection beyond the host's share was closed"
    );
}

/// Start `crci-node --listen` with `extra` arguments; its address, its process, and the number of
/// messages it has accepted, counted by a thread that also keeps a full pipe from blocking the node.
fn start_node(extra: &[&str]) -> (String, std::process::Child, Arc<AtomicUsize>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_crci-node"))
        .args(["--listen", "127.0.0.1:0", "--label", "receiver"])
        .args(extra)
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start node");
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let start = Instant::now();
    let mut line = String::new();
    while start.elapsed() < Duration::from_secs(10) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.starts_with("Node ") && line.contains(" listening on ") => {
                let addr = line
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .to_string();
                let accepted = Arc::new(AtomicUsize::new(0));
                let counter = accepted.clone();
                std::thread::spawn(move || {
                    for line in reader.lines().map_while(Result::ok) {
                        counter.fetch_add(line.matches("accepted '").count(), Ordering::SeqCst);
                    }
                });
                return (addr, child, accepted);
            }
            _ => {}
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("node did not start listening");
}

/// One host sends 20 fresh, validly signed messages from each of 40 identities (each its own
/// origin, so the per-origin budget never applies); how many the node accepts, and how long it was given.
fn accepted_from_one_host_with_many_identities(extra: &[&str]) -> (usize, f64) {
    let (addr, mut child, accepted) = start_node(extra);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let start = Instant::now();
    rt.block_on(async {
        let mut conns = Vec::new();
        for _ in 0..40 {
            let identity = Identity::new("flooder");
            // Spread over the host's handshake budget (16, then 2 per second).
            loop {
                match DialedConnection::connect_from(&addr, &identity.signing_key, ip(40)).await {
                    Ok(conn) => break conns.push((identity, conn)),
                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            }
            let (identity, conn) = conns.last_mut().unwrap();
            for seq in 1..=20u64 {
                let _ = conn.send(&signed(identity, &format!("f-{seq}"), seq)).await;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    // Until nothing new is accepted for two seconds: a slow machine needs time to drain.
    let drain = Instant::now();
    let (mut count, mut last) = (accepted.load(Ordering::SeqCst), Instant::now());
    while last.elapsed() < Duration::from_secs(2) && drain.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(50));
        let now = accepted.load(Ordering::SeqCst);
        if now != count {
            (count, last) = (now, Instant::now());
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let _ = child.kill();
    let _ = child.wait();
    (count, elapsed)
}

#[test]
fn a_host_gets_one_message_budget_however_many_identities_it_uses() {
    let (accepted, secs) = accepted_from_one_host_with_many_identities(&[]);
    // The host's budget: a burst of 20, then 20 per second while it was sending and draining.
    let bound = 20.0 + 20.0 * secs;
    println!("one host, 40 identities, 800 fresh messages: {accepted} accepted in {secs:.1} s");
    assert!(
        (accepted as f64) <= bound,
        "{accepted} accepted in {secs:.1} s, bound {bound:.0}"
    );
    assert!(accepted >= 20, "only {accepted} accepted");
}

#[test]
fn without_per_host_limits_every_identity_brings_a_fresh_budget() {
    let (accepted, _) = accepted_from_one_host_with_many_identities(&["--per-host-limits", "off"]);
    assert_eq!(accepted, 800, "each identity's burst of 20 is accepted");
}
