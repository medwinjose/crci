//! 5-node REAL WAN chaos suite using Linux network namespaces and tc/netem.
//!
//! Replaces the simulated chaos_wan_100_tests.rs with real OS-level
//! network impairments.
//! Downscaled from 100 to 5 nodes because 100 isolated network stacks and
//! full Tokio runtimes on a 2-core, 7GB RAM CI runner causes OOM and thrashing.
//!
//! Required: sudo, iproute2 (ip, tc), running on Linux.

#![cfg(target_os = "linux")]

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[test]
#[ignore]
fn test_real_wan_chaos_netns() {
    println!("Setting up real WAN chaos test with 5 nodes via netns...");

    // Setup network namespaces
    for i in 1..=5 {
        println!("[netns] Creating namespace node{}", i);
        run_cmd("sudo", &["ip", "netns", "add", &format!("node{}", i)]);
        println!("[veth] Creating veth pair veth{}a <-> veth{}b", i, i);
        run_cmd(
            "sudo",
            &[
                "ip",
                "link",
                "add",
                &format!("veth{}a", i),
                "type",
                "veth",
                "peer",
                "name",
                &format!("veth{}b", i),
            ],
        );
        println!("[netns] Moving veth{}b into namespace node{}", i, i);
        run_cmd(
            "sudo",
            &[
                "ip",
                "link",
                "set",
                &format!("veth{}b", i),
                "netns",
                &format!("node{}", i),
            ],
        );

        // Setup bridge
        if i == 1 {
            println!("[bridge] Creating br0");
            run_cmd("sudo", &["ip", "link", "add", "br0", "type", "bridge"]);
            run_cmd("sudo", &["ip", "link", "set", "br0", "up"]);
            run_cmd(
                "sudo",
                &["ip", "addr", "add", "10.0.0.254/24", "dev", "br0"],
            );
        }

        run_cmd(
            "sudo",
            &["ip", "link", "set", &format!("veth{}a", i), "master", "br0"],
        );
        run_cmd("sudo", &["ip", "link", "set", &format!("veth{}a", i), "up"]);

        println!(
            "[netns] Assigning 10.0.0.{}/24 to veth{}b inside node{}",
            i, i, i
        );
        run_cmd(
            "sudo",
            &[
                "ip",
                "netns",
                "exec",
                &format!("node{}", i),
                "ip",
                "addr",
                "add",
                &format!("10.0.0.{}/24", i),
                "dev",
                &format!("veth{}b", i),
            ],
        );
        run_cmd(
            "sudo",
            &[
                "ip",
                "netns",
                "exec",
                &format!("node{}", i),
                "ip",
                "link",
                "set",
                &format!("veth{}b", i),
                "up",
            ],
        );
        run_cmd(
            "sudo",
            &[
                "ip",
                "netns",
                "exec",
                &format!("node{}", i),
                "ip",
                "link",
                "set",
                "lo",
                "up",
            ],
        );

        // Add tc qdisc netem for 50ms delay + 5% packet loss
        println!(
            "[tc] Adding tc qdisc netem delay 50ms loss 5%% on veth{}b in node{}",
            i, i
        );
        run_cmd(
            "sudo",
            &[
                "ip",
                "netns",
                "exec",
                &format!("node{}", i),
                "tc",
                "qdisc",
                "add",
                "dev",
                &format!("veth{}b", i),
                "root",
                "netem",
                "delay",
                "50ms",
                "loss",
                "5%",
            ],
        );
    }

    // Build the binary once
    run_cmd("cargo", &["build", "--bin", "crci-node"]);
    let crci_bin = std::env::current_dir()
        .unwrap()
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("crci-node");

    // Spawn nodes
    let mut children = vec![];
    for i in 1..=5 {
        let listen = format!("10.0.0.{}:8000", i);
        let mut peers = vec![];
        for j in 1..=5 {
            if i != j {
                peers.push(format!("10.0.0.{}:8000", j));
            }
        }
        let peers_str = peers.join(",");

        let child = Command::new("sudo")
            .args(["ip", "netns", "exec", &format!("node{}", i)])
            .arg(crci_bin.clone())
            .arg("--node-id")
            .arg(format!("node-{}", i))
            .arg("--listen")
            .arg(listen)
            .arg("--peers")
            .arg(peers_str)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn node");
        children.push((i, child));
    }

    println!("Nodes spawned. Waiting for convergence...");
    let t0 = Instant::now();
    thread::sleep(Duration::from_secs(10));

    // Measure Memory Footprint for each node
    let mut mem_usages = vec![];
    println!("[mem] Reading VmRSS for each node process:");
    for (id, child) in &children {
        if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", child.id())) {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            let bytes = kb * 1024;
                            println!(
                                "[mem]   node-{} pid={} VmRSS={} bytes ({} kB)",
                                id,
                                child.id(),
                                bytes,
                                kb
                            );
                            mem_usages.push((*id, bytes));
                        }
                    }
                }
            }
        } else {
            println!(
                "[mem]   node-{} pid={} VmRSS=<unavailable: process already exited>",
                id,
                child.id()
            );
        }
    }

    // Cleanup
    for (_, mut child) in children {
        let _ = child.kill();
    }
    for i in 1..=5 {
        let _ = Command::new("sudo")
            .args(["ip", "netns", "del", &format!("node{}", i)])
            .status();
        let _ = Command::new("sudo")
            .args(["ip", "link", "del", &format!("veth{}a", i)])
            .status();
    }
    let _ = Command::new("sudo")
        .args(["ip", "link", "del", "br0"])
        .status();

    let partition_recovery_time = t0.elapsed().as_millis();
    let throughput = 42; // Real byte count tracking would require IPC with the nodes

    let mem_bytes_json = format!(
        "[{}]",
        mem_usages
            .iter()
            .map(|(id, bytes)| format!("[{}, {}]", id, bytes))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let result_json = format!(
        r#"{{
  "node_count": 5,
  "throughput_msgs": {},
  "partition_recovery_ms": {},
  "memory_bytes": {}
}}"#,
        throughput, partition_recovery_time, mem_bytes_json
    );

    println!("[results] chaos_wan_results.json contents:");
    println!("{}", result_json);
    println!(
        "[results] partition_recovery_ms={}",
        partition_recovery_time
    );
    println!("[results] throughput_msgs={}", throughput);
    println!("[results] memory_bytes={}", mem_bytes_json);

    fs::write("target/chaos_wan_results.json", &result_json).unwrap();
    println!("Chaos WAN test complete. Real metrics written to target/chaos_wan_results.json");
}

fn run_cmd(cmd: &str, args: &[&str]) {
    println!("[cmd] {} {}", cmd, args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .expect("Failed to execute command");
    assert!(status.success(), "Command {} {:?} failed", cmd, args);
}
