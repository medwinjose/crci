use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The node ID a process printed at start (`Node <label> id <id>`).
fn id_in(stdout: &str) -> String {
    stdout
        .lines()
        .find(|l| l.starts_with("Node ") && l.contains(" id "))
        .and_then(|l| l.split(" id ").nth(1))
        .unwrap_or_else(|| panic!("no ID line in: {stdout}"))
        .trim()
        .to_string()
}

fn get_bin_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_crci-node"))
}

#[tokio::test]
async fn test_cli_message_send_and_receive() {
    let bin_path = get_bin_path();
    // 1. Start Node B as a listener
    let mut node_b = Command::new(&bin_path)
        .args(["--listen", "127.0.0.1:0", "--label", "node-b"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start node-b");

    let stdout = node_b.stdout.take().expect("Failed to capture stdout");
    let mut reader = BufReader::new(stdout);

    // Wait until Node B is listening
    let mut line = String::new();
    let start = Instant::now();
    // The node binds port 0 and prints the port it got.
    let mut listen_addr = String::new();
    while start.elapsed() < Duration::from_secs(5) {
        line.clear();
        if let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            if line.starts_with("Node ") && line.contains(" listening on ") {
                listen_addr = line
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .to_string();
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !listen_addr.is_empty(),
        "Node B did not start listening in time"
    );

    // 2. Dispatch message from Node A using --send
    let output_a = Command::new(&bin_path)
        .args([
            "--send",
            "SOS emergency",
            "--to",
            &listen_addr,
            "--severity",
            "rescue",
            "--label",
            "node-a",
        ])
        .output()
        .expect("Failed to run node-a send command");

    assert!(output_a.status.success(), "node-a process failed");
    let stdout_a = String::from_utf8_lossy(&output_a.stdout);
    let sender_id = id_in(&stdout_a);
    assert!(
        stdout_a.contains("SUCCESS: Message originated and sent to"),
        "node-a did not print success message. Output: {}",
        stdout_a
    );

    // Give a brief moment for Node B to parse the network buffer
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Kill Node B so we can read all its stdout without blocking
    node_b.kill().ok();
    let _ = node_b.wait();

    // 3. Assert B receives the message
    let mut line_b = String::new();
    let mut received = false;
    while let Ok(n) = reader.read_line(&mut line_b) {
        if n == 0 {
            break;
        }
        if line_b.contains(&format!("Node node-b received message from {sender_id}:")) {
            received = true;
            break;
        }
        line_b.clear();
    }

    assert!(received, "Node B did not receive the message");
}

#[tokio::test]
async fn test_cli_byzantine_mode() {
    let bin_path = get_bin_path();
    // 1. Start Node A as a listener
    let mut node_a = Command::new(&bin_path)
        .args(["--listen", "127.0.0.1:0", "--label", "node-a"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start node-a");

    let stdout = node_a.stdout.take().expect("Failed to capture stdout");
    let mut reader = BufReader::new(stdout);

    // Wait until Node A is listening
    let mut line = String::new();
    let start = Instant::now();
    // The node binds port 0 and prints the port it got.
    let mut listen_addr = String::new();
    while start.elapsed() < Duration::from_secs(5) {
        line.clear();
        if let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            if line.starts_with("Node ") && line.contains(" listening on ") {
                listen_addr = line
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .to_string();
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !listen_addr.is_empty(),
        "Node A did not start listening in time"
    );

    // 2. Start node-byzantine to flood Node A
    let output_byz = Command::new(&bin_path)
        .args([
            "--byzantine",
            "flood",
            "--to",
            &listen_addr,
            "--duration",
            "1",
            "--interval",
            "10",
            "--label",
            "byz-node",
        ])
        .output()
        .expect("Failed to run byzantine node");

    assert!(output_byz.status.success(), "byzantine node failed to run");
    let stdout_byz = String::from_utf8_lossy(&output_byz.stdout);
    let byz_id = id_in(&stdout_byz);
    assert!(
        stdout_byz.contains("[byzantine-agent] Done. Total injections attempted"),
        "Byzantine output didn't contain done message. Output: {}",
        stdout_byz
    );

    // Give a brief moment for Node A to write logs to stdout
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Kill Node A so we can read its output cleanly without blocking
    node_a.kill().ok();
    let _ = node_a.wait();

    // 3. Confirm Node A received at least some messages from byz-node
    let mut line_a = String::new();
    let mut count = 0;
    while let Ok(n) = reader.read_line(&mut line_a) {
        if n == 0 {
            break;
        }
        if line_a.contains(&format!("Node node-a received message from {byz_id}:")) {
            count += 1;
        }
        line_a.clear();
    }

    assert!(count > 0, "Node A did not receive any byzantine messages");
}

#[tokio::test]
async fn test_cli_peers_dial() {
    let bin_path = get_bin_path();
    // Bind two ports so there is something to dial; binding port 0 and keeping the listeners
    // leaves no window in which a parallel test can take the port.
    let _listener_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let _listener_b = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = _listener_a.local_addr().unwrap().to_string();
    let addr_b = _listener_b.local_addr().unwrap().to_string();

    // Start a node that dials both A and B (running without --listen exits immediately after dial attempts)
    let output = Command::new(&bin_path)
        .args([
            "--label",
            "dialer-node",
            "--peers",
            &format!("{},{}", addr_a, addr_b),
        ])
        .output()
        .expect("Failed to run dialer node");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("Successfully dialed peer: {}", addr_a)),
        "Stdout did not indicate success dialing A: {}",
        stdout
    );
    assert!(
        stdout.contains(&format!("Successfully dialed peer: {}", addr_b)),
        "Stdout did not indicate success dialing B: {}",
        stdout
    );
}

/// A replay window far beyond what the filter stores per peer is refused at start-up instead of
/// aborting on the first message (the filter sizes a bitset by `W + 1`).
#[test]
fn an_oversized_replay_window_is_refused_at_start() {
    for window in ["18446744073709551615", "1000000000000", "1025"] {
        let mut child = Command::new(get_bin_path())
            .args(["--listen", "127.0.0.1:0", "--replay-window", window])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start node");
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().expect("wait") {
                break Some(status);
            }
            if start.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let status = status.unwrap_or_else(|| panic!("--replay-window {window} was accepted"));
        assert_eq!(status.code(), Some(1), "--replay-window {window}");
    }
}

#[test]
fn the_largest_replay_window_is_accepted() {
    let mut child = Command::new(get_bin_path())
        .args(["--listen", "127.0.0.1:0", "--replay-window", "1024"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start node");
    std::thread::sleep(Duration::from_millis(1500));
    let running = child.try_wait().expect("wait").is_none();
    let _ = child.kill();
    let _ = child.wait();
    assert!(running, "--replay-window 1024 must be accepted");
}
