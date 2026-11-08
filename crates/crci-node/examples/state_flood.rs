//! Memory under a flood of validly signed messages from many identities.
//!
//! One `NodeRuntime` receives `--messages` messages (rescue type, so they are also kept in
//! the rescue store) spread round-robin over `--origins` origins and `--senders` transport
//! senders, and hands what it delivers to the application pipeline as the TCP node does
//! (`--no-pipeline`: the protocol state alone). The simulated clock runs at the pace that keeps
//! every sender and origin inside its budget, so each message reaches the state that is being
//! measured. Every `--every` messages it writes a CSV line to stderr: live heap bytes (counting
//! allocator), the size of each per-node table, origins forgotten and process RSS (Linux).
//! The node's own log lines go to stdout.
//!
//!   cargo run --release -p crci-node --example state_flood -- \
//!       --messages 1000000 --origins 1000000 --senders 50 > /dev/null 2> flood.csv

use crci_core::identity::Identity;
use crci_core::integration::{GossipPipeline, MessageKind, PipelineMessage};
use crci_core::message::{Message, MessageType, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            let now = LIVE.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Resident set of this process in KiB (Linux; 0 elsewhere).
fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |pages| pages * 4)
}

fn arg(name: &str, default: usize) -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let messages = arg("--messages", 100_000);
    let origins = arg("--origins", messages).max(1);
    let senders = arg("--senders", 50).max(1);
    let every = arg("--every", messages.div_ceil(20).max(1));
    let no_pipeline = std::env::args().any(|a| a == "--no-pipeline");
    // Origins whose identities are kept and reused; above this each message has a new origin.
    let keep = origins.min(5_000);

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.use_router = false;
    let mut pipeline = GossipPipeline::new();
    let mut since_round = Duration::ZERO;
    let dt = Duration::from_secs_f64((0.05 / senders as f64).max(0.2 / origins as f64));

    let heap_before_ids = LIVE.load(Ordering::Relaxed);
    let identities: Vec<Identity> = (0..keep).map(|i| Identity::new(&format!("o{i}"))).collect();
    let harness_bytes = LIVE.load(Ordering::Relaxed) - heap_before_ids;
    let base = LIVE.load(Ordering::Relaxed) - harness_bytes;

    eprintln!(
        "messages,live_bytes,peak_bytes,origin_budgets,replay_origins,seen,persistent,\
         observations,recent_origins,forgotten,rss_kib"
    );
    let mut done = 0usize;
    while done < messages {
        let chunk = every.min(messages - done);
        // One message per step, so the clock advances between messages.
        for i in done..done + chunk {
            let o = i % origins;
            let fresh;
            let id = if o < keep {
                &identities[o]
            } else {
                fresh = Identity::new(&format!("o{o}"));
                &fresh
            };
            let mut m = Message::new(
                &format!("m{i}"),
                &format!("o{o}"),
                Signal::new(3, true, false, true, 3, Visibility::Direct),
                None,
            );
            m.message_type = MessageType::RescueRequest;
            m.seq = (i / origins + 1) as u64;
            let wire = WireMessage::from_message(&m, id);
            if let Ok(bytes) = serde_json::to_vec(&wire) {
                if let Ok(mut guard) = inbox.lock() {
                    guard
                        .entry(node.id.clone())
                        .or_default()
                        .push((format!("s{}", i % senders), bytes));
                }
            }
            node.sybil_guard.advance_time(dt);
            // As the TCP node does with what it delivers, and every 5 s.
            for msg in node.process_inbox_delivered() {
                if no_pipeline {
                    continue;
                }
                pipeline.register_bootstrap("zone-host", &msg.origin);
                let _ = pipeline.process(&PipelineMessage {
                    id: msg.id.clone(),
                    origin_node: msg.origin.clone(),
                    zone: "zone-host".to_string(),
                    severity: msg.severity,
                    kind: MessageKind::Rescue,
                    payload_bytes: 0,
                    reputation: 0.5,
                    round: 0,
                    seq: msg.seq,
                });
            }
            since_round += dt;
            // The message count stands in for time when a run gives every sender its budget
            // with almost no simulated time passing; the node's rounds are real seconds.
            if since_round >= Duration::from_secs(5) || i % 1000 == 999 {
                since_round = Duration::ZERO;
                pipeline.next_round();
            }
        }
        done += chunk;
        let live = LIVE
            .load(Ordering::Relaxed)
            .saturating_sub(base + harness_bytes);
        eprintln!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            done,
            live,
            PEAK.load(Ordering::Relaxed)
                .saturating_sub(base + harness_bytes),
            node.origin_budgets.len(),
            node.replay_filter.tracked_origins(),
            node.seen_messages.len(),
            node.persistent_messages.len(),
            node.observations.len(),
            node.recent_messages.len(),
            node.origins_forgotten,
            rss_kib(),
        );
    }
    eprintln!(
        "accepted={} sybil_tracked_peers={}",
        node.metrics.messages_accepted.load(Ordering::Relaxed),
        node.sybil_guard.tracked_peers()
    );
}
