//! `LogGate` and `FairLogGate`: held-back accounting and the documented line bound.
#![no_main]

use arbitrary::Arbitrary;
use crci_core::runtime::{FairLogGate, LogGate, FAIR_LOG_WINDOW};
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Arbitrary, Debug)]
struct Step {
    key: u8,
    wait_ms: u16,
}

#[derive(Arbitrary, Debug)]
struct Input {
    per_second: u8,
    soft: u8,
    floor: u8,
    steps: Vec<Step>,
}

fuzz_target!(|input: Input| {
    let mut now = Instant::now();
    let soft = input.soft as u32;
    let floor = input.floor as u32;
    let mut simple = LogGate::new(input.per_second as u32);
    let mut fair = FairLogGate::new(soft, floor);

    let mut window_start: Option<Instant> = None;
    let mut printed = 0u64;
    let mut senders: HashSet<u8> = HashSet::new();
    let mut refused = 0u64;

    for step in input.steps.iter().take(8192) {
        now += Duration::from_millis(step.wait_ms as u64);
        let _ = simple.pass(&step.key.to_string(), now);

        if window_start.is_none_or(|s| now.saturating_duration_since(s) >= FAIR_LOG_WINDOW) {
            window_start = Some(now);
            printed = 0;
            senders.clear();
        }
        senders.insert(step.key);
        match fair.pass(&step.key.to_string(), now) {
            Some(_) => printed += 1,
            None => refused += 1,
        }
        // Documented bound: about 2 x soft + floor x senders lines per window.
        let bound = 2 * soft as u64 + floor as u64 * senders.len() as u64;
        assert!(
            printed <= bound,
            "{printed} lines in a window, bound {bound}"
        );
        assert_eq!(fair.held_back_total(), refused, "held-back total");
    }
});
