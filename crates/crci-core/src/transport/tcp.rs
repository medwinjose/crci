use super::{NetworkMessage, NodeId, Transport, TransportError, TransportType};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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
// from one connection is useless on another. A node ID is derived from its key
// (`identity::derive_node_id`), and each side refuses a peer whose declared ID is not
// derived from the key it proved. The listener attributes every frame to the dialer's
// authenticated node ID, so a peer cannot get another node struck by claiming its ID. Frame signatures bind each frame to the session and to its position,
// so a party on the path can drop frames but cannot inject, alter or reorder them.
// Nothing is encrypted.

/// Domain tag for handshake signatures.
pub const HANDSHAKE_DOMAIN: &[u8] = b"CRCI-TCP-HANDSHAKE-v1\0";
/// Domain tag for frame signatures.
pub const FRAME_DOMAIN: &[u8] = b"CRCI-TCP-FRAME-v2\0";
/// Length of each side's handshake nonce, in bytes.
pub const NONCE_LEN: usize = 32;

const MAX_HANDSHAKE_BYTES: u32 = 4 * 1024;
const MAX_FRAME_BYTES: u32 = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the listener waits for the next frame on an authenticated connection. A
/// frame that has started must still arrive within `IO_TIMEOUT`.
const FRAME_IDLE_TIMEOUT: Duration = Duration::from_secs(600);
/// Frames queued per peer for `TcpTransport::relay`; further frames are dropped and logged.
const RELAY_QUEUE_LEN: usize = 1024;
/// How long the listener keeps refusing a banned identity before it lets one probe frame
/// through, so the node can notice a readmission (a ban lasts at least 300 s).
pub const SHUN_PROBE_INTERVAL: Duration = Duration::from_secs(30);
/// Frames per second the listener reads from one connection, sustained. Honest links carry about
/// one frame per message (a few per second, tens at the measured network capacity) plus bounded
/// sync answers, so this is far above honest use; it bounds what a sender that is never struck
/// (a flood of fresh messages above its budget) can make the receiver parse and verify.
pub const FRAME_PACING_RATE: f64 = 200.0;
/// Frames a connection may send at once before pacing applies.
pub const FRAME_PACING_BURST: u32 = 400;

/// Token bucket for the frames read from one connection. When it is empty the reader waits,
/// which fills the kernel's buffers and slows the sender through TCP flow control.
struct FramePacer {
    tokens: f64,
    last: std::time::Instant,
    rate: f64,
    burst: f64,
}

impl FramePacer {
    fn new(rate: f64, burst: u32) -> Self {
        Self {
            tokens: f64::from(burst),
            last: std::time::Instant::now(),
            rate,
            burst: f64::from(burst),
        }
    }

    /// Take a token for the next frame; the time to wait first, if none is available.
    fn take(&mut self) -> Option<Duration> {
        let now = std::time::Instant::now();
        self.tokens = (self.tokens + (now - self.last).as_secs_f64() * self.rate).min(self.burst);
        self.last = now;
        self.tokens -= 1.0;
        (self.tokens < 0.0).then(|| Duration::from_secs_f64(-self.tokens / self.rate))
    }
}

/// Addresses with a relay queue, connection and task. Above any degree measured (22); past it the
/// address idle longest loses them, and its queued frames. A configured peer in use is never
/// the idle one.
pub const MAX_RELAY_TARGETS: usize = 256;

/// Entries above which expired shun entries are purged on insert.
const SHUN_PURGE_ABOVE: usize = 1024;

/// Authenticated identities whose connections the listener closes without reading them.
/// A node adds an identity when it bans it; an entry expires after `SHUN_PROBE_INTERVAL`,
/// and the node renews it whenever the probe frame shows the peer is still banned.
#[derive(Default)]
struct ShunList {
    until: std::sync::RwLock<HashMap<String, std::time::Instant>>,
}

impl ShunList {
    fn shun(&self, identity: &str, interval: Duration) {
        let now = std::time::Instant::now();
        let mut map = self.until.write().unwrap_or_else(|e| e.into_inner());
        if map.len() >= SHUN_PURGE_ABOVE {
            map.retain(|_, t| *t > now);
        }
        map.insert(identity.to_string(), now + interval);
    }

    fn is_shunned(&self, identity: &str) -> bool {
        let map = self.until.read().unwrap_or_else(|e| e.into_inner());
        map.get(identity)
            .is_some_and(|t| std::time::Instant::now() < *t)
    }
}

// ─── Listener admission per host ─────────────────────────────────────────────
//
// Keys are free, so a limit per identity limits nothing: one host can sign with as many
// identities as it likes. A TCP connection's source address cannot be forged without the
// three-way handshake, so the listener charges its limits to the source host (an IPv4
// address, or an IPv6 /64) instead: connections, handshakes per second and frames per second
// are shared by every connection and identity of one host. An honest node is one host with
// one relay link and a short sync connection now and then, so it never reaches them.
//
// When every slot is taken, a connection from a host that holds fewer connections than the
// host holding the most is admitted by closing the least recently active connection of that
// host; otherwise it is refused. A host with no connection is therefore never refused, and an
// adversary needs more hosts than slots it wants to keep before it can displace a host with
// one or two connections.

/// Connections the listener keeps open at once.
pub const MAX_CONNECTIONS: usize = 100;
/// Connections one host may hold at once (handshakes in progress included); a further one closes
/// that host's least recently active connection, so a host cannot lock itself out with dead ones.
pub const MAX_CONNECTIONS_PER_HOST: usize = 8;
/// Connections one authenticated identity may hold at once; a further one closes its least
/// recently active connection (a reconnect replaces a half-dead link).
pub const MAX_CONNECTIONS_PER_IDENTITY: usize = 4;
/// Connections one host may open at once before `HOST_ACCEPT_RATE` applies.
pub const HOST_ACCEPT_BURST: f64 = 16.0;
/// Connections per second one host may open, sustained; each costs the listener a handshake (an
/// Ed25519 signature and a verification). An honest node opens one per reconnect and per sync
/// request: at most one per few seconds per neighbour.
pub const HOST_ACCEPT_RATE: f64 = 2.0;
/// Hosts whose counters are kept; past this, hosts without a connection are forgotten.
const MAX_TRACKED_HOSTS: usize = 4096;

/// Limits of a listener. `Default` is what a node runs with.
#[derive(Clone, Copy, Debug)]
pub struct ListenerLimits {
    /// Frames per second and burst read from one host (`FRAME_PACING_RATE`, `FRAME_PACING_BURST`).
    pub pacing_rate: f64,
    pub pacing_burst: u32,
    pub max_connections: usize,
    pub per_host: usize,
    pub per_identity: usize,
    pub accept_rate: f64,
    pub accept_burst: f64,
    /// False: every connection is its own host and a full listener refuses (the behaviour before
    /// per-host admission, kept for experiments that measure what it changes).
    pub per_host_limits: bool,
}

impl Default for ListenerLimits {
    fn default() -> Self {
        Self {
            pacing_rate: FRAME_PACING_RATE,
            pacing_burst: FRAME_PACING_BURST,
            max_connections: MAX_CONNECTIONS,
            per_host: MAX_CONNECTIONS_PER_HOST,
            per_identity: MAX_CONNECTIONS_PER_IDENTITY,
            accept_rate: HOST_ACCEPT_RATE,
            accept_burst: HOST_ACCEPT_BURST,
            per_host_limits: true,
        }
    }
}

/// The host a source address is charged to: an IPv4 address, or the /64 of an IPv6 address
/// (one site usually holds a whole /64).
pub fn host_of(addr: &SocketAddr) -> String {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => v4.to_string(),
        std::net::IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

struct HostState {
    accept_tokens: f64,
    accept_last: std::time::Instant,
    pacer: Arc<std::sync::Mutex<FramePacer>>,
}

impl HostState {
    fn pacer_full(&self, now: std::time::Instant) -> bool {
        let p = self.pacer.lock().unwrap_or_else(|e| e.into_inner());
        p.tokens + now.saturating_duration_since(p.last).as_secs_f64() * p.rate >= p.burst
    }
}

struct ConnEntry {
    host: String,
    identity: Option<String>,
    /// Admission order, to break ties towards the newest connection.
    opened: u64,
    /// Milliseconds since the registry started at the last frame (or the admission).
    last_active: Arc<AtomicU64>,
    cancel: CancellationToken,
}

/// What a connection gets on admission.
struct Admitted {
    id: u64,
    cancel: CancellationToken,
    pacer: Arc<std::sync::Mutex<FramePacer>>,
    last_active: Arc<AtomicU64>,
    started: std::time::Instant,
}

struct ConnRegistry {
    limits: ListenerLimits,
    started: std::time::Instant,
    next: u64,
    conns: HashMap<u64, ConnEntry>,
    hosts: HashMap<String, HostState>,
    /// Host of each identity's latest authenticated connection.
    identity_host: HashMap<String, String>,
}

impl ConnRegistry {
    fn new(limits: ListenerLimits) -> Self {
        Self {
            limits,
            started: std::time::Instant::now(),
            next: 0,
            conns: HashMap::new(),
            hosts: HashMap::new(),
            identity_host: HashMap::new(),
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn count(&self, host: &str) -> usize {
        self.conns.values().filter(|c| c.host == host).count()
    }

    /// Close and forget connection `id`.
    fn evict(&mut self, id: u64) {
        if let Some(entry) = self.conns.remove(&id) {
            entry.cancel.cancel();
        }
    }

    /// The least recently active connection among those `filter` selects.
    fn idlest(&self, filter: impl Fn(&ConnEntry) -> bool) -> Option<u64> {
        self.conns
            .iter()
            .filter(|(_, c)| filter(c))
            .min_by_key(|(_, c)| (c.last_active.load(Ordering::Relaxed), c.opened))
            .map(|(id, _)| *id)
    }

    /// Admit a connection from `addr`, closing another one if the rules above say so; `None`
    /// refuses it. Returns the number of connections closed to make room as well.
    fn admit(&mut self, addr: &SocketAddr, parent: &CancellationToken) -> (Option<Admitted>, u64) {
        let id = self.next;
        self.next += 1;
        let limits = self.limits;
        let host = if limits.per_host_limits {
            host_of(addr)
        } else {
            format!("connection-{id}")
        };
        let mut evicted = 0;
        if limits.per_host_limits {
            let now = std::time::Instant::now();
            if self.hosts.len() >= MAX_TRACKED_HOSTS {
                // Only hosts without a connection whose budgets are full again: a forgotten host
                // comes back with full budgets, so forgetting gains it nothing.
                let live: std::collections::HashSet<String> =
                    self.conns.values().map(|c| c.host.clone()).collect();
                self.hosts.retain(|h, state| {
                    let accept_full = state.accept_tokens
                        + (now - state.accept_last).as_secs_f64() * limits.accept_rate
                        >= limits.accept_burst;
                    live.contains(h) || !accept_full || !state.pacer_full(now)
                });
            }
            let state = self.hosts.entry(host.clone()).or_insert_with(|| HostState {
                accept_tokens: limits.accept_burst,
                accept_last: now,
                pacer: Arc::new(std::sync::Mutex::new(FramePacer::new(
                    limits.pacing_rate,
                    limits.pacing_burst,
                ))),
            });
            state.accept_tokens = (state.accept_tokens
                + (now - state.accept_last).as_secs_f64() * limits.accept_rate)
                .min(limits.accept_burst);
            state.accept_last = now;
            if state.accept_tokens < 1.0 {
                return (None, 0);
            }
            state.accept_tokens -= 1.0;
            if self.count(&host) >= limits.per_host {
                if let Some(old) = self.idlest(|c| c.host == host) {
                    self.evict(old);
                    evicted += 1;
                }
            }
        }
        if self.conns.len() >= limits.max_connections {
            if !limits.per_host_limits {
                return (None, evicted);
            }
            let own = self.count(&host);
            let mut sizes: HashMap<&str, (usize, u64)> = HashMap::new();
            for c in self.conns.values() {
                let e = sizes.entry(c.host.as_str()).or_insert((0, 0));
                e.0 += 1;
                e.1 = e.1.max(c.opened);
            }
            // The host holding the most connections; among equals, the one that connected last.
            let largest = sizes
                .iter()
                .max_by_key(|(_, (n, newest))| (*n, *newest))
                .map(|(h, (n, _))| (h.to_string(), *n));
            match largest {
                Some((big, n)) if n > own => {
                    if let Some(old) = self.idlest(|c| c.host == big) {
                        self.evict(old);
                        evicted += 1;
                    }
                }
                _ => return (None, evicted),
            }
        }
        let pacer = match self.hosts.get(&host) {
            Some(state) => state.pacer.clone(),
            None => Arc::new(std::sync::Mutex::new(FramePacer::new(
                limits.pacing_rate,
                limits.pacing_burst,
            ))),
        };
        let cancel = parent.child_token();
        let last_active = Arc::new(AtomicU64::new(self.elapsed_ms()));
        self.conns.insert(
            id,
            ConnEntry {
                host,
                identity: None,
                opened: id,
                last_active: last_active.clone(),
                cancel: cancel.clone(),
            },
        );
        (
            Some(Admitted {
                id,
                cancel,
                pacer,
                last_active,
                started: self.started,
            }),
            evicted,
        )
    }

    /// Record that connection `id` authenticated as `identity`; returns the number of that
    /// identity's older connections closed to stay within `per_identity`.
    fn authenticated(&mut self, id: u64, identity: &str) -> u64 {
        let Some(host) = self.conns.get(&id).map(|c| c.host.clone()) else {
            return 0;
        };
        if let Some(entry) = self.conns.get_mut(&id) {
            entry.identity = Some(identity.to_string());
        }
        if !self.limits.per_host_limits {
            return 0;
        }
        if self.identity_host.len() >= MAX_TRACKED_HOSTS {
            let live: std::collections::HashSet<String> = self
                .conns
                .values()
                .filter_map(|c| c.identity.clone())
                .collect();
            self.identity_host.retain(|i, _| live.contains(i));
        }
        self.identity_host.insert(identity.to_string(), host);
        let mut evicted = 0;
        while self
            .conns
            .values()
            .filter(|c| c.identity.as_deref() == Some(identity))
            .count()
            > self.limits.per_identity
        {
            match self.idlest(|c| c.identity.as_deref() == Some(identity) && c.opened != id) {
                Some(old) => {
                    self.evict(old);
                    evicted += 1;
                }
                None => break,
            }
        }
        evicted
    }
}

/// Removes a connection from the registry when its task ends.
struct RegistryGuard {
    registry: Arc<std::sync::Mutex<ConnRegistry>>,
    id: u64,
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        let mut reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.conns.remove(&self.id);
    }
}

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

/// The identity a connection's frames are attributed to: the declared node ID, if it is the
/// one derived from the public key the peer proved it holds.
pub fn peer_identity(node_id: &str, public_key: &VerifyingKey) -> Option<NodeId> {
    (node_id == crate::identity::derive_node_id(public_key.as_bytes())).then(|| node_id.to_string())
}

/// The node ID of `key`.
fn node_id_of(key: &SigningKey) -> NodeId {
    crate::identity::derive_node_id(key.verifying_key().as_bytes())
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
/// and the whole message (every signed field, the origin's signature and the per-hop
/// sync-answer tag, which only the sending neighbour can vouch for).
pub fn frame_signing_bytes(session: &[u8; 32], counter: u64, message: &NetworkMessage) -> Vec<u8> {
    let mut out = FRAME_DOMAIN.to_vec();
    out.extend_from_slice(session);
    out.extend_from_slice(&counter.to_be_bytes());
    put_bytes(&mut out, &message.signing_bytes());
    put_bytes(&mut out, &message.signature_bytes);
    match &message.answers_sync {
        None => out.push(0),
        Some(request) => {
            out.push(1);
            put_bytes(&mut out, request.as_bytes());
        }
    }
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
    read_json_after_idle(stream, max_len, IO_TIMEOUT).await
}

/// Like `read_json`, but waits up to `idle` for the first byte of the value; once it has
/// started, the rest must arrive within `IO_TIMEOUT`.
async fn read_json_after_idle<T: DeserializeOwned>(
    stream: &mut TcpStream,
    max_len: u32,
    idle: Duration,
) -> std::io::Result<T> {
    let mut first = [0u8; 1];
    match tokio::time::timeout(idle, stream.read_exact(&mut first)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "read timeout",
            ))
        }
    }
    let read = async {
        let mut rest = [0u8; 3];
        stream.read_exact(&mut rest).await?;
        let len = u32::from_be_bytes([first[0], rest[0], rest[1], rest[2]]);
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
    if dialer.nonce.len() != NONCE_LEN {
        return Err(auth_error("invalid dialer nonce"));
    }
    let Some(dialer_key) = parse_key(&dialer.public_key) else {
        return Err(auth_error("invalid dialer public key"));
    };
    let Some(identity) = peer_identity(&dialer.node_id, &dialer_key) else {
        return Err(auth_error("dialer node ID is not derived from its key"));
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
        identity,
        key: dialer_key,
        session: session_id(&transcript),
    })
}

/// An authenticated outgoing connection: the handshake is done and frames can be sent.
pub struct DialedConnection {
    stream: TcpStream,
    key: SigningKey,
    session: [u8; 32],
    next_counter: u64,
    /// The listener's authenticated node ID, as its frames to this node will be attributed.
    pub peer: NodeId,
}

impl DialedConnection {
    /// Dial `target` and authenticate with `key` (as the node ID derived from it); the
    /// listener must prove its own key before this returns.
    pub async fn connect(
        target: &str,
        key: &SigningKey,
    ) -> Result<DialedConnection, TransportError> {
        Self::connect_from(target, key, None).await
    }

    /// `connect` from the local address `source` (any port) instead of the one the system
    /// picks; tests and experiments use it to appear as several hosts.
    pub async fn connect_from(
        target: &str,
        key: &SigningKey,
        source: Option<std::net::IpAddr>,
    ) -> Result<DialedConnection, TransportError> {
        let connecting = async {
            match source {
                None => TcpStream::connect(target).await,
                Some(ip) => {
                    let to: SocketAddr = target.parse().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "target is not an address",
                        )
                    })?;
                    let socket = if to.is_ipv4() {
                        tokio::net::TcpSocket::new_v4()?
                    } else {
                        tokio::net::TcpSocket::new_v6()?
                    };
                    socket.bind(SocketAddr::new(ip, 0))?;
                    socket.connect(to).await
                }
            }
        };
        let mut stream = match tokio::time::timeout(IO_TIMEOUT, connecting).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(map_send_error(e)),
            Err(_) => return Err(TransportError::ConnectionFailed("Connect timed out".into())),
        };
        if let Err(e) = configure_socket(&stream).and_then(|()| limit_unacknowledged(&stream)) {
            return Err(TransportError::ConnectionFailed(format!(
                "Socket configuration failed: {}",
                e
            )));
        }
        match dialer_handshake(&mut stream, key).await {
            Ok((peer, session)) => Ok(DialedConnection {
                stream,
                key: key.clone(),
                session,
                next_counter: 0,
                peer,
            }),
            Err(e) => {
                let _ = stream.shutdown().await;
                Err(e)
            }
        }
    }

    /// Send `message` as the next signed frame.
    pub async fn send(&mut self, message: &NetworkMessage) -> Result<(), TransportError> {
        let frame = sign_frame(&self.key, &self.session, self.next_counter, message);
        write_json(&mut self.stream, &frame)
            .await
            .map_err(map_send_error)?;
        self.next_counter += 1;
        Ok(())
    }

    /// True when the listener has closed the connection (or sent anything at all: it never
    /// does after the handshake). Checked before a frame is written, because a write to a
    /// connection the peer already closed can succeed locally and lose the frame.
    pub fn is_closed(&self) -> bool {
        let mut probe = [0u8; 1];
        match self.stream.try_read(&mut probe) {
            Ok(_) => true,
            Err(e) => e.kind() != std::io::ErrorKind::WouldBlock,
        }
    }

    /// Close the connection.
    pub async fn close(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

/// Dial `target`, authenticate with `key`, check the listener's proof of its own key, and send
/// `messages` as signed frames on that one connection. Returns the listener's authenticated
/// identity.
pub async fn send_authenticated(
    target: &str,
    key: &SigningKey,
    messages: &[NetworkMessage],
) -> Result<NodeId, TransportError> {
    let mut conn = DialedConnection::connect(target, key).await?;
    let mut result = Ok(());
    for message in messages {
        result = conn.send(message).await;
        if result.is_err() {
            break;
        }
    }
    let peer = conn.peer.clone();
    conn.close().await;
    result.map(|()| peer)
}

/// Dialer side of the handshake; returns the listener's identity and the session ID.
async fn dialer_handshake(
    stream: &mut TcpStream,
    key: &SigningKey,
) -> Result<(NodeId, [u8; 32]), TransportError> {
    let own_key = key.verifying_key();
    let hello: ServerHello = read_json(stream, MAX_HANDSHAKE_BYTES)
        .await
        .map_err(map_send_error)?;
    let Some(listener_key) = parse_key(&hello.public_key) else {
        return Err(TransportError::Authentication(
            "invalid listener public key".into(),
        ));
    };
    if hello.nonce.len() != NONCE_LEN {
        return Err(TransportError::Authentication(
            "malformed listener hello".into(),
        ));
    }
    let Some(listener_id) = peer_identity(&hello.node_id, &listener_key) else {
        return Err(TransportError::Authentication(
            "listener node ID is not derived from its key".into(),
        ));
    };
    if listener_key == own_key {
        return Err(TransportError::Authentication(
            "dialed this node's own listener".into(),
        ));
    }

    let mut auth = ClientAuth {
        node_id: node_id_of(key),
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
    Ok((listener_id, session_id(&transcript)))
}

/// Message type of the frame a dialer sends first on every connection that replaces an
/// earlier one to the same peer: frames in the earlier connection may have been lost, so
/// the listener asks the dialer for a sync. Consumed by the transport, never delivered.
pub const LINK_RESUMED_TYPE: &str = "link_resumed";

/// A connection whose last write is older than this has had it acknowledged or aborted, so
/// finding it closed later loses nothing. Well above `IO_TIMEOUT`: TCP aborts only when a
/// retransmission timer fires, and the relay task looks once a second. At 6 s a late abort
/// went unnoticed and its frames waited for the next anti-entropy round (measured: 34 s).
const ABORT_WINDOW: Duration = Duration::from_secs(15);

fn link_resumed(node_id: &str) -> NetworkMessage {
    NetworkMessage {
        id: format!("link-{node_id}"),
        origin: node_id.to_string(),
        origin_pubkey: Vec::new(),
        severity: 0,
        confidence: 0,
        needs_help: false,
        can_help_others: false,
        location_confirmed: false,
        visibility: String::new(),
        note: None,
        message_type: LINK_RESUMED_TYPE.to_string(),
        origin_active: true,
        signature_bytes: Vec::new(),
        seq: 0,
        sync: None,
        answers_sync: None,
    }
}

/// The persistent connection to one peer and what may have been lost on it.
#[derive(Default)]
struct Link {
    conn: Option<DialedConnection>,
    /// A connection to this peer existed before, so a new one starts with `link_resumed`.
    dialed: bool,
    last_write: Option<std::time::Instant>,
    /// A frame failed or the connection died after a recent write: reconnect without
    /// waiting for the next frame, so the peer learns of the loss.
    lost: bool,
}

impl Link {
    async fn connect(&mut self, outbound: &Outbound, addr: &str) -> Result<(), TransportError> {
        outbound.handshakes.fetch_add(1, Ordering::SeqCst);
        let mut conn = DialedConnection::connect(addr, &outbound.key).await?;
        if self.dialed {
            conn.send(&link_resumed(&outbound.node_id)).await?;
        }
        self.dialed = true;
        self.lost = false;
        self.conn = Some(conn);
        Ok(())
    }
}

/// Messages remembered per listener as sent to it: as many as one relay queue can lag behind another.
const SENT_REMEMBERED: usize = RELAY_QUEUE_LEN;

/// The last messages sent to one listener, with the address each was sent to (both as hashes).
struct SentTo {
    recent: std::collections::VecDeque<(u64, u64)>,
    last_used: std::time::Instant,
}

/// One authenticated outgoing connection per peer address, reused for every frame and
/// re-established when it has closed. Sends to different peers take different locks, so a
/// slow or unreachable peer delays only its own frames.
struct Outbound {
    node_id: String,
    key: SigningKey,
    links: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Link>>>>,
    /// Relay queue and last use per address. A sync request names where its answer goes, so a
    /// peer chooses addresses: see `MAX_RELAY_TARGETS`.
    queues: std::sync::Mutex<HashMap<String, (mpsc::Sender<NetworkMessage>, std::time::Instant)>>,
    /// Gossip sent to each authenticated listener, whatever address it was reached at.
    sent: std::sync::Mutex<HashMap<NodeId, SentTo>>,
    tag_key: std::collections::hash_map::RandomState,
    handshakes: Arc<AtomicU64>,
    failures: Arc<AtomicU64>,
}

impl Outbound {
    fn slot(&self, addr: &str) -> Arc<tokio::sync::Mutex<Link>> {
        let mut links = self.links.lock().unwrap_or_else(|e| e.into_inner());
        links.entry(addr.to_string()).or_default().clone()
    }

    /// Whether `message` may go to `peer`, the node that answers at `addr`. Two addresses can lead
    /// to one node: it is configured twice, or a malicious peer forwards the connections it accepts
    /// to someone else (it can read and delay them, not alter them). That node would take the
    /// second copy of a message for a replay by this one, so gossip leaves to a node at one address
    /// only, whichever is first. A sync answer leaves only to the node that asked: the address
    /// comes from the request, and any other node would take the answer for replays.
    fn may_send(&self, peer: &NodeId, addr: &str, message: &NetworkMessage) -> bool {
        use std::hash::BuildHasher;
        if let Some(request) = &message.answers_sync {
            return crate::runtime::is_request_of(request, peer);
        }
        let now = std::time::Instant::now();
        let msg_tag = self.tag_key.hash_one(crate::runtime::message_key(message));
        let addr_tag = self.tag_key.hash_one(addr);
        let mut sent = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        if sent.len() >= MAX_RELAY_TARGETS && !sent.contains_key(peer) {
            let idle = sent
                .iter()
                .min_by_key(|(_, s)| s.last_used)
                .map(|(id, _)| id.clone());
            if let Some(id) = idle {
                sent.remove(&id);
            }
        }
        let to_peer = sent.entry(peer.clone()).or_insert_with(|| SentTo {
            recent: std::collections::VecDeque::new(),
            last_used: now,
        });
        to_peer.last_used = now;
        if let Some((_, at)) = to_peer.recent.iter().rev().find(|(m, _)| *m == msg_tag) {
            return *at == addr_tag;
        }
        if to_peer.recent.len() >= SENT_REMEMBERED {
            to_peer.recent.pop_front();
        }
        to_peer.recent.push_back((msg_tag, addr_tag));
        true
    }

    async fn send(&self, addr: &str, message: &NetworkMessage) -> Result<(), TransportError> {
        let slot = self.slot(addr);
        let mut link = slot.lock().await;
        // A write that fails on an established connection is retried once on a new one.
        let mut last_err = TransportError::ConnectionFailed("no attempt made".into());
        for _ in 0..2 {
            if link.conn.as_ref().is_some_and(DialedConnection::is_closed) {
                link.conn = None;
            }
            if link.conn.is_none() {
                if let Err(e) = link.connect(self, addr).await {
                    link.lost = true;
                    return Err(e);
                }
            }
            let Some(conn) = link.conn.as_mut() else {
                continue;
            };
            if !self.may_send(&conn.peer, addr, message) {
                log::debug!("'{}' not sent to {} at {}", message.id, conn.peer, addr);
                return Ok(());
            }
            match conn.send(message).await {
                Ok(()) => {
                    link.last_write = Some(std::time::Instant::now());
                    return Ok(());
                }
                Err(e) => {
                    link.conn = None;
                    last_err = e;
                }
            }
        }
        link.lost = true;
        Err(last_err)
    }

    /// Between frames: notice a connection that died soon after a write, and reconnect
    /// to a peer whose frames may have been lost, so it can recover them by a sync.
    async fn maintain(&self, addr: &str) {
        let slot = self.slot(addr);
        let mut link = slot.lock().await;
        let recent = link.last_write.is_some_and(|t| t.elapsed() < ABORT_WINDOW);
        if recent && link.conn.as_ref().is_some_and(DialedConnection::is_closed) {
            link.conn = None;
            link.lost = true;
        }
        if link.lost && link.conn.is_none() {
            // A failure leaves `lost` set; the next tick tries again.
            let _ = link.connect(self, addr).await;
        }
    }
}

pub struct TcpTransport {
    pub listen_addr: SocketAddr,
    pub rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, NetworkMessage)>>,
    /// Authenticated identity of each dialer that re-established its relay connection
    /// (`link_resumed`). Frames in the dialer's previous connection may be lost, so the node
    /// asks that dialer for a sync.
    pub new_links: tokio::sync::Mutex<mpsc::Receiver<NodeId>>,
    pub accepted_count: Arc<AtomicU64>,
    pub rejected_count: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU64>,
    /// Connections closed because the handshake failed or a frame's signature, counter or
    /// size was wrong. No frame from such a connection reaches `receive`.
    pub auth_failures: Arc<AtomicU64>,
    /// Outgoing handshakes started by this transport (one per new connection).
    pub dialed_handshakes: Arc<AtomicU64>,
    /// Relayed frames dropped because the peer could not be reached or the write failed.
    pub relay_failures: Arc<AtomicU64>,
    /// Relayed frames dropped because the peer's queue was full.
    pub relay_queue_full: Arc<AtomicU64>,
    /// Frames whose connection had to wait (frame pacing) before the next frame was read.
    pub paced_frames: Arc<AtomicU64>,
    /// Connections the listener closed to admit another one (`ListenerLimits`).
    pub evicted_connections: Arc<AtomicU64>,
    registry: Arc<std::sync::Mutex<ConnRegistry>>,
    pub node_id: String,
    signing_key: SigningKey,
    outbound: Arc<Outbound>,
    shun: Arc<ShunList>,
    pub cancel_token: CancellationToken,
}

impl TcpTransport {
    /// Listen on `addr` as the node ID derived from `signing_key`, proving the key to every
    /// dialer and signing every outgoing connection with it. A node uses the same key here as
    /// for its messages.
    pub async fn bind(addr: SocketAddr, signing_key: SigningKey) -> std::io::Result<Self> {
        Self::bind_with_pacing(addr, signing_key, FRAME_PACING_RATE, FRAME_PACING_BURST).await
    }

    /// `bind` with an explicit frame rate (per second) and burst for each host's frames.
    pub async fn bind_with_pacing(
        addr: SocketAddr,
        signing_key: SigningKey,
        pacing_rate: f64,
        pacing_burst: u32,
    ) -> std::io::Result<Self> {
        let limits = ListenerLimits {
            pacing_rate,
            pacing_burst,
            ..ListenerLimits::default()
        };
        Self::bind_with_limits(addr, signing_key, limits).await
    }

    /// `bind` with explicit listener limits.
    pub async fn bind_with_limits(
        addr: SocketAddr,
        signing_key: SigningKey,
        limits: ListenerLimits,
    ) -> std::io::Result<Self> {
        let node_id = node_id_of(&signing_key);
        let registry = Arc::new(std::sync::Mutex::new(ConnRegistry::new(limits)));
        let evicted_connections = Arc::new(AtomicU64::new(0));
        let listener = TcpListener::bind(addr).await?;
        let (tx, rx) = mpsc::channel(1024);
        // shortcut: a full channel drops the report, the periodic sync still covers the link.
        let (links_tx, links_rx) = mpsc::channel(256);
        let accepted = Arc::new(AtomicU64::new(0));
        let rejected = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicU64::new(0));
        let auth_failures = Arc::new(AtomicU64::new(0));
        let cancel_token = CancellationToken::new();
        let shun = Arc::new(ShunList::default());
        let paced_frames = Arc::new(AtomicU64::new(0));

        let actual_addr = listener.local_addr().unwrap_or(addr);

        let accepted_clone = accepted.clone();
        let rejected_clone = rejected.clone();
        let active_clone = active.clone();
        let auth_failures_clone = auth_failures.clone();
        let cancel_token_clone = cancel_token.clone();
        let local_node_id = node_id.clone();
        let listener_key = signing_key.clone();
        let listener_shun = shun.clone();
        let listener_paced = paced_frames.clone();
        let listener_registry = registry.clone();
        let listener_evicted = evicted_connections.clone();

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
                    Ok((mut stream, peer_addr)) => {
                        // Reset backoff on success
                        backoff = std::time::Duration::from_millis(10);

                        // Admission per host (ListenerLimits).
                        let (admitted, evicted) = {
                            let mut reg =
                                listener_registry.lock().unwrap_or_else(|e| e.into_inner());
                            reg.admit(&peer_addr, &cancel_token_clone)
                        };
                        listener_evicted.fetch_add(evicted, Ordering::Relaxed);
                        let Some(admitted) = admitted else {
                            log::warn!(
                                "TCP connection from {} refused (listener limits)",
                                peer_addr
                            );
                            rejected_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            let _ = stream.shutdown().await;
                            continue;
                        };

                        active_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        accepted_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                        let tx_clone = tx.clone();
                        let node_id_for_task = local_node_id.clone();
                        let key_for_task = listener_key.clone();
                        let active_for_task = active_clone.clone();
                        let auth_failures_for_task = auth_failures_clone.clone();
                        let cancel_for_task = admitted.cancel.clone();
                        let links_for_task = links_tx.clone();
                        let registry_for_task = listener_registry.clone();
                        let evicted_for_task = listener_evicted.clone();
                        let shun_for_task = listener_shun.clone();
                        let paced_for_task = listener_paced.clone();

                        tokio::spawn(async move {
                            // RAII connection guard to decrement count on drop (preventing leaks)
                            let _guard = ConnectionGuard(active_for_task);
                            let _entry = RegistryGuard {
                                registry: registry_for_task.clone(),
                                id: admitted.id,
                            };

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
                                    new_links: links_for_task,
                                    shun: shun_for_task,
                                    pacer: admitted.pacer,
                                    paced: paced_for_task,
                                    registry: registry_for_task,
                                    conn_id: admitted.id,
                                    last_active: admitted.last_active,
                                    started: admitted.started,
                                    evicted: evicted_for_task,
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

        let dialed_handshakes = Arc::new(AtomicU64::new(0));
        let relay_failures = Arc::new(AtomicU64::new(0));
        let outbound = Arc::new(Outbound {
            node_id: node_id.clone(),
            key: signing_key.clone(),
            links: std::sync::Mutex::new(HashMap::new()),
            queues: std::sync::Mutex::new(HashMap::new()),
            sent: std::sync::Mutex::new(HashMap::new()),
            tag_key: std::collections::hash_map::RandomState::new(),
            handshakes: dialed_handshakes.clone(),
            failures: relay_failures.clone(),
        });

        Ok(Self {
            listen_addr: actual_addr,
            rx: tokio::sync::Mutex::new(rx),
            new_links: tokio::sync::Mutex::new(links_rx),
            accepted_count: accepted,
            rejected_count: rejected,
            active_connections: active,
            auth_failures,
            dialed_handshakes,
            relay_failures,
            relay_queue_full: Arc::new(AtomicU64::new(0)),
            paced_frames,
            evicted_connections,
            registry,
            node_id,
            signing_key,
            outbound,
            shun,
            cancel_token,
        })
    }

    /// Stop reading the connections of `identity` (a banned peer): they are closed and its
    /// new connections refused for `SHUN_PROBE_INTERVAL`, after which one frame is let
    /// through for the node to test whether the peer was readmitted. The node calls this
    /// again when the probe shows it is still banned.
    /// The host (`host_of`) of `identity`'s latest authenticated connection, if it has one and
    /// the listener charges limits per host. A node charges that identity's message budget to it.
    pub fn peer_host(&self, identity: &str) -> Option<String> {
        let reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.identity_host.get(identity).cloned()
    }

    pub fn shun_peer(&self, identity: &str) {
        self.shun_peer_for(identity, SHUN_PROBE_INTERVAL);
    }

    /// `shun_peer` with an explicit interval.
    pub fn shun_peer_for(&self, identity: &str, interval: Duration) {
        self.shun.shun(identity, interval);
    }

    /// The public key this transport proves to its peers.
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Queue `message` for `addr` without waiting. A task per peer sends the queue in
    /// order over that peer's persistent connection, so a slow or unreachable peer delays
    /// only its own frames. A frame that cannot be delivered is logged and dropped;
    /// anti-entropy recovers it. Must be called inside a Tokio runtime.
    /// Addresses that currently have a relay queue.
    pub fn relay_targets(&self) -> usize {
        self.outbound
            .queues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    pub fn relay(&self, addr: &str, message: NetworkMessage) -> Result<(), TransportError> {
        let sender = {
            let mut queues = self
                .outbound
                .queues
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let now = std::time::Instant::now();
            match queues.get_mut(addr) {
                Some((tx, last_used)) => {
                    *last_used = now;
                    tx.clone()
                }
                None => {
                    if queues.len() >= MAX_RELAY_TARGETS {
                        let idle = queues
                            .iter()
                            .min_by_key(|(_, (_, last_used))| *last_used)
                            .map(|(target, _)| target.clone());
                        if let Some(target) = idle {
                            // Dropping the sender ends that queue's task.
                            queues.remove(&target);
                            self.outbound
                                .links
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&target);
                        }
                    }
                    let (tx, mut rx) = mpsc::channel::<NetworkMessage>(RELAY_QUEUE_LEN);
                    let outbound = self.outbound.clone();
                    let cancel = self.cancel_token.clone();
                    let target = addr.to_string();
                    tokio::spawn(async move {
                        let mut tick = tokio::time::interval(Duration::from_secs(1));
                        loop {
                            let message = tokio::select! {
                                m = rx.recv() => match m {
                                    Some(m) => m,
                                    None => break,
                                },
                                _ = tick.tick() => {
                                    outbound.maintain(&target).await;
                                    continue;
                                }
                                _ = cancel.cancelled() => break,
                            };
                            if let Err(e) = outbound.send(&target, &message).await {
                                outbound.failures.fetch_add(1, Ordering::Relaxed);
                                log::warn!("relay of '{}' to {} failed: {}", message.id, target, e);
                            }
                        }
                    });
                    queues.insert(addr.to_string(), (tx.clone(), now));
                    tx
                }
            }
        };
        sender.try_send(message).map_err(|_| {
            self.relay_queue_full.fetch_add(1, Ordering::Relaxed);
            TransportError::ConnectionFailed(format!("relay queue for {} is full or closed", addr))
        })
    }

    /// Open an authenticated connection to `addr` as this node.
    pub async fn dial(&self, addr: &str) -> Result<DialedConnection, TransportError> {
        DialedConnection::connect(addr, &self.signing_key).await
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

/// Abort a dialed connection whose sent data stays unacknowledged for `IO_TIMEOUT`, so the
/// next frame reconnects instead of waiting out TCP's exponential retransmission backoff
/// (tens of seconds at 20 % packet loss), which would hold up gossip and sync answers alike.
/// Frames left in the aborted connection are not re-sent; the relay task reconnects with
/// `link_resumed` and the peer asks for a sync, which recovers them.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn limit_unacknowledged(stream: &tokio::net::TcpStream) -> std::io::Result<()> {
    let socket = unsafe { get_socket(stream) };
    socket.set_tcp_user_timeout(Some(IO_TIMEOUT))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn limit_unacknowledged(_stream: &tokio::net::TcpStream) -> std::io::Result<()> {
    Ok(())
}

struct ConnectionContext {
    local_node_id: String,
    key: SigningKey,
    auth_failures: Arc<AtomicU64>,
    new_links: mpsc::Sender<NodeId>,
    shun: Arc<ShunList>,
    /// Frame budget of this connection's host, shared with its other connections.
    pacer: Arc<std::sync::Mutex<FramePacer>>,
    paced: Arc<AtomicU64>,
    registry: Arc<std::sync::Mutex<ConnRegistry>>,
    conn_id: u64,
    last_active: Arc<AtomicU64>,
    /// The registry's start, the zero of `last_active`.
    started: std::time::Instant,
    evicted: Arc<AtomicU64>,
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
    // A banned identity gets no further service; the handshake is the only cost it can cause.
    if ctx.shun.is_shunned(&peer.identity) {
        let _ = stream.shutdown().await;
        return;
    }
    let evicted = {
        let mut reg = ctx.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.authenticated(ctx.conn_id, &peer.identity)
    };
    ctx.evicted.fetch_add(evicted, Ordering::Relaxed);

    let mut expected_counter: u64 = 0;
    let mut link_reported = false;
    loop {
        if ctx.shun.is_shunned(&peer.identity) {
            break;
        }
        let wait = ctx.pacer.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(wait) = wait {
            ctx.paced.fetch_add(1, Ordering::Relaxed);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = cancel_token.cancelled() => break,
            }
        }
        // Shutdown immediately on any failure (ASYNC-013).
        let frame_res = tokio::select! {
            res = read_json_after_idle::<TcpFrame>(&mut stream, MAX_FRAME_BYTES, FRAME_IDLE_TIMEOUT) => res,
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

        // Banned while this frame was in flight: skip the signature check and close.
        if ctx.shun.is_shunned(&peer.identity) {
            break;
        }

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
        ctx.last_active
            .store(ctx.started.elapsed().as_millis() as u64, Ordering::Relaxed);

        // At most one report per connection, so a peer gets syncs only by reconnecting.
        if frame.message.message_type == LINK_RESUMED_TYPE {
            if !link_reported {
                link_reported = true;
                let _ = ctx.new_links.try_send(peer.identity.clone());
            }
            continue;
        }

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
        self.outbound.send(&addr.to_string(), message).await
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

#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dialed_connection_gives_up_on_unacknowledged_data() {
        let listener = TcpTransport::bind(
            "127.0.0.1:0".parse().unwrap(),
            SigningKey::from_bytes(&[7; 32]),
        )
        .await
        .unwrap();
        let conn = DialedConnection::connect(
            &listener.listen_addr.to_string(),
            &SigningKey::from_bytes(&[9; 32]),
        )
        .await
        .unwrap();
        let socket = unsafe { get_socket(&conn.stream) };
        assert_eq!(socket.tcp_user_timeout().unwrap(), Some(IO_TIMEOUT));
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    fn addr(host: &str, port: u16) -> SocketAddr {
        format!("{host}:{port}").parse().unwrap()
    }

    fn v4(i: usize) -> String {
        format!("10.0.{}.{}", i / 250, i % 250 + 1)
    }

    /// A registry with no handshake-rate limit, so tests can open connections back to back.
    fn registry() -> ConnRegistry {
        ConnRegistry::new(ListenerLimits {
            accept_burst: 1e9,
            ..ListenerLimits::default()
        })
    }

    /// Admit a connection from `host`; true if it was admitted.
    fn open(reg: &mut ConnRegistry, root: &CancellationToken, host: &str) -> bool {
        reg.admit(&addr(host, 4000), root).0.is_some()
    }

    #[test]
    fn hosts_are_addresses_and_ipv6_slash_64s() {
        assert_eq!(host_of(&addr("10.1.2.3", 1)), host_of(&addr("10.1.2.3", 2)));
        assert_ne!(host_of(&addr("10.1.2.3", 1)), host_of(&addr("10.1.2.4", 1)));
        assert_eq!(
            host_of(&addr("[2001:db8:1:2:aaaa::1]", 1)),
            host_of(&addr("[2001:db8:1:2:bbbb::9]", 1))
        );
        assert_ne!(
            host_of(&addr("[2001:db8:1:2::1]", 1)),
            host_of(&addr("[2001:db8:1:3::1]", 1))
        );
        assert_eq!(host_of(&addr("[::ffff:10.1.2.3]", 1)), "10.1.2.3");
    }

    #[test]
    fn one_host_keeps_at_most_its_share_and_only_its_newest_connections() {
        let root = CancellationToken::new();
        let mut reg = registry();
        let mut tokens = Vec::new();
        for _ in 0..40 {
            let (admitted, _) = reg.admit(&addr("10.0.0.1", 5), &root);
            tokens.push(
                admitted
                    .expect("a host is never refused for its own connections")
                    .cancel,
            );
        }
        assert_eq!(reg.count("10.0.0.1"), MAX_CONNECTIONS_PER_HOST);
        assert_eq!(
            tokens.iter().filter(|t| t.is_cancelled()).count(),
            40 - MAX_CONNECTIONS_PER_HOST
        );
        assert!(
            tokens[40 - MAX_CONNECTIONS_PER_HOST..]
                .iter()
                .all(|t| !t.is_cancelled()),
            "the connections kept are the newest"
        );
    }

    #[test]
    fn a_host_without_a_connection_is_admitted_when_the_listener_is_full() {
        let root = CancellationToken::new();
        let mut reg = registry();
        // Twelve adversary hosts hold eight connections each, a thirteenth holds four.
        for i in 0..12 {
            for _ in 0..8 {
                assert!(open(&mut reg, &root, &v4(i)));
            }
        }
        for _ in 0..4 {
            assert!(open(&mut reg, &root, &v4(12)));
        }
        assert_eq!(reg.conns.len(), MAX_CONNECTIONS);
        assert!(
            open(&mut reg, &root, "192.168.0.1"),
            "a new host must get in"
        );
        assert_eq!(reg.conns.len(), MAX_CONNECTIONS);
        assert_eq!(
            reg.count(&v4(12)),
            4,
            "the smaller host keeps its connections"
        );
        assert_eq!(
            (0..12).map(|i| reg.count(&v4(i))).sum::<usize>(),
            95,
            "one connection of a largest host was closed"
        );
    }

    #[test]
    fn a_host_holding_as_many_as_every_other_host_is_refused_when_full() {
        let root = CancellationToken::new();
        let mut reg = registry();
        for i in 0..MAX_CONNECTIONS {
            assert!(open(&mut reg, &root, &v4(i)));
        }
        assert!(
            !open(&mut reg, &root, &v4(0)),
            "no host holds more than this one"
        );
        assert!(
            open(&mut reg, &root, "192.168.0.1"),
            "a host with none still gets in"
        );
    }

    /// Hosts with one or two connections each (honest neighbours) are never displaced by an
    /// adversary that controls fewer hosts than it would need to make every host as small as
    /// theirs, however often it reconnects.
    #[test]
    fn honest_hosts_survive_an_adversary_with_too_few_hosts() {
        let root = CancellationToken::new();
        let mut reg = registry();
        let mut honest = Vec::new();
        for i in 0..20 {
            for _ in 0..2 {
                let (admitted, _) = reg.admit(&addr(&format!("172.16.0.{}", i + 1), 9), &root);
                honest.push(admitted.unwrap().cancel);
            }
        }
        // 60 slots left. To close an honest connection the adversary must hold every slot
        // it has in hosts of at most two connections: at least 30 hosts. It has 29.
        for round in 0..20_000 {
            let host = v4(round % 29);
            let _ = open(&mut reg, &root, &host);
        }
        assert_eq!(reg.conns.len(), MAX_CONNECTIONS);
        assert!(
            honest.iter().all(|t| !t.is_cancelled()),
            "an honest connection was closed"
        );
    }

    #[test]
    fn a_host_opens_connections_at_a_bounded_rate() {
        let root = CancellationToken::new();
        let mut reg = ConnRegistry::new(ListenerLimits {
            accept_rate: 1e-9,
            ..ListenerLimits::default()
        });
        let admitted = (0..100)
            .filter(|_| open(&mut reg, &root, "10.9.9.9"))
            .count();
        assert_eq!(admitted, HOST_ACCEPT_BURST as usize);
        assert!(
            open(&mut reg, &root, "10.9.9.10"),
            "another host is not affected"
        );
    }

    #[test]
    fn a_host_gains_nothing_when_the_listener_forgets_it() {
        let root = CancellationToken::new();
        // No refill to speak of, so the result does not depend on how long the loop takes.
        let mut reg = ConnRegistry::new(ListenerLimits {
            accept_rate: 1e-9,
            ..ListenerLimits::default()
        });
        let spent = (0..100)
            .filter(|_| open(&mut reg, &root, "10.9.9.9"))
            .count();
        assert_eq!(spent, HOST_ACCEPT_BURST as usize);
        reg.conns.clear();
        // Enough other hosts, each gone again, to make the listener forget hosts.
        for i in 0..(2 * MAX_TRACKED_HOSTS) {
            let host = format!(
                "10.{}.{}.{}",
                100 + i / 62_500,
                (i / 250) % 250,
                i % 250 + 1
            );
            assert!(open(&mut reg, &root, &host));
            reg.conns.clear();
        }
        assert!(reg.hosts.len() <= 2 * MAX_TRACKED_HOSTS + 1);
        assert!(
            !open(&mut reg, &root, "10.9.9.9"),
            "a forgotten host must not get a fresh handshake budget"
        );
    }

    #[test]
    fn one_identity_keeps_at_most_its_share_of_connections() {
        let root = CancellationToken::new();
        let mut reg = registry();
        let mut tokens = Vec::new();
        for i in 0..7 {
            let (admitted, _) = reg.admit(&addr(&v4(i), 1), &root);
            let admitted = admitted.unwrap();
            reg.authenticated(admitted.id, "same-identity");
            tokens.push(admitted.cancel);
        }
        let live = tokens.iter().filter(|t| !t.is_cancelled()).count();
        assert_eq!(live, MAX_CONNECTIONS_PER_IDENTITY);
        assert!(!tokens[6].is_cancelled(), "the newest connection is kept");
    }

    #[test]
    fn without_per_host_limits_a_full_listener_refuses() {
        let root = CancellationToken::new();
        let mut reg = ConnRegistry::new(ListenerLimits {
            per_host_limits: false,
            ..ListenerLimits::default()
        });
        for _ in 0..MAX_CONNECTIONS {
            assert!(open(&mut reg, &root, "10.0.0.1"));
        }
        assert!(!open(&mut reg, &root, "192.168.0.1"));
    }
}
