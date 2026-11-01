use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// ─── Authenticated connections ───────────────────────────────────────────────
//
// Every connection starts with a mutual Ed25519 challenge-response, then carries frames
// signed by the dialer's key:
//
//   listener → dialer   ServerHello { node_id, public_key, nonce_l }
//   dialer   → listener ClientAuth  { node_id, public_key, nonce_d, sig_d(transcript) }
//   listener → dialer   ServerAuth  { sig_l(transcript) }
//   dialer   → listener TcpFrame    { counter, message, sig_d(session, counter, message) } …
//
// The transcript holds both node IDs, both keys and both fresh nonces, so a signature
// from one connection is useless on another. The listener attributes every frame to the
// dialer's authenticated identity, `peer_identity(node_id, key)`; a claimed node ID with
// a different key is a different identity, so a peer cannot get another node struck by
// claiming its ID. Frame signatures bind each frame to the session and to its position,
// so a party on the path can drop frames but cannot inject, alter or reorder them.
// Nothing is encrypted.

/// Domain tag for handshake signatures.
pub const HANDSHAKE_DOMAIN: &[u8] = b"CRCI-TCP-HANDSHAKE-v1\0";
/// Domain tag for frame signatures.
pub const FRAME_DOMAIN: &[u8] = b"CRCI-TCP-FRAME-v1\0";
/// Length of each side's handshake nonce, in bytes.
pub const NONCE_LEN: usize = 32;
/// Longest node ID accepted in a handshake (same limit as message origins).
pub const MAX_NODE_ID_LEN: usize = 64;

const MAX_HANDSHAKE_BYTES: u32 = 4 * 1024;
const MAX_FRAME_BYTES: u32 = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Handshake message 1, listener → dialer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerHello {
    pub node_id: NodeId,
    pub public_key: Vec<u8>,
    pub nonce: Vec<u8>,
}

/// Handshake message 2, dialer → listener: the dialer's identity and its signature over
/// the transcript.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientAuth {
    pub node_id: NodeId,
    pub public_key: Vec<u8>,
    pub nonce: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Handshake message 3, listener → dialer: the listener's signature over the transcript.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerAuth {
    pub signature: Vec<u8>,
}

/// One frame after the handshake: an origin-signed message, numbered from 0 within the
/// connection and signed by the dialer for this session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TcpFrame {
    pub counter: u64,
    pub message: NetworkMessage,
    pub signature: Vec<u8>,
}

/// Which side of the handshake a signature comes from. The role is signed, so the
/// listener's signature can never be presented as the dialer's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeRole {
    Dialer,
    Listener,
}

impl HandshakeRole {
    fn tag(self) -> &'static [u8] {
        match self {
            HandshakeRole::Dialer => b"dialer",
            HandshakeRole::Listener => b"listener",
        }
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// The identity a listener attributes a connection's frames to: the node ID the dialer
/// declared, joined to the full public key it proved it holds.
pub fn peer_identity(node_id: &str, public_key: &VerifyingKey) -> NodeId {
    format!("{}#{}", node_id, to_hex(public_key.as_bytes()))
}

/// Handshake transcript: the domain tag, then the listener's and the dialer's node ID,
/// public key and nonce, each length-prefixed.
pub fn handshake_transcript(hello: &ServerHello, dialer: &ClientAuth) -> Vec<u8> {
    let mut out = HANDSHAKE_DOMAIN.to_vec();
    for field in [
        hello.node_id.as_bytes(),
        &hello.public_key,
        &hello.nonce,
        dialer.node_id.as_bytes(),
        &dialer.public_key,
        &dialer.nonce,
    ] {
        put_bytes(&mut out, field);
    }
    out
}

fn role_payload(role: HandshakeRole, transcript: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(transcript.len() + 16);
    put_bytes(&mut out, role.tag());
    out.extend_from_slice(transcript);
    out
}

/// Sign the transcript in `role`.
pub fn sign_handshake(key: &SigningKey, role: HandshakeRole, transcript: &[u8]) -> Vec<u8> {
    key.sign(&role_payload(role, transcript))
        .to_bytes()
        .to_vec()
}

fn verify_handshake(
    key: &VerifyingKey,
    role: HandshakeRole,
    transcript: &[u8],
    signature: &[u8],
) -> bool {
    let Ok(sig) = <[u8; 64]>::try_from(signature) else {
        return false;
    };
    key.verify_strict(
        &role_payload(role, transcript),
        &Signature::from_bytes(&sig),
    )
    .is_ok()
}

/// Session identifier: SHA-256 of the transcript.
pub fn session_id(transcript: &[u8]) -> [u8; 32] {
    Sha256::digest(transcript).into()
}

/// Bytes the dialer signs for frame `counter`: the domain tag, the session, the counter
/// and the whole message (every signed field and the origin's signature).
pub fn frame_signing_bytes(session: &[u8; 32], counter: u64, message: &NetworkMessage) -> Vec<u8> {
    let mut out = FRAME_DOMAIN.to_vec();
    out.extend_from_slice(session);
    out.extend_from_slice(&counter.to_be_bytes());
    put_bytes(&mut out, &message.signing_bytes());
    put_bytes(&mut out, &message.signature_bytes);
    out
}

/// Sign frame `counter` of `session`.
pub fn sign_frame(
    key: &SigningKey,
    session: &[u8; 32],
    counter: u64,
    message: &NetworkMessage,
) -> TcpFrame {
    TcpFrame {
        counter,
        message: message.clone(),
        signature: key
            .sign(&frame_signing_bytes(session, counter, message))
            .to_bytes()
            .to_vec(),
    }
}

fn parse_key(bytes: &[u8]) -> Option<VerifyingKey> {
    let arr = <[u8; 32]>::try_from(bytes).ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

fn fresh_nonce() -> Vec<u8> {
    let mut nonce = vec![0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    nonce
}

fn valid_node_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_NODE_ID_LEN
}

fn auth_error(reason: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, reason.to_string())
}

/// Write one length-prefixed JSON value.
pub async fn write_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "value too large"))?;
    let write = async {
        stream.write_u32(len).await?;
        stream.write_all(&bytes).await
    };
    match tokio::time::timeout(IO_TIMEOUT, write).await {
        Ok(res) => res,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "write timeout",
        )),
    }
}

/// Read one length-prefixed JSON value of at most `max_len` bytes.
pub async fn read_json<T: DeserializeOwned>(
    stream: &mut TcpStream,
    max_len: u32,
) -> std::io::Result<T> {
    let read = async {
        let len = stream.read_u32().await?;
        if len > max_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("oversized payload ({} bytes)", len),
            ));
        }
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;
        serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    };
    match tokio::time::timeout(IO_TIMEOUT, read).await {
        Ok(res) => res,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "read timeout",
        )),
    }
}

/// An authenticated dialer, as seen by the listener.
struct AuthenticatedPeer {
    identity: NodeId,
    key: VerifyingKey,
    session: [u8; 32],
}

/// Listener side of the handshake. Returns the dialer's authenticated identity, or an
/// error before any frame is read.
async fn accept_handshake(
    stream: &mut TcpStream,
    local_node_id: &str,
    key: &SigningKey,
) -> std::io::Result<AuthenticatedPeer> {
    let own_key = key.verifying_key();
    let hello = ServerHello {
        node_id: local_node_id.to_string(),
        public_key: own_key.to_bytes().to_vec(),
        nonce: fresh_nonce(),
    };
    write_json(stream, &hello).await?;

    let dialer: ClientAuth = read_json(stream, MAX_HANDSHAKE_BYTES).await?;
    if !valid_node_id(&dialer.node_id) {
        return Err(auth_error("invalid dialer node ID"));
    }
    if dialer.nonce.len() != NONCE_LEN {
        return Err(auth_error("invalid dialer nonce"));
    }
    let Some(dialer_key) = parse_key(&dialer.public_key) else {
        return Err(auth_error("invalid dialer public key"));
    };
    if dialer_key == own_key {
        return Err(auth_error("connection from this node's own key"));
    }
    let transcript = handshake_transcript(&hello, &dialer);
    if !verify_handshake(
        &dialer_key,
        HandshakeRole::Dialer,
        &transcript,
        &dialer.signature,
    ) {
        return Err(auth_error("dialer handshake signature invalid"));
    }

    let reply = ServerAuth {
        signature: sign_handshake(key, HandshakeRole::Listener, &transcript),
    };
    write_json(stream, &reply).await?;

    Ok(AuthenticatedPeer {
        identity: peer_identity(&dialer.node_id, &dialer_key),
        key: dialer_key,
        session: session_id(&transcript),
    })
}

/// Dial `target`, authenticate as `node_id` with `key`, check the listener's proof of its
/// own key, and send `messages` as signed frames on that one connection.
pub async fn send_authenticated(
    target: &str,
    node_id: &str,
    key: &SigningKey,
    messages: &[NetworkMessage],
) -> Result<(), TransportError> {
    let mut stream = match tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(target)).await {
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
    let result = dialer_session(&mut stream, node_id, key, messages).await;
    let _ = stream.shutdown().await;
    result
}

async fn dialer_session(
    stream: &mut TcpStream,
    node_id: &str,
    key: &SigningKey,
    messages: &[NetworkMessage],
) -> Result<(), TransportError> {
    let own_key = key.verifying_key();
    let hello: ServerHello = read_json(stream, MAX_HANDSHAKE_BYTES)
        .await
        .map_err(map_send_error)?;
    let Some(listener_key) = parse_key(&hello.public_key) else {
        return Err(TransportError::Authentication(
            "invalid listener public key".into(),
        ));
    };
    if hello.nonce.len() != NONCE_LEN || !valid_node_id(&hello.node_id) {
        return Err(TransportError::Authentication(
            "malformed listener hello".into(),
        ));
    }
    if listener_key == own_key {
        return Err(TransportError::Authentication(
            "dialed this node's own listener".into(),
        ));
    }

    let mut auth = ClientAuth {
        node_id: node_id.to_string(),
        public_key: own_key.to_bytes().to_vec(),
        nonce: fresh_nonce(),
        signature: Vec::new(),
    };
    let transcript = handshake_transcript(&hello, &auth);
    auth.signature = sign_handshake(key, HandshakeRole::Dialer, &transcript);
    write_json(stream, &auth).await.map_err(map_send_error)?;

    let reply: ServerAuth = read_json(stream, MAX_HANDSHAKE_BYTES)
        .await
        .map_err(map_send_error)?;
    if !verify_handshake(
        &listener_key,
        HandshakeRole::Listener,
        &transcript,
        &reply.signature,
    ) {
        return Err(TransportError::Authentication(
            "listener handshake signature invalid".into(),
        ));
    }

    let session = session_id(&transcript);
    for (counter, message) in (0u64..).zip(messages) {
        let frame = sign_frame(key, &session, counter, message);
        write_json(stream, &frame).await.map_err(map_send_error)?;
    }
    Ok(())
}

pub struct TcpTransport {
    pub listen_addr: SocketAddr,
    pub rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, NetworkMessage)>>,
    pub accepted_count: Arc<AtomicU64>,
    pub rejected_count: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU64>,
    /// Connections closed because the handshake failed or a frame's signature, counter or
    /// size was wrong. No frame from such a connection reaches `receive`.
    pub auth_failures: Arc<AtomicU64>,
    pub node_id: String,
    signing_key: SigningKey,
    pub cancel_token: CancellationToken,
}

impl TcpTransport {
    /// Listen on `addr` as `node_id`, proving `signing_key` to every dialer and signing
    /// every outgoing connection with it. A node uses the same key here as for its messages.
    pub async fn bind(
        node_id: String,
        addr: SocketAddr,
        signing_key: SigningKey,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (tx, rx) = mpsc::channel(1024);
        let accepted = Arc::new(AtomicU64::new(0));
        let rejected = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicU64::new(0));
        let auth_failures = Arc::new(AtomicU64::new(0));
        let cancel_token = CancellationToken::new();

        let actual_addr = listener.local_addr().unwrap_or(addr);

        let accepted_clone = accepted.clone();
        let rejected_clone = rejected.clone();
        let active_clone = active.clone();
        let auth_failures_clone = auth_failures.clone();
        let cancel_token_clone = cancel_token.clone();
        let local_node_id = node_id.clone();
        let listener_key = signing_key.clone();

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
                        let key_for_task = listener_key.clone();
                        let active_for_task = active_clone.clone();
                        let auth_failures_for_task = auth_failures_clone.clone();
                        let cancel_for_task = cancel_token_clone.clone();

                        tokio::spawn(async move {
                            // RAII connection guard to decrement count on drop (preventing leaks)
                            let _guard = ConnectionGuard(active_for_task);

                            if let Err(e) = configure_socket(&stream) {
                                log::warn!("Failed to configure accepted socket: {}", e);
                                let _ = stream.shutdown().await;
                                return;
                            }

                            handle_connection(
                                stream,
                                tx_clone,
                                ConnectionContext {
                                    local_node_id: node_id_for_task,
                                    key: key_for_task,
                                    auth_failures: auth_failures_for_task,
                                },
                                cancel_for_task,
                            )
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
            auth_failures,
            node_id,
            signing_key,
            cancel_token,
        })
    }

    /// The public key this transport proves to its peers.
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
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

struct ConnectionContext {
    local_node_id: String,
    key: SigningKey,
    auth_failures: Arc<AtomicU64>,
}

/// Authenticate the dialer, then deliver its frames, attributed to its authenticated
/// identity, until the connection closes. Any handshake or frame failure closes the
/// connection; nothing from it is delivered after the failure.
async fn handle_connection(
    mut stream: TcpStream,
    tx: mpsc::Sender<(NodeId, NetworkMessage)>,
    ctx: ConnectionContext,
    cancel_token: CancellationToken,
) {
    let handshake = tokio::select! {
        res = accept_handshake(&mut stream, &ctx.local_node_id, &ctx.key) => res,
        _ = cancel_token.cancelled() => {
            let _ = stream.shutdown().await;
            return;
        }
    };
    let peer = match handshake {
        Ok(peer) => peer,
        Err(e) => {
            ctx.auth_failures.fetch_add(1, Ordering::SeqCst);
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                log::warn!("TCP handshake rejected: {}", e);
            } else {
                log_io_error("TCP handshake failed", &e);
            }
            let _ = stream.shutdown().await;
            return;
        }
    };
    log::debug!("authenticated TCP peer {}", peer.identity);

    let mut expected_counter: u64 = 0;
    loop {
        // Shutdown immediately on any failure (ASYNC-013).
        let frame_res = tokio::select! {
            res = read_json::<TcpFrame>(&mut stream, MAX_FRAME_BYTES) => res,
            _ = cancel_token.cancelled() => break,
        };
        let frame = match frame_res {
            Ok(frame) => frame,
            Err(e) => {
                // A clean close after the last frame is the normal end of a connection.
                if e.kind() != std::io::ErrorKind::UnexpectedEof {
                    if e.kind() == std::io::ErrorKind::InvalidData {
                        ctx.auth_failures.fetch_add(1, Ordering::SeqCst);
                    }
                    log_io_error("TCP read error", &e);
                }
                break;
            }
        };

        let signed_ok = <[u8; 64]>::try_from(frame.signature.as_slice())
            .map(|sig| {
                peer.key
                    .verify_strict(
                        &frame_signing_bytes(&peer.session, frame.counter, &frame.message),
                        &Signature::from_bytes(&sig),
                    )
                    .is_ok()
            })
            .unwrap_or(false);
        if frame.counter != expected_counter || !signed_ok {
            ctx.auth_failures.fetch_add(1, Ordering::SeqCst);
            log::warn!(
                "TCP frame from {} rejected (counter {}, expected {}, signature valid: {}); closing",
                peer.identity,
                frame.counter,
                expected_counter,
                signed_ok
            );
            break;
        }
        expected_counter += 1;

        if tx
            .send((peer.identity.clone(), frame.message))
            .await
            .is_err()
        {
            break;
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
        send_authenticated(
            &addr.to_string(),
            &self.node_id,
            &self.signing_key,
            std::slice::from_ref(message),
        )
        .await
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
