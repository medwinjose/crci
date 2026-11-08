//! Ties the TLA+ protocol model to the code: random behaviours of the model
//! (`docs/tla/protocol-traces.jsonl`, written by `docs/tla/check.py --export-traces`) are replayed
//! through real `NodeRuntime` instances, one per honest node. Each model step becomes the same
//! call on the code, with real signatures and real serialized frames, and after every step every
//! node must hold what the model says: accepted messages, reputation and bans, the seen cache and
//! the replay window. Requests, answers and relays are compared too.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{message_key, NodeRuntime, WireMessage, SYNC_COOLDOWN, SYNC_MAX_ORIGINS};
use crci_core::transport::SharedInbox;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TRACES: &str = include_str!("../../../docs/tla/protocol-traces.jsonl");
/// One model Tick (1 unit of score = 0.01).
const TICK: Duration = Duration::from_secs(100);
/// Answering needs the 5 s cooldown to pass, which the model does not have; the recovery this
/// lets in stays well below one unit per trace.
const SCORE_TOLERANCE: f32 = 0.015;

#[derive(Default, Clone)]
struct Expected {
    delivered: BTreeSet<(String, u64)>,
    rep: BTreeMap<String, (i64, bool)>,
    seen: BTreeSet<(String, u64, String, String)>,
    window: BTreeMap<String, (u64, Vec<u64>)>,
}

#[derive(Default)]
struct Coverage {
    accepted: u64,
    strikes: u64,
    excused: u64,
    banned_drops: u64,
    forged: u64,
    stale: u64,
    stale_repeats: u64,
    own_echoes: u64,
    copies: u64,
    answers: u64,
    answered_frames: u64,
    ticks: u64,
    clears: u64,
    id_reuse: u64,
    own_id_new_seq: u64,
}

struct Replay {
    inbox: SharedInbox,
    nodes: BTreeMap<String, NodeRuntime>,
    expected: BTreeMap<String, Expected>,
    actual_delivered: BTreeMap<String, BTreeSet<(String, u64)>>,
    identities: HashMap<String, Identity>,
    wires: HashMap<(String, String, u64), WireMessage>,
    requests: HashMap<(String, String, u64), WireMessage>,
    seen_keys: HashMap<(String, String, u64), String>,
    trace: String,
    step: usize,
}

fn text(v: &Value, field: &str) -> String {
    v[field].as_str().unwrap().to_string()
}

fn pair(v: &Value) -> (String, u64) {
    (v[0].as_str().unwrap().to_string(), v[1].as_u64().unwrap())
}

impl Replay {
    fn new(trace: &Value, label: String) -> Replay {
        let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
        let mut nodes = BTreeMap::new();
        let mut expected = BTreeMap::new();
        let mut actual_delivered = BTreeMap::new();
        let window = trace["w"].as_u64().unwrap();
        let edges: Vec<(String, String)> = trace["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e[0].as_str().unwrap().to_string(),
                    e[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        for honest in trace["honest"].as_array().unwrap() {
            let id = honest.as_str().unwrap();
            let mut node = NodeRuntime::new(id, "zone", inbox.clone());
            node.use_router = false;
            node.sybil_guard.use_manual_clock();
            node.replay_filter.window_size = window;
            let mut want = Expected::default();
            for (a, b) in &edges {
                let peer = if a == id {
                    b
                } else if b == id {
                    a
                } else {
                    continue;
                };
                node.peers.push(peer.clone());
                want.rep.insert(peer.clone(), (50, false));
            }
            nodes.insert(id.to_string(), node);
            expected.insert(id.to_string(), want);
            actual_delivered.insert(id.to_string(), BTreeSet::new());
        }
        Replay {
            inbox,
            nodes,
            expected,
            actual_delivered,
            identities: HashMap::new(),
            wires: HashMap::new(),
            requests: HashMap::new(),
            seen_keys: HashMap::new(),
            trace: label,
            step: 0,
        }
    }

    fn at(&self) -> String {
        format!("{}, step {}", self.trace, self.step)
    }

    /// The signing key of `origin`: a node's own, or the adversary's.
    fn sign(&mut self, id: &str, origin: &str, seq: u64) -> WireMessage {
        let mut msg = Message::new(
            id,
            origin,
            Signal::new(3, true, false, true, 3, Visibility::Direct),
            None,
        );
        msg.seq = seq;
        if let Some(node) = self.nodes.get(origin) {
            return WireMessage::from_message(&msg, &node.identity);
        }
        let identity = self
            .identities
            .entry(origin.to_string())
            .or_insert_with(|| Identity::new(origin));
        WireMessage::from_message(&msg, identity)
    }

    fn frame(&mut self, id: &str, origin: &str, seq: u64, signature_valid: bool) -> WireMessage {
        if !signature_valid {
            let mut wire = self.sign(id, origin, seq);
            wire.signature_bytes[0] ^= 1;
            return wire;
        }
        let key = (origin.to_string(), id.to_string(), seq);
        if let Some(wire) = self.wires.get(&key) {
            return wire.clone();
        }
        let wire = self.sign(id, origin, seq);
        self.wires.insert(key, wire.clone());
        wire
    }

    fn deliver(&mut self, receiver: &str, sender: &str, wire: &WireMessage) {
        let bytes = serde_json::to_vec(wire).unwrap();
        self.inbox
            .lock()
            .unwrap()
            .entry(receiver.to_string())
            .or_default()
            .push((sender.to_string(), bytes));
        let delivered = self
            .nodes
            .get_mut(receiver)
            .unwrap()
            .process_inbox_delivered();
        let held = self.actual_delivered.get_mut(receiver).unwrap();
        held.extend(delivered.into_iter().map(|w| (w.origin, w.seq)));
    }

    /// Everything the code sent since the last call, without chain-head announcements (the
    /// model leaves them out: they travel in their own counter and window).
    fn drain_sent(&mut self) -> Vec<(String, WireMessage)> {
        let frames: Vec<_> = self.inbox.lock().unwrap().drain().collect();
        let mut out = Vec::new();
        for (dest, queued) in frames {
            for (_, bytes) in queued {
                let wire: WireMessage = serde_json::from_slice(&bytes).unwrap();
                if !wire.message_type.contains("ChainHeadAnnouncement") {
                    out.push((dest.clone(), wire));
                }
            }
        }
        out
    }

    fn check_relays(&self, sent: &[(String, WireMessage)], want: &Value) {
        let got: BTreeSet<&str> = sent.iter().map(|(dest, _)| dest.as_str()).collect();
        let want: BTreeSet<&str> = want
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        assert_eq!(got, want, "{}: relayed to", self.at());
    }

    fn apply_changes(&mut self, event: &Value) {
        let Some(changes) = event["x"].as_object() else {
            return;
        };
        for (node, delta) in changes {
            let want = self.expected.get_mut(node).unwrap();
            if let Some(d) = delta["d"].as_array() {
                want.delivered = d.iter().map(pair).collect();
            }
            if let Some(rep) = delta["rep"].as_object() {
                want.rep = rep
                    .iter()
                    .map(|(p, r)| (p.clone(), (r[0].as_i64().unwrap(), r[1].as_bool().unwrap())))
                    .collect();
            }
            if let Some(seen) = delta["seen"].as_array() {
                want.seen = seen
                    .iter()
                    .map(|x| {
                        (
                            x[0].as_str().unwrap().to_string(),
                            x[1].as_u64().unwrap(),
                            x[2].as_str().unwrap().to_string(),
                            x[3].as_str().unwrap().to_string(),
                        )
                    })
                    .collect();
            }
            if let Some(window) = delta["win"].as_object() {
                want.window = window
                    .iter()
                    .map(|(o, w)| {
                        let held = w[1].as_array().unwrap();
                        (
                            o.clone(),
                            (
                                w[0].as_u64().unwrap(),
                                held.iter().map(|k| k.as_u64().unwrap()).collect(),
                            ),
                        )
                    })
                    .collect();
            }
        }
    }

    /// Every node holds what the model says it holds.
    fn compare(&mut self) {
        let at = self.at();
        let names: Vec<String> = self.nodes.keys().cloned().collect();
        for name in names {
            let want = self.expected[&name].clone();
            assert_eq!(
                self.actual_delivered[&name], want.delivered,
                "{at}: accepted by {name}"
            );
            let want_seen: BTreeSet<(String, String)> = want
                .seen
                .iter()
                .map(|(origin, seq, id, sender)| (self.seen_key(origin, *seq, id), sender.clone()))
                .collect();
            let node = self.nodes.get_mut(&name).unwrap();
            for (peer, (score, banned)) in &want.rep {
                let got = node.sybil_guard.reputation(peer);
                assert!(
                    (got - *score as f32 / 100.0).abs() < SCORE_TOLERANCE,
                    "{at}: {name}'s score of {peer}: code {got}, model {score}"
                );
                assert_eq!(
                    node.sybil_guard.is_banned(peer),
                    *banned,
                    "{at}: {name}'s ban of {peer} at {got}"
                );
            }
            let want_seen: BTreeSet<(String, u64)> = want_seen
                .into_iter()
                .map(|(key, sender)| (key, node.sender_tag(&sender)))
                .collect();
            let got_seen: BTreeSet<(String, u64)> = node
                .seen_messages
                .iter()
                .filter(|(key, _)| !key.contains(":sync-") && !key.contains(":head-"))
                .flat_map(|(key, senders)| senders.iter().map(move |s| (key.clone(), *s)))
                .collect();
            assert_eq!(got_seen, want_seen, "{at}: {name}'s seen cache");
            let got_window: BTreeMap<String, (u64, Vec<u64>)> = node
                .replay_filter
                .digest(SYNC_MAX_ORIGINS)
                .into_iter()
                .map(|(origin, max, held)| (origin, (max, held)))
                .collect();
            assert_eq!(got_window, want.window, "{at}: {name}'s replay window");
        }
    }

    /// The seen-cache key of this origin, sequence number and ID.
    fn seen_key(&mut self, origin: &str, seq: u64, id: &str) -> String {
        let cached = (origin.to_string(), id.to_string(), seq);
        if let Some(key) = self.seen_keys.get(&cached) {
            return key.clone();
        }
        let key = message_key(&self.sign(id, origin, seq));
        self.seen_keys.insert(cached, key.clone());
        key
    }

    fn originate(&mut self, event: &Value) {
        let origin = text(event, "o");
        let seq = event["k"].as_u64().unwrap();
        let id = format!("{origin}/{seq}");
        let msg = Message::new(
            &id,
            &origin,
            Signal::new(3, true, false, true, 3, Visibility::Direct),
            None,
        );
        self.nodes.get_mut(&origin).unwrap().originate(msg);
        let sent = self.drain_sent();
        self.check_relays(&sent, &event["to"]);
        let wire = sent[0].1.clone();
        assert_eq!(wire.seq, seq, "{}: sequence number", self.at());
        self.wires.insert((origin.clone(), id, seq), wire);
        self.actual_delivered
            .get_mut(&origin)
            .unwrap()
            .insert((origin, seq));
    }

    fn request(&mut self, event: &Value) {
        let (r, s) = (text(event, "r"), text(event, "s"));
        let rid = event["rid"].as_u64().unwrap();
        let node = self.nodes.get_mut(&r).unwrap();
        let request = node.build_sync_request(None);
        node.open_sync_session(&s, &request);
        let digest = request.sync.as_ref().unwrap();
        let got: BTreeSet<(String, u64)> = digest
            .origins
            .iter()
            .flat_map(|o| o.held.iter().map(move |k| (o.origin.clone(), *k)))
            .collect();
        let want: BTreeSet<(String, u64)> =
            event["held"].as_array().unwrap().iter().map(pair).collect();
        assert_eq!(got, want, "{}: digest of held messages", self.at());
        let got: BTreeMap<String, u64> = digest
            .origins
            .iter()
            .map(|o| (o.origin.clone(), o.seq_max))
            .collect();
        let want: BTreeMap<String, u64> = event["max"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(o, k)| (o.clone(), k.as_u64().unwrap()))
            .collect();
        assert_eq!(got, want, "{}: digest of highest numbers", self.at());
        self.requests.insert((r, s, rid), request);
    }

    fn answer(&mut self, event: &Value, coverage: &mut Coverage) {
        let (r, s) = (text(event, "r"), text(event, "s"));
        let rid = event["rid"].as_u64().unwrap();
        let request = self.requests[&(r.clone(), s.clone(), rid)].clone();
        self.nodes
            .get_mut(&s)
            .unwrap()
            .sybil_guard
            .advance_time(SYNC_COOLDOWN);
        self.deliver(&s, &r, &request);
        let sent = self.drain_sent();
        let mut got: Vec<(String, u64)> = Vec::new();
        for (dest, wire) in &sent {
            assert_eq!(*dest, r, "{}: answer sent to", self.at());
            assert_eq!(
                wire.answers_sync.as_deref(),
                Some(request.id.as_str()),
                "{}: answer tag",
                self.at()
            );
            got.push((wire.origin.clone(), wire.seq));
        }
        got.sort();
        let want: Vec<(String, u64)> = event["batch"]
            .as_array()
            .unwrap()
            .iter()
            .map(pair)
            .collect();
        assert_eq!(got, want, "{}: answer batch", self.at());
        coverage.answers += 1;
        coverage.answered_frames += want.len() as u64;
    }

    fn receive(&mut self, event: &Value, coverage: &mut Coverage) {
        let (s, r) = (text(event, "s"), text(event, "r"));
        let (id, origin) = (text(event, "id"), text(event, "o"));
        let seq = event["k"].as_u64().unwrap();
        let tag = event["tag"].as_u64().unwrap();
        let signature_valid = event["sig"].as_bool().unwrap();

        // An adversary's tag may name a request made to someone else: the code must refuse it.
        let answers = (tag != 0).then(|| {
            self.requests
                .get(&(r.clone(), s.clone(), tag))
                .or_else(|| {
                    self.requests
                        .iter()
                        .find(|((from, _, rid), _)| *from == r && *rid == tag)
                        .map(|(_, wire)| wire)
                })
                .map_or_else(|| "no-such-request".to_string(), |wire| wire.id.clone())
        });
        let mut wire = self.frame(&id, &origin, seq, signature_valid);
        wire.answers_sync = answers;

        let node = &self.nodes[&r];
        let before = (
            node.byzantine_events,
            node.sync_excused_duplicates,
            node.sybil_banned_events,
            node.messages_handled,
        );
        if !signature_valid {
            coverage.forged += 1;
        }
        if origin == r {
            coverage.own_echoes += 1;
        }
        if node.replay_filter.is_out_of_window(&origin, seq) {
            coverage.stale += 1;
            if node
                .seen_messages
                .get(&message_key(&wire))
                .is_some_and(|senders| senders.contains(&node.sender_tag(&s)))
            {
                coverage.stale_repeats += 1;
            }
        }
        if id.split('/').next() != Some(origin.as_str()) {
            coverage.id_reuse += 1;
        } else if id != format!("{origin}/{seq}") {
            coverage.own_id_new_seq += 1;
        }

        self.deliver(&r, &s, &wire);

        let node = &self.nodes[&r];
        coverage.strikes += node.byzantine_events - before.0;
        coverage.excused += node.sync_excused_duplicates - before.1;
        coverage.banned_drops += node.sybil_banned_events - before.2;
        let accepted = node.messages_handled - before.3;
        coverage.accepted += accepted;
        if accepted == 0 && signature_valid && event["x"].is_null() {
            coverage.copies += 1;
        }
        let sent = self.drain_sent();
        self.check_relays(&sent, &event["to"]);
    }

    fn step(&mut self, event: &Value, coverage: &mut Coverage) {
        self.step += 1;
        match event["a"].as_str().unwrap() {
            "originate" => self.originate(event),
            "request" => self.request(event),
            "answer" => self.answer(event, coverage),
            "receive" => self.receive(event, coverage),
            "tick" => {
                coverage.ticks += 1;
                for node in self.nodes.values_mut() {
                    node.sybil_guard.advance_time(TICK);
                }
            }
            "clear" => {
                coverage.clears += 1;
                let node = self.nodes.get_mut(&text(event, "n")).unwrap();
                node.seen_messages.clear();
            }
            other => panic!("unknown event {other}"),
        }
        self.apply_changes(event);
        self.compare();
    }
}

#[test]
fn the_code_does_what_the_model_does_on_every_step_of_its_behaviours() {
    let mut coverage = Coverage::default();
    let (mut traces, mut steps) = (0, 0);
    for (index, line) in TRACES.lines().enumerate() {
        let trace: Value = serde_json::from_str(line).unwrap();
        let label = format!("trace {} (set {})", index + 1, text(&trace, "set"));
        let mut replay = Replay::new(&trace, label);
        for event in trace["events"].as_array().unwrap() {
            replay.step(event, &mut coverage);
            steps += 1;
        }
        traces += 1;
    }
    println!(
        "{traces} traces, {steps} steps: accepted {} strikes {} excused {} banned-drops {} \
         forged {} stale {} stale-repeats {} own-echoes {} copies {} answers {} answered-frames {} ticks {} \
         clears {} id-reuse {} own-id-new-seq {}",
        coverage.accepted,
        coverage.strikes,
        coverage.excused,
        coverage.banned_drops,
        coverage.forged,
        coverage.stale,
        coverage.stale_repeats,
        coverage.own_echoes,
        coverage.copies,
        coverage.answers,
        coverage.answered_frames,
        coverage.ticks,
        coverage.clears,
        coverage.id_reuse,
        coverage.own_id_new_seq
    );
    // The traces must reach every branch they are meant to compare, or this test proves nothing.
    assert!(traces >= 100, "only {traces} traces: stale export?");
    for (what, count) in [
        ("accepted", coverage.accepted),
        ("strikes", coverage.strikes),
        ("excused", coverage.excused),
        ("banned drops", coverage.banned_drops),
        ("forged frames", coverage.forged),
        ("stale frames", coverage.stale),
        ("stale repeats", coverage.stale_repeats),
        ("own echoes", coverage.own_echoes),
        ("copies", coverage.copies),
        ("answers", coverage.answers),
        ("answered frames", coverage.answered_frames),
        ("ticks", coverage.ticks),
        ("seen-cache clears", coverage.clears),
        ("reused IDs", coverage.id_reuse),
        (
            "own IDs with a new sequence number",
            coverage.own_id_new_seq,
        ),
    ] {
        assert!(count > 0, "no {what} in the traces");
    }
}
