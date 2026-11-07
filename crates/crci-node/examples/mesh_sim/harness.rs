//! In-process mesh simulation: unmodified `NodeRuntime` instances on a given overlay, no sockets.
//!
//! The loop follows `crci-node`'s receive path: each frame goes through
//! `process_inbox_delivered` on its own, what the node queued is relayed (twice by an attacker
//! once its fault is active), and every peer answers to its authenticated identity
//! (`node_id#key`) and is addressed by a separate name, as on TCP. A hop takes one tick, every
//! link delivers in order and nothing is lost. Time is simulated (`use_manual_clock`), so a tick
//! says nothing about real latency; the result depends only on the scenario.

use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{DefenceMode, NodeRuntime};
use crci_core::sybil::INITIAL_SCORE;
use crci_core::transport::{LegacyTransport, SharedInbox, SimTransport};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// `crci-node`'s anti-entropy interval.
const SYNC_INTERVAL_MS: u64 = 30_000;
/// Least time between two sync requests to one peer (`crci-node`'s resume spacing).
const SYNC_RESUME_SPACING_MS: u64 = 8_000;
/// Messages originated earlier are "warmup" in the testbed analysis (`analyze.py`).
const WARMUP_MS: u64 = 5_000;
/// Without a ban by every honest neighbour of every attacker, "post" starts this long after the
/// fault.
const POST_FALLBACK_MS: u64 = 20_000;

/// Switches of the evaluation: the defence mode (baselines), the strike threshold and the replay
/// window (ablations), and an injected fault on honest nodes. All default to the protocol as
/// shipped, so a scenario without `config` is the one `docs/benchmarks.md` describes.
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// "full" (or empty), "static-bucket" or "vanilla": see `DefenceMode`.
    pub defence: String,
    /// Strikes that ban a peer (3 when absent).
    pub strikes_to_ban: Option<u32>,
    /// Replay window W (64 when absent; 0 is strict-monotonic).
    pub replay_window: Option<u64>,
    /// Probability that a frame sent by an honest node reaches its peer twice. An injected
    /// transport fault, to give honest peers something to be struck for; not a measured
    /// network property.
    pub honest_dup_prob: f64,
    /// Seed of the injected fault.
    pub dup_seed: u64,
}

#[derive(Deserialize)]
pub struct Scenario {
    pub n: usize,
    pub edges: Vec<(usize, usize)>,
    /// The node whose messages count for `downstream`.
    pub source: Option<usize>,
    pub attackers: Vec<usize>,
    pub downstream: Vec<usize>,
    /// Per node: messages to originate, interval, first origination (`testbed.py` plan).
    pub plan: Vec<(u64, u64, u64)>,
    /// Per node: first anti-entropy round.
    pub sync_phase_ms: Vec<u64>,
    pub window_ms: u64,
    pub fault_at_ms: u64,
    pub drain_ms: u64,
    pub replay_copies: usize,
    pub tick_ms: u64,
    #[serde(default)]
    pub config: Config,
    /// Contact trace: (start ms, end ms, a, b). When present, a frame crosses a link only while
    /// its contact is active (everything else is dropped), anti-entropy runs only towards active
    /// peers, and both ends sync when a contact starts. `edges` must hold every contact pair.
    #[serde(default)]
    pub contacts: Vec<(u64, u64, usize, usize)>,
}

#[derive(Serialize, Debug, PartialEq, Default)]
pub struct Outcome {
    pub honest_nodes: usize,
    pub attackers: usize,
    pub frames: u64,
    pub messages: usize,
    /// Bans of an honest node, by anyone.
    pub false_evictions: u64,
    pub honest_nodes_banned: usize,
    /// Strikes on honest senders, by anyone.
    pub honest_strikes: u64,
    pub attacker_strikes: u64,
    /// (honest node, neighbouring attacker) pairs, and those where the node banned the attacker.
    pub coverage_links: u64,
    pub coverage_banned: u64,
    /// With a contact trace only: the same pairs restricted to those with a contact that ended
    /// after the fault began (an attacker cannot be struck on a link it never uses again).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_links_in_contact: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_banned_in_contact: Option<u64>,
    pub attackers_fully_banned: usize,
    pub readmissions: u64,
    /// Records of senders that are not neighbours, over all nodes' guards at the end.
    pub non_neighbour_records: u64,
    /// Simulated time from the fault to each ban of an attacker (ticks, not real latency).
    pub evict_sim_median_ms: Option<u64>,
    pub evict_sim_max_ms: Option<u64>,
    /// Connected components of the graph without the attackers.
    pub honest_components: usize,
    pub largest_component_frac: f64,
    pub msgs_pre: usize,
    pub msgs_post: usize,
    pub delivery_pre: Option<f64>,
    pub delivery_post: Option<f64>,
    pub delivery_post_min: Option<f64>,
    /// Delivery after the bans among honest nodes in the origin's component.
    pub delivery_post_reachable: Option<f64>,
    pub delivery_post_reachable_min: Option<f64>,
    pub downstream_pre: (u64, u64),
    pub downstream_post: (u64, u64),
    /// Honest receivers that accepted a post-ban message from an attacker.
    pub accepted_via_attacker_post: u64,
    pub rate_drops: u64,
    pub origin_drops: u64,
    pub sync_requests: u64,
    pub sync_excused: u64,
    pub seen_cache_clears: u64,
    pub state_seen_ids_mean: f64,
    pub state_seen_ids_max: usize,
    pub state_seen_senders_mean: f64,
    pub state_recent_msgs_mean: f64,
    pub state_recent_msgs_max: usize,
    pub state_window_seqs_mean: f64,
    pub state_known_keys_mean: f64,
    pub state_known_keys_max: usize,
    /// Origins forgotten because of the per-node limits, over all nodes.
    pub origins_forgotten: u64,
    /// Frames from attackers delivered to honest nodes that had not banned them yet.
    pub attacker_frames_admitted: u64,
    /// Signature verifications performed, over all nodes.
    pub verifications: u64,
    /// (honest message, honest receiver) pairs delivered, over the whole run.
    pub honest_deliveries: u64,
    /// Honest-honest links (one per direction), and those on which the observer ever banned.
    pub honest_links_total: u64,
    pub honest_links_ever_banned: u64,
    /// Delivery of honest messages originated from the fault to the end of the window,
    /// whether or not anyone was banned (the baselines never ban).
    pub delivery_attack: Option<f64>,
    /// Honest deliveries per second in that period.
    pub goodput_per_s: f64,
}

struct Emission {
    node: usize,
    message: usize,
}

struct MessageRecord {
    origin: usize,
    at_ms: u64,
    got: Vec<bool>,
    via_attacker: u64,
}

/// One end of every link a node has: what it last knew about that neighbour.
#[derive(Clone, Copy)]
struct LinkView {
    score: f32,
    banned: bool,
    ever_banned: bool,
}

fn name(i: usize) -> String {
    format!("n{i:03}")
}

fn address(i: usize) -> String {
    format!("a{i}")
}

fn identity_of(node: &NodeRuntime) -> String {
    let key: String = node
        .identity
        .verifying_key
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{}#{}", node.id, key)
}

/// Node index of an authenticated identity (`n012#...`).
fn index_of(identity: &str) -> Option<usize> {
    identity.get(1..)?.split('#').next()?.parse().ok()
}

fn take_all(inbox: &SharedInbox) -> HashMap<String, Vec<(String, Vec<u8>)>> {
    std::mem::take(&mut *inbox.lock().unwrap_or_else(|e| e.into_inner()))
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values.fold((0.0, 0u64), |(s, c), v| (s + v, c + 1));
    (count > 0).then(|| sum / count as f64)
}

/// Component label per node of the graph without `removed` nodes (`usize::MAX` for those).
fn components(adj: &[Vec<usize>], removed: &[bool]) -> (Vec<usize>, Vec<usize>) {
    let mut label = vec![usize::MAX; adj.len()];
    let mut sizes = Vec::new();
    for start in 0..adj.len() {
        if removed[start] || label[start] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut size = 1;
        label[start] = id;
        let mut queue = VecDeque::from([start]);
        while let Some(u) = queue.pop_front() {
            for &v in &adj[u] {
                if !removed[v] && label[v] == usize::MAX {
                    label[v] = id;
                    size += 1;
                    queue.push_back(v);
                }
            }
        }
        sizes.push(size);
    }
    (label, sizes)
}

struct Sim<'a> {
    scenario: &'a Scenario,
    nodes: Vec<NodeRuntime>,
    identities: Vec<String>,
    adj: Vec<Vec<usize>>,
    attacker: Vec<bool>,
    /// Frames the node queued, for an attacker: relayed from here so they can be repeated.
    attacker_outbox: Vec<Option<SharedInbox>>,
    incoming: SharedInbox,
    outgoing: SharedInbox,
    clocks_ms: Vec<u64>,
    links: Vec<Vec<LinkView>>,
    messages: Vec<MessageRecord>,
    message_index: HashMap<String, usize>,
    evict_ms: Vec<u64>,
    out: Outcome,
    /// Active contact pairs (smaller index first); unused without a trace.
    active: HashSet<(usize, usize)>,
    last_sync: HashMap<(usize, usize), u64>,
}

impl Sim<'_> {
    fn link_up(&self, a: usize, b: usize) -> bool {
        self.scenario.contacts.is_empty() || self.active.contains(&(a.min(b), a.max(b)))
    }

    fn advance(&mut self, i: usize, now_ms: u64) {
        let by = now_ms - self.clocks_ms[i];
        if by > 0 {
            self.nodes[i]
                .sybil_guard
                .advance_time(Duration::from_millis(by));
            self.clocks_ms[i] = now_ms;
        }
    }

    /// Move what attacker `i` queued to the links, `copies` times each.
    fn flush_attacker(&mut self, i: usize, copies: usize) {
        let Some(outbox) = &self.attacker_outbox[i] else {
            return;
        };
        let queued = take_all(outbox);
        let mut out = self.outgoing.lock().unwrap_or_else(|e| e.into_inner());
        for (dest, frames) in queued {
            let link = out.entry(dest).or_default();
            for frame in frames {
                for _ in 1..copies {
                    link.push(frame.clone());
                }
                link.push(frame);
            }
        }
    }

    fn copies(&self, now_ms: u64) -> usize {
        if now_ms >= self.scenario.fault_at_ms {
            self.scenario.replay_copies
        } else {
            1
        }
    }

    /// Update what receiver `i` knows about `sender` after one of its frames: strikes are
    /// always charged to the sender of the frame just processed, so a ban can only start here.
    fn observe(&mut self, i: usize, sender: usize, now_ms: u64) {
        let Ok(pos) = self.adj[i].binary_search(&sender) else {
            return;
        };
        let identity = &self.identities[sender];
        let guard = &mut self.nodes[i].sybil_guard;
        if guard.available_tokens(identity).is_none() && !guard.is_banned(identity) {
            return;
        }
        let score = guard.reputation(identity);
        let banned = guard.is_banned(identity);
        let link = &mut self.links[i][pos];
        if score < link.score - 1e-4 {
            if self.attacker[sender] {
                self.out.attacker_strikes += 1;
            } else {
                self.out.honest_strikes += 1;
            }
        }
        link.score = score;
        if banned && !link.banned {
            if self.attacker[sender] {
                if !link.ever_banned && !self.attacker[i] {
                    self.evict_ms
                        .push(now_ms.saturating_sub(self.scenario.fault_at_ms));
                }
            } else {
                self.out.false_evictions += 1;
            }
            link.ever_banned = true;
        } else if !banned && link.banned {
            self.out.readmissions += 1;
        }
        link.banned = banned;
    }

    fn deliver_frame(&mut self, i: usize, sender_identity: String, bytes: Vec<u8>, now_ms: u64) {
        self.out.frames += 1;
        self.advance(i, now_ms);
        let sender = index_of(&sender_identity);
        if let Some(s) = sender {
            if self.attacker[s] && !self.attacker[i] {
                if let Ok(pos) = self.adj[i].binary_search(&s) {
                    self.out.attacker_frames_admitted += u64::from(!self.links[i][pos].banned);
                }
            }
        }
        self.incoming
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(self.nodes[i].id.clone(), vec![(sender_identity, bytes)]);
        let delivered = self.nodes[i].process_inbox_delivered();
        for wire in &delivered {
            if let Some(&m) = self.message_index.get(&wire.id) {
                let record = &mut self.messages[m];
                record.got[i] = true;
                if sender.is_some_and(|s| self.attacker[s]) && !self.attacker[i] {
                    record.via_attacker += 1;
                }
            }
        }
        if let Some(s) = sender {
            self.observe(i, s, now_ms);
        }
        if self.attacker[i] {
            let copies = self.copies(now_ms);
            self.flush_attacker(i, copies);
        }
    }

    fn originate(&mut self, emission: &Emission, now_ms: u64) {
        let i = emission.node;
        self.advance(i, now_ms);
        let msg_id = message_id(i, emission.message);
        self.nodes[i].originate(Message::new(
            &msg_id,
            &name(i),
            Signal::new(3, false, false, true, 3, Visibility::Direct),
            Some("testbed"),
        ));
        if self.attacker[i] {
            let copies = self.copies(now_ms);
            self.flush_attacker(i, copies);
        }
    }

    /// One anti-entropy round of node `i`, as `crci-node` runs it: a signed request to every
    /// peer, sent once whatever the fault, with a session opened first.
    fn sync_round(&mut self, i: usize, now_ms: u64) {
        self.advance(i, now_ms);
        for pos in 0..self.adj[i].len() {
            let peer = self.adj[i][pos];
            if self.link_up(i, peer) {
                self.sync_one(i, peer, now_ms);
            }
        }
        self.flush_attacker(i, 1);
    }

    fn sync_one(&mut self, i: usize, peer: usize, now_ms: u64) {
        let request = self.nodes[i].build_sync_request(Some(address(i)));
        self.nodes[i].open_sync_session(&self.identities[peer], &request);
        if let Ok(payload) = serde_json::to_vec(&request) {
            self.nodes[i].transport.send(&address(peer), &payload);
        }
        self.last_sync.insert((i, peer), now_ms);
        self.out.sync_requests += 1;
    }

    /// A contact started: each end asks the other, as a resumed link does, unless it asked
    /// less than the resume spacing ago.
    fn contact_started(&mut self, a: usize, b: usize, now_ms: u64) {
        for (x, y) in [(a, b), (b, a)] {
            let recent = self
                .last_sync
                .get(&(x, y))
                .is_some_and(|&t| now_ms.saturating_sub(t) < SYNC_RESUME_SPACING_MS);
            if !recent {
                self.advance(x, now_ms);
                self.sync_one(x, y, now_ms);
                self.flush_attacker(x, 1);
            }
        }
    }

    fn finish(mut self) -> Outcome {
        let s = self.scenario;
        let n = s.n;
        let honest: Vec<usize> = (0..n).filter(|&i| !self.attacker[i]).collect();
        self.out.honest_nodes = honest.len();
        self.out.attackers = n - honest.len();
        self.out.messages = self.messages.len();

        // Coverage: honest observers of each attacker.
        let mut post_from = None;
        let mut all_banned = true;
        for &a in &s.attackers {
            let mut full = true;
            for &observer in &self.adj[a] {
                if self.attacker[observer] {
                    continue;
                }
                self.out.coverage_links += 1;
                let view_pos = self.adj[observer].binary_search(&a).unwrap_or(0);
                let banned = self.links[observer][view_pos].ever_banned;
                if !s.contacts.is_empty() {
                    let in_contact = s.contacts.iter().any(|&(_, end, x, y)| {
                        end > s.fault_at_ms && (x, y) == (a.min(observer), a.max(observer))
                            || end > s.fault_at_ms && (y, x) == (a.min(observer), a.max(observer))
                    });
                    if in_contact {
                        *self.out.coverage_links_in_contact.get_or_insert(0) += 1;
                        *self.out.coverage_banned_in_contact.get_or_insert(0) += u64::from(banned);
                    }
                }
                if banned {
                    self.out.coverage_banned += 1;
                } else {
                    full = false;
                }
            }
            if full {
                self.out.attackers_fully_banned += 1;
            } else {
                all_banned = false;
            }
        }
        if !s.attackers.is_empty() {
            let from = if all_banned {
                s.fault_at_ms + self.evict_ms.iter().copied().max().unwrap_or(0)
            } else {
                s.fault_at_ms + POST_FALLBACK_MS
            };
            post_from = Some(from);
        }
        let mut evict = std::mem::take(&mut self.evict_ms);
        evict.sort_unstable();
        self.out.evict_sim_median_ms = evict.get(evict.len() / 2).copied();
        self.out.evict_sim_max_ms = evict.last().copied();

        let mut banned_nodes = vec![false; n];
        for i in 0..n {
            for (pos, &peer) in self.adj[i].iter().enumerate() {
                if self.links[i][pos].ever_banned && !self.attacker[peer] {
                    banned_nodes[peer] = true;
                }
            }
        }
        self.out.honest_nodes_banned = banned_nodes.iter().filter(|b| **b).count();

        // Locality: no node holds a record of any node it is not linked to.
        let mut is_neighbour = vec![usize::MAX; n];
        for i in 0..n {
            for &peer in &self.adj[i] {
                is_neighbour[peer] = i;
            }
            for (j, identity) in self.identities.iter().enumerate() {
                if j != i
                    && is_neighbour[j] != i
                    && self.nodes[i]
                        .sybil_guard
                        .available_tokens(identity)
                        .is_some()
                {
                    self.out.non_neighbour_records += 1;
                }
            }
        }

        let (label, sizes) = components(&self.adj, &self.attacker);
        self.out.honest_components = sizes.len();
        self.out.largest_component_frac = match honest.len() {
            0 => 0.0,
            h => sizes.iter().copied().max().unwrap_or(0) as f64 / h as f64,
        };

        // Delivery by origination time, as `analyze.py` defines the periods.
        let (mut pre, mut post, mut post_reach) = (Vec::new(), Vec::new(), Vec::new());
        for record in &self.messages {
            if self.attacker[record.origin] {
                continue;
            }
            let period_pre = record.at_ms >= WARMUP_MS && record.at_ms < s.fault_at_ms;
            let period_post = post_from.is_some_and(|from| record.at_ms >= from)
                || (s.attackers.is_empty() && record.at_ms >= s.fault_at_ms);
            let receivers = honest.iter().filter(|&&x| x != record.origin);
            let (mut got, mut total, mut got_reach, mut reach) = (0u64, 0u64, 0u64, 0u64);
            for &x in receivers {
                total += 1;
                let same = label[x] == label[record.origin];
                reach += u64::from(same);
                if record.got[x] {
                    got += 1;
                    got_reach += u64::from(same);
                }
            }
            if total == 0 {
                continue;
            }
            if period_pre {
                pre.push(got as f64 / total as f64);
            }
            if period_post {
                post.push(got as f64 / total as f64);
                if reach > 0 {
                    post_reach.push(got_reach as f64 / reach as f64);
                }
                self.out.accepted_via_attacker_post += record.via_attacker;
            }
            if Some(record.origin) == s.source {
                for &x in &s.downstream {
                    let tally = if period_pre {
                        &mut self.out.downstream_pre
                    } else if period_post {
                        &mut self.out.downstream_post
                    } else {
                        continue;
                    };
                    tally.0 += u64::from(record.got[x]);
                    tally.1 += 1;
                }
            }
        }
        let min = |v: &[f64]| v.iter().copied().reduce(f64::min);
        self.out.msgs_pre = pre.len();
        self.out.msgs_post = post.len();
        self.out.delivery_pre = mean(pre.iter().copied());
        self.out.delivery_post = mean(post.iter().copied());
        self.out.delivery_post_min = min(&post);
        self.out.delivery_post_reachable = mean(post_reach.iter().copied());
        self.out.delivery_post_reachable_min = min(&post_reach);

        // Protocol state per node at the end of the run.
        let per_node = |f: &dyn Fn(&NodeRuntime) -> usize| -> (f64, usize) {
            let values: Vec<usize> = self.nodes.iter().map(f).collect();
            (
                values.iter().sum::<usize>() as f64 / n as f64,
                values.iter().copied().max().unwrap_or(0),
            )
        };
        let (seen_mean, seen_max) = per_node(&|x| x.seen_messages.len());
        let (senders_mean, _) = per_node(&|x| x.seen_messages.values().map(|s| s.len()).sum());
        let (recent_mean, recent_max) =
            per_node(&|x| x.recent_messages.values().map(|m| m.len()).sum());
        let (window_mean, _) = per_node(&|x| {
            x.replay_filter
                .digest(usize::MAX)
                .iter()
                .map(|(_, _, held)| held.len())
                .sum()
        });
        let (keys_mean, keys_max) = per_node(&|x| x.known_keys.len());
        self.out.state_seen_ids_mean = seen_mean;
        self.out.state_seen_ids_max = seen_max;
        self.out.state_seen_senders_mean = senders_mean;
        self.out.state_recent_msgs_mean = recent_mean;
        self.out.state_recent_msgs_max = recent_max;
        self.out.state_window_seqs_mean = window_mean;
        self.out.state_known_keys_mean = keys_mean;
        self.out.state_known_keys_max = keys_max;
        // Honest links and what the observers did with them.
        for &i in &honest {
            for (pos, &peer) in self.adj[i].iter().enumerate() {
                if self.attacker[peer] {
                    continue;
                }
                self.out.honest_links_total += 1;
                self.out.honest_links_ever_banned += u64::from(self.links[i][pos].ever_banned);
            }
        }
        // Delivery and goodput from the fault to the end of the window.
        let (mut attack_ratios, mut attack_got) = (Vec::new(), 0u64);
        for record in &self.messages {
            if self.attacker[record.origin] {
                continue;
            }
            let receivers = honest.iter().filter(|&&x| x != record.origin);
            let (mut got, mut total) = (0u64, 0u64);
            for &x in receivers {
                total += 1;
                got += u64::from(record.got[x]);
            }
            self.out.honest_deliveries += got;
            if total > 0 && record.at_ms >= s.fault_at_ms && record.at_ms < s.window_ms {
                attack_ratios.push(got as f64 / total as f64);
                attack_got += got;
            }
        }
        self.out.delivery_attack = mean(attack_ratios.iter().copied());
        let attack_s = s.window_ms.saturating_sub(s.fault_at_ms) as f64 / 1000.0;
        self.out.goodput_per_s = if attack_s > 0.0 {
            attack_got as f64 / attack_s
        } else {
            0.0
        };
        for node in &self.nodes {
            self.out.verifications += node
                .metrics
                .sig_verifications_performed
                .load(Ordering::Relaxed);
            self.out.rate_drops += node.sybil_rate_limited_events;
            self.out.origin_drops += node.origin_rate_limited_events;
            self.out.sync_excused += node.sync_excused_duplicates;
            self.out.origins_forgotten += node.origins_forgotten;
            self.out.seen_cache_clears +=
                node.metrics.seen_messages_evictions.load(Ordering::Relaxed);
        }
        self.out
    }
}

/// A small seeded generator (SplitMix64) for the injected fault.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn message_id(origin: usize, message: usize) -> String {
    // The length of a testbed message ID (a UUID), so per-node state compares.
    format!("{origin:08x}-0000-4000-8000-{message:012x}")
}

pub fn run(scenario: &Scenario) -> Outcome {
    let n = scenario.n;
    let tick = scenario.tick_ms.max(1);
    let end_ms = scenario.window_ms + scenario.drain_ms;
    let mut adj = vec![Vec::new(); n];
    for &(a, b) in &scenario.edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    for list in &mut adj {
        list.sort_unstable();
    }
    let mut attacker = vec![false; n];
    for &a in &scenario.attackers {
        attacker[a] = true;
    }

    let incoming: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let outgoing: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut nodes = Vec::with_capacity(n);
    let mut identities = Vec::with_capacity(n);
    let mut attacker_outbox = Vec::with_capacity(n);
    for i in 0..n {
        let mut node = NodeRuntime::new(&name(i), "sim", incoming.clone());
        node.use_router = false;
        node.sybil_guard.use_manual_clock();
        match scenario.config.defence.as_str() {
            "" => {}
            mode => match mode.parse::<DefenceMode>() {
                Ok(mode) => node.set_defence_mode(mode),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            },
        }
        if let Some(k) = scenario.config.strikes_to_ban {
            node.sybil_guard.set_strikes_to_ban(k);
        }
        if let Some(w) = scenario.config.replay_window {
            node.replay_filter.window_size = w;
        }
        let identity = identity_of(&node);
        let outbox = attacker[i].then(|| -> SharedInbox { Arc::new(Mutex::new(HashMap::new())) });
        let link = outbox.clone().unwrap_or_else(|| outgoing.clone());
        node.transport = SimTransport::new(&identity, link);
        for &peer in &adj[i] {
            node.add_peer(&address(peer));
        }
        nodes.push(node);
        identities.push(identity);
        attacker_outbox.push(outbox);
    }

    // Everything that happens by the clock, in a fixed order.
    let mut messages = Vec::new();
    let mut message_index = HashMap::new();
    let mut emissions: Vec<(u64, Emission)> = Vec::new();
    for (i, &(count, interval, offset)) in scenario.plan.iter().enumerate() {
        for k in 0..count {
            let at_ms = offset + k * interval;
            let message = messages.len();
            message_index.insert(message_id(i, message), message);
            messages.push(MessageRecord {
                origin: i,
                at_ms,
                got: vec![false; n],
                via_attacker: 0,
            });
            emissions.push((at_ms, Emission { node: i, message }));
        }
    }
    emissions.sort_by_key(|(at, e)| (*at, e.node, e.message));
    let mut syncs: Vec<(u64, usize)> = Vec::new();
    for (i, &phase) in scenario.sync_phase_ms.iter().enumerate() {
        let mut at = phase;
        while at <= end_ms {
            syncs.push((at, i));
            at += SYNC_INTERVAL_MS;
        }
    }
    syncs.sort_unstable();

    let links = adj
        .iter()
        .map(|list| {
            vec![
                LinkView {
                    score: INITIAL_SCORE,
                    banned: false,
                    ever_banned: false,
                };
                list.len()
            ]
        })
        .collect();
    let mut sim = Sim {
        scenario,
        nodes,
        identities,
        adj,
        attacker,
        attacker_outbox,
        incoming,
        outgoing,
        clocks_ms: vec![0; n],
        links,
        messages,
        message_index,
        evict_ms: Vec::new(),
        out: Outcome::default(),
        active: HashSet::new(),
        last_sync: HashMap::new(),
    };
    // Contact starts and ends by time, ends first.
    let mut contact_events: Vec<(u64, bool, usize, usize)> = Vec::new();
    for &(start, end, a, b) in &scenario.contacts {
        contact_events.push((start, true, a.min(b), a.max(b)));
        contact_events.push((end, false, a.min(b), a.max(b)));
    }
    contact_events.sort_unstable();
    let mut next_contact = 0;

    let dup_prob = scenario.config.honest_dup_prob;
    let mut dup_rng = SplitMix64(scenario.config.dup_seed);
    let mut next_emission = 0;
    let mut next_sync = 0;
    let mut in_flight: Vec<Vec<(String, Vec<u8>)>> = vec![Vec::new(); n];
    let mut now_ms = 0;
    while now_ms <= end_ms {
        while let Some(&(at, start, a, b)) = contact_events.get(next_contact) {
            if at > now_ms {
                break;
            }
            if start {
                sim.active.insert((a, b));
                sim.contact_started(a, b, now_ms);
            } else {
                sim.active.remove(&(a, b));
            }
            next_contact += 1;
        }
        // Frames sent in the previous tick arrive, nodes in index order.
        for (i, frames) in in_flight.iter_mut().enumerate() {
            for (sender, bytes) in std::mem::take(frames) {
                sim.deliver_frame(i, sender, bytes, now_ms);
            }
        }
        while let Some((at, emission)) = emissions.get(next_emission) {
            if *at > now_ms {
                break;
            }
            sim.originate(emission, now_ms);
            next_emission += 1;
        }
        while let Some(&(at, i)) = syncs.get(next_sync) {
            if at > now_ms {
                break;
            }
            sim.sync_round(i, now_ms);
            next_sync += 1;
        }
        // Destinations in a fixed order, so the injected fault draws the same numbers every run.
        let mut batches: Vec<_> = take_all(&sim.outgoing).into_iter().collect();
        batches.sort_by(|a, b| a.0.cmp(&b.0));
        for (dest, frames) in batches {
            let Some(i) = dest.get(1..).and_then(|d| d.parse::<usize>().ok()) else {
                continue;
            };
            for frame in frames {
                if index_of(&frame.0).is_some_and(|s| !sim.link_up(s, i)) {
                    continue;
                }
                let duplicate = dup_prob > 0.0
                    && index_of(&frame.0).is_some_and(|s| !sim.attacker[s])
                    && dup_rng.next_f64() < dup_prob;
                if duplicate {
                    in_flight[i].push(frame.clone());
                }
                in_flight[i].push(frame);
            }
        }
        now_ms += tick;
    }
    sim.finish()
}
