//! An adversary host with free identities, for the testbed's identity-rotation experiments.
//!
//! Dials every address in `--targets` from each local address in `--sources` (a comma list;
//! empty: the system's choice) and, for `--duration-s` seconds after `--start-delay-ms`, does
//! one of three things on each (source, target) pair:
//!
//! - `hold`: keeps `--per-source` authenticated connections open and idle, opening a new one
//!   (under a fresh identity) whenever the target closes one. A listener that only counts
//!   connections is full after 100; one that charges each source host keeps serving others.
//! - `forge`: sends `--rate` messages per second with a broken signature under a fresh identity,
//!   and switches to a new identity (a new connection) once the target closes the old one,
//!   which it does when it bans it; `--per-source` such loops run at once per address.
//! - `flood`: sends `--rate` validly signed fresh messages per second, each from a new origin
//!   identity, switching the connection's identity every `--rotate-every` messages.
//!
//! Prints one line of totals at the end:
//!
//!   host_attack --mode forge --targets 10.0.0.2:7000 --sources 10.0.200.1 --rate 400

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::WireMessage;
use crci_core::transport::tcp::DialedConnection;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn num<T: std::str::FromStr>(name: &str, default: T) -> T {
    arg(name, "").parse().unwrap_or(default)
}

#[derive(Default)]
struct Totals {
    connections: AtomicU64,
    refused: AtomicU64,
    closed_by_target: AtomicU64,
    identities: AtomicU64,
    frames: AtomicU64,
}

fn fresh_message(origin: &Identity, n: u64) -> WireMessage {
    let mut msg = Message::new(
        &format!("ha-{}-{n}", &origin.id[..8]),
        &origin.id,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = n;
    WireMessage::from_message(&msg, origin)
}

async fn connect(
    target: &str,
    source: Option<IpAddr>,
    totals: &Totals,
) -> Option<(Identity, DialedConnection)> {
    let identity = Identity::new("adversary");
    totals.identities.fetch_add(1, Ordering::Relaxed);
    match DialedConnection::connect_from(target, &identity.signing_key, source).await {
        Ok(conn) => {
            totals.connections.fetch_add(1, Ordering::Relaxed);
            Some((identity, conn))
        }
        Err(_) => {
            totals.refused.fetch_add(1, Ordering::Relaxed);
            None
        }
    }
}

async fn hold(
    target: String,
    source: Option<IpAddr>,
    per_source: usize,
    until: Instant,
    totals: Arc<Totals>,
) {
    let mut held: Vec<DialedConnection> = Vec::new();
    while Instant::now() < until {
        let before = held.len();
        held.retain(|c| !c.is_closed());
        totals
            .closed_by_target
            .fetch_add((before - held.len()) as u64, Ordering::Relaxed);
        while held.len() < per_source && Instant::now() < until {
            match connect(&target, source, &totals).await {
                Some((_, conn)) => held.push(conn),
                None => break,
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn send_loop(
    mode: &str,
    target: String,
    source: Option<IpAddr>,
    rate: f64,
    rotate_every: u64,
    until: Instant,
    totals: Arc<Totals>,
) {
    let mut tick = tokio::time::interval(Duration::from_secs_f64(1.0 / rate.max(0.1)));
    let mut n = 0u64;
    while Instant::now() < until {
        let Some((identity, mut conn)) = connect(&target, source, &totals).await else {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        let mut sent_here = 0u64;
        while Instant::now() < until {
            tick.tick().await;
            if conn.is_closed() {
                totals.closed_by_target.fetch_add(1, Ordering::Relaxed);
                break;
            }
            n += 1;
            let wire = if mode == "forge" {
                let mut wire = fresh_message(&identity, n);
                if let Some(b) = wire.signature_bytes.first_mut() {
                    *b ^= 0x01;
                }
                wire
            } else {
                fresh_message(&Identity::new("origin"), 1)
            };
            if conn.send(&wire).await.is_err() {
                totals.closed_by_target.fetch_add(1, Ordering::Relaxed);
                break;
            }
            totals.frames.fetch_add(1, Ordering::Relaxed);
            sent_here += 1;
            if mode == "flood" && sent_here >= rotate_every {
                conn.close().await;
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mode = arg("--mode", "hold");
    let targets: Vec<String> = arg("--targets", "")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let sources: Vec<Option<IpAddr>> = {
        let list: Vec<Option<IpAddr>> = arg("--sources", "")
            .split(',')
            .filter_map(|s| s.parse().ok())
            .map(Some)
            .collect();
        if list.is_empty() {
            vec![None]
        } else {
            list
        }
    };
    let per_source: usize = num("--per-source", 8);
    let rate: f64 = num("--rate", 100.0);
    let rotate_every: u64 = num("--rotate-every", 20);
    let duration = Duration::from_secs(num("--duration-s", 60));
    tokio::time::sleep(Duration::from_millis(num("--start-delay-ms", 0))).await;

    let started = Instant::now();
    let until = started + duration;
    let totals = Arc::new(Totals::default());
    let mut tasks = Vec::new();
    for target in &targets {
        for source in &sources {
            let (target, source, totals) = (target.clone(), *source, totals.clone());
            let mode = mode.clone();
            // A forger runs `per_source` loops at once; a flood is one loop at its total rate.
            let loops = if mode == "forge" {
                per_source.max(1)
            } else {
                1
            };
            for _ in 0..loops {
                let (target, totals, mode) = (target.clone(), totals.clone(), mode.clone());
                tasks.push(tokio::spawn(async move {
                    if mode == "hold" {
                        hold(target, source, per_source, until, totals).await;
                    } else {
                        send_loop(&mode, target, source, rate, rotate_every, until, totals).await;
                    }
                }));
            }
        }
    }
    for task in tasks {
        let _ = task.await;
    }
    println!(
        "host_attack mode={} targets={} sources={} connections={} refused={} closed_by_target={} identities={} frames={} seconds={:.1}",
        mode,
        targets.len(),
        sources.len(),
        totals.connections.load(Ordering::Relaxed),
        totals.refused.load(Ordering::Relaxed),
        totals.closed_by_target.load(Ordering::Relaxed),
        totals.identities.load(Ordering::Relaxed),
        totals.frames.load(Ordering::Relaxed),
        started.elapsed().as_secs_f64()
    );
}
