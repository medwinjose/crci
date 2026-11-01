//! Paced replays against strike-damped, time-based reputation recovery.
//!
//! One receiver (`NodeRuntime`), one adversarial transport peer `atk` that authors its own
//! messages. The adversary sends a fresh, validly signed message every `FILLER_S` seconds
//! and, every `period` seconds, resends its latest message unchanged: an exact duplicate
//! from the same transport peer inside the replay window, i.e. one strike. Time is
//! simulated (`SybilGuard::use_manual_clock` + `advance_time`), so each run covers
//! `HORIZON_S` seconds and gives the same output every time.
//!
//! Outputs (written to `benches/results/`):
//! - `recovery_paced_sweep.csv`: one row per replay period.
//! - `recovery_trajectory.csv`: the adversary's score every `SAMPLE_S` seconds for a few
//!   periods.
//!
//! Run: `cargo bench -p crci-node --bench recovery_bench`

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, WireMessage};
use crci_core::sybil::{RECOVERY_PER_SEC, STRIKE_PENALTY};
use crci_core::transport::SharedInbox;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ATK: &str = "atk";
const HORIZON_S: u64 = 6 * 3_600;
const FILLER_S: u64 = 10;
const SAMPLE_S: u64 = 10;
const PERIODS_S: [u64; 14] = [
    1, 15, 30, 60, 120, 180, 240, 270, 290, 300, 330, 450, 600, 1_200,
];
const TRAJECTORY_PERIODS_S: [u64; 3] = [120, 290, 330];

struct Outcome {
    period_s: u64,
    replays_sent: u64,
    strikes: u64,
    first_ban_s: Option<u64>,
    banned_seconds: u64,
    final_score: f32,
    min_score: f32,
    trajectory: Vec<(u64, f32)>,
}

fn deliver(node: &mut NodeRuntime, inbox: &SharedInbox, wire: &WireMessage) {
    let bytes = match serde_json::to_vec(wire) {
        Ok(b) => b,
        Err(_) => return,
    };
    if let Ok(mut guard) = inbox.lock() {
        guard
            .entry("rx".to_string())
            .or_default()
            .push((ATK.to_string(), bytes));
    }
    node.process_inbox();
}

fn run(period_s: u64) -> Outcome {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("rx", "zone-a", inbox.clone());
    node.sybil_guard.use_manual_clock();
    let key = Identity::new(ATK);
    let atk = ATK.to_string();

    let mut seq = 0u64;
    let mut latest: Option<WireMessage> = None;
    let mut out = Outcome {
        period_s,
        replays_sent: 0,
        strikes: 0,
        first_ban_s: None,
        banned_seconds: 0,
        final_score: 0.0,
        min_score: f32::MAX,
        trajectory: Vec::new(),
    };

    // One simulated second per step: filler, replay and sampling all fall on whole seconds.
    for t in 0..=HORIZON_S {
        if t > 0 {
            node.sybil_guard.advance_time(Duration::from_secs(1));
        }
        if t % FILLER_S == 0 {
            seq += 1;
            let mut msg = Message::new(
                &format!("atk-{seq}"),
                ATK,
                Signal::new(3, false, false, true, 3, Visibility::Direct),
                None,
            );
            msg.seq = seq;
            let wire = WireMessage::from_message(&msg, &key);
            deliver(&mut node, &inbox, &wire);
            latest = Some(wire);
        }
        if t > 0 && t % period_s == 0 {
            if let Some(wire) = latest.clone() {
                let before = node.byzantine_events;
                deliver(&mut node, &inbox, &wire);
                out.replays_sent += 1;
                out.strikes += node.byzantine_events - before;
            }
        }
        let score = node.sybil_guard.reputation(&atk);
        out.min_score = out.min_score.min(score);
        if node.sybil_guard.is_banned(&atk) {
            out.banned_seconds += 1;
            out.first_ban_s.get_or_insert(t);
        }
        if t % SAMPLE_S == 0 {
            out.trajectory.push((t, score));
        }
    }
    out.final_score = node.sybil_guard.reputation(&atk);
    out
}

fn main() -> std::io::Result<()> {
    let forgiveness_s = STRIKE_PENALTY / RECOVERY_PER_SEC;
    println!(
        "Paced replays vs strike-damped recovery: horizon {} s, filler every {} s, \
         one strike forgiven in {:.0} s",
        HORIZON_S, FILLER_S, forgiveness_s
    );

    let mut sweep = String::from(
        "period_s,replays_sent,strikes,first_ban_s,banned_fraction,strikes_per_hour,min_score,final_score\n",
    );
    let mut trajectory = String::from("period_s,t_s,score\n");
    println!(
        "{:>9} {:>8} {:>8} {:>12} {:>10} {:>13} {:>9}",
        "period_s", "replays", "strikes", "first_ban_s", "banned_%", "strikes_per_h", "min_score"
    );
    for period_s in PERIODS_S {
        let o = run(period_s);
        let banned_fraction = o.banned_seconds as f64 / (HORIZON_S + 1) as f64;
        let strikes_per_hour = o.strikes as f64 * 3_600.0 / HORIZON_S as f64;
        let first_ban = o
            .first_ban_s
            .map_or_else(|| "never".to_string(), |t| t.to_string());
        println!(
            "{:>9} {:>8} {:>8} {:>12} {:>9.1}% {:>13.2} {:>9.3}",
            o.period_s,
            o.replays_sent,
            o.strikes,
            first_ban,
            banned_fraction * 100.0,
            strikes_per_hour,
            o.min_score
        );
        let _ = writeln!(
            sweep,
            "{},{},{},{},{:.4},{:.2},{:.3},{:.3}",
            o.period_s,
            o.replays_sent,
            o.strikes,
            o.first_ban_s.map_or_else(String::new, |t| t.to_string()),
            banned_fraction,
            strikes_per_hour,
            o.min_score,
            o.final_score
        );
        if TRAJECTORY_PERIODS_S.contains(&period_s) {
            for (t, score) in &o.trajectory {
                let _ = writeln!(trajectory, "{},{},{:.4}", period_s, t, score);
            }
        }
    }
    std::fs::create_dir_all("benches/results")?;
    std::fs::write("benches/results/recovery_paced_sweep.csv", sweep)?;
    std::fs::write("benches/results/recovery_trajectory.csv", trajectory)?;
    println!("\nWrote benches/results/recovery_paced_sweep.csv and recovery_trajectory.csv");
    Ok(())
}
