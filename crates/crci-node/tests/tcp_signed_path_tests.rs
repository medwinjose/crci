//! Signed messages over TCP between real `crci-node` processes.
//!
//! A receiver runs `crci-node --listen`; a second process sends to it over loopback TCP
//! (`--send` for honest traffic, `--byzantine <mode>` for an adversary). The receiver's
//! stdout shows what its `NodeRuntime` did with each message. Every process prints its
//! key-derived node ID at start (`Node <label> id <id>`); logs name peers and origins by it.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

fn bin_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.file_name().unwrap() == "deps" {
        path.pop();
    }
    path.push("crci-node");
    path
}

/// A port that is free now, for a node that must be named before it starts. It comes from a
/// range below every operating system's ephemeral range (Windows 49152+, Linux 32768+), so a
/// node that binds port 0 in another test cannot be given it between choosing and binding.
fn free_addr() -> String {
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
    let base = 20_000 + (std::process::id() % 5_000) as u16;
    loop {
        let port = base + NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) % 5_000;
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return format!("127.0.0.1:{}", port);
        }
    }
}

struct Receiver {
    child: Child,
    reader: BufReader<ChildStdout>,
    addr: String,
    id: String,
}

/// The node ID a process printed at start.
fn id_in(stdout: &str) -> String {
    stdout
        .lines()
        .find(|l| l.starts_with("Node ") && l.contains(" id "))
        .and_then(|l| l.split(" id ").nth(1))
        .unwrap_or_else(|| panic!("no ID line in:\n{stdout}"))
        .trim()
        .to_string()
}

fn start_receiver(node_id: &str) -> Receiver {
    start_node(node_id, &[])
}

fn start_node(node_id: &str, extra_args: &[&str]) -> Receiver {
    // Port 0: the node binds a free port itself and prints it, so no other test can take it
    // between choosing and binding.
    let mut child = Command::new(bin_path())
        .args(["--listen", "127.0.0.1:0", "--label", node_id])
        .args(extra_args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start receiver");
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    let start = Instant::now();
    let mut line = String::new();
    let mut id = String::new();
    while start.elapsed() < Duration::from_secs(10) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.starts_with("Node ") && line.contains(" id ") => id = id_in(&line),
            Ok(_) if line.starts_with("Node ") && line.contains(" listening on ") => {
                let addr = line
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .to_string();
                return Receiver {
                    child,
                    reader,
                    addr,
                    id,
                };
            }
            _ => {}
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("receiver {} did not start listening", node_id);
}

impl Receiver {
    /// Give the receiver time to process, stop it and return everything it printed.
    fn finish(mut self) -> String {
        std::thread::sleep(Duration::from_millis(500));
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut out = String::new();
        let _ = self.reader.read_to_string(&mut out);
        out
    }
}

impl Drop for Receiver {
    // Stop the receiver even when a test fails before `finish`.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn run_adversary(target: &str, node_id: &str, mode: &str) -> String {
    let out = Command::new(bin_path())
        .args([
            "--byzantine",
            mode,
            "--to",
            target,
            "--label",
            node_id,
            "--duration",
            "1",
            "--interval",
            "100",
        ])
        .output()
        .expect("failed to run adversary");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "adversary failed: {}", stdout);
    stdout
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.lines().filter(|l| l.contains(needle)).count()
}

/// Originate one signed rescue message from a new node labelled `label` and send it to
/// `target`. Returns the sender's node ID (the message's origin).
fn send_signed(target: &str, label: &str) -> String {
    let out = Command::new(bin_path())
        .args([
            "--send",
            "SOS emergency",
            "--to",
            target,
            "--severity",
            "rescue",
            "--label",
            label,
        ])
        .output()
        .expect("failed to run sender");
    assert!(out.status.success());
    id_in(&String::from_utf8_lossy(&out.stdout))
}

#[test]
fn accepted_messages_are_relayed_to_a_dialed_peer() {
    let downstream = start_receiver("tcp-c");
    let relay = start_node("tcp-b", &["--peers", &downstream.addr]);

    // Two different origins: a router that keyed next hops by origin would send the
    // second message to node ID "node-a", which TCP cannot address.
    let a = send_signed(&relay.addr, "node-a");
    let x = send_signed(&relay.addr, "node-x");
    let relay_id = relay.id.clone();
    let relay_log = relay.finish();
    let log = downstream.finish();

    assert_eq!(
        count(&relay_log, "accepted '"),
        2,
        "relay log:\n{}",
        relay_log
    );
    assert_eq!(count(&log, "accepted '"), 2, "downstream log:\n{}", log);
    assert!(
        log.contains(&format!("origin {a}, via {relay_id})")),
        "downstream log:\n{}",
        log
    );
    assert!(
        log.contains(&format!("origin {x}, via {relay_id})")),
        "downstream log:\n{}",
        log
    );
}

#[test]
fn honest_signed_message_is_accepted_over_tcp() {
    let receiver = start_receiver("tcp-r1");
    let a = send_signed(&receiver.addr, "node-a");
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 1, "receiver log:\n{}", log);
    assert!(
        log.contains(&format!("origin {a}, via {a})")),
        "receiver log:\n{}",
        log
    );
    assert_eq!(
        count(&log, "INVALID SIGNATURE"),
        0,
        "receiver log:\n{}",
        log
    );
}

#[test]
fn signed_replay_over_tcp_bans_the_sender_after_three_strikes() {
    let receiver = start_receiver("tcp-r2");
    let adversary = id_in(&run_adversary(&receiver.addr, "byz-replay", "replay"));
    let log = receiver.finish();

    // The first copy is accepted; the next three are strikes; the third strike bans.
    assert_eq!(count(&log, "accepted '"), 1, "receiver log:\n{}", log);
    assert_eq!(
        count(&log, &format!("MALICIOUS REPLAY from '{adversary}'")),
        3,
        "receiver log:\n{}",
        log
    );
    assert!(
        log.lines()
            .any(|l| l.contains(&format!(" banned {adversary} at_ms="))),
        "receiver log:\n{}",
        log
    );
}

#[test]
fn bad_signature_over_tcp_is_rejected() {
    let receiver = start_receiver("tcp-r3");
    let adversary = id_in(&run_adversary(
        &receiver.addr,
        "byz-badsig",
        "bad_signature",
    ));
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 0, "receiver log:\n{}", log);
    assert!(
        count(&log, "INVALID SIGNATURE") >= 1,
        "receiver log:\n{}",
        log
    );
    assert!(
        log.lines()
            .any(|l| l.contains(&format!(" banned {adversary} at_ms="))),
        "receiver log:\n{}",
        log
    );
}

#[test]
fn unsigned_message_over_tcp_is_rejected() {
    let receiver = start_receiver("tcp-r4");
    run_adversary(&receiver.addr, "byz-unsigned", "unsigned");
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 0, "receiver log:\n{}", log);
    assert!(
        count(&log, "INVALID SIGNATURE") >= 1,
        "receiver log:\n{}",
        log
    );
}

#[test]
fn unsigned_mass_casualty_event_over_tcp_is_rejected() {
    let receiver = start_receiver("tcp-r5");
    run_adversary(&receiver.addr, "byz-mce", "unsigned_mce");
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 0, "receiver log:\n{}", log);
    assert!(
        count(&log, "INVALID SIGNATURE") >= 1,
        "receiver log:\n{}",
        log
    );
}

/// Identity (node ID) the receiver attributed message `id` to, from its log.
fn sender_of(log: &str, id: &str) -> String {
    let line = log
        .lines()
        .find(|l| l.contains("received message from") && l.contains(&format!("id={id},")))
        .unwrap_or_else(|| {
            panic!(
                "no receive line for {id}:
{log}"
            )
        });
    let after = line.split("received message from ").nth(1).unwrap_or("");
    after.split(':').next().unwrap_or("").to_string()
}

/// An adversary that takes an honest node's label and replays must not get that node
/// penalised: strikes bind to the node ID the connection authenticated, which is derived from
/// the adversary's own key.
#[test]
fn adversary_with_an_honest_label_cannot_get_it_banned() {
    let receiver = start_receiver("tcp-r6");
    // The real node-a sends one honest message.
    let honest_id = send_signed(&receiver.addr, "node-a");
    // An adversary labelled "node-a" then replays until it is banned.
    let adversary_id = id_in(&run_adversary(&receiver.addr, "node-a", "replay"));
    let log = receiver.finish();

    let honest_line = log
        .lines()
        .find(|l| l.contains("accepted '") && !l.contains("replayed-msg-1"))
        .unwrap_or_else(|| {
            panic!(
                "honest message not accepted:
{log}"
            )
        });
    let honest = honest_line
        .split("via ")
        .nth(1)
        .unwrap_or("")
        .split(')')
        .next()
        .unwrap_or("")
        .to_string();
    let adversary = sender_of(&log, "replayed-msg-1");
    assert_eq!(
        honest, honest_id,
        "receiver log:
{}",
        log
    );
    assert_eq!(
        adversary, adversary_id,
        "receiver log:
{}",
        log
    );
    assert_ne!(
        honest, adversary,
        "receiver log:
{}",
        log
    );

    // Three strikes and a ban, all on the adversary's identity.
    assert_eq!(
        count(&log, &format!("MALICIOUS REPLAY from '{adversary}'")),
        3,
        "receiver log:
{}",
        log
    );
    assert!(
        log.contains(&format!(" banned {adversary} at_ms=")),
        "receiver log:
{}",
        log
    );
    assert_eq!(
        count(&log, &format!("from '{honest}'")),
        0,
        "receiver log:
{}",
        log
    );
    assert_eq!(
        count(&log, &format!(" banned {honest} at_ms=")),
        0,
        "receiver log:
{}",
        log
    );
}

/// A node that starts after messages were sent catches up through anti-entropy: it asks
/// its dialed peer with a digest, the peer answers with what it lacks, and nobody is
/// struck.
#[test]
fn late_node_catches_up_through_anti_entropy() {
    let a = start_receiver("tcp-sa");
    let origins: Vec<String> = ["node-x1", "node-x2", "node-x3"]
        .iter()
        .map(|label| send_signed(&a.addr, label))
        .collect();
    let b = start_node("tcp-sb", &["--peers", &a.addr]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    let (a_id, b_id) = (a.id.clone(), b.id.clone());
    let b_log = b.finish();
    let a_log = a.finish();

    assert!(
        b_log.contains("requested sync from"),
        "late node log:\n{}",
        b_log
    );
    assert!(
        a_log.contains(&format!("answering sync request from '{b_id}'"))
            && a_log.contains("with 3 message(s)"),
        "first node log:\n{}",
        a_log
    );
    for origin in &origins {
        assert!(
            b_log.contains(&format!("(origin {origin}, via {a_id})")),
            "late node log:\n{}",
            b_log
        );
    }
    assert_eq!(
        count(&b_log, "MALICIOUS REPLAY"),
        0,
        "late node log:\n{}",
        b_log
    );
    assert_eq!(
        count(&a_log, "MALICIOUS REPLAY"),
        0,
        "first node log:\n{}",
        a_log
    );
    assert_eq!(
        count(&b_log, "REPLAY dropped"),
        0,
        "late node log:\n{}",
        b_log
    );
}

/// A node more than one answer behind (`SYNC_MAX_BATCH` = 16 messages) asks again as soon as the
/// resume spacing allows, not at the next 30 s round: 40 missing messages arrive in three answers
/// (16, 16, 8) about 8 s apart.
#[test]
fn a_node_behind_by_more_than_one_answer_asks_again_at_once() {
    let a = start_node(
        "tcp-full-a",
        &[
            "--emit-count",
            "40",
            "--emit-interval-ms",
            "20",
            "--emit-delay-ms",
            "0",
        ],
    );
    std::thread::sleep(Duration::from_secs(2));
    let b = start_node("tcp-full-b", &["--peers", &a.addr]);
    let a_id = a.id.clone();
    // Without asking again the second answer comes at the 30 s round; 20 s leaves 4 s of margin
    // over the third answer (about 16 s after the first request) on a slow runner.
    std::thread::sleep(Duration::from_secs(20));
    let b_log = b.finish();
    let a_log = a.finish();
    assert_eq!(
        count(&b_log, &format!("(origin {a_id}, via {a_id})")),
        40,
        "late node log:\n{b_log}\nfirst node log:\n{a_log}"
    );
    assert!(
        a_log.contains("with 16 message(s)") && a_log.contains("with 8 message(s)"),
        "first node log:\n{a_log}"
    );
    assert_eq!(
        count(&b_log, "MALICIOUS REPLAY"),
        0,
        "late node log:\n{b_log}"
    );
    assert_eq!(
        count(&a_log, "MALICIOUS REPLAY"),
        0,
        "first node log:\n{a_log}"
    );
}

/// A node started before its peer still relays to it once the peer is up: relay targets
/// are the configured list, not the peers that answered at start-up.
#[test]
fn node_started_before_its_peer_relays_to_it_once_it_is_up() {
    let downstream_addr = free_addr();
    let relay = start_node("tcp-early", &["--peers", &downstream_addr]);

    // The peer is not running yet.
    send_signed(&relay.addr, "node-before");
    std::thread::sleep(Duration::from_millis(500));

    let mut downstream = Command::new(bin_path())
        .args(["--listen", &downstream_addr, "--label", "tcp-late"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start downstream node");
    let mut reader = BufReader::new(downstream.stdout.take().unwrap());
    let mut line = String::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.starts_with("Node ") && line.contains(" listening on ") => break,
            _ => {}
        }
    }

    let after = send_signed(&relay.addr, "node-after");
    std::thread::sleep(Duration::from_millis(1000));
    let _ = downstream.kill();
    let _ = downstream.wait();
    let mut log = String::new();
    let _ = reader.read_to_string(&mut log);
    let relay_id = relay.id.clone();
    let relay_log = relay.finish();

    assert!(
        log.contains(&format!("(origin {after}, via {relay_id})")),
        "downstream log:
{}
relay log:
{}",
        log,
        relay_log
    );
}

/// `--emit-count` originates signed messages that a peer accepts, with timestamps that
/// experiments use to measure dissemination latency.
#[test]
fn emitted_messages_reach_a_peer_with_timestamps() {
    let downstream = start_receiver("tcp-emit-sink");
    let emitter = start_node(
        "tcp-emitter",
        &[
            "--peers",
            &downstream.addr,
            "--emit-count",
            "3",
            "--emit-interval-ms",
            "100",
            "--emit-delay-ms",
            "200",
        ],
    );
    std::thread::sleep(Duration::from_millis(1000));
    let emitter_id = emitter.id.clone();
    let emitter_log = emitter.finish();
    let log = downstream.finish();

    assert_eq!(
        count(&emitter_log, "originated '"),
        3,
        "emitter log:\n{}",
        emitter_log
    );
    let accepted: Vec<&str> = log
        .lines()
        .filter(|l| {
            l.contains("accepted '")
                && l.contains(&format!("(origin {emitter_id}, via {emitter_id})"))
        })
        .collect();
    assert_eq!(accepted.len(), 3, "downstream log:\n{}", log);
    assert!(
        accepted.iter().all(|l| l.contains(" at_ms=")),
        "downstream log:\n{}",
        log
    );
}

/// In the replay fault mode a node sends every gossip message twice (the default); its
/// peer strikes it once per repeat and bans it after the third strike.
#[test]
fn a_replaying_node_is_banned_by_its_peer() {
    let downstream = start_receiver("tcp-fault-sink");
    let faulty = start_node(
        "tcp-faulty",
        &[
            "--peers",
            &downstream.addr,
            "--emit-count",
            "3",
            "--emit-interval-ms",
            "100",
            "--emit-delay-ms",
            "200",
            "--replay-after-ms",
            "0",
        ],
    );
    std::thread::sleep(Duration::from_millis(1000));
    let faulty_id = faulty.id.clone();
    let faulty_log = faulty.finish();
    let log = downstream.finish();

    assert!(
        faulty_log.contains("fault: sending every gossip message 2 times"),
        "faulty log:\n{}",
        faulty_log
    );
    assert_eq!(count(&log, "accepted '"), 3, "downstream log:\n{}", log);
    assert_eq!(
        count(&log, &format!("MALICIOUS REPLAY from '{faulty_id}'")),
        3,
        "downstream log:\n{}",
        log
    );
    assert_eq!(
        count(&log, &format!("Node tcp-fault-sink banned {faulty_id} ")),
        1,
        "downstream log:\n{}",
        log
    );
}

/// A relay that forwards forged signatures is banned after three strikes. Its connection is
/// then closed and not read again (its reconnects are refused until the probe interval), so
/// the 300 frames it keeps sending cost the receiver nothing, and no log line each.
#[test]
fn a_banned_forger_is_disconnected_and_costs_nothing() {
    let sink = start_receiver("tcp-forge-sink");
    let forger = start_node(
        "tcp-forger",
        &[
            "--peers",
            &sink.addr,
            "--emit-count",
            "300",
            "--emit-interval-ms",
            "5",
            "--emit-delay-ms",
            "200",
            "--replay-after-ms",
            "0",
            "--replay-copies",
            "1",
            "--corrupt-signatures",
        ],
    );
    std::thread::sleep(Duration::from_millis(3500));
    let forger_id = forger.id.clone();
    drop(forger);
    let log = sink.finish();

    assert_eq!(
        count(&log, "accepted '"),
        0,
        "sink log:
{}",
        log
    );
    assert_eq!(
        count(&log, "INVALID SIGNATURE"),
        3,
        "sink log:
{}",
        log
    );
    assert_eq!(
        count(&log, &format!("Node tcp-forge-sink banned {forger_id} ")),
        1,
        "sink log:
{}",
        log
    );
    assert_eq!(
        count(&log, "SYBIL GUARD DROP"),
        0,
        "a banned peer's frames must not log one line each:
{}",
        log
    );
    // Frames that were already in flight when the ban took effect may still arrive; the
    // other ~290 must never be read.
    let received = count(&log, &format!("received message from {forger_id}:"));
    assert!(
        received <= 60,
        "{} frames reached the node after three strikes:
{}",
        received,
        log
    );
}

/// The evaluation's baselines run in the real node: a `--defence vanilla` receiver counts a
/// replaying peer's repeats but never bans it, and `--strikes-to-ban 1` bans at the first one.
#[test]
fn defence_flags_change_what_a_receiver_does_with_a_replayer() {
    let full = start_node("tcp-def-full", &[]);
    let adversary = id_in(&run_adversary(&full.addr, "byz-full", "replay"));
    let full_log = full.finish();
    assert_eq!(
        count(&full_log, &format!(" banned {adversary} ")),
        1,
        "{full_log}"
    );

    let vanilla = start_node("tcp-def-vanilla", &["--defence", "vanilla"]);
    let adversary = id_in(&run_adversary(&vanilla.addr, "byz-vanilla", "replay"));
    let vanilla_log = vanilla.finish();
    assert!(
        count(
            &vanilla_log,
            &format!("MALICIOUS REPLAY from '{adversary}'")
        ) > 3,
        "the strikes are still counted:
{vanilla_log}"
    );
    assert_eq!(
        count(&vanilla_log, &format!(" banned {adversary} ")),
        0,
        "{vanilla_log}"
    );

    let strict = start_node("tcp-def-k1", &["--strikes-to-ban", "1"]);
    let adversary = id_in(&run_adversary(&strict.addr, "byz-k1", "replay"));
    let strict_log = strict.finish();
    assert_eq!(
        count(&strict_log, &format!("MALICIOUS REPLAY from '{adversary}'")),
        1,
        "k = 1 bans at the first strike:
{strict_log}"
    );
    assert_eq!(
        count(&strict_log, &format!(" banned {adversary} ")),
        1,
        "{strict_log}"
    );
}

/// A TCP proxy in front of one listener. While `discard` is set it swallows what the dialer
/// sends (the dialer's TCP sees it acknowledged, as with frames left in an aborted
/// connection); `cut` closes every proxied connection and resumes forwarding.
struct LossyProxy {
    addr: String,
    discard: std::sync::Arc<std::sync::atomic::AtomicBool>,
    open: std::sync::Arc<std::sync::Mutex<Vec<std::net::TcpStream>>>,
}

impl LossyProxy {
    fn start(target: std::sync::Arc<std::sync::Mutex<String>>) -> LossyProxy {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let discard = Arc::new(AtomicBool::new(false));
        let open: Arc<Mutex<Vec<std::net::TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let (d, o) = (discard.clone(), open.clone());
        std::thread::spawn(move || {
            for client in listener.incoming().flatten() {
                let dest = target.lock().unwrap().clone();
                let Ok(server) = std::net::TcpStream::connect(&dest) else {
                    continue;
                };
                o.lock().unwrap().push(client.try_clone().unwrap());
                o.lock().unwrap().push(server.try_clone().unwrap());
                let (mut c_in, mut s_out) =
                    (client.try_clone().unwrap(), server.try_clone().unwrap());
                let discard = d.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 8192];
                    while let Ok(n) = c_in.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        if !discard.load(Ordering::SeqCst)
                            && std::io::Write::write_all(&mut s_out, &buf[..n]).is_err()
                        {
                            break;
                        }
                    }
                    let _ = s_out.shutdown(std::net::Shutdown::Both);
                });
                let (mut s_in, mut c_out) = (server, client);
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut s_in, &mut c_out);
                    let _ = c_out.shutdown(std::net::Shutdown::Both);
                });
            }
        });
        LossyProxy {
            addr,
            discard,
            open,
        }
    }

    fn cut(&self) {
        for s in self.open.lock().unwrap().drain(..) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        self.discard
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A frame written into a relay connection that is then lost reaches the peer soon after
/// the connection is re-established: the sender reconnects and tells the peer, which asks
/// it for a sync, instead of waiting for the next periodic exchange (30 s).
#[test]
fn frame_lost_with_a_relay_connection_is_recovered_after_reconnect() {
    lose_a_frame_then_cut(true);
}

/// The same when the sender has nothing more to send: it reconnects by itself.
#[test]
fn frame_lost_on_a_link_that_then_goes_quiet_is_recovered() {
    lose_a_frame_then_cut(false);
}

fn lose_a_frame_then_cut(traffic_after: bool) {
    let target = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let proxy = LossyProxy::start(target.clone());
    let a = start_node("tcp-loss-a", &["--peers", &proxy.addr]);
    let b = start_node("tcp-loss-b", &["--peers", &a.addr]);
    *target.lock().unwrap() = b.addr.clone();
    // Let both start-up syncs finish, then open A's relay connection to B, and let any sync
    // that connection's opening scheduled pass (one cooldown), so it cannot mask the loss.
    std::thread::sleep(Duration::from_millis(1500));
    let mut expected = vec![send_signed(&a.addr, "node-before-loss")];
    std::thread::sleep(Duration::from_secs(8));

    proxy
        .discard
        .store(true, std::sync::atomic::Ordering::SeqCst);
    expected.push(send_signed(&a.addr, "node-lost"));
    std::thread::sleep(Duration::from_millis(500));
    proxy.cut();
    if traffic_after {
        // The next relay finds the connection closed and dials a new one.
        expected.push(send_signed(&a.addr, "node-after-loss"));
    }
    std::thread::sleep(Duration::from_secs(10));
    let a_id = a.id.clone();
    let b_log = b.finish();
    let a_log = a.finish();

    for origin in expected {
        assert!(
            b_log.contains(&format!("(origin {origin}, via {a_id})")),
            "{origin} missing at B\nB log:\n{b_log}\nA log:\n{a_log}"
        );
    }
    assert_eq!(count(&b_log, "MALICIOUS REPLAY"), 0, "B log:\n{b_log}");
    assert_eq!(count(&a_log, "MALICIOUS REPLAY"), 0, "A log:\n{a_log}");
}

/// A peer that never answers (192.0.2.1 is reserved for documentation, RFC 5737) must not
/// hold up start-up: the node listens within a few seconds, whatever its peers do.
#[test]
fn an_unreachable_peer_does_not_delay_start_up() {
    let started = std::time::Instant::now();
    let mut child = std::process::Command::new(bin_path())
        .args([
            "--listen",
            "127.0.0.1:0",
            "--label",
            "tcp-startup",
            "--peers",
            "192.0.2.1:7000,192.0.2.2:7000",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start node");
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut listening = None;
    let mut line = String::new();
    while started.elapsed() < std::time::Duration::from_secs(30) {
        line.clear();
        match std::io::BufRead::read_line(&mut reader, &mut line) {
            Ok(0) => break,
            Ok(_) if line.starts_with("Node ") && line.contains(" listening on ") => {
                listening = Some(started.elapsed());
                break;
            }
            _ => {}
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let listening = listening.expect("node never started listening");
    assert!(
        listening < std::time::Duration::from_secs(5),
        "node took {:?} to start listening",
        listening
    );
}

/// With `--key-file` a node keeps its ID across restarts; a wrong passphrase stops it at start
/// instead of making a new key.
#[test]
fn a_node_with_a_key_file_keeps_its_id_across_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("node.key");
    let pass = dir.path().join("pass.txt");
    std::fs::write(&pass, "correct horse\n").unwrap();
    let args = [
        "--key-file",
        key.to_str().unwrap(),
        "--key-passphrase-file",
        pass.to_str().unwrap(),
    ];
    let first = start_node("tcp-keep", &args);
    let first_id = first.id.clone();
    drop(first);
    let second = start_node("tcp-keep-again", &args);
    assert_eq!(second.id, first_id, "a restart must keep the node ID");
    drop(second);
    let fresh = start_node("tcp-keep", &[]);
    assert_ne!(
        fresh.id, first_id,
        "without a key file every start has a new ID"
    );
    drop(fresh);

    std::fs::write(&pass, "wrong\n").unwrap();
    let out = Command::new(bin_path())
        .args(["--listen", "127.0.0.1:0", "--label", "tcp-keep"])
        .args(args)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Cannot load the node key"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// An impostor that announces an honest node's ID first, over TCP, does not silence it: the
/// claim is not authentic (its origin is not derived from the impostor's key), so the receiver
/// strikes the impostor, and the honest node's message, which arrives later, is accepted.
#[test]
fn an_impostor_announcing_an_honest_id_over_tcp_does_not_silence_it() {
    use crci_core::identity::Identity;
    use crci_core::message::{Message, Signal, Visibility};
    use crci_core::runtime::WireMessage;

    let receiver = start_receiver("tcp-squat-sink");
    let honest = start_node(
        "tcp-squat-honest",
        &[
            "--peers",
            &receiver.addr,
            "--emit-count",
            "1",
            "--emit-delay-ms",
            "1500",
        ],
    );
    let honest_id = honest.id.clone();

    let impostor = Identity::new("tcp-squat-honest");
    let mut msg = Message::new(
        "claim-1",
        &honest_id,
        Signal::new(5, true, false, true, 3, Visibility::Direct),
        None,
    );
    msg.seq = 1;
    let mut claim = WireMessage::from_message(&msg, &impostor);
    claim.origin = honest_id.clone();
    claim.sign_with(&impostor);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(crci_core::transport::tcp::send_authenticated(
            &receiver.addr,
            &impostor.signing_key,
            &[claim],
        ))
        .unwrap();

    std::thread::sleep(Duration::from_millis(2500));
    drop(honest);
    let log = receiver.finish();

    assert_eq!(count(&log, "ORIGIN NOT DERIVED FROM KEY"), 1, "{log}");
    assert_eq!(
        count(&log, &format!("(origin {honest_id}, via {honest_id})")),
        1,
        "the honest node's message must be accepted:\n{log}"
    );
    assert_eq!(count(&log, "PUBKEY MISMATCH"), 0, "{log}");
}

/// A node restarted from its key file sends under the same ID to a peer that kept its messages
/// from the first run. Its new messages (and its sync requests) are numbered above the first
/// run's, so the peer accepts them and strikes nothing; without the stored floor they would
/// reuse sequence numbers the peer holds and be struck as replays.
#[test]
fn a_node_restarted_from_its_key_file_is_not_struck_by_its_peer() {
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("node.key");
    let pass = dir.path().join("pass.txt");
    std::fs::write(&pass, "pw\n").unwrap();
    let peer = start_receiver("tcp-restart-peer");
    let args = |emit: &'static str| {
        vec![
            "--key-file".to_string(),
            key.to_str().unwrap().to_string(),
            "--key-passphrase-file".to_string(),
            pass.to_str().unwrap().to_string(),
            "--peers".to_string(),
            peer.addr.clone(),
            "--emit-count".to_string(),
            emit.to_string(),
            "--emit-interval-ms".to_string(),
            "100".to_string(),
            "--emit-delay-ms".to_string(),
            "300".to_string(),
        ]
    };
    let first_args = args("2");
    let first = start_node(
        "tcp-restarted",
        &first_args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    std::thread::sleep(Duration::from_millis(1200));
    let id = first.id.clone();
    drop(first);
    let second_args = args("2");
    let second = start_node(
        "tcp-restarted",
        &second_args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(second.id, id);
    std::thread::sleep(Duration::from_millis(1200));
    drop(second);
    let log = peer.finish();

    assert_eq!(
        count(&log, &format!("(origin {id}, via {id})")),
        4,
        "every message of both runs is accepted:\n{log}"
    );
    assert_eq!(count(&log, "MALICIOUS REPLAY"), 0, "{log}");
    assert_eq!(count(&log, " banned "), 0, "{log}");
}
