//! `ReplayFilter` against an independent reference model: every verdict, the number of
//! tracked origins and the number of peer records must agree.
#![no_main]

use arbitrary::Arbitrary;
use crci_core::replay::{ReplayFilter, ReplayVerdict, MAX_PEERS_PER_ORIGIN};
use libfuzzer_sys::fuzz_target;
use std::collections::{BTreeSet, HashMap};

#[derive(Arbitrary, Debug)]
enum Op {
    Check {
        origin: u8,
        seq: u16,
        mode: u8,
        sender: u8,
    },
    Forget {
        origin: u8,
    },
}

#[derive(Arbitrary, Debug)]
struct Input {
    window: u16,
    ops: Vec<Op>,
}

/// Reference: per origin the highest seq and, per attributed sender (None = the overflow
/// record), the seqs inside the window.
#[derive(Default)]
struct Model {
    max: u64,
    peers: Vec<(Option<u8>, BTreeSet<u64>)>,
}

#[derive(Debug)]
enum Want {
    Accept,
    Redundant,
    Replayed,
    OutOfWindow,
}

fn seq_of(seq: u16, mode: u8) -> u64 {
    match mode % 3 {
        0 => seq as u64,
        1 => (seq as u64) << 40,
        _ => u64::MAX - seq as u64,
    }
}

fn record(m: &mut Model, seq: u64, sender: u8) {
    let known = m.peers.iter().any(|(p, _)| *p == Some(sender));
    let who = if !known && m.peers.len() >= MAX_PEERS_PER_ORIGIN {
        None
    } else {
        Some(sender)
    };
    match m.peers.iter_mut().find(|(p, _)| *p == who) {
        Some((_, set)) => {
            set.insert(seq);
        }
        None => m.peers.push((who, BTreeSet::from([seq]))),
    }
}

fuzz_target!(|input: Input| {
    let w = (input.window % 300) as u64;
    let mut filter = ReplayFilter::new();
    filter.window_size = w;
    let mut model: HashMap<u8, Model> = HashMap::new();

    for op in input.ops.iter().take(4096) {
        match *op {
            Op::Forget { origin } => {
                filter.forget(&origin.to_string());
                model.remove(&origin);
            }
            Op::Check {
                origin,
                seq,
                mode,
                sender,
            } => {
                let seq = seq_of(seq, mode);
                let got = filter.check_and_record(&origin.to_string(), seq, &format!("s{sender}"));
                let want = match model.get_mut(&origin) {
                    None => {
                        let mut m = Model {
                            max: seq,
                            peers: Vec::new(),
                        };
                        record(&mut m, seq, sender);
                        model.insert(origin, m);
                        Want::Accept
                    }
                    Some(m) if seq > m.max => {
                        m.max = seq;
                        let floor = m.max.saturating_sub(w);
                        for (_, set) in &mut m.peers {
                            set.retain(|s| *s >= floor);
                        }
                        m.peers.retain(|(_, set)| !set.is_empty());
                        record(m, seq, sender);
                        Want::Accept
                    }
                    Some(m) if (seq as u128) + (w as u128) < m.max as u128 => Want::OutOfWindow,
                    Some(m) => {
                        let replay = m
                            .peers
                            .iter()
                            .any(|(p, set)| *p == Some(sender) && set.contains(&seq));
                        let held = m.peers.iter().any(|(_, set)| set.contains(&seq));
                        record(m, seq, sender);
                        if replay {
                            Want::Replayed
                        } else if held {
                            Want::Redundant
                        } else {
                            Want::Accept
                        }
                    }
                };
                let ok = matches!(
                    (&got, &want),
                    (ReplayVerdict::Accept, Want::Accept)
                        | (ReplayVerdict::Redundant { .. }, Want::Redundant)
                        | (ReplayVerdict::Replayed { .. }, Want::Replayed)
                        | (ReplayVerdict::OutOfWindow { .. }, Want::OutOfWindow)
                );
                assert!(
                    ok,
                    "origin {origin} seq {seq} sender {sender} w {w}: got {got:?}, want {want:?}"
                );
            }
        }
        assert_eq!(filter.tracked_origins(), model.len());
        let records: usize = model.values().map(|m| m.peers.len()).sum();
        assert_eq!(filter.peer_records(), records, "peer records");
    }
});
