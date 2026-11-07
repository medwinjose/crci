//! The in-process mesh simulation: determinism, and the outcomes that can be worked out by hand.

#[path = "../examples/mesh_sim/harness.rs"]
mod harness;

use harness::{run, Config, Scenario};

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
        config: Config::default(),
        contacts: Vec::new(),
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

#[test]
fn a_contact_trace_limits_delivery_to_the_time_respecting_paths_and_strikes_nobody() {
    // 0-1 are in contact for the first 30 s, 1-2 from 40 s: what node 0 sends later never
    // reaches node 2, and nothing crosses a link while it is down.
    let mut s = scenario(3, vec![(0, 1), (1, 2)], Vec::new(), 1);
    s.fault_at_ms = s.window_ms;
    s.contacts = vec![(0, 30_000, 0, 1), (40_000, 65_000, 1, 2)];
    let out = run(&s);
    assert_eq!(out.false_evictions, 0);
    assert_eq!(out.honest_strikes, 0);
    assert!(out.sync_requests > 0);
    // Without the trace every message reaches both receivers.
    let mut open = scenario(3, vec![(0, 1), (1, 2)], Vec::new(), 1);
    open.fault_at_ms = open.window_ms;
    assert_eq!(run(&open).delivery_pre, Some(1.0));
    // Node 1 holds about half the messages and node 2 only some of those: well below 1.
    assert!(out.delivery_pre.is_some_and(|d| d > 0.2 && d < 0.8));
    assert_eq!(run(&s), out);
}

fn with_config(mut s: Scenario, config: Config) -> Scenario {
    s.config = config;
    s
}

#[test]
fn the_baselines_strike_a_replayer_but_never_ban_it() {
    for defence in ["static-bucket", "vanilla"] {
        let config = Config {
            defence: defence.to_string(),
            ..Config::default()
        };
        let out = run(&with_config(
            scenario(20, ring_edges(20), vec![5], 2),
            config,
        ));
        assert_eq!(out.coverage_banned, 0, "{defence}");
        assert!(
            out.attacker_strikes > 4,
            "{defence}: strikes are still counted"
        );
        // Nobody is banned, so the attacker's frames keep being admitted.
        assert!(out.attacker_frames_admitted > 100, "{defence}");
        assert_eq!(out.delivery_pre, Some(1.0), "{defence}");
    }
}

#[test]
fn a_lower_strike_threshold_bans_sooner_and_admits_fewer_attacker_frames() {
    let run_k = |k: u32| {
        run(&with_config(
            scenario(20, ring_edges(20), vec![5], 2),
            Config {
                strikes_to_ban: Some(k),
                ..Config::default()
            },
        ))
    };
    let (k1, k3, k5) = (run_k(1), run_k(3), run_k(5));
    for out in [&k1, &k3, &k5] {
        assert_eq!(
            out.coverage_banned, out.coverage_links,
            "every k bans the attacker"
        );
    }
    assert!(k1.attacker_frames_admitted < k3.attacker_frames_admitted);
    assert!(k3.attacker_frames_admitted < k5.attacker_frames_admitted);
    assert!(k1.evict_sim_max_ms <= k3.evict_sim_max_ms);
    assert!(k3.evict_sim_max_ms <= k5.evict_sim_max_ms);
}

#[test]
fn injected_duplicates_strike_honest_peers_and_the_threshold_decides_how_many_are_banned() {
    let run_q = |q: f64, k: u32| {
        run(&with_config(
            scenario(20, ring_edges(20), Vec::new(), 1),
            Config {
                strikes_to_ban: Some(k),
                honest_dup_prob: q,
                dup_seed: 7,
                ..Config::default()
            },
        ))
    };
    let clean = run_q(0.0, 1);
    assert_eq!(clean.honest_strikes, 0);
    assert_eq!(clean.false_evictions, 0);

    let (k1, k3, k5) = (run_q(0.1, 1), run_q(0.1, 3), run_q(0.1, 5));
    assert!(k1.honest_strikes > 0, "duplicates must strike honest peers");
    assert!(k1.false_evictions > 0, "k = 1 bans on the first duplicate");
    assert!(k1.false_evictions >= k3.false_evictions);
    assert!(k3.false_evictions >= k5.false_evictions);
    assert!(k1.honest_links_ever_banned > k5.honest_links_ever_banned);
    assert!(k1.honest_links_total > 0);
    assert_eq!(
        run_q(0.1, 3),
        k3,
        "the injected fault is seeded: same input, same outcome"
    );
}

#[test]
fn goodput_and_verifications_are_reported() {
    let out = run(&scenario(20, ring_edges(20), vec![5], 2));
    assert!(out.verifications > 0);
    assert!(out.honest_deliveries > 0);
    assert!(out.delivery_attack.is_some_and(|d| d > 0.99));
    assert!(out.goodput_per_s > 0.0);
}
