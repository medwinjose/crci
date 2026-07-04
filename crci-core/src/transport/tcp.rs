use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct TcpTransport {
    pub listen_addr: SocketAddr,
    pub rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, NetworkMessage)>>,
    pub accepted_count: Arc<AtomicU64>,
    pub rejected_count: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU64>,
    pub node_id: String,
    pub cancel_token: CancellationToken,
}

impl TcpTransport {
    pub async fn bind(node_id: String, addr: SocketAddr) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (tx, rx) = mpsc::channel(1024);
        let accepted = Arc::new(AtomicU64::new(0));
        let rejected = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicU64::new(0));
        let cancel_token = CancellationToken::new();

        let actual_addr = listener.local_addr().unwrap_or(addr);

        let accepted_clone = accepted.clone();
        let rejected_clone = rejected.clone();
        let active_clone = active.clone();
        let cancel_token_clone = cancel_token.clone();
        let local_node_id = node_id.clone();

        // Spawn background listener accept task
        tokio::spawn(async move {
            let mut backoff = std::time::Duration::from_millis(10);

            loop {
                let accept_res = tokio::select! {
                    res = listener.accept() => res,
                    _ = cancel_token_clone.cancelled() => {
                        log::info!("TcpListener accept loop terminated via cancellation token.");
                        break;
                    }
                };

                match accept_res {
                    Ok((mut stream, _peer_addr)) => {
                        // Reset backoff on success
                        backoff = std::time::Duration::from_millis(10);

                        // Connection limit check (ASYNC-012)
                        let current_active = active_clone.load(std::sync::atomic::Ordering::SeqCst);
                        if current_active >= 100 {
                            log::warn!("TCP connection limit reached (100). Rejecting client.");
                            rejected_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            let _ = stream.shutdown().await;
                            continue;
                        }

                        active_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        accepted_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                        let tx_clone = tx.clone();
                        let node_id_for_task = local_node_id.clone();
                        let active_for_task = active_clone.clone();
                        let cancel_for_task = cancel_token_clone.clone();

                        tokio::spawn(async move {
                            // RAII connection guard to decrement count on drop (preventing leaks)
                            let _guard = ConnectionGuard(active_for_task);

                            if let Err(e) = configure_socket(&stream) {
                                log::warn!("Failed to configure accepted socket: {}", e);
                                let _ = stream.shutdown().await;
                                return;
                            }

                            handle_connection(stream, tx_clone, node_id_for_task, cancel_for_task)
                                .await;
                        });
                    }
                    Err(ref e) if is_transient_error(e) => {
                        log::warn!(
                            "TcpListener accept failed with transient error: {}. Backing off...",
                            e
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(backoff) => {}
                            _ = cancel_token_clone.cancelled() => {
                                break;
                            }
                        }
                        backoff = (backoff * 2).min(std::time::Duration::from_secs(1));
                    }
                    Err(e) => {
                        log::error!("TcpListener accept failed with permanent error: {}. Shutting down listener.", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            listen_addr: actual_addr,
            rx: tokio::sync::Mutex::new(rx),
            accepted_count: accepted,
            rejected_count: rejected,
            active_connections: active,
            node_id,
            cancel_token,
        })
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

struct ConnectionGuard(Arc<AtomicU64>);
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(windows)]
unsafe fn get_socket(stream: &tokio::net::TcpStream) -> std::mem::ManuallyDrop<socket2::Socket> {
    use std::os::windows::io::{AsRawSocket, FromRawSocket};
    std::mem::ManuallyDrop::new(socket2::Socket::from_raw_socket(stream.as_raw_socket()))
}

#[cfg(unix)]
unsafe fn get_socket(stream: &tokio::net::TcpStream) -> std::mem::ManuallyDrop<socket2::Socket> {
    use std::os::unix::io::{AsRawFd, FromRawFd};
    std::mem::ManuallyDrop::new(socket2::Socket::from_raw_fd(stream.as_raw_fd()))
}

fn configure_socket(stream: &tokio::net::TcpStream) -> std::io::Result<()> {
    stream.set_nodelay(true)?; // TCP_NODELAY set (ASYNC-010)

    // Set SO_KEEPALIVE and custom timeout parameters (ASYNC-008, ASYNC-009)
    let socket = unsafe { get_socket(stream) };
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(std::time::Duration::from_secs(30))
        .with_interval(std::time::Duration::from_secs(5));

    socket.set_tcp_keepalive(&keepalive)?;
    socket.set_keepalive(true)?;

    Ok(())
}

async fn handle_connection(
    mut stream: TcpStream,
    tx: mpsc::Sender<(NodeId, NetworkMessage)>,
    local_node_id: String,
    cancel_token: CancellationToken,
) {
    loop {
        // Read length prefix with timeout (ASYNC-011) and cancellation check
        let len_res = tokio::select! {
            res = tokio::time::timeout(std::time::Duration::from_secs(5), stream.read_u32()) => {
                match res {
                    Ok(Ok(len)) => Ok(len),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Handshake/Read timeout")),
                }
            }
            _ = cancel_token.cancelled() => {
                let _ = stream.shutdown().await;
                return;
            }
        };

        let len = match len_res {
            Ok(len) => len,
            Err(e) => {
                log_io_error("TCP read error", &e);
                let _ = stream.shutdown().await; // Shutdown immediately on failure (ASYNC-013)
                return;
            }
        };

        if len > 1024 * 1024 {
            log::warn!("Oversized payload rejected ({} bytes)", len);
            let _ = stream.shutdown().await;
            return;
        }

        let mut buf = vec![0u8; len as usize];

        // Read payload with timeout
        let payload_res = tokio::select! {
            res = tokio::time::timeout(std::time::Duration::from_secs(5), stream.read_exact(&mut buf)) => {
                match res {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Payload timeout")),
                }
            }
            _ = cancel_token.cancelled() => {
                let _ = stream.shutdown().await;
                return;
            }
        };

        if let Err(e) = payload_res {
            log_io_error("TCP payload read error", &e);
            let _ = stream.shutdown().await;
            return;
        }

        if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&buf) {
            if msg.origin_node == local_node_id {
                continue;
            }

            let sender_id = msg.origin_node.clone();
            if tx.send((sender_id, msg)).await.is_err() {
                break;
            }
        } else {
            log::warn!("Invalid JSON message payload received.");
            let _ = stream.shutdown().await;
            return;
        }
    }
    let _ = stream.shutdown().await;
}

fn log_io_error(context: &str, e: &std::io::Error) {
    match e.kind() {
        std::io::ErrorKind::ConnectionReset => {
            log::warn!("{}: ConnectionReset occurred.", context);
        }
        std::io::ErrorKind::NotConnected => {
            log::warn!("{}: NotConnected occurred.", context);
        }
        std::io::ErrorKind::BrokenPipe => {
            log::warn!("{}: BrokenPipe occurred.", context);
        }
        std::io::ErrorKind::TimedOut => {
            log::warn!("{}: Timed out.", context);
        }
        _ => {
            log::debug!("{}: Generic I/O error: {}", context, e);
        }
    }
}

fn map_send_error(e: std::io::Error) -> TransportError {
    match e.kind() {
        std::io::ErrorKind::ConnectionReset => {
            TransportError::ConnectionFailed("Connection reset by peer".into())
        }
        std::io::ErrorKind::NotConnected => {
            TransportError::ConnectionFailed("Socket is not connected".into())
        }
        std::io::ErrorKind::BrokenPipe => {
            // Graceful handling of BrokenPipe (ASYNC-003)
            log::warn!("BrokenPipe encountered during send: peer disconnected.");
            TransportError::ConnectionFailed("Broken pipe".into())
        }
        _ if e.raw_os_error() == Some(libc_enobufs_value()) => {
            // ENOBUFS handling (ASYNC-007)
            log::warn!("ENOBUFS encountered: OS socket buffer full.");
            TransportError::ConnectionFailed("No buffer space available".into())
        }
        _ => TransportError::ConnectionFailed(e.to_string()),
    }
}

fn is_transient_error(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::Interrupted
    ) || e.raw_os_error() == Some(libc_emfile_value())
        || e.raw_os_error() == Some(libc_enfile_value())
}

#[cfg(windows)]
fn libc_emfile_value() -> i32 {
    10024
}
#[cfg(unix)]
fn libc_emfile_value() -> i32 {
    24
}

#[cfg(windows)]
fn libc_enfile_value() -> i32 {
    10024
}
#[cfg(unix)]
fn libc_enfile_value() -> i32 {
    23
}

#[cfg(windows)]
fn libc_enobufs_value() -> i32 {
    10055
}
#[cfg(unix)]
fn libc_enobufs_value() -> i32 {
    105
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    async fn send(&self, peer: &NodeId, message: &NetworkMessage) -> Result<(), TransportError> {
        let addr: SocketAddr = peer.parse().map_err(|_| {
            TransportError::ConnectionFailed(format!("Invalid SocketAddr: {}", peer))
        })?;

        // 5-second timeout on connecting (ASYNC-011)
        let stream =
            match tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(addr))
                .await
            {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => return Err(map_send_error(e)),
                Err(_) => return Err(TransportError::ConnectionFailed("Connect timed out".into())),
            };

        if let Err(e) = configure_socket(&stream) {
            return Err(TransportError::ConnectionFailed(format!(
                "Socket configuration failed: {}",
                e
            )));
        }

        let payload = serde_json::to_vec(message)
            .map_err(|e| TransportError::Serialisation(e.to_string()))?;

        let mut stream = stream;

        // Write prefix length with timeout (ASYNC-011)
        let write_len_res = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.write_u32(payload.len() as u32),
        )
        .await;

        match write_len_res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                let _ = stream.shutdown().await;
                return Err(map_send_error(e));
            }
            Err(_) => {
                let _ = stream.shutdown().await;
                return Err(TransportError::ConnectionFailed(
                    "Write prefix timeout".into(),
                ));
            }
        }

        // Write payload with timeout
        let write_payload_res = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.write_all(&payload),
        )
        .await;

        match write_payload_res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                let _ = stream.shutdown().await;
                return Err(map_send_error(e));
            }
            Err(_) => {
                let _ = stream.shutdown().await;
                return Err(TransportError::ConnectionFailed(
                    "Write payload timeout".into(),
                ));
            }
        }

        let _ = stream.shutdown().await;
        Ok(())
    }

    async fn receive(&self) -> Result<(NodeId, NetworkMessage), TransportError> {
        let mut rx = self.rx.lock().await;
        if let Some(msg) = rx.recv().await {
            Ok(msg)
        } else {
            Err(TransportError::ConnectionFailed("Channel closed".into()))
        }
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Tcp
    }

    fn is_available(&self) -> bool {
        true
    }
}
