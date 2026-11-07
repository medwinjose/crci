//! `SybilGuard` under many transport identities: no panic, and the record table stays bounded.
#![no_main]

use arbitrary::Arbitrary;
use crci_core::sybil::{SybilGuard, MAX_TRACKED_PEERS};
use libfuzzer_sys::fuzz_target;
use std::time::Duration;

#[derive(Arbitrary, Debug)]
enum Op {
    Check(u16),
    Violation(u16),
    Banned(u16),
    Advance(u16),
    /// Many new identities at once: the table limit needs over a thousand.
    Burst {
        start: u16,
        count: u16,
    },
}

fn peer(n: u16) -> String {
    // Half are socket addresses, so the subnet correlation runs.
    if n % 2 == 0 {
        format!("10.0.{}.{}:9000", (n >> 8) as u8, n as u8)
    } else {
        format!("node{n}#key")
    }
}

fuzz_target!(|ops: Vec<Op>| {
    let mut guard = SybilGuard::new(0);
    guard.use_manual_clock();
    for op in ops.iter().take(8192) {
        match *op {
            Op::Check(n) => {
                let _ = guard.check(&peer(n));
            }
            Op::Violation(n) => guard.report_violation(&peer(n)),
            Op::Banned(n) => {
                let _ = guard.is_banned(&peer(n));
            }
            Op::Advance(ms) => guard.advance_time(Duration::from_millis(ms as u64 * 100)),
            Op::Burst { start, count } => {
                for i in 0..(count % 3000) {
                    let n = start.wrapping_add(i);
                    if i % 4 == 0 {
                        guard.report_violation(&peer(n));
                    } else {
                        let _ = guard.check(&peer(n));
                    }
                }
            }
        }
        assert!(
            guard.tracked_peers() <= MAX_TRACKED_PEERS,
            "records over the limit"
        );
    }
});
