use crci_core::runtime::NodeRuntime;
use crci_core::transport::{NetworkMessage, TcpTransport, Transport};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;

#[tokio::test]
async fn byzantine_peer_does_not_corrupt_honest_connection(
) -> Result<(), Box<dyn std::error::Error>> {
    // Node A — honest listener
    let inbox_a = Arc::new(Mutex::new(HashMap::new()));
    let node_a = Arc::new(Mutex::new(NodeRuntime::new("node-a", "zone-a", inbox_a)));

    let addr_a_req: SocketAddr = "127.0.0.1:19003".parse()?;
    let tcp_a = TcpTransport::bind("node-a".to_string(), addr_a_req).await?;
    let addr_a = tcp_a.listen_addr;

    let node_a_clone = node_a.clone();
    tokio::spawn(async move {
        while let Ok((sender_id, _msg)) = tcp_a.receive().await {
            if let Ok(mut na) = node_a_clone.lock() {
                na.add_peer(&sender_id);
            }
        }
    });

    // Node B — legitimate peer
    let inbox_b = Arc::new(Mutex::new(HashMap::new()));
    let node_b = Arc::new(Mutex::new(NodeRuntime::new("node-b", "zone-b", inbox_b)));

    let addr_b_req: SocketAddr = "127.0.0.1:19004".parse()?;
    let tcp_b = TcpTransport::bind("node-b".to_string(), addr_b_req).await?;
    let _addr_b = tcp_b.listen_addr;

    // Node B dials Node A
    {
        let mut nb = node_b.lock().unwrap();
        nb.add_peer(&addr_a.to_string());
    }

    let handshake_msg = NetworkMessage {
        id: "handshake-b".to_string(),
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
        if count_a >= 1 {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("Legitimate handshake did not complete");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Node C — Byzantine adversary via raw TcpStream
    let mut byzantine = tokio::net::TcpStream::connect(addr_a)
        .await
        .expect("Byzantine peer failed to connect");

    for _ in 0..10 {
        // We write the length prefix followed by garbage so the TcpTransport's read loop triggers `from_slice`
        let garbage = b"BYZANTINE_GARBAGE\n";
        byzantine
            .write_u32(garbage.len() as u32)
            .await
            .expect("Failed to write length");
        byzantine
            .write_all(garbage)
            .await
            .expect("Failed to write garbage");
    }
    drop(byzantine); // close connection

    // Assert: Node B remains connected throughout and node_a never added Node C
    tokio::time::sleep(Duration::from_millis(500)).await;

    let count_a = node_a.lock().unwrap().peers.len();
    assert!(
        count_a >= 1,
        "Node A lost its legitimate peer after Byzantine attack"
    );
    assert_eq!(
        count_a, 1,
        "Node A should only have 1 legitimate peer, not the Byzantine one"
    );

    Ok(())
}
