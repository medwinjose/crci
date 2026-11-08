//! A flood of validly signed messages through the real TCP path, from many identities.
//!
//! Opens `--connections` authenticated connections to `--target`, each under a fresh identity,
//! and sends on each `--rate` messages per second (the per-sender budget is 20/s, so the default
//! stays inside it), each signed by a fresh origin identity, until `--messages` messages in all
//! have been sent. Every message is new to the receiver, so it reaches every per-origin table.
//! Prints the totals at the end.
//!
//!   cargo run --release -p crci-node --example tcp_flood -- \
//!       --target 127.0.0.1:7001 --connections 90 --rate 20 --messages 1000000

use crci_core::identity::Identity;
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::tcp::DialedConnection;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() {
    let target: String = arg("--target", "127.0.0.1:7001".to_string());
    let connections: u64 = arg("--connections", 90);
    let rate: u64 = arg("--rate", 20).max(1);
    let messages: u64 = arg("--messages", 1_000_000);
    let per_connection = messages.div_ceil(connections);

    let sent = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    let mut tasks = Vec::new();
    for c in 0..connections {
        let target = target.clone();
        let sent = sent.clone();
        let failed = failed.clone();
        tasks.push(tokio::spawn(async move {
            let name = format!("flood-c{c}");
            let me = Identity::new(&name);
            let Ok(mut conn) = DialedConnection::connect(&target, &me.signing_key).await else {
                failed.fetch_add(per_connection, Ordering::Relaxed);
                return;
            };
            let mut tick = tokio::time::interval(Duration::from_secs_f64(1.0 / rate as f64));
            for i in 0..per_connection {
                tick.tick().await;
                let origin = format!("o{c}x{i}");
                let mut msg = Message::new(
                    &format!("m{c}x{i}"),
                    &origin,
                    Signal::new(3, true, false, true, 3, Visibility::Direct),
                    None,
                );
                msg.message_type = MessageType::RescueRequest;
                msg.seq = 1;
                let wire = WireMessage::from_message(&msg, &Identity::new(&origin));
                if conn.send(&wire).await.is_ok() {
                    sent.fetch_add(1, Ordering::Relaxed);
                } else {
                    failed.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
        }));
    }
    for task in tasks {
        let _ = task.await;
    }
    println!(
        "sent={} failed={} connections={} rate_per_connection={} seconds={:.1}",
        sent.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed),
        connections,
        rate,
        started.elapsed().as_secs_f64()
    );
}
