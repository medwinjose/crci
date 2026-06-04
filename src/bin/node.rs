use crci::integration::{MessageKind, PipelineMessage};
use crci::transport::TcpGossipNode;
use std::env;
use std::io::Write;
use std::net::{TcpListener, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| {
        std::fs::read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "node-unknown".to_string())
            .trim()
            .to_string()
    });
    let node_zone = env::var("NODE_ZONE").unwrap_or_else(|_| "zone-a".to_string());
    let node_type = env::var("NODE_TYPE").unwrap_or_else(|_| "honest".to_string());
    let is_honest = node_type == "honest";

    thread::sleep(Duration::from_secs(3));

    let mut peers = Vec::new();
    for svc in &["honest-node:8080", "byzantine-node:8080"] {
        if let Ok(addrs) = svc.to_socket_addrs() {
            peers.extend(addrs.map(|a| a.to_string()));
        }
    }

    println!(
        "[{}] Started. Zone: {}. Peers: {}",
        node_id,
        node_zone,
        peers.len()
    );
    let tcp_node = TcpGossipNode::new(&node_id, &node_zone, peers);

    {
        let mut pipeline = tcp_node.pipeline.lock().unwrap();
        for i in 1..=50 {
            pipeline.register_bootstrap(&node_zone, &format!("honest-node-{}", i));
            pipeline.register_bootstrap(&node_zone, &format!("byzantine-node-{}", i));
        }
        pipeline.register_bootstrap(&node_zone, &node_id);
    }

    let peer_reps = Arc::new(Mutex::new(std::collections::HashMap::<String, f64>::new()));
    let peer_reps_metrics = peer_reps.clone();

    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:9090").unwrap();
        for mut stream in listener.incoming().flatten() {
            let reps = peer_reps_metrics.lock().unwrap();
            let mut body = String::from("# HELP crci_node_reputation Reputation of CRCI nodes\n# TYPE crci_node_reputation gauge\n");
            for (id, rep) in reps.iter() {
                body.push_str(&format!(
                    "crci_node_reputation{{node_id=\"{}\"}} {}\n",
                    id, rep
                ));
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let _listener_handle = tcp_node.start_listener(8080);
    let start_time = Instant::now();
    let mut seq = 0u64;
    let mut rescue_sent = false;

    let received_msgs = tcp_node.received.clone();
    let peer_reps_consensus = peer_reps.clone();
    let node_id_cons = node_id.clone();
    let node_zone_cons = node_zone.clone();

    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        let msgs = { received_msgs.lock().unwrap().clone() };
        let mut zone_sevs = Vec::new();
        for msg in &msgs {
            if msg.zone == node_zone_cons && msg.kind == MessageKind::Normal {
                zone_sevs.push(msg.severity);
            }
        }
        if zone_sevs.len() >= 3 {
            zone_sevs.sort();
            let median = zone_sevs[zone_sevs.len() / 2];
            let mut node_sevs = std::collections::HashMap::<String, Vec<u8>>::new();
            for msg in &msgs {
                if msg.zone == node_zone_cons && msg.kind == MessageKind::Normal {
                    node_sevs
                        .entry(msg.origin_node.clone())
                        .or_default()
                        .push(msg.severity);
                }
            }
            let mut reps = peer_reps_consensus.lock().unwrap();
            for (nid, sevs) in node_sevs {
                let avg = sevs.iter().map(|&s| s as f64).sum::<f64>() / sevs.len() as f64;
                if (avg - median as f64).abs() > 2.0 {
                    let rep = reps.entry(nid.clone()).or_insert(1.0);
                    *rep = (*rep - 0.2).max(0.0);
                    println!(
                        "[{}] Penalty {}: dev > 2 (rep: {:.2})",
                        node_id_cons, nid, *rep
                    );
                }
            }
        }
    });

    while start_time.elapsed() < Duration::from_secs(60) {
        thread::sleep(Duration::from_secs(2));
        seq += 1;

        let mut severity = 3u8;
        if !is_honest {
            let rand_val = (start_time.elapsed().as_millis() as u64 + seq) % 100;
            if rand_val < 30 {
                severity = if rand_val.is_multiple_of(2) { 1 } else { 5 };
            }
        }

        tcp_node.broadcast(PipelineMessage {
            id: format!("msg-{}-{}", node_id, seq),
            origin_node: node_id.clone(),
            zone: node_zone.clone(),
            severity,
            kind: MessageKind::Normal,
            payload_bytes: 10,
            reputation: 1.0,
            round: 0,
            seq,
        });

        if is_honest && !rescue_sent && start_time.elapsed() >= Duration::from_secs(5) {
            let mut honest_ips = Vec::new();
            if let Ok(addrs) = "honest-node:8080".to_socket_addrs() {
                honest_ips.extend(addrs.map(|a| a.ip().to_string()));
            }
            honest_ips.sort();

            let am_first = honest_ips
                .first()
                .map(|ip| {
                    if let Ok(local_addrs) = node_id.to_socket_addrs() {
                        local_addrs.into_iter().any(|la| la.ip().to_string() == *ip)
                    } else {
                        node_id.contains("-1") || node_id == "honest-node-1"
                    }
                })
                .unwrap_or(false);

            if am_first || node_id == "honest-node-1" {
                println!("[{}] Sending emergency rescue request!", node_id);
                tcp_node.broadcast(PipelineMessage {
                    id: "rescue-30".to_string(),
                    origin_node: node_id.clone(),
                    zone: node_zone.clone(),
                    severity: 5,
                    kind: MessageKind::Rescue,
                    payload_bytes: 10,
                    reputation: 1.0,
                    round: 0,
                    seq: 999,
                });
                rescue_sent = true;
            }
        }
    }

    let pipeline = tcp_node.pipeline.lock().unwrap();
    println!(
        "[{}] Final stats: active_rescues={} total_received={}",
        node_id,
        pipeline.ttl_store.rescue_count(),
        tcp_node.received.lock().unwrap().len()
    );
}
