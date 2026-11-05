//! The in-process mesh simulation: determinism, and the outcomes that can be worked out by hand.

#[path = "../examples/mesh_sim/harness.rs"]
mod harness;

use harness::{run, Scenario};

/// Ring with chords on `n` nodes: every node has degree 4, so no single removal disconnects it.
fn ring_edges(n: usize) -> Vec<(usize, usize)> {
    (0..n)
        .flat_map(|i| [(i, (i + 1) % n), (i, (i + 2) % n)])
        .collect()
}

/// 60 s at 1 message/s from node 0, a fault at 20 s, anti-entropy every 30 s.
fn scenario(
    n: usize,
    edges: Vec<(usize, usize)>,
    attackers: Vec<usize>,
    copies: usize,
) -> Scenario {
    let mut plan = vec![(0, 1000, 0); n];
    plan[0] = (60, 1000, 0);
    Scenario {
        n,
        edges,
        source: Some(0),
        attackers,
        downstream: Vec::new(),
        plan,
        sync_phase_ms: (0..n as u64).map(|i| 1000 + 977 * i).collect(),
        window_ms: 60_000,
        fault_at_ms: 20_000,
        drain_ms: 5_000,
        replay_copies: copies,
        tick_ms: 10,
    }
}

#[test]
fn replaying_relay_is_banned_by_every_neighbour_and_nobody_else_is() {
    let out = run(&scenario(20, ring_edges(20), vec![5], 2));
    assert_eq!(out.coverage_links, 4);
    assert_eq!(out.coverage_banned, 4);
    assert_eq!(out.attackers_fully_banned, 1);
    assert_eq!(out.false_evictions, 0);
    assert_eq!(out.honest_strikes, 0);
    assert_eq!(out.non_neighbour_records, 0);
    assert_eq!(out.delivery_pre, Some(1.0));
    assert_eq!(out.delivery_post, Some(1.0));
    assert_eq!(out.accepted_via_attacker_post, 0);
    // Three relayed messages, one a second, the first right at the fault.
    assert!(out.evict_sim_max_ms.is_some_and(|ms| ms <= 3_000));
}

#[test]
fn an_attacker_that_does_not_repeat_is_not_struck() {
    let out = run(&scenario(20, ring_edges(20), vec![5], 1));
    assert_eq!(out.coverage_banned, 0);
    assert_eq!(out.attacker_strikes, 0);
    assert_eq!(out.false_evictions, 0);
}

#[test]
fn no_attacker_means_no_strike_and_full_delivery() {
    let out = run(&scenario(20, ring_edges(20), Vec::new(), 2));
    assert_eq!(out.attackers, 0);
    assert_eq!(out.honest_strikes, 0);
    assert_eq!(out.false_evictions, 0);
    assert_eq!(out.delivery_pre, Some(1.0));
    assert_eq!(out.delivery_post, Some(1.0));
    assert_eq!(out.rate_drops + out.origin_drops, 0);
    assert!(out.sync_requests > 0);
}

#[test]
fn attackers_on_a_cut_split_the_honest_nodes_and_delivery_stays_inside_the_component() {
    // Path 0-1-2-3-4 plus a triangle at each end: node 2 is the only way across.
    let edges = vec![
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (0, 5),
        (1, 5),
        (3, 6),
        (4, 6),
    ];
    let out = run(&scenario(7, edges, vec![2], 2));
    assert_eq!(out.honest_components, 2);
    assert_eq!(out.coverage_banned, 2);
    assert_eq!(out.delivery_post_reachable, Some(1.0));
    // Node 0's component holds {1, 5} of the 5 other honest nodes.
    assert!(out.delivery_post.is_some_and(|d| (d - 0.4).abs() < 1e-9));
    assert_eq!(out.false_evictions, 0);
}

#[test]
fn same_scenario_same_outcome() {
    let a = run(&scenario(30, ring_edges(30), vec![3, 11], 2));
    let b = run(&scenario(30, ring_edges(30), vec![3, 11], 2));
    assert_eq!(a, b);
    assert!(a.frames > 0);
}
