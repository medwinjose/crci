//! Extended benchmark harness for CRCI.
//!
//! Provides the quantitative measurements needed for the research paper
//! that Session 18's bench.rs did not cover:
//!   - TTL pruning memory savings
//!   - Rate limiter throughput under load
//!   - Discovery convergence time
//!   - AEDA decision latency
//!   - Priority queue throughput and ordering
//!   - End-to-end pipeline throughput
//!
//! All benchmarks use deterministic seed 42 where randomness is needed.
//! All results are reproducible.

use crate::aeda::{AedaEngine, RescueEvent};
use crate::discovery::{Beacon, DiscoveryEngine};
use crate::integration::{GossipPipeline, MessageKind, PipelineMessage};
use crate::security::{PriorityMessageQueue, ProtocolMessageKind, QueuedMessage};
use crate::ttl::{StoredMessage, TtlStore};
use crate::validation::RateLimiter;
use std::time::Instant;

// ── Deterministic LCG (same as bench.rs pattern) ─────────────────

struct Lcg {
    state: u64,
}
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg {
            state: seed.wrapping_add(1),
        }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
    fn next_usize_range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() as usize % (hi - lo))
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (u32::MAX as f32)
    }
}

// ── Benchmark 1: TTL Pruning Impact ──────────────────────────────

fn bench_ttl_pruning() {
    let node_count = 100usize;
    let msgs_per_round = 5usize;
    let rounds = 500u64;

    // With pruning
    let mut store_pruned = TtlStore::new();
    // Allow messages to persist until TTL expiration (50 rounds) rather than being evicted early by default storage cap (200).
    store_pruned.set_max_stored_messages(30_000);
    let mut store_unpruned_count: u64 = 0;

    let t0 = Instant::now();
    for round in 0..rounds {
        for i in 0..(node_count * msgs_per_round) {
            let id = format!("msg-r{round}-{i}");
            store_pruned.store(StoredMessage::new_normal(&id, round));
            store_unpruned_count += 1;
        }
        store_pruned.prune(round);
    }
    let wall_ms = t0.elapsed().as_millis();

    let final_active = store_pruned.active_count() as u64;
    // Memory saved measures the percentage of historical messages that have been reclaimed
    // (either pruned due to TTL expiration or evicted due to capacity limits)
    // and are no longer occupying active memory, relative to the total messages generated.
    let pct_saved = if store_unpruned_count > 0 {
        100.0 - (final_active as f32 / store_unpruned_count as f32 * 100.0)
    } else {
        0.0
    };

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 6 — TTL Pruning Memory Impact                     │");
    println!("├────────────────────────┬─────────────────────────────────────┤");
    println!("│  Rounds simulated      │ {:>35} │", rounds);
    println!("│  Total msgs generated  │ {:>35} │", store_unpruned_count);
    println!("│  Active (with pruning) │ {:>35} │", final_active);
    println!(
        "│  Total pruned          │ {:>35} │",
        store_pruned.total_pruned
    );
    println!("│  Memory saved          │ {:>33.1}% │", pct_saved);
    println!("│  Wall time             │ {:>33} ms │", wall_ms);
    println!("└────────────────────────┴─────────────────────────────────────┘");
}

// ── Benchmark 2: Rate Limiter Throughput ─────────────────────────

fn bench_rate_limiter() {
    let total_msgs = 100_000usize;
    let unique_nodes = 10_000usize;
    let mut rng = Lcg::new(42);
    let mut limiter = RateLimiter::new();

    let t0 = Instant::now();
    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut current_round = 0u64;

    for i in 0..total_msgs {
        // Reset every 100 messages to simulate round boundaries
        if i % 100 == 0 {
            current_round += 1;
            limiter.reset_round();
        }
        let node_id = format!("node-{:05}", rng.next_usize_range(0, unique_nodes));
        match limiter.check_and_record(&node_id) {
            Ok(_) => accepted += 1,
            Err(_) => rejected += 1,
        }
    }
    let wall_ms = t0.elapsed().as_millis().max(1) as u64;
    let total_per_s = total_msgs as u64 * 1000 / wall_ms;
    let accepted_per_s = accepted * 1000 / wall_ms;
    let _ = current_round;

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 7 — Rate Limiter Throughput Under Load            │");
    println!("├────────────────────────┬─────────────────────────────────────┤");
    println!("│  Total messages        │ {:>35} │", total_msgs);
    println!("│  Unique nodes          │ {:>35} │", unique_nodes);
    println!("│  Accepted              │ {:>35} │", accepted);
    println!("│  Rejected              │ {:>35} │", rejected);
    println!("│  Throughput            │ {:>31} msg/s │", total_per_s);
    println!("│  Accept throughput     │ {:>31} msg/s │", accepted_per_s);
    println!("│  Wall time             │ {:>33} ms │", wall_ms);
    println!("└────────────────────────┴─────────────────────────────────────┘");
}

// ── Benchmark 3: Discovery Convergence ───────────────────────────

fn bench_discovery_convergence(node_count: usize, use_exchange: bool) -> u64 {
    let zone = "bench-zone";
    let mut engines: Vec<DiscoveryEngine> = (0..node_count)
        .map(|i| {
            let id = format!(
                "{:016x}",
                (i as u64).wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1)
            );
            DiscoveryEngine::new(&id, zone)
        })
        .collect();

    let ids: Vec<String> = engines.iter().map(|e| e.local_node_id.clone()).collect();

    for round in 1..=500u64 {
        // Beacon round
        if round % 10 == 0 {
            let beacons: Vec<Beacon> = engines
                .iter_mut()
                .map(|e| e.generate_beacon(round, "FULL"))
                .collect();
            for engine in &mut engines {
                for beacon in &beacons {
                    engine.receive_beacon(beacon, round);
                }
            }
        }

        // Peer exchange every 20 rounds (if enabled)
        if use_exchange && round % 20 == 0 && engines.len() >= 2 {
            let shared = engines[0].generate_peer_exchange(&ids[1], round);
            let zone_hint = engines[0].local_zone.clone();
            let via = engines[0].local_node_id.clone();
            engines[1].receive_peer_exchange(&via, &shared, &zone_hint, round);
        }

        // Check if all nodes know all others
        let all_known = engines.iter().all(|e| e.known_peers() == node_count - 1);
        if all_known {
            return round;
        }
    }
    500 // DNF
}

fn bench_discovery() {
    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 8 — Discovery Convergence Time                    │");
    println!("├──────────┬────────────────────────┬────────────────────────────┤");
    println!("│  Nodes   │  Beacon-only (rounds)  │  Beacon+Exchange (rounds)  │");
    println!("├──────────┼────────────────────────┼────────────────────────────┤");
    for &n in &[10usize, 50, 100] {
        let t0 = Instant::now();
        let beacon_rounds = bench_discovery_convergence(n, false);
        let exchange_rounds = bench_discovery_convergence(n, true);
        let wall_ms = t0.elapsed().as_millis();
        let beacon_str = if beacon_rounds == 500 {
            "DNF (>500)".to_string()
        } else {
            format!("{beacon_rounds}")
        };
        let exchange_str = if exchange_rounds == 500 {
            "DNF (>500)".to_string()
        } else {
            format!("{exchange_rounds}")
        };
        println!(
            "│ {:>8} │ {:>22} │ {:>24} ({wall_ms} ms) │",
            n, beacon_str, exchange_str
        );
    }
    println!("└──────────┴────────────────────────┴────────────────────────────┘");
}

// ── Benchmark 4: AEDA Decision Latency ───────────────────────────

fn bench_aeda_latency() {
    let event_count = 10_000usize;
    let zone_count = 100usize;
    let mut rng = Lcg::new(42);
    let mut engine = AedaEngine::new();

    let t0 = Instant::now();
    for i in 0..event_count {
        let zone = format!("zone-{:03}", rng.next_usize_range(0, zone_count));
        let severity = (rng.next_usize_range(1, 6)) as u8;
        let reputation = 0.5 + rng.next_f32() * 0.5; // 0.5–1.0
        engine.process(
            RescueEvent {
                node_id: format!("node-{i}"),
                zone,
                severity,
                round: i as u64 / 10,
                reputation,
            },
            i as u64 / 10,
        );
    }
    let wall_ms = t0.elapsed().as_millis().max(1) as u64;
    let decisions_per_s = engine.decisions().len() as u64 * 1000 / wall_ms;

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 9 — AEDA Decision Latency                         │");
    println!("├────────────────────────┬─────────────────────────────────────┤");
    println!("│  Rescue events         │ {:>35} │", event_count);
    println!("│  Zones                 │ {:>35} │", zone_count);
    println!(
        "│  Total decisions       │ {:>35} │",
        engine.decisions().len()
    );
    println!(
        "│  Escalations           │ {:>35} │",
        engine.count_escalations()
    );
    println!(
        "│  Disinfo flags         │ {:>35} │",
        engine.count_disinfo()
    );
    println!("│  Decision throughput   │ {:>31} dec/s │", decisions_per_s);
    println!("│  Wall time             │ {:>33} ms │", wall_ms);
    println!("└────────────────────────┴─────────────────────────────────────┘");
}

// ── Benchmark 5: Priority Queue Throughput + Ordering ────────────

fn bench_priority_queue() {
    let total = 100_000usize;
    let mut rng = Lcg::new(42);
    let mut q = PriorityMessageQueue::new();

    // Enqueue
    let t0 = Instant::now();
    for i in 0..total {
        let kind = match rng.next_usize_range(0, 10) {
            0 => ProtocolMessageKind::Rescue,
            1..=2 => ProtocolMessageKind::Hazard,
            _ => ProtocolMessageKind::Normal,
        };
        q.push(QueuedMessage {
            id: format!("m{i}"),
            kind,
            round: i as u64,
        });
    }
    let enqueue_ms = t0.elapsed().as_millis().max(1) as u64;

    // Dequeue and verify ordering
    let t1 = Instant::now();
    let mut last_priority = 255u8;
    let mut ordering_ok = true;
    let mut dequeued = 0u64;
    while let Some(msg) = q.pop() {
        if msg.kind.priority() > last_priority {
            ordering_ok = false;
        }
        last_priority = msg.kind.priority();
        dequeued += 1;
    }
    let dequeue_ms = t1.elapsed().as_millis().max(1) as u64;

    let enqueue_per_s = total as u64 * 1000 / enqueue_ms;
    let dequeue_per_s = dequeued * 1000 / dequeue_ms;

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 10 — Priority Queue Throughput + Ordering         │");
    println!("├────────────────────────┬─────────────────────────────────────┤");
    println!("│  Messages              │ {:>35} │", total);
    println!("│  Enqueue throughput    │ {:>31} msg/s │", enqueue_per_s);
    println!("│  Dequeue throughput    │ {:>31} msg/s │", dequeue_per_s);
    println!(
        "│  Priority ordering     │ {:>35} │",
        if ordering_ok {
            "✅ correct (Rescue→Hazard→Normal)"
        } else {
            "❌ VIOLATED"
        }
    );
    println!("│  Enqueue wall time     │ {:>33} ms │", enqueue_ms);
    println!("│  Dequeue wall time     │ {:>33} ms │", dequeue_ms);
    println!("└────────────────────────┴─────────────────────────────────────┘");
}

// ── Benchmark 6: End-to-End Pipeline Throughput ──────────────────

fn bench_pipeline_e2e() {
    let total = 10_000usize;
    let mut rng = Lcg::new(42);
    let mut pipeline = GossipPipeline::new();

    // Register 10 nodes with varying battery
    for i in 0..10u8 {
        pipeline.update_battery(&format!("node-{i:02}"), 20 + i * 8);
    }

    let t0 = Instant::now();
    for i in 0..total {
        let node_id = format!("node-{:02}", rng.next_usize_range(0, 10));
        let kind = match rng.next_usize_range(0, 20) {
            0..=1 => MessageKind::Panic,
            2..=6 => MessageKind::Rescue,
            7..=19 => {
                // ~5% invalid severity
                if rng.next_usize_range(0, 20) == 0 {
                    // We'll signal invalid with severity=0
                    let msg = PipelineMessage {
                        id: format!("msg-{i}"),
                        origin_node: node_id,
                        zone: "bench-zone".to_string(),
                        severity: 0, // invalid
                        kind: MessageKind::Normal,
                        payload_bytes: 50,
                        reputation: 1.0,
                        round: i as u64,
                        seq: i as u64,
                    };
                    pipeline.process(&msg);
                    continue;
                }
                MessageKind::Normal
            }
            _ => MessageKind::Normal,
        };

        let severity = rng.next_usize_range(1, 6) as u8;
        let msg = PipelineMessage {
            id: format!("msg-{i}"),
            origin_node: node_id,
            zone: "bench-zone".to_string(),
            severity,
            kind,
            payload_bytes: 50 + rng.next_usize_range(0, 100),
            reputation: 0.5 + rng.next_f32() * 0.5,
            round: i as u64,
            seq: i as u64,
        };
        pipeline.process(&msg);

        // Advance round every 20 messages
        if i % 20 == 0 {
            pipeline.next_round();
        }
    }
    let wall_ms = t0.elapsed().as_millis().max(1) as u64;
    let total_per_s = total as u64 * 1000 / wall_ms;

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  BENCHMARK 11 — End-to-End Pipeline Throughput               │");
    println!("├────────────────────────┬─────────────────────────────────────┤");
    println!("│  Total messages        │ {:>35} │", total);
    println!("│  Accepted              │ {:>35} │", pipeline.accepted);
    println!("│  Rejected              │ {:>35} │", pipeline.rejected);
    println!("│  Throttled             │ {:>35} │", pipeline.throttled);
    println!("│  Throughput            │ {:>31} msg/s │", total_per_s);
    println!("│  Wall time             │ {:>33} ms │", wall_ms);
    println!("└────────────────────────┴─────────────────────────────────────┘");
}

// ── Entry point ───────────────────────────────────────────────────

/// Run all extended benchmarks.
pub fn run_extended_benchmarks() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  SESSION 28 — EXTENDED BENCHMARKS                            ║");
    println!("║  TTL / RateLimit / Discovery / AEDA / Queue / Pipeline       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    bench_ttl_pruning();
    bench_rate_limiter();
    bench_discovery();
    bench_aeda_latency();
    bench_priority_queue();
    bench_pipeline_e2e();

    println!();
    println!("  ✅ Session 28 extended benchmarks complete.");
    println!("  These numbers complete the quantitative claims for the paper.");
}
