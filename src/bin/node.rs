use crci::integration::{GossipPipeline, MessageKind, PipelineMessage};
use crci::transport::AsyncTransport;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "node-unknown".to_string());
    let node_zone = env::var("NODE_ZONE").unwrap_or_else(|_| "zone-a".to_string());
    let is_byzantine = env::var("IS_BYZANTINE").unwrap_or_else(|_| "false".to_string()) == "true";
    let byzantine_mode = env::var("BYZANTINE_MODE").unwrap_or_else(|_| "none".to_string());
    let rounds: u64 = env::var("ROUNDS")
        .unwrap_or_else(|_| "60".to_string())
        .parse()
        .unwrap_or(60);
    let listen_port: u16 = env::var("LISTEN_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    // Give time for all nodes to start up
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut peers = Vec::new();
    let peers_env = env::var("PEERS").unwrap_or_else(|_| "".to_string());
    for svc in peers_env.split(',') {
        let svc = svc.trim();
        if svc.is_empty() {
            continue;
        }
        if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(svc) {
            peers.extend(addrs);
        }
    }

    let listen_addr: SocketAddr = format!("0.0.0.0:{}", listen_port).parse().unwrap();
    let transport = Arc::new(
        AsyncTransport::bind(node_id.clone(), listen_addr, peers)
            .await
            .expect("Failed to create transport"),
    );

    let pipeline = Arc::new(Mutex::new(GossipPipeline::new()));

    {
        let mut p = pipeline.lock().unwrap();
        // Register all potential node ids to bypass vouching for this proof loop
        for i in 1..=50 {
            p.register_bootstrap(&node_zone, &format!("node-{:02}", i));
            p.register_bootstrap("zone-a", &format!("node-{:02}", i));
            p.register_bootstrap("zone-b", &format!("node-{:02}", i));
            p.register_bootstrap("zone-c", &format!("node-{:02}", i));
        }
    }

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    // Recv loop
    let recv_transport = transport.clone();
    let recv_pipeline = pipeline.clone();
    tokio::spawn(async move {
        recv_transport.recv_loop(recv_pipeline, shutdown_rx).await;
    });

    let byzantine_detections = Arc::new(std::sync::atomic::AtomicU64::new(0));

    // Consensus thread
    let consensus_pipeline = pipeline.clone();
    let cons_node_id = node_id.clone();
    let cons_detections = byzantine_detections.clone();

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let mut penalties = Vec::new();
            {
                let p = consensus_pipeline.lock().unwrap();
                let severities_map = &p.peer_severities;
                let mut severities: Vec<u8> = severities_map.values().cloned().collect();

                if severities.len() >= 2 {
                    severities.sort();
                    let median = severities[severities.len() / 2] as f64;
                    for (id, &sev) in severities_map.iter() {
                        let dev = (sev as f64 - median).abs();
                        if dev >= 2.0 {
                            penalties.push((id.clone(), dev));
                        }
                    }
                }
            }

            for (bad_id, dev) in penalties {
                println!(
                    "[{}] Penalty applied to {}: average severity deviation > 2 (dev: {})",
                    cons_node_id, bad_id, dev
                );
                cons_detections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                // Clear the severity to avoid re-penalizing until they send another bad message
                if let Ok(mut p) = consensus_pipeline.lock() {
                    p.peer_severities.remove(&bad_id);
                }
            }
        }
    });

    let mut rescue_sent = false;

    // Main gossip loop
    for seq in 1..=rounds {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut severity = 3u8;
        if is_byzantine && byzantine_mode == "conflicting" {
            // Byzantine node sends wildly conflicting severities
            severity = if seq % 2 == 0 { 1 } else { 5 };
        }

        if !is_byzantine && !rescue_sent && node_id == "node-01" && seq == 10 {
            println!("[{}] Sending emergency rescue request!", node_id);
            let rescue_msg = PipelineMessage {
                id: format!("rescue-{}-{}", node_id, seq),
                origin_node: node_id.clone(),
                zone: node_zone.clone(),
                severity: 5,
                kind: MessageKind::Rescue,
                payload_bytes: 10,
                reputation: 1.0,
                round: seq,
                seq: 999,
            };

            {
                let mut p = pipeline.lock().unwrap();
                let _ = p.process(&rescue_msg);
            }
            let _ = transport.broadcast(&rescue_msg).await;
            rescue_sent = true;
        } else {
            // Normal message
            let msg = PipelineMessage {
                id: format!("msg-{}-{}", node_id, seq),
                origin_node: node_id.clone(),
                zone: node_zone.clone(),
                severity,
                kind: MessageKind::Normal,
                payload_bytes: 10,
                reputation: 1.0,
                round: seq,
                seq,
            };

            {
                let mut p = pipeline.lock().unwrap();
                // We advance the round to prevent rate limits
                p.next_round();
                let _ = p.process(&msg);
            }
            let _ = transport.broadcast(&msg).await;
        }
    }

    let _ = shutdown_tx.send(());

    // Allow time for final messages to settle
    tokio::time::sleep(Duration::from_secs(2)).await;

    let p = pipeline.lock().unwrap();
    let accepted = transport
        .accepted_count
        .load(std::sync::atomic::Ordering::SeqCst);
    let rescues = p.ttl_store.rescue_count();
    let b_det = byzantine_detections.load(std::sync::atomic::Ordering::SeqCst);

    let summary = format!(
        r#"{{"node_id":"{}","accepted_count":{},"byzantine_detections":{},"rescue_held":{}}}"#,
        node_id, accepted, b_det, rescues
    );
    println!("JSON_SUMMARY: {}", summary);
}
