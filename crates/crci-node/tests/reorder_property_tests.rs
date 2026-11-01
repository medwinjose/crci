//! Property-based tests: honest reordering never evicts a peer.
//!
//! Model. Each origin numbers its messages with increasing sequence numbers; the receiver
//! sees some of them (loss allowed), each through one or more honest relays, and every
//! relay delivers a given message at most once. Deliveries arrive reordered. The
//! *reordering depth* of a delivery is `seq_max − seq`, where `seq_max` is the highest
//! sequence number from that origin delivered before it; the depth of a schedule is the
//! largest such value.
//!
//! Schedules are built by giving every delivery a key `seq + jitter` with
//! `jitter ∈ [0, D]` and delivering in key order. Any earlier delivery `t` then satisfies
//! `t ≤ t + jitter_t ≤ seq + jitter_seq ≤ seq + D`, so the depth is at most `D`
//! (each test also measures it).
//!
//! Properties, with W = 64 the replay window:
//! 1. D ≤ W: every first delivery is accepted, every further copy (from another relay) is
//!    redundant; no verdict is a replay or out of window. On the full `NodeRuntime` path,
//!    every message is accepted once and no relay loses reputation.
//! 2. Any depth: no relay is ever struck. Packets older than the window are dropped, not
//!    penalised.
//!
//! Each property runs `CASES` generated cases.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::replay::{ReplayFilter, ReplayVerdict, DEFAULT_WINDOW_SIZE};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::transport::SharedInbox;
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

const CASES: u32 = 256;
const W: u64 = DEFAULT_WINDOW_SIZE;

/// One delivery: (origin index, sequence number, relay index).
type Delivery = (usize, u64, usize);

/// Messages of one origin: each sequence number with the relays (bitmask, non-empty)
/// that deliver it, one jitter per relay, and one tie-breaker per relay.
type OriginPlan = Vec<(u64, u8, Vec<u64>, Vec<u32>)>;

fn origin_plan(
    max_msgs: usize,
    relays: usize,
    max_jitter: u64,
) -> impl Strategy<Value = OriginPlan> {
    // Sequence numbers: a start plus gaps, mostly small (consecutive numbering with some
    // loss), sometimes large, so schedules span more than the window.
    let gap = prop_oneof![1u64..=3, 1u64..=80];
    // Jitter: the extremes 0 and `max_jitter` are drawn often, so schedules hit the
    // depth bound exactly.
    let jitter = prop_oneof![Just(0u64), Just(max_jitter), 0..=max_jitter];
    (1u64..50, prop::collection::vec(gap, 1..=max_msgs)).prop_flat_map(move |(start, gaps)| {
        let seqs: Vec<u64> = gaps
            .iter()
            .scan(start, |acc, g| {
                let s = *acc;
                *acc += g;
                Some(s)
            })
            .collect();
        let n = seqs.len();
        (
            Just(seqs),
            prop::collection::vec(1u8..(1u8 << relays), n),
            prop::collection::vec(prop::collection::vec(jitter.clone(), relays), n),
            prop::collection::vec(prop::collection::vec(any::<u32>(), relays), n),
        )
            .prop_map(|(seqs, masks, jitters, ties)| {
                seqs.into_iter()
                    .zip(masks)
                    .zip(jitters)
                    .zip(ties)
                    .map(|(((s, m), j), t)| (s, m, j, t))
                    .collect()
            })
    })
}

/// Arrival order for all origins: deliveries sorted by `seq + jitter`, ties broken at
/// random. Origins are interleaved by the same key.
fn schedule(plans: &[OriginPlan], relays: usize) -> Vec<Delivery> {
    let mut keyed = Vec::new();
    for (origin, plan) in plans.iter().enumerate() {
        for (seq, mask, jitters, ties) in plan {
            for relay in 0..relays {
                if mask & (1 << relay) != 0 {
                    keyed.push((seq + jitters[relay], ties[relay], (origin, *seq, relay)));
                }
            }
        }
    }
    keyed.sort_by_key(|&(key, tie, _)| (key, tie));
    keyed.into_iter().map(|(_, _, d)| d).collect()
}

/// Largest `seq_max − seq` over the schedule (per origin).
fn reordering_depth(deliveries: &[Delivery]) -> u64 {
    let mut seq_max: HashMap<usize, u64> = HashMap::new();
    let mut depth = 0;
    for &(origin, seq, _) in deliveries {
        let max = seq_max.entry(origin).or_insert(seq);
        depth = depth.max(max.saturating_sub(seq));
        *max = (*max).max(seq);
    }
    depth
}

/// Depth bound `D ≤ W` and a schedule built with jitter in `[0, D]`.
fn bounded_schedule(
    max_origins: usize,
    max_msgs: usize,
    max_relays: usize,
) -> impl Strategy<Value = (u64, usize, usize, Vec<Delivery>)> {
    // D = W, the bound under test, is drawn half the time.
    (prop_oneof![Just(W), 0..=W], 1..=max_origins, 1..=max_relays).prop_flat_map(
        move |(d, origins, relays)| {
            prop::collection::vec(origin_plan(max_msgs, relays, d), origins)
                .prop_map(move |plans| (d, origins, relays, schedule(&plans, relays)))
        },
    )
}

/// A schedule with jitter up to 1 000, far beyond the window.
fn unbounded_schedule(
    max_origins: usize,
    max_msgs: usize,
    max_relays: usize,
) -> impl Strategy<Value = (usize, usize, Vec<Delivery>)> {
    (1..=max_origins, 1..=max_relays).prop_flat_map(move |(origins, relays)| {
        prop::collection::vec(origin_plan(max_msgs, relays, 1_000), origins)
            .prop_map(move |plans| (origins, relays, schedule(&plans, relays)))
    })
}

fn origin_name(i: usize) -> String {
    format!("origin-{i}")
}

fn relay_name(i: usize) -> String {
    format!("relay-{i}")
}

/// Run the schedule through a fresh `NodeRuntime` as signed messages; returns the node
/// and the number of distinct messages.
fn run_signed(origins: usize, deliveries: &[Delivery]) -> (NodeRuntime, usize) {
    let keys: Vec<Identity> = (0..origins)
        .map(|i| Identity::new(&origin_name(i)))
        .collect();
    let mut wires: HashMap<(usize, u64), WireMessage> = HashMap::new();
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "zone-a", inbox.clone());
    {
        let mut guard = inbox.lock().unwrap();
        let queue = guard.entry("receiver".to_string()).or_default();
        for &(origin, seq, relay) in deliveries {
            let wire = wires.entry((origin, seq)).or_insert_with(|| {
                let name = origin_name(origin);
                let mut msg = Message::new(
                    &format!("{name}-{seq}"),
                    &name,
                    Signal::new(3, false, false, true, 3, Visibility::Direct),
                    None,
                );
                msg.seq = seq;
                WireMessage::from_message(&msg, &keys[origin])
            });
            queue.push((relay_name(relay), serde_json::to_vec(wire).unwrap()));
        }
    }
    node.process_inbox();
    (node, wires.len())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// Property 1 on the replay filter: with depth ≤ W, every first delivery is accepted
    /// and every other copy is redundant.
    #[test]
    fn filter_accepts_every_message_reordered_within_the_window(
        (d, _origins, _relays, deliveries) in bounded_schedule(3, 40, 4)
    ) {
        prop_assert!(reordering_depth(&deliveries) <= d);
        let mut filter = ReplayFilter::new();
        let mut seen = HashSet::new();
        for &(origin, seq, relay) in &deliveries {
            let verdict = filter.check_and_record(&origin_name(origin), seq, &relay_name(relay));
            if seen.insert((origin, seq)) {
                prop_assert_eq!(verdict, ReplayVerdict::Accept, "seq {} (D = {})", seq, d);
            } else {
                prop_assert_eq!(
                    verdict,
                    ReplayVerdict::Redundant { received_seq: seq },
                    "seq {} (D = {})", seq, d
                );
            }
        }
    }

    /// Property 1 on the full signed path: with depth ≤ W, every message is accepted
    /// exactly once and no relay is struck. At most 20 deliveries per relay, so the
    /// token bucket (burst 20) never drops honest traffic in the test.
    #[test]
    fn honest_relays_are_never_struck_and_lose_nothing_within_the_window(
        (d, origins, relays, deliveries) in bounded_schedule(2, 10, 3)
    ) {
        prop_assert!(reordering_depth(&deliveries) <= d);
        let (mut node, distinct) = run_signed(origins, &deliveries);

        prop_assert_eq!(node.byzantine_events, 0);
        prop_assert_eq!(
            node.metrics.messages_accepted.load(Ordering::Relaxed) as usize,
            distinct
        );
        for relay in 0..relays {
            let rep = node.sybil_guard.reputation(&relay_name(relay));
            prop_assert!(rep >= 0.5, "relay {} reputation {} (D = {})", relay, rep, d);
        }
    }

    /// Property 2 on the replay filter: at any depth, an honest schedule never produces a
    /// replay verdict (the only verdict that strikes).
    #[test]
    fn filter_never_reports_a_replay_for_honest_relays_at_any_depth(
        (_origins, _relays, deliveries) in unbounded_schedule(3, 40, 4)
    ) {
        let mut filter = ReplayFilter::new();
        for &(origin, seq, relay) in &deliveries {
            let verdict = filter.check_and_record(&origin_name(origin), seq, &relay_name(relay));
            let is_replay = matches!(verdict, ReplayVerdict::Replayed { .. });
            prop_assert!(!is_replay, "seq {} from relay {}", seq, relay);
        }
    }

    /// Property 2 on the full signed path: at any depth, no honest relay is struck.
    #[test]
    fn honest_relays_are_never_struck_at_any_depth(
        (origins, relays, deliveries) in unbounded_schedule(2, 10, 3)
    ) {
        let (mut node, _distinct) = run_signed(origins, &deliveries);
        for relay in 0..relays {
            let rep = node.sybil_guard.reputation(&relay_name(relay));
            prop_assert!(rep >= 0.5, "relay {} reputation {}", relay, rep);
        }
    }
}

/// The bound is tight: a message one step deeper than the window is dropped (no strike).
#[test]
fn depth_one_beyond_the_window_drops_without_a_strike() {
    let mut filter = ReplayFilter::new();
    assert!(filter.check_and_record("o", W + 1, "r").is_accept());
    assert!(filter.check_and_record("o", 1, "r").is_accept()); // depth W: accepted
    let mut filter = ReplayFilter::new();
    assert!(filter.check_and_record("o", W + 2, "r").is_accept());
    assert_eq!(
        filter.check_and_record("o", 1, "r"), // depth W + 1
        ReplayVerdict::OutOfWindow {
            received_seq: 1,
            window_floor: 2
        }
    );
}

/// The generator respects its bound on a fixed example.
#[test]
fn schedule_depth_never_exceeds_the_jitter_bound() {
    let plan: OriginPlan = (1..=50u64)
        .map(|s| (s, 0b11, vec![(s * 7) % 11, (s * 3) % 11], vec![s as u32, 0]))
        .collect();
    let deliveries = schedule(&[plan], 2);
    assert!(reordering_depth(&deliveries) <= 10);
    let distinct: BTreeSet<u64> = deliveries.iter().map(|d| d.1).collect();
    assert_eq!(distinct.len(), 50);
}
