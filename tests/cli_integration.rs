use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn get_bin_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove dependency file name
    if path.file_name().unwrap() == "deps" {
        path.pop(); // remove deps folder
    }
    path.push("crci-node");
    path
}

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn test_cli_message_send_and_receive() {
    let bin_path = get_bin_path();
    let port = get_free_port();
    let listen_addr = format!("127.0.0.1:{}", port);

    // 1. Start Node B as a listener
    let mut node_b = Command::new(&bin_path)
        .args(["--listen", &listen_addr, "--node-id", "node-b"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start node-b");

    let stdout = node_b.stdout.take().expect("Failed to capture stdout");
    let mut reader = BufReader::new(stdout);

    // Wait until Node B is listening
    let mut line = String::new();
    let start = Instant::now();
    let mut listening = false;
    while start.elapsed() < Duration::from_secs(5) {
        line.clear();
        if let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            if line.contains("listening on") {
                listening = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(listening, "Node B did not start listening in time");

    // 2. Dispatch message from Node A using --send
    let output_a = Command::new(&bin_path)
        .args([
            "--send",
            "SOS emergency",
            "--to",
            &listen_addr,
            "--severity",
            "rescue",
            "--node-id",
            "node-a",
        ])
        .output()
        .expect("Failed to run node-a send command");

    assert!(output_a.status.success(), "node-a process failed");
    let stdout_a = String::from_utf8_lossy(&output_a.stdout);
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
        if line_b.contains("Node node-b received message from node-a") {
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
    let port = get_free_port();
    let listen_addr = format!("127.0.0.1:{}", port);

    // 1. Start Node A as a listener
    let mut node_a = Command::new(&bin_path)
        .args(["--listen", &listen_addr, "--node-id", "node-a"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start node-a");

    let stdout = node_a.stdout.take().expect("Failed to capture stdout");
    let mut reader = BufReader::new(stdout);

    // Wait until Node A is listening
    let mut line = String::new();
    let start = Instant::now();
    let mut listening = false;
    while start.elapsed() < Duration::from_secs(5) {
        line.clear();
        if let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            if line.contains("listening on") {
                listening = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(listening, "Node A did not start listening in time");

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
            "--node-id",
            "byz-node",
        ])
        .output()
        .expect("Failed to run byzantine node");

    assert!(output_byz.status.success(), "byzantine node failed to run");
    let stdout_byz = String::from_utf8_lossy(&output_byz.stdout);
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
        if line_a.contains("Node node-a received message from byz-node") {
            count += 1;
        }
        line_a.clear();
    }

    assert!(count > 0, "Node A did not receive any byzantine messages");
}

#[tokio::test]
async fn test_cli_peers_dial() {
    let bin_path = get_bin_path();
    let port_a = get_free_port();
    let port_b = get_free_port();
    let addr_a = format!("127.0.0.1:{}", port_a);
    let addr_b = format!("127.0.0.1:{}", port_b);

    // Bind two ports so there is something to dial
    let _listener_a = std::net::TcpListener::bind(&addr_a).unwrap();
    let _listener_b = std::net::TcpListener::bind(&addr_b).unwrap();

    // Start a node that dials both A and B (running without --listen exits immediately after dial attempts)
    let output = Command::new(&bin_path)
        .args([
            "--node-id",
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
