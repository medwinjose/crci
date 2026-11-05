//! Run one mesh simulation: `mesh_sim SCENARIO.json OUTCOME.json`.
//!
//! `scripts/sim/run_sim.py` writes the scenarios and collects the outcomes. `NodeRuntime` prints
//! every event, so send stdout to /dev/null for large runs.

mod harness;

use serde::Serialize;
use std::error::Error;
use std::time::Instant;

#[derive(Serialize)]
struct Report {
    #[serde(flatten)]
    outcome: harness::Outcome,
    runtime_s: f64,
    /// Peak resident memory of this process (Linux only).
    peak_rss_kib: Option<u64>,
}

fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, scenario_path, outcome_path] = args.as_slice() else {
        return Err("usage: mesh_sim SCENARIO.json OUTCOME.json".into());
    };
    let scenario: harness::Scenario = serde_json::from_slice(&std::fs::read(scenario_path)?)?;
    let started = Instant::now();
    let outcome = harness::run(&scenario);
    let report = Report {
        outcome,
        runtime_s: started.elapsed().as_secs_f64(),
        peak_rss_kib: peak_rss_kib(),
    };
    std::fs::write(outcome_path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
