use crci_core::runtime::NodeRuntime;
use crci_core::transport::{NetworkMessage, TcpTransport, Transport};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

#[tokio::test]
async fn two_node_handshake_peer_count_reaches_one() -> Result<(), Box<dyn std::error::Error>> {
    // We wrap NodeRuntime in Arc<Mutex> so we can mutate it from the receive loop
    // and still assert on it in the main test loop.
    let inbox_a = Arc::new(Mutex::new(HashMap::new()));
    let node_a = Arc::new(Mutex::new(NodeRuntime::new("node-a", "zone-a", inbox_a)));

    let inbox_b = Arc::new(Mutex::new(HashMap::new()));
    let node_b = Arc::new(Mutex::new(NodeRuntime::new("node-b", "zone-b", inbox_b)));

    // Bind real TcpTransports on loopback with port 0 (OS assigned)
    let addr_a_req: SocketAddr = "127.0.0.1:0".parse()?;
    let tcp_a = TcpTransport::bind("node-a".to_string(), addr_a_req).await?;
    let addr_a = tcp_a.listen_addr;

    let addr_b_req: SocketAddr = "127.0.0.1:0".parse()?;
    let tcp_b = TcpTransport::bind("node-b".to_string(), addr_b_req).await?;
    let _addr_b = tcp_b.listen_addr;

    // Spawn a background listener for Node A to process incoming TCP messages
    // Since NodeRuntime doesn't yet natively integrate TcpTransport, we bridge it
    // in this test to prove the end-to-end handshake flow.
    let node_a_clone = node_a.clone();
    tokio::spawn(async move {
        while let Ok((sender_id, _msg)) = tcp_a.receive().await {
            // When a message is received, add the sender as a peer
            if let Ok(mut na) = node_a_clone.lock() {
                na.add_peer(&sender_id);
            }
        }
    });

    // Node B dials Node A
    // In NodeRuntime, connecting to a peer means adding it to the peer list
    {
        let mut nb = node_b.lock().unwrap();
        nb.add_peer(&addr_a.to_string());
    }

    // To simulate the handshake over TCP, Node B sends a network message to Node A
    let handshake_msg = NetworkMessage {
        id: "handshake-1".to_string(),
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

    // Bounded wait — poll peer_count with timeout
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let count_a = node_a.lock().unwrap().peers.len();
        let count_b = node_b.lock().unwrap().peers.len();

        if count_a >= 1 && count_b >= 1 {
            break;
        }

        if tokio::time::Instant::now() > deadline {
            panic!(
                "Handshake did not complete within 5 seconds. Node A peers: {}, Node B peers: {}",
                count_a, count_b
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let count_a = node_a.lock().unwrap().peers.len();
    let count_b = node_b.lock().unwrap().peers.len();
    assert_eq!(count_a, 1, "Node A should see exactly one peer");
    assert_eq!(count_b, 1, "Node B should see exactly one peer");

    Ok(())
}
