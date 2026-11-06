//! Replay-window ablation: what the window size W buys and costs, from one build and one command.
//!
//! `cargo bench -p crci-node --bench ablation_bench` drives a real `NodeRuntime` (signed
//! messages, the full admission path, a simulated clock) and writes
//! `benches/results/ablation_window.csv` and the two tables printed at the end.
//!
//! Reordering: an honest origin sends 200 messages, 4 per second (below the per-origin budget),
//! through one relay, in blocks of D + 1 delivered in reverse, so the latest message of a block
//! arrives first and the earliest D sequence numbers behind it. A message is delivered when it
//! is inside the window, i.e. when D <= W. Late packets outside the window are dropped without a
//! strike, so W = 0 (strict-monotonic) costs messages, not bans.
//!
//! Replay lag: a relay delivers message A, the origin's sequence advances by L, and the relay
//! delivers A again five times. A repeat inside the window (L <= W) is a strike and three ban the
//! relay; an older one is dropped as stale, so the relay is never banned.
//!
//! State: the window keeps at most W + 1 sequence numbers per origin (`window_entries`).
//!
//! Strike threshold k against a paced attacker: one replay every P seconds for six simulated
//! hours. Recovery forgives `penalty / 0.0005` seconds' worth of score per strike, and the
//! penalty is `0.45 / k`, so a larger k forgives a strike sooner and tolerates a faster pace
//! without a ban. Written to `benches/results/ablation_paced.csv`.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::SybilGuard;
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NODE: &str = "node-a";
const ORIGIN: &str = "origin-h";
const MESSAGES: u64 = 200;
/// Spacing of the origin's messages: 4 per second, under the per-origin budget of 5.
const SPACING: Duration = Duration::from_millis(250);
const WINDOWS: [u64; 6] = [0, 8, 16, 32, 64, 128];
const DEPTHS: [u64; 8] = [0, 1, 2, 4, 8, 16, 32, 64];
const LAGS: [u64; 5] = [1, 8, 32, 64, 128];
const REPLAYS: usize = 5;
const KS: [u32; 3] = [1, 3, 5];
const PACES_S: [u64; 12] = [30, 60, 90, 120, 150, 180, 210, 240, 270, 300, 330, 600];
const HORIZON_S: u64 = 6 * 3600;

struct Cell {
    sent: u64,
    delivered: u64,
    strikes: u64,
    banned: bool,
    window_entries: usize,
}

fn new_node(window: u64) -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new(NODE, "zone-a", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.replay_filter.window_size = window;
    (node, inbox)
}

fn signed(origin: &Identity, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        &format!("{ORIGIN}-{seq}"),
        ORIGIN,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, origin)
}

fn deliver(
    node: &mut NodeRuntime,
    inbox: &SharedInbox,
    relay: &str,
    wire: &WireMessage,
) -> Result<(), Box<dyn Error>> {
    let bytes = serde_json::to_vec(wire)?;
    inbox
        .lock()
        .map_err(|_| "inbox poisoned")?
        .entry(NODE.to_string())
        .or_default()
        .push((relay.to_string(), bytes));
    node.process_inbox();
    Ok(())
}

fn finish(node: &mut NodeRuntime, relays: &[&str], sent: u64) -> Cell {
    let banned = relays
        .iter()
        .any(|r| node.sybil_guard.is_banned(&r.to_string()));
    Cell {
        sent,
        delivered: node.messages_handled,
        strikes: node.byzantine_events,
        banned,
        window_entries: node
            .replay_filter
            .digest(usize::MAX)
            .iter()
            .map(|(_, _, held)| held.len())
            .sum(),
    }
}

fn reorder(window: u64, depth: u64) -> Result<Cell, Box<dyn Error>> {
    let (mut node, inbox) = new_node(window);
    let origin = Identity::new(ORIGIN);
    let block = depth + 1;
    let mut start = 1;
    while start <= MESSAGES {
        let end = (start + block - 1).min(MESSAGES);
        for seq in (start..=end).rev() {
            deliver(&mut node, &inbox, "relay-h", &signed(&origin, seq))?;
            node.sybil_guard.advance_time(SPACING);
        }
        start = end + 1;
    }
    Ok(finish(&mut node, &["relay-h"], MESSAGES))
}

fn replay(window: u64, lag: u64) -> Result<Cell, Box<dyn Error>> {
    let (mut node, inbox) = new_node(window);
    let origin = Identity::new(ORIGIN);
    let victim = signed(&origin, 1);
    deliver(&mut node, &inbox, "relay-a", &victim)?;
    for seq in 2..=lag + 1 {
        node.sybil_guard.advance_time(SPACING);
        deliver(&mut node, &inbox, "relay-h", &signed(&origin, seq))?;
    }
    for _ in 0..REPLAYS {
        deliver(&mut node, &inbox, "relay-a", &victim)?;
    }
    // The replaying relay delivered one message and repeated it REPLAYS times.
    Ok(finish(&mut node, &["relay-a"], 1 + lag))
}

/// One strike every `pace_s` seconds against a guard with threshold `k`: (banned, strikes made,
/// second of the ban).
fn paced(k: u32, pace_s: u64) -> (bool, u64, Option<u64>) {
    let mut guard = SybilGuard::new(0);
    guard.use_manual_clock();
    guard.set_strikes_to_ban(k);
    let attacker = "attacker".to_string();
    let (mut t, mut strikes) = (0, 0);
    while t + pace_s <= HORIZON_S {
        guard.advance_time(Duration::from_secs(pace_s));
        t += pace_s;
        strikes += 1;
        guard.report_violation(&attacker);
        if guard.is_banned(&attacker) {
            return (true, strikes, Some(t));
        }
    }
    (false, strikes, None)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut csv =
        String::from("experiment,window,param,sent,delivered,strikes,banned,window_entries\n");
    let mut reorder_table = String::from("| Reorder depth D |");
    let mut replay_table = String::from("| Replay lag L |");
    let mut sep = String::from("| :--- |");
    for w in WINDOWS {
        write!(reorder_table, " W = {w} |")?;
        write!(replay_table, " W = {w} |")?;
        sep.push_str(" :--- |");
    }
    writeln!(reorder_table)?;
    writeln!(replay_table)?;
    writeln!(reorder_table, "{sep}")?;
    writeln!(replay_table, "{sep}")?;

    for depth in DEPTHS {
        write!(reorder_table, "| {depth} |")?;
        for window in WINDOWS {
            let c = reorder(window, depth)?;
            writeln!(
                csv,
                "reorder,{window},{depth},{},{},{},{},{}",
                c.sent, c.delivered, c.strikes, c.banned, c.window_entries
            )?;
            let pct = 100.0 * c.delivered as f64 / c.sent as f64;
            write!(
                reorder_table,
                " {pct:.0} %{} |",
                if c.banned || c.strikes > 0 {
                    " (struck)"
                } else {
                    ""
                }
            )?;
        }
        writeln!(reorder_table)?;
    }
    for lag in LAGS {
        write!(replay_table, "| {lag} |")?;
        for window in WINDOWS {
            let c = replay(window, lag)?;
            writeln!(
                csv,
                "replay,{window},{lag},{},{},{},{},{}",
                c.sent, c.delivered, c.strikes, c.banned, c.window_entries
            )?;
            write!(
                replay_table,
                " {} |",
                if c.banned {
                    "banned"
                } else if c.strikes > 0 {
                    "struck"
                } else {
                    "ignored"
                }
            )?;
        }
        writeln!(replay_table)?;
    }

    let mut paced_csv = String::from(
        "k,pace_s,banned,strikes,banned_at_s
",
    );
    let mut paced_table = String::from("| Seconds between replays |");
    let mut paced_sep = String::from("| :--- |");
    for k in KS {
        write!(paced_table, " k = {k} |")?;
        paced_sep.push_str(" :--- |");
    }
    writeln!(paced_table)?;
    writeln!(paced_table, "{paced_sep}")?;
    for pace in PACES_S {
        write!(paced_table, "| {pace} |")?;
        for k in KS {
            let (banned, strikes, at) = paced(k, pace);
            writeln!(
                paced_csv,
                "{k},{pace},{banned},{strikes},{}",
                at.map(|t| t.to_string()).unwrap_or_default()
            )?;
            match at {
                Some(t) => write!(paced_table, " banned after {strikes} (at {t} s) |")?,
                None => write!(paced_table, " never ({strikes} in 6 h) |")?,
            }
        }
        writeln!(paced_table)?;
    }

    std::fs::create_dir_all("benches/results")?;
    std::fs::write("benches/results/ablation_paced.csv", &paced_csv)?;
    std::fs::write("benches/results/ablation_window.csv", &csv)?;
    println!("\n=== Window ablation: share of an honest origin's 200 messages delivered ===");
    println!("{reorder_table}");
    println!(
        "=== Window ablation: five repeats of a message the origin has since moved L past ==="
    );
    println!("{replay_table}");
    println!("=== Strike threshold: one replay every P seconds for six simulated hours ===");
    println!("{paced_table}");
    println!("Wrote benches/results/ablation_window.csv and ablation_paced.csv");
    Ok(())
}
