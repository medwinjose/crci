use crci_core::integration::{MessageKind, PipelineMessage};
use crci_core::transport::{
    BleConfig, BleTransport, LoraConfig, LoraTransport, TcpTransport, Transport, TransportError,
    TransportMultiplexer,
};
use std::net::SocketAddr;
use tokio::time::Duration;

fn dummy_message(origin: &str) -> PipelineMessage {
    PipelineMessage {
        id: format!("msg-{}", origin),
        origin_node: origin.to_string(),
        zone: "zone-a".to_string(),
        severity: 3,
        kind: MessageKind::Normal,
        payload_bytes: 10,
        reputation: 1.0,
        round: 1,
        seq: 1,
    }
}

#[tokio::test]
async fn tcp_transport_send_receive_roundtrip() {
    let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let tcp1 = TcpTransport::bind("node-1".to_string(), addr1)
        .await
        .unwrap();
    let tcp2 = TcpTransport::bind("node-2".to_string(), addr2)
        .await
        .unwrap();

    let dest_node_id = tcp2.listen_addr.to_string();
    let msg = dummy_message("node-1");

    // Give listeners time to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    tcp1.send(&dest_node_id, &msg).await.unwrap();

    let (sender, received) = tokio::time::timeout(Duration::from_secs(2), tcp2.receive())
        .await
        .expect("Timeout waiting for message")
        .expect("Receive failed");

    assert_eq!(sender, "node-1");
    assert_eq!(received.id, "msg-node-1");
}

#[tokio::test]
async fn ble_transport_returns_not_available() {
    let ble = BleTransport::new(BleConfig {
        device_name: "test".to_string(),
        service_uuid: "000".to_string(),
        mtu: 512,
    });

    assert!(!ble.is_available());

    let msg = dummy_message("node-1");
    let res = ble.send(&"peer".to_string(), &msg).await;
    assert!(matches!(res, Err(TransportError::NotAvailable)));
}

#[tokio::test]
async fn lora_transport_returns_not_available() {
    let lora = LoraTransport::new(LoraConfig {
        frequency_hz: 915_000_000,
        spreading_factor: 7,
        bandwidth_khz: 125,
        coding_rate: 5,
    });

    assert!(!lora.is_available());

    let msg = dummy_message("node-1");
    let res = lora.send(&"peer".to_string(), &msg).await;
    assert!(matches!(res, Err(TransportError::NotAvailable)));
}

#[tokio::test]
async fn multiplexer_falls_through_to_tcp() {
    let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let tcp1 = TcpTransport::bind("node-1".to_string(), addr1)
        .await
        .unwrap();
    let tcp2 = TcpTransport::bind("node-2".to_string(), addr2)
        .await
        .unwrap();

    let lora = LoraTransport::new(LoraConfig {
        frequency_hz: 915_000_000,
        spreading_factor: 7,
        bandwidth_khz: 125,
        coding_rate: 5,
    });

    let multiplexer = TransportMultiplexer::new(vec![Box::new(lora), Box::new(tcp1)]);
    let dest_node_id = tcp2.listen_addr.to_string();
    let msg = dummy_message("node-1");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send through multiplexer (Lora is unavailable, should fall through to TCP)
    multiplexer.send(&dest_node_id, &msg).await.unwrap();

    // Receive on TCP2
    let (sender, received) = tokio::time::timeout(Duration::from_secs(2), tcp2.receive())
        .await
        .expect("Timeout waiting for message")
        .expect("Receive failed");

    assert_eq!(sender, "node-1");
    assert_eq!(received.id, "msg-node-1");
}

#[tokio::test]
async fn multiplexer_all_fail_returns_error() {
    let lora = LoraTransport::new(LoraConfig {
        frequency_hz: 915_000_000,
        spreading_factor: 7,
        bandwidth_khz: 125,
        coding_rate: 5,
    });

    let ble = BleTransport::new(BleConfig {
        device_name: "test".to_string(),
        service_uuid: "000".to_string(),
        mtu: 512,
    });

    let multiplexer = TransportMultiplexer::new(vec![Box::new(lora), Box::new(ble)]);
    let msg = dummy_message("node-1");

    let res = multiplexer.send(&"peer".to_string(), &msg).await;
    assert!(matches!(res, Err(TransportError::NotAvailable)));
}

#[tokio::test]
async fn test_tcp_transport_cancellation() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tcp = TcpTransport::bind("node-1".to_string(), addr)
        .await
        .unwrap();
    let bound_addr = tcp.listen_addr;

    // Drop the transport
    drop(tcp);

    // Give a brief moment for cleanup tasks to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Attempt to bind to the same exact port again. If port was freed, it should succeed!
    let tcp2 = TcpTransport::bind("node-2".to_string(), bound_addr).await;
    assert!(
        tcp2.is_ok(),
        "Failed to re-bind to freed port: {:?}",
        tcp2.err()
    );
}

#[tokio::test]
async fn test_tcp_transport_connection_limit() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tcp = TcpTransport::bind("node-alpha".to_string(), addr)
        .await
        .unwrap();
    let target_addr = tcp.listen_addr;

    // Dial the listener 101 times
    let mut streams = Vec::new();
    for _ in 0..101 {
        if let Ok(stream) = tokio::net::TcpStream::connect(target_addr).await {
            streams.push(stream);
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    // Give the listener time to accept
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Assert that rejected count is >= 1, and accepted count is >= 100
    let rejected = tcp.rejected_count.load(std::sync::atomic::Ordering::SeqCst);
    let accepted = tcp.accepted_count.load(std::sync::atomic::Ordering::SeqCst);

    assert!(
        accepted >= 100,
        "Accepted connections should be 100, got {}",
        accepted
    );
    assert!(
        rejected >= 1,
        "Rejected connections should be >= 1, got {}",
        rejected
    );
}

#[tokio::test]
async fn test_tcp_transport_handshake_timeout() {
    use tokio::io::AsyncReadExt;
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tcp = TcpTransport::bind("node-alpha".to_string(), addr)
        .await
        .unwrap();
    let target_addr = tcp.listen_addr;

    // Connect to the transport but send absolutely nothing
    let mut stream = tokio::net::TcpStream::connect(target_addr).await.unwrap();

    // Give a small delay to make sure acceptor increments the active count
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify connection remains open initially
    assert_eq!(
        tcp.active_connections
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    // Wait for the 5-second timeout plus some buffer
    tokio::time::sleep(Duration::from_millis(5500)).await;

    // The reader task should have timed out and closed the connection, decrementing active connections
    assert_eq!(
        tcp.active_connections
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );

    // Assert that the stream was closed (read returns 0/EOF)
    let mut buf = [0u8; 10];
    let read_bytes = stream.read(&mut buf).await.unwrap_or(0);
    assert_eq!(read_bytes, 0, "Stream should be closed by remote timeout");
}
