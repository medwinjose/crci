//! Signed messages over TCP between real `crci-node` processes.
//!
//! A receiver runs `crci-node --listen`; a second process sends to it over loopback TCP
//! (`--send` for honest traffic, `--byzantine <mode>` for an adversary). The receiver's
//! stdout shows what its `NodeRuntime` did with each message.

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

fn free_addr() -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("127.0.0.1:{}", port)
}

struct Receiver {
    child: Child,
    reader: BufReader<ChildStdout>,
    addr: String,
}

fn start_receiver(node_id: &str) -> Receiver {
    start_node(node_id, &[])
}

fn start_node(node_id: &str, extra_args: &[&str]) -> Receiver {
    let addr = free_addr();
    let mut child = Command::new(bin_path())
        .args(["--listen", &addr, "--node-id", node_id])
        .args(extra_args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start receiver");
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    let start = Instant::now();
    let mut line = String::new();
    while start.elapsed() < Duration::from_secs(10) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.contains("listening on") => {
                return Receiver {
                    child,
                    reader,
                    addr,
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
            "--node-id",
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

/// Originate one signed rescue message from `node_id` and send it to `target`.
fn send_signed(target: &str, node_id: &str) {
    let out = Command::new(bin_path())
        .args([
            "--send",
            "SOS emergency",
            "--to",
            target,
            "--severity",
            "rescue",
            "--node-id",
            node_id,
        ])
        .output()
        .expect("failed to run sender");
    assert!(out.status.success());
}

#[test]
fn accepted_messages_are_relayed_to_a_dialed_peer() {
    let downstream = start_receiver("tcp-c");
    let relay = start_node("tcp-b", &["--peers", &downstream.addr]);

    // Two different origins: a router that keyed next hops by origin would send the
    // second message to node ID "node-a", which TCP cannot address.
    send_signed(&relay.addr, "node-a");
    send_signed(&relay.addr, "node-x");
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
        log.contains("origin node-a, via tcp-b#"),
        "downstream log:\n{}",
        log
    );
    assert!(
        log.contains("origin node-x, via tcp-b#"),
        "downstream log:\n{}",
        log
    );
}

#[test]
fn honest_signed_message_is_accepted_over_tcp() {
    let receiver = start_receiver("tcp-r1");
    send_signed(&receiver.addr, "node-a");
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 1, "receiver log:\n{}", log);
    assert!(
        log.contains("origin node-a, via node-a#"),
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
    run_adversary(&receiver.addr, "byz-replay", "replay");
    let log = receiver.finish();

    // The first copy is accepted; the next three are strikes; the third strike bans.
    assert_eq!(count(&log, "accepted '"), 1, "receiver log:\n{}", log);
    assert_eq!(
        count(&log, "MALICIOUS REPLAY from 'byz-replay#"),
        3,
        "receiver log:\n{}",
        log
    );
    assert!(
        log.lines()
            .any(|l| l.contains("peer byz-replay#") && l.contains("is banned")),
        "receiver log:\n{}",
        log
    );
}

#[test]
fn bad_signature_over_tcp_is_rejected() {
    let receiver = start_receiver("tcp-r3");
    run_adversary(&receiver.addr, "byz-badsig", "bad_signature");
    let log = receiver.finish();

    assert_eq!(count(&log, "accepted '"), 0, "receiver log:\n{}", log);
    assert!(
        count(&log, "INVALID SIGNATURE") >= 1,
        "receiver log:\n{}",
        log
    );
    assert!(
        log.lines()
            .any(|l| l.contains("peer byz-badsig#") && l.contains("is banned")),
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

/// Identity (`node-id#key`) the receiver attributed message `id` to, from its log.
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

/// An adversary that claims an honest node's ID and replays must not get that node
/// penalised: strikes bind to the key the connection authenticated, not to a claimed ID.
#[test]
fn adversary_claiming_an_honest_id_cannot_get_it_banned() {
    let receiver = start_receiver("tcp-r6");
    // The real node-a sends one honest message.
    send_signed(&receiver.addr, "node-a");
    // An adversary then declares itself "node-a" and replays until it is banned.
    run_adversary(&receiver.addr, "node-a", "replay");
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
        .trim_end_matches(')')
        .to_string();
    let adversary = sender_of(&log, "replayed-msg-1");
    assert!(
        honest.starts_with("node-a#"),
        "receiver log:
{}",
        log
    );
    assert!(
        adversary.starts_with("node-a#"),
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
        log.contains(&format!("peer {adversary} is banned")),
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
        count(&log, &format!("peer {honest} is banned")),
        0,
        "receiver log:
{}",
        log
    );
}
