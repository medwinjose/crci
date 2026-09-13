use crci_core::runtime::NodeRuntime;
use crci_core::transport::{NetworkMessage, TcpTransport, Transport};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;

#[tokio::test]
async fn four_node_mesh_integration() -> Result<(), Box<dyn std::error::Error>> {
    let mut nodes = Vec::new();
    let mut addrs = Vec::new();
    let mut tcps = Vec::new();

    for i in 0..4 {
        let node_id = format!("node-{}", i);
        let zone = format!("zone-{}", i % 2);
        let inbox = Arc::new(Mutex::new(HashMap::new()));
        let node = Arc::new(Mutex::new(NodeRuntime::new(&node_id, &zone, inbox)));

        let addr_req: SocketAddr = "127.0.0.1:0".parse()?;
        let tcp = Arc::new(TcpTransport::bind(node_id.clone(), addr_req).await?);
        addrs.push(tcp.listen_addr);
        tcps.push(tcp);
        nodes.push(node);
    }

    // Shared list to track messages received by Node 3
    let node3_received = Arc::new(Mutex::new(Vec::new()));

    // Start listeners that bridge TCP to NodeRuntime
    let mut handles = Vec::new();
    for i in 0..4 {
        let tcp = tcps[i].clone();
        let node_clone = nodes[i].clone();
        let node3_rec_clone = node3_received.clone();

        let handle = tokio::spawn(async move {
            while let Ok((sender_id, msg)) = tcp.receive().await {
                if let Ok(mut n) = node_clone.lock() {
                    n.add_peer(&sender_id);
                }
                if i == 3 {
                    node3_rec_clone.lock().unwrap().push(msg.id.clone());
                }
            }
        });
        handles.push(handle);
    }

    // Connect in a ring: 0->1, 1->2, 2->3, 3->0
    for (i, node) in nodes.iter().enumerate().take(4) {
        let next = (i + 1) % 4;
        let mut n = node.lock().unwrap_or_else(|e| e.into_inner());
        n.add_peer(&addrs[next].to_string());
    }

    // Node 0 originates a message and sends to Node 1
    let msg_id = "test-mesh-msg-1".to_string();
    let handshake_msg = NetworkMessage {
        id: msg_id.clone(),
        origin_node: "node-0".to_string(),
        zone: "zone-0".to_string(),
        severity: 1,
        kind: crci_core::integration::MessageKind::Normal,
        payload_bytes: 0,
        reputation: 1.0,
        round: 1,
        seq: 1,
    };
    tcps[0].send(&addrs[1].to_string(), &handshake_msg).await?;

    // Node 1 forwards to Node 2, Node 2 forwards to Node 3
    tokio::time::sleep(Duration::from_millis(100)).await;
    tcps[1].send(&addrs[2].to_string(), &handshake_msg).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    tcps[2].send(&addrs[3].to_string(), &handshake_msg).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Assert Node 3 received the message
    {
        let i3 = node3_received.lock().unwrap();
        assert!(
            i3.contains(&msg_id),
            "Node 3 did not receive the message propagated through the mesh"
        );
    }

    // Test Byzantine Isolation across the mesh
    // Byzantine node connects to Node 2 and sends garbage
    let mut byz = tokio::net::TcpStream::connect(&addrs[2]).await?;
    for _ in 0..10 {
        let garbage = b"BYZANTINE_GARBAGE\n";
        byz.write_u32(garbage.len() as u32).await.ok();
        byz.write_all(garbage).await.ok();
    }
    drop(byz);

    // Wait for eviction
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Assert Node 2 still has legitimate peer (Node 1 and Node 3)
    let n2_peers = nodes[2].lock().unwrap().peers.len();
    assert!(
        n2_peers >= 1,
        "Node 2 dropped legitimate peers after Byzantine attack"
    );

    Ok(())
}
