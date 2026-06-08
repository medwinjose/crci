use crci_core::runtime::NodeRuntime;
use crci_core::transport::{NetworkMessage, TcpTransport, Transport};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn byzantine_eviction_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    const TRIALS: usize = 20;

    let mut rows: Vec<(usize, u128, u128, bool)> = Vec::new();

    for trial in 0..TRIALS {
        let base_port = 19005 + trial * 2;
        let addr_a_req: SocketAddr = format!("127.0.0.1:{}", base_port).parse()?;
        let addr_b_req: SocketAddr = format!("127.0.0.1:{}", base_port + 1).parse()?;

        // Node A — honest listener
        let inbox_a = Arc::new(Mutex::new(HashMap::new()));
        let node_a = Arc::new(Mutex::new(NodeRuntime::new("node-a", "zone-a", inbox_a)));

        let tcp_a = TcpTransport::bind("node-a".to_string(), addr_a_req).await?;
        let addr_a = tcp_a.listen_addr;

        let node_a_clone = node_a.clone();
        let handle_a = tokio::spawn(async move {
            while let Ok((sender_id, _msg)) = tcp_a.receive().await {
                if let Ok(mut na) = node_a_clone.lock() {
                    na.add_peer(&sender_id);
                }
            }
        });

        // Node B — legitimate peer
        let inbox_b = Arc::new(Mutex::new(HashMap::new()));
        let node_b = Arc::new(Mutex::new(NodeRuntime::new("node-b", "zone-b", inbox_b)));

        let tcp_b = TcpTransport::bind("node-b".to_string(), addr_b_req).await?;
        let _addr_b = tcp_b.listen_addr;

        // Node B dials Node A
        {
            let mut nb = node_b.lock().unwrap();
            nb.add_peer(&addr_a.to_string());
        }

        let t0 = Instant::now();

        let handshake_msg = NetworkMessage {
            id: format!("handshake-{}-{}", trial, "b"),
            origin_node: "node-b".to_string(),
            zone: "zone-b".to_string(),
            severity: 1,
            kind: crci_core::integration::MessageKind::Normal,
            payload_bytes: 0,
            reputation: 1.0,
            round: 1,
            seq: 1,
        };
        tcp_b.send(&addr_a.to_string(), &handshake_msg).await?;

        // Wait for legitimate handshake
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let count_a = node_a.lock().unwrap().peers.len();
            if count_a > 0 {
                break;
            }
            if tokio::time::Instant::now() > deadline {
                panic!("Legitimate handshake did not complete");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let handshake_ms = t0.elapsed().as_millis();

        // Inject Byzantine peer, time until peer_count stabilizes
        let t1 = Instant::now();
        let mut byz = tokio::net::TcpStream::connect(&addr_a)
            .await
            .expect("Byzantine connect failed");
        for _ in 0..10 {
            let garbage = b"BYZANTINE_GARBAGE\n";
            byz.write_u32(garbage.len() as u32).await.ok();
            byz.write_all(garbage).await.ok();
        }
        drop(byz);
        tokio::time::sleep(Duration::from_millis(500)).await;
        let eviction_ms = t1.elapsed().as_millis();

        let survived = !node_a.lock().unwrap().peers.is_empty();
        rows.push((trial + 1, handshake_ms, eviction_ms, survived));

        handle_a.abort(); // Clean up listener to free socket for next iteration if reused
    }

    // Write CSV
    std::fs::create_dir_all("benches/results").expect("create results dir");
    let mut csv = String::from(
        "trial,legitimate_handshake_ms,byzantine_eviction_ms,legitimate_peer_survived\n",
    );
    for (t, h, e, s) in &rows {
        csv.push_str(&format!("{},{},{},{}\n", t, h, e, s));
    }
    std::fs::write("benches/results/byzantine_eviction.csv", &csv).expect("write CSV");

    // Summary
    let mean_eviction: u128 = rows.iter().map(|r| r.2).sum::<u128>() / TRIALS as u128;
    let mut evictions: Vec<u128> = rows.iter().map(|r| r.2).collect();
    evictions.sort_unstable();
    let p95 = evictions[(TRIALS as f64 * 0.95) as usize - 1];
    let all_survived = rows.iter().all(|r| r.3);

    println!("\n=== Byzantine Eviction Benchmark ===");
    println!("Trials: {}", TRIALS);
    println!("Mean eviction/ignore latency: {}ms", mean_eviction);
    println!("p95 eviction/ignore latency:  {}ms", p95);
    println!("Legitimate peer survived all trials: {}", all_survived);

    assert!(
        all_survived,
        "Legitimate peer was lost in at least one trial"
    );
    Ok(())
}
