//! Ways a third party or an accident could get an honest node struck for a repeat it did not
//! choose to make, and frame fields nobody signed that decided whether a repeat is a strike.

use crci_core::identity::Identity;
use crci_core::message::{Message, Signal, Visibility};
use crci_core::runtime::{NodeRuntime, SyncDigest, WireMessage, SYNC_REQUEST_TYPE};
use crci_core::transport::tcp::{send_authenticated, TcpTransport};
use crci_core::transport::{SharedInbox, Transport};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Processes ────────────────────────────────────────────────────────────────

/// A free port below every ephemeral range, for nodes whose address must be known in advance.
fn free_addr() -> String {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 14_000 + (std::process::id() % 5_000) as u16;
    loop {
        let port = base + NEXT.fetch_add(1, Ordering::SeqCst) % 1_000;
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return format!("127.0.0.1:{port}");
        }
    }
}

struct Node {
    child: Child,
    reader: BufReader<ChildStdout>,
    id: String,
}

fn start(label: &str, addr: &str, extra: &[&str]) -> Node {
    let mut child = Command::new(env!("CARGO_BIN_EXE_crci-node"))
        .args(["--listen", addr, "--label", label])
        .args(extra)
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start node");
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let (mut line, mut id) = (String::new(), String::new());
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(10) {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(rest) = line.strip_prefix(&format!("Node {label} id ")) {
            id = rest.trim().to_string();
        }
        if line.contains(" listening on ") && line.starts_with("Node ") {
            return Node { child, reader, id };
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("{label} did not start listening");
}

impl Node {
    fn finish(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut out = String::new();
        let _ = self.reader.read_to_string(&mut out);
        out
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn count(log: &str, needle: &str) -> usize {
    log.lines().filter(|l| l.contains(needle)).count()
}

/// A sync request names the address its answer goes to. A requester that names a third node must
/// not make the answerer send that node a second copy of everything it already relayed to it:
/// the third node would strike the answerer for replays.
#[test]
fn a_sync_answer_aimed_at_a_third_party_does_not_frame_the_answerer() {
    let (victim_addr, answerer_addr) = (free_addr(), free_addr());
    let victim = start("victim", &victim_addr, &[]);
    let answerer = start(
        "answerer",
        &answerer_addr,
        &[
            "--peers",
            &victim_addr,
            "--emit-count",
            "5",
            "--emit-interval-ms",
            "50",
            "--emit-delay-ms",
            "100",
        ],
    );
    let answerer_id = answerer.id.clone();
    std::thread::sleep(Duration::from_millis(1500));

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut attacker = NodeRuntime::new("attacker", "z", inbox);
    let request = attacker.build_sync_request(Some(victim_addr.clone()));
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(send_authenticated(
            &answerer_addr,
            &attacker.identity.signing_key,
            &[request],
        ))
        .unwrap();
    std::thread::sleep(Duration::from_millis(1500));

    let answered = answerer.finish();
    let log = victim.finish();
    assert_eq!(
        count(&answered, "answering sync request from"),
        1,
        "{answered}"
    );
    assert_eq!(
        count(&log, &format!("(origin {answerer_id}, via {answerer_id})")),
        5,
        "{log}"
    );
    assert_eq!(count(&log, "MALICIOUS REPLAY"), 0, "{log}");
    assert_eq!(count(&log, " banned "), 0, "{log}");
}

/// Forwards every byte of each connection to `target`, as a malicious peer (or a NAT, or a second
/// address of the same machine) can.
fn start_proxy(target: String) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for inbound in listener.incoming().flatten() {
            let Ok(outbound) = std::net::TcpStream::connect(&target) else {
                continue;
            };
            for (mut from, mut to) in [
                (inbound.try_clone().unwrap(), outbound.try_clone().unwrap()),
                (outbound, inbound),
            ] {
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut from, &mut to);
                    let _ = to.shutdown(std::net::Shutdown::Both);
                });
            }
        }
    });
    addr
}

/// A peer address that leads to a node this node already relays to (a peer that forwards its
/// connections there, or one node configured under two addresses) must not make this node send
/// that node everything twice.
#[test]
fn a_peer_address_that_leads_to_another_peer_does_not_frame_the_sender() {
    let sink_addr = free_addr();
    let sink = start("sink", &sink_addr, &[]);
    let proxy = start_proxy(sink_addr.clone());
    let sender = start(
        "sender",
        &free_addr(),
        &[
            "--peers",
            &format!("{sink_addr},{proxy}"),
            "--emit-count",
            "5",
            "--emit-interval-ms",
            "100",
            "--emit-delay-ms",
            "300",
        ],
    );
    let sender_id = sender.id.clone();
    std::thread::sleep(Duration::from_millis(2500));
    drop(sender);
    let log = sink.finish();
    assert_eq!(
        count(&log, &format!("(origin {sender_id}, via {sender_id})")),
        5,
        "{log}"
    );
    assert_eq!(count(&log, "MALICIOUS REPLAY"), 0, "{log}");
    assert_eq!(count(&log, " banned "), 0, "{log}");
}

/// A relay restarted from its key file keeps its ID but not its memory. Catching up from one
/// neighbour must not make it relay again, to another, what its previous run already relayed.
#[test]
fn a_relay_restarted_from_its_key_file_does_not_relay_twice() {
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("node.key");
    let pass = dir.path().join("pass.txt");
    std::fs::write(&pass, "pw\n").unwrap();
    let (source_addr, relay_addr, sink_addr) = (free_addr(), free_addr(), free_addr());
    let sink = start("sink", &sink_addr, &[]);
    let source = start(
        "source",
        &source_addr,
        &[
            "--peers",
            &relay_addr,
            "--emit-count",
            "8",
            "--emit-interval-ms",
            "50",
            "--emit-delay-ms",
            "1500",
        ],
    );
    let peers = format!("{source_addr},{sink_addr}");
    let args = [
        "--key-file",
        key.to_str().unwrap(),
        "--key-passphrase-file",
        pass.to_str().unwrap(),
        "--peers",
        &peers,
    ];
    let relay = start("relay", &relay_addr, &args);
    // Past the periodic save of what was relayed; the process is then killed, not stopped.
    std::thread::sleep(Duration::from_millis(9000));
    let first = relay.finish();
    assert_eq!(count(&first, "accepted '"), 8, "{first}");
    let again = start("relay", &relay_addr, &args);
    std::thread::sleep(Duration::from_millis(3000));
    let second = again.finish();
    drop(source);
    let log = sink.finish();
    assert_eq!(count(&second, "accepted '"), 8, "it catches up:\n{second}");
    assert_eq!(count(&log, "accepted '"), 8, "{log}");
    assert_eq!(count(&log, "MALICIOUS REPLAY"), 0, "{log}");
    assert_eq!(count(&log, " banned "), 0, "{log}");
}

// ── One node ─────────────────────────────────────────────────────────────────

fn receiver() -> (NodeRuntime, SharedInbox) {
    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut node = NodeRuntime::new("receiver", "z", inbox.clone());
    node.sybil_guard.use_manual_clock();
    node.use_router = false;
    (node, inbox)
}

fn signed(identity: &Identity, id: &str, seq: u64) -> WireMessage {
    let mut msg = Message::new(
        id,
        &identity.id,
        Signal::new(3, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = seq;
    WireMessage::from_message(&msg, identity)
}

fn deliver(node: &mut NodeRuntime, inbox: &SharedInbox, from: &str, wire: &WireMessage) -> usize {
    inbox
        .lock()
        .unwrap()
        .entry(node.id.clone())
        .or_default()
        .push((from.to_string(), serde_json::to_vec(wire).unwrap()));
    node.process_inbox_delivered().len()
}

/// A repeat is judged stale against the origin's window. The copy's own `origin` field is not
/// authenticated, so naming an origin whose window is far ahead must not make a replay stale.
#[test]
fn a_replay_that_names_another_origin_is_still_struck() {
    let (mut node, inbox) = receiver();
    let (quiet, busy) = (Identity::new("quiet"), Identity::new("busy"));
    let wire = signed(&quiet, "m-1", 1);
    assert_eq!(deliver(&mut node, &inbox, "relay", &wire), 1);
    assert_eq!(
        deliver(&mut node, &inbox, "other", &signed(&busy, "b-1", 500)),
        1
    );
    let mut replay = wire.clone();
    replay.origin = busy.id.clone();
    deliver(&mut node, &inbox, "relay", &replay);
    assert_eq!(node.byzantine_events, 1, "a disguised replay is a strike");
}

/// The same with the message type, which selects the window of control messages.
#[test]
fn a_replay_relabelled_as_a_control_message_is_still_struck() {
    let (mut node, inbox) = receiver();
    let origin = Identity::new("origin");
    let wire = signed(&origin, "m-1", 1);
    assert_eq!(deliver(&mut node, &inbox, "relay", &wire), 1);
    let mut control = signed(&origin, "sync-500", 500);
    control.message_type = SYNC_REQUEST_TYPE.to_string();
    control.sync = Some(SyncDigest {
        reply_to: None,
        origins: Vec::new(),
    });
    control.sign_with(&origin);
    deliver(&mut node, &inbox, "other", &control);
    let mut replay = wire.clone();
    replay.message_type = SYNC_REQUEST_TYPE.to_string();
    deliver(&mut node, &inbox, "relay", &replay);
    assert_eq!(node.byzantine_events, 1, "a relabelled replay is a strike");
}

/// A sequence number near the end of the range must not overflow the answer's window test.
#[test]
fn a_sync_request_about_the_last_sequence_numbers_is_answered_without_overflow() {
    let (mut node, inbox) = receiver();
    let origin = Identity::new("origin");
    assert_eq!(
        deliver(
            &mut node,
            &inbox,
            "relay",
            &signed(&origin, "m", u64::MAX - 1)
        ),
        1
    );
    let mut asker = NodeRuntime::new("asker", "z", inbox.clone());
    asker
        .replay_filter
        .check_and_record(&origin.id, u64::MAX, "p");
    let asker_id = asker.id.clone();
    let request = asker.build_sync_request(None);
    deliver(&mut node, &inbox, &asker_id, &request);
    let answers = inbox.lock().unwrap().remove(&asker_id).unwrap_or_default();
    assert_eq!(answers.len(), 1);
}

// ── One transport ────────────────────────────────────────────────────────────

async fn bind() -> TcpTransport {
    TcpTransport::bind(
        "127.0.0.1:0".parse().unwrap(),
        Identity::new("t").signing_key,
    )
    .await
    .unwrap()
}

/// The transport sends an answer only to the node that asked.
#[tokio::test]
async fn an_answer_leaves_only_to_the_requester() {
    let (sender, sink) = (bind().await, bind().await);
    let target = sink.listen_addr.to_string();
    let author = Identity::new("author");
    let mut for_another = signed(&author, "m-1", 1);
    for_another.answers_sync = Some(format!("sync-{}-1", author.id));
    let mut for_sink = signed(&author, "m-2", 2);
    for_sink.answers_sync = Some(format!("sync-{}-1", sink.node_id));
    sender.relay(&target, for_another).unwrap();
    sender.relay(&target, for_sink).unwrap();
    let (_, got) = tokio::time::timeout(Duration::from_secs(3), sink.receive())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.id, "m-2");
}

// ── What an earlier run relayed ──────────────────────────────────────────────

/// At or below a mark (plus the margin) a message is accepted but not relayed; above, it is.
#[test]
fn messages_an_earlier_run_may_have_relayed_are_accepted_but_not_relayed() {
    for (margin, relayed_from) in [(0, 6), (3, 9)] {
        let (mut node, inbox) = receiver();
        let origin = Identity::new("origin");
        node.add_peer("neighbour");
        node.relayed_before = HashMap::from([(origin.id.clone(), 5)]);
        node.relay_margin = margin;
        for seq in 4..=10 {
            let wire = signed(&origin, &format!("m-{seq}"), seq);
            assert_eq!(deliver(&mut node, &inbox, "relay", &wire), 1);
        }
        let relayed: Vec<u64> = inbox
            .lock()
            .unwrap()
            .remove("neighbour")
            .unwrap_or_default()
            .iter()
            .filter_map(|(_, b)| serde_json::from_slice::<WireMessage>(b).ok())
            .filter(|w| w.origin == origin.id)
            .map(|w| w.seq)
            .collect();
        assert_eq!(relayed, (relayed_from..=10).collect::<Vec<u64>>());
        assert_eq!(node.relay_marks().get(&origin.id), Some(&10));
    }
    // An origin not heard in this run keeps the mark of the earlier one.
    let (mut node, _) = receiver();
    node.relayed_before = HashMap::from([("gone".to_string(), 7)]);
    assert_eq!(node.relay_marks().get("gone"), Some(&7));
}

/// The marks survive a restart encrypted, in a file that does not grow; a run that did not stop
/// cleanly adds a margin; a damaged file is replaced, never a reason not to start.
#[tokio::test]
async fn relay_marks_are_saved_with_the_key() {
    use crci_core::identity::{RelayMarkStore, RELAY_MARKS_MARGIN};
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("node.key");
    let file = dir.path().join("node.key.relayed");
    let marks = HashMap::from([("origin".to_string(), 12u64)]);

    let (store, none, _, problem) = RelayMarkStore::open(&key, "pw").await.unwrap();
    assert!(none.is_empty() && problem.is_none());
    store.save(&marks, false).await.unwrap();
    let size = std::fs::metadata(&file).unwrap().len();
    for _ in 0..50 {
        store.save(&marks, false).await.unwrap();
    }
    assert_eq!(std::fs::metadata(&file).unwrap().len(), size);
    let raw = std::fs::read(&file).unwrap();
    assert!(!raw.windows(6).any(|w| w == b"origin"), "stored in clear");

    let (store, loaded, margin, _) = RelayMarkStore::open(&key, "pw").await.unwrap();
    assert_eq!((loaded, margin), (marks.clone(), RELAY_MARKS_MARGIN));
    store.save(&marks, true).await.unwrap();
    let (_, loaded, margin, _) = RelayMarkStore::open(&key, "pw").await.unwrap();
    assert_eq!((loaded, margin), (marks.clone(), 0), "a clean stop");
    let (_, _, margin, _) = RelayMarkStore::open(&key, "pw").await.unwrap();
    assert_eq!(margin, RELAY_MARKS_MARGIN, "the start before did not stop");

    std::fs::write(&file, [0u8; 3]).unwrap();
    let (_, loaded, _, _) = RelayMarkStore::open(&key, "pw").await.unwrap();
    assert!(loaded.is_empty());
}
