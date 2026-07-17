//! Peer discovery protocol for CRCI.
//!
//! Three layered mechanisms:
//!   1. Beacon — periodic signed broadcast announcing existence
//!   2. Peer Exchange — gossip-layer peer list sharing
//!   3. Zone Bootstrap — first-node coordinator election
//!
//! No real I/O. All state is in-memory for simulation.
//! In production, Beacon would go over BLE/WiFi Direct/LoRa.

use std::collections::{HashMap, VecDeque};

// ── Constants ─────────────────────────────────────────────────────

/// How often a node broadcasts a beacon (in gossip rounds).
const BEACON_INTERVAL_ROUNDS: u64 = 10;

/// Max peers shared in one peer-exchange message (LoRa constraint).
const MAX_PEERS_PER_EXCHANGE: usize = 8;

/// Rounds a node must be the sole beacon sender before becoming
/// zone coordinator.
const ZONE_BOOTSTRAP_ROUNDS: u64 = 30;

/// Max peers a node tracks in its peer table.
const MAX_PEER_TABLE_SIZE: usize = 200;

// ── Beacon ────────────────────────────────────────────────────────

/// A beacon is a signed broadcast announcement.
/// In production, this is ~80 bytes and fits in one BLE/LoRa frame.
#[derive(Debug, Clone)]
pub struct Beacon {
    /// Sender's node ID (16-char pubkey hash from security.rs).
    pub node_id: String,
    /// Sender's claimed zone.
    pub zone: String,
    /// Round when this beacon was sent.
    pub round: u64,
    /// Truncated signature (first 8 bytes, for simulation).
    pub sig_prefix: String,
    /// Battery tier: "FULL", "LOW", or "CRITICAL".
    pub battery_tier: String,
}

impl Beacon {
    pub fn new(node_id: &str, zone: &str, round: u64, battery_tier: &str) -> Self {
        // Deterministic sig simulation: hash of node_id + round
        let sig = format!("{:08x}", {
            let mut h: u64 = 0xcbf29ce484222325;
            for b in node_id.bytes().chain(round.to_le_bytes()) {
                h ^= b as u64;
                h = h.wrapping_mul(0x00000100000001b3);
            }
            h
        });
        Beacon {
            node_id: node_id.to_string(),
            zone: zone.to_string(),
            round,
            sig_prefix: sig[..8].to_string(),
            battery_tier: battery_tier.to_string(),
        }
    }
}

// ── Peer record ───────────────────────────────────────────────────

/// A discovered peer entry in the local peer table.
#[derive(Debug, Clone)]
pub struct PeerRecord {
    pub node_id: String,
    pub zone: String,
    /// Round this peer was last heard from.
    pub last_seen: u64,
    /// How the peer was discovered.
    pub discovery_method: DiscoveryMethod,
    /// Estimated battery tier.
    pub battery_tier: String,
    /// Whether this peer has been vouched into the zone registry.
    pub zone_vouched: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryMethod {
    /// Heard directly from the peer's beacon.
    DirectBeacon,
    /// Learned about via peer exchange from another node.
    PeerExchange { via: String },
    /// Pre-configured bootstrap node.
    Bootstrap,
}

// ── Peer table ────────────────────────────────────────────────────

/// Per-node peer table. Tracks all known peers and their state.
#[derive(Default)]
pub struct PeerTable {
    peers: HashMap<String, PeerRecord>,
    /// Ordered insertion for eviction (oldest first when full).
    insertion_order: VecDeque<String>,
    pub total_discovered: u64,
    pub total_evicted: u64,
}

impl PeerTable {
    pub fn new() -> Self {
        PeerTable::default()
    }

    /// Add or update a peer. Returns true if this is a new discovery.
    pub fn upsert(&mut self, record: PeerRecord) -> bool {
        let is_new = !self.peers.contains_key(&record.node_id);
        if is_new {
            // BFT-003: limit to max 3 peers per IP subnet
            if let Some(subnet) = Self::get_subnet(&record.node_id) {
                let subnet_count = self
                    .peers
                    .values()
                    .filter(|p| Self::get_subnet(&p.node_id) == Some(subnet.clone()))
                    .count();
                if subnet_count >= 3 {
                    log::warn!(
                        "Sybil Clustering Guard (BFT-003): Rejected peer {} due to subnet limit.",
                        record.node_id
                    );
                    return false;
                }
            }

            // BFT-004: reject if public key XOR hash matches an existing peer exactly
            let new_hash = Self::hash_id(&record.node_id);
            let has_collision = self
                .peers
                .values()
                .any(|p| Self::hash_id(&p.node_id) == new_hash);
            if has_collision {
                log::warn!(
                    "XOR Collision Guard (BFT-004): Rejected peer {} due to duplicate XOR hash.",
                    record.node_id
                );
                return false;
            }

            if self.peers.len() >= MAX_PEER_TABLE_SIZE {
                // Evict oldest
                if let Some(oldest) = self.insertion_order.pop_front() {
                    self.peers.remove(&oldest);
                    self.total_evicted += 1;
                }
            }
            self.insertion_order.push_back(record.node_id.clone());
            self.total_discovered += 1;
        } else {
            // Update existing — keep insertion order, just refresh fields
            if let Some(existing) = self.peers.get_mut(&record.node_id) {
                existing.last_seen = record.last_seen;
                existing.battery_tier = record.battery_tier.clone();
            }
        }
        self.peers.insert(record.node_id.clone(), record);
        is_new
    }

    fn get_subnet(id: &str) -> Option<String> {
        if let Ok(addr) = id.parse::<std::net::SocketAddr>() {
            match addr.ip() {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    Some(format!("{}.{}.{}", octets[0], octets[1], octets[2]))
                }
                std::net::IpAddr::V6(ipv6) => {
                    let segments = ipv6.segments();
                    Some(format!("{:x}:{:x}", segments[0], segments[1]))
                }
            }
        } else {
            None
        }
    }

    pub fn get(&self, node_id: &str) -> Option<&PeerRecord> {
        self.peers.get(node_id)
    }

    pub fn known_count(&self) -> usize {
        self.peers.len()
    }

    /// Peers in a specific zone.
    pub fn peers_in_zone(&self, zone: &str) -> Vec<&PeerRecord> {
        self.peers.values().filter(|p| p.zone == zone).collect()
    }

    /// Peers seen within the last N rounds (alive peers).
    pub fn recently_seen(&self, current_round: u64, window: u64) -> Vec<&PeerRecord> {
        self.peers
            .values()
            .filter(|p| current_round.saturating_sub(p.last_seen) <= window)
            .collect()
    }

    pub fn peers_for_exchange(&self, exclude: &str, current_round: u64) -> Vec<String> {
        // XOR distance K-bucket accelerator:
        // We want to share peers that are "closest" to the requesting node in XOR space.
        // This accelerates discovery across the network.
        let exclude_hash = Self::hash_id(exclude);
        let mut candidates: Vec<(&str, u64)> = self
            .peers
            .values()
            .filter(|p| p.node_id != exclude)
            .map(|p| (p.node_id.as_str(), Self::hash_id(&p.node_id) ^ exclude_hash))
            .collect();
        // Sort by XOR distance (closest first)
        candidates.sort_by_key(|b| b.1);
        candidates
            .iter()
            .take(MAX_PEERS_PER_EXCHANGE)
            .map(|(id, _)| {
                let _ = current_round; // suppress unused warning
                id.to_string()
            })
            .collect()
    }

    pub fn mark_vouched(&mut self, node_id: &str) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.zone_vouched = true;
        }
    }

    pub fn all_peers(&self) -> impl Iterator<Item = &PeerRecord> {
        self.peers.values()
    }

    /// Helper to hash a node ID into a u64 for XOR distance
    fn hash_id(id: &str) -> u64 {
        let mut h = 0xcbf29ce484222325;
        for b in id.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x00000100000001b3);
        }
        h
    }
}

// ── Discovery engine ──────────────────────────────────────────────

/// Per-node discovery engine. Manages beacons, peer exchange,
/// and zone bootstrap tracking.
pub struct DiscoveryEngine {
    pub local_node_id: String,
    pub local_zone: String,
    pub peer_table: PeerTable,
    /// Stats
    pub beacons_sent: u64,
    pub beacons_received: u64,
    pub exchanges_performed: u64,
    /// Rounds this node has been sole beacon sender in its zone.
    sole_beacon_rounds: u64,
    /// Whether this node is a zone coordinator.
    pub is_zone_coordinator: bool,
    /// Discovery event log (last 20 events).
    event_log: VecDeque<String>,
}

impl DiscoveryEngine {
    pub fn new(node_id: &str, zone: &str) -> Self {
        DiscoveryEngine {
            local_node_id: node_id.to_string(),
            local_zone: zone.to_string(),
            peer_table: PeerTable::new(),
            beacons_sent: 0,
            beacons_received: 0,
            exchanges_performed: 0,
            sole_beacon_rounds: 0,
            is_zone_coordinator: false,
            event_log: VecDeque::new(),
        }
    }

    /// Should this node send a beacon this round?
    pub fn should_beacon(&self, round: u64) -> bool {
        round.is_multiple_of(BEACON_INTERVAL_ROUNDS)
    }

    /// Generate this node's beacon for the current round.
    pub fn generate_beacon(&mut self, round: u64, battery_tier: &str) -> Beacon {
        self.beacons_sent += 1;
        Beacon::new(&self.local_node_id, &self.local_zone, round, battery_tier)
    }

    /// Process an incoming beacon from another node.
    /// Returns true if this is a newly discovered peer.
    pub fn receive_beacon(&mut self, beacon: &Beacon, current_round: u64) -> bool {
        if beacon.node_id == self.local_node_id {
            return false; // own beacon, ignore
        }
        self.beacons_received += 1;
        let is_new = self.peer_table.upsert(PeerRecord {
            node_id: beacon.node_id.clone(),
            zone: beacon.zone.clone(),
            last_seen: beacon.round,
            discovery_method: DiscoveryMethod::DirectBeacon,
            battery_tier: beacon.battery_tier.clone(),
            zone_vouched: false,
        });
        if is_new {
            self.log(format!(
                "r{current_round}: discovered {} via beacon (zone={}, batt={})",
                &beacon.node_id[..8.min(beacon.node_id.len())],
                beacon.zone,
                beacon.battery_tier
            ));
        }
        // Zone bootstrap check: are we alone?
        let zone_peers = self.peer_table.peers_in_zone(&self.local_zone).len();
        if zone_peers == 0 {
            self.sole_beacon_rounds += 1;
            if self.sole_beacon_rounds >= ZONE_BOOTSTRAP_ROUNDS && !self.is_zone_coordinator {
                self.is_zone_coordinator = true;
                self.log(format!(
                    "r{current_round}: became zone coordinator for {}",
                    self.local_zone
                ));
            }
        } else {
            self.sole_beacon_rounds = 0;
        }
        is_new
    }

    /// Generate a peer exchange payload (list of known peer IDs to share).
    pub fn generate_peer_exchange(
        &mut self,
        requesting_node: &str,
        current_round: u64,
    ) -> Vec<String> {
        self.exchanges_performed += 1;
        self.peer_table
            .peers_for_exchange(requesting_node, current_round)
    }

    /// Process a peer exchange — learn about peers shared by another node.
    /// Returns number of newly discovered peers.
    pub fn receive_peer_exchange(
        &mut self,
        via_node: &str,
        peer_ids: &[String],
        zone_hint: &str,
        current_round: u64,
    ) -> usize {
        let mut newly_found = 0;
        for peer_id in peer_ids.iter().take(MAX_PEERS_PER_EXCHANGE) {
            if *peer_id == self.local_node_id {
                continue;
            }
            let is_new = self.peer_table.upsert(PeerRecord {
                node_id: peer_id.clone(),
                zone: zone_hint.to_string(),
                last_seen: current_round,
                discovery_method: DiscoveryMethod::PeerExchange {
                    via: via_node.to_string(),
                },
                battery_tier: "UNKNOWN".to_string(),
                zone_vouched: false,
            });
            if is_new {
                newly_found += 1;
                self.log(format!(
                    "r{current_round}: discovered {} via exchange from {}",
                    &peer_id[..8.min(peer_id.len())],
                    &via_node[..8.min(via_node.len())]
                ));
            }
        }
        newly_found
    }

    /// How many unique peers does this node know about?
    pub fn known_peers(&self) -> usize {
        self.peer_table.known_count()
    }

    /// Is a specific node known?
    pub fn knows_peer(&self, node_id: &str) -> bool {
        self.peer_table.get(node_id).is_some()
    }

    fn log(&mut self, msg: String) {
        if self.event_log.len() >= 20 {
            self.event_log.pop_front();
        }
        self.event_log.push_back(msg);
    }

    pub fn event_log(&self) -> impl Iterator<Item = &String> {
        self.event_log.iter()
    }
}

// ── Multi-node discovery simulation ──────────────────────────────

/// Simulate discovery across a network of nodes.
/// Returns: (total beacons exchanged, total peer discoveries)
pub fn simulate_discovery(node_count: usize, rounds: u64) -> (u64, u64) {
    let mut engines: Vec<DiscoveryEngine> = (0..node_count)
        .map(|i| {
            let id = format!("{:016x}", (i as u64).wrapping_mul(0x9e3779b97f4a7c15));
            let zone = if i < node_count / 2 {
                "zone-alpha"
            } else {
                "zone-beta"
            };
            DiscoveryEngine::new(&id, zone)
        })
        .collect();

    let mut total_beacons: u64 = 0;
    let mut total_discoveries: u64 = 0;

    for round in 1..=rounds {
        // Collect beacons this round
        let beacons: Vec<Beacon> = engines
            .iter_mut()
            .filter(|e| e.should_beacon(round))
            .map(|e| e.generate_beacon(round, "FULL"))
            .collect();

        total_beacons += beacons.len() as u64;

        // Each engine receives all beacons
        for engine in &mut engines {
            for beacon in &beacons {
                if engine.receive_beacon(beacon, round) {
                    total_discoveries += 1;
                }
            }
        }

        // Round 20: simulate peer exchange between engine[0] and engine[1]
        if round == 20 && engines.len() >= 2 {
            let id_1 = engines[1].local_node_id.clone();
            let peers = engines[0].generate_peer_exchange(&id_1, round);
            let zone = engines[0].local_zone.clone();
            let id_0 = engines[0].local_node_id.clone();
            if engines.len() > 1 {
                let found = engines[1].receive_peer_exchange(&id_0, &peers, &zone, round);
                total_discoveries += found as u64;
            }
        }
    }

    (total_beacons, total_discoveries)
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine(id: &str, zone: &str) -> DiscoveryEngine {
        DiscoveryEngine::new(id, zone)
    }

    #[test]
    fn test_beacon_interval_respected() {
        let engine = make_engine("a1b2c3d4e5f6a7b8", "zone-1");
        assert!(!engine.should_beacon(1));
        assert!(engine.should_beacon(10));
        assert!(!engine.should_beacon(11));
        assert!(engine.should_beacon(20));
    }

    #[test]
    fn test_own_beacon_ignored() {
        let mut engine = make_engine("a1b2c3d4e5f6a7b8", "zone-1");
        let own_beacon = engine.generate_beacon(10, "FULL");
        let result = engine.receive_beacon(&own_beacon, 10);
        assert!(!result);
        assert_eq!(engine.known_peers(), 0);
    }

    #[test]
    fn test_beacon_discovery() {
        let mut engine_a = make_engine("a1b2c3d4e5f6a7b8", "zone-1");
        let mut engine_b = make_engine("b2c3d4e5f6a7b8c9", "zone-1");
        let beacon_b = engine_b.generate_beacon(10, "FULL");
        let discovered = engine_a.receive_beacon(&beacon_b, 10);
        assert!(discovered);
        assert_eq!(engine_a.known_peers(), 1);
        assert!(engine_a.knows_peer("b2c3d4e5f6a7b8c9"));
    }

    #[test]
    fn test_duplicate_beacon_not_double_counted() {
        let mut engine_a = make_engine("a1b2c3d4e5f6a7b8", "zone-1");
        let mut engine_b = make_engine("b2c3d4e5f6a7b8c9", "zone-1");
        let beacon_b1 = engine_b.generate_beacon(10, "FULL");
        let beacon_b2 = Beacon::new("b2c3d4e5f6a7b8c9", "zone-1", 20, "LOW");
        let first = engine_a.receive_beacon(&beacon_b1, 10);
        let second = engine_a.receive_beacon(&beacon_b2, 20);
        assert!(first);
        assert!(!second); // not new, just update
        assert_eq!(engine_a.known_peers(), 1);
    }

    #[test]
    fn test_peer_exchange_discovers_indirect_peers() {
        let mut engine_a = make_engine("aaaaaaaaaaaaaaaa", "zone-1");
        let mut engine_b = make_engine("bbbbbbbbbbbbbbbb", "zone-1");
        let mut engine_c = make_engine("cccccccccccccccc", "zone-1");
        let mut engine_d = make_engine("dddddddddddddddd", "zone-1");

        // B knows C and D directly
        let beacon_c = engine_c.generate_beacon(10, "FULL");
        let beacon_d = engine_d.generate_beacon(10, "FULL");
        engine_b.receive_beacon(&beacon_c, 10);
        engine_b.receive_beacon(&beacon_d, 10);

        // A exchanges with B — should learn about C and D
        let shared = engine_b.generate_peer_exchange("aaaaaaaaaaaaaaaa", 20);
        let found = engine_a.receive_peer_exchange("bbbbbbbbbbbbbbbb", &shared, "zone-1", 20);
        assert!(
            found >= 1,
            "A should discover at least 1 new peer via exchange"
        );
        assert!(engine_a.knows_peer("cccccccccccccccc") || engine_a.knows_peer("dddddddddddddddd"));
    }

    #[test]
    fn test_peer_exchange_cap_respected() {
        let mut engine_a = make_engine("aaaaaaaaaaaaaaaa", "zone-1");
        // Create 12 fake peers in engine_a's table
        for i in 0..12u64 {
            let id = format!("{i:016x}");
            engine_a.peer_table.upsert(PeerRecord {
                node_id: id,
                zone: "zone-1".to_string(),
                last_seen: i,
                discovery_method: DiscoveryMethod::DirectBeacon,
                battery_tier: "FULL".to_string(),
                zone_vouched: false,
            });
        }
        let shared = engine_a.generate_peer_exchange("requester-node", 50);
        assert!(shared.len() <= MAX_PEERS_PER_EXCHANGE);
    }

    #[test]
    fn test_peers_in_zone_filter() {
        let mut engine = make_engine("aaaaaaaaaaaaaaaa", "zone-1");
        engine.peer_table.upsert(PeerRecord {
            node_id: "peer-zone1-a".to_string(),
            zone: "zone-1".to_string(),
            last_seen: 1,
            discovery_method: DiscoveryMethod::DirectBeacon,
            battery_tier: "FULL".to_string(),
            zone_vouched: false,
        });
        engine.peer_table.upsert(PeerRecord {
            node_id: "peer-zone2-b".to_string(),
            zone: "zone-2".to_string(),
            last_seen: 1,
            discovery_method: DiscoveryMethod::DirectBeacon,
            battery_tier: "FULL".to_string(),
            zone_vouched: false,
        });
        assert_eq!(engine.peer_table.peers_in_zone("zone-1").len(), 1);
        assert_eq!(engine.peer_table.peers_in_zone("zone-2").len(), 1);
    }

    #[test]
    fn test_recently_seen_filter() {
        let mut engine = make_engine("aaaaaaaaaaaaaaaa", "zone-1");
        engine.peer_table.upsert(PeerRecord {
            node_id: "recent-peer".to_string(),
            zone: "zone-1".to_string(),
            last_seen: 95,
            discovery_method: DiscoveryMethod::DirectBeacon,
            battery_tier: "FULL".to_string(),
            zone_vouched: false,
        });
        engine.peer_table.upsert(PeerRecord {
            node_id: "stale-peer".to_string(),
            zone: "zone-1".to_string(),
            last_seen: 10,
            discovery_method: DiscoveryMethod::DirectBeacon,
            battery_tier: "FULL".to_string(),
            zone_vouched: false,
        });
        // At round 100, window of 20: only recent-peer (last_seen=95) qualifies
        let recent = engine.peer_table.recently_seen(100, 20);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].node_id, "recent-peer");
    }

    #[test]
    fn test_peer_table_eviction_at_cap() {
        let mut table = PeerTable::new();
        for i in 0..=MAX_PEER_TABLE_SIZE {
            table.upsert(PeerRecord {
                node_id: format!("node-{i:04}"),
                zone: "zone-1".to_string(),
                last_seen: i as u64,
                discovery_method: DiscoveryMethod::DirectBeacon,
                battery_tier: "FULL".to_string(),
                zone_vouched: false,
            });
        }
        assert_eq!(table.known_count(), MAX_PEER_TABLE_SIZE);
        assert_eq!(table.total_evicted, 1);
    }

    #[test]
    fn test_multi_node_simulation() {
        let (beacons, discoveries) = simulate_discovery(10, 50);
        assert!(beacons > 0, "should have sent beacons");
        assert!(discoveries > 0, "should have made discoveries");
    }

    #[test]
    fn test_beacon_deterministic_sig() {
        let b1 = Beacon::new("a1b2c3d4e5f6a7b8", "zone-1", 42, "FULL");
        let b2 = Beacon::new("a1b2c3d4e5f6a7b8", "zone-1", 42, "FULL");
        assert_eq!(b1.sig_prefix, b2.sig_prefix);
    }

    #[test]
    fn test_discovery_method_tracked() {
        let mut engine_a = make_engine("aaaaaaaaaaaaaaaa", "zone-1");
        let mut engine_b = make_engine("bbbbbbbbbbbbbbbb", "zone-1");
        let beacon = engine_b.generate_beacon(10, "FULL");
        engine_a.receive_beacon(&beacon, 10);
        let peer = engine_a.peer_table.get("bbbbbbbbbbbbbbbb").unwrap();
        assert_eq!(peer.discovery_method, DiscoveryMethod::DirectBeacon);
    }
}
