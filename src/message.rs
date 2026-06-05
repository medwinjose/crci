// ─── Message Types ────────────────────────────────────────────────────────────
// Every piece of information that flows through the CRCI mesh is a Message.
// Signals carry the raw sensor reading. Messages wrap signals with identity,
// routing metadata, and crisis-specific fields added in session 14.

use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};

// ─── Visibility ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Visibility {
    Direct,
    Indirect,
    Unknown,
}

// ─── MessageType ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MessageType {
    Normal,
    RescueRequest,
    MassCasualtyEvent,
    Panic,
    ChainHeadAnnouncement {
        head_hash: [u8; 32],
        head_seq: u64,
    },
}

impl MessageType {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            MessageType::Normal => "normal",
            MessageType::RescueRequest => "rescue",
            MessageType::MassCasualtyEvent => "mce",
            MessageType::Panic => "panic",
            MessageType::ChainHeadAnnouncement { .. } => "chain_head",
        }
    }
}

// ─── MessagePriority ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum MessagePriority {
    #[allow(dead_code)]
    Low,
    Normal,
    High,
    Critical,
}

// ─── GpsCoord (inline — no separate scenario module needed here) ──────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpsCoord {
    pub lat: f64,
    pub lon: f64,
}

impl GpsCoord {
    pub fn new(lat: f64, lon: f64) -> Self {
        GpsCoord { lat, lon }
    }

    #[allow(dead_code)]
    pub fn distance_metres(&self, other: &GpsCoord) -> f64 {
        let r = 6_371_000.0_f64;
        let dlat = (other.lat - self.lat).to_radians();
        let dlon = (other.lon - self.lon).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + self.lat.to_radians().cos()
                * other.lat.to_radians().cos()
                * (dlon / 2.0).sin().powi(2);
        r * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
    }
}

// ─── Signal ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Signal {
    pub severity: u8,
    pub needs_help: bool,
    pub can_help_others: bool,
    pub location_confirmed: bool,
    pub confidence: u8,
    pub visibility: Visibility,
    // Session 14 additions
    pub gps: Option<GpsCoord>,
    pub resource_type: Option<String>,
}

impl Signal {
    pub fn new(
        severity: u8,
        needs_help: bool,
        can_help_others: bool,
        location_confirmed: bool,
        confidence: u8,
        visibility: Visibility,
    ) -> Signal {
        assert!((1..=5).contains(&severity));
        assert!((1..=5).contains(&confidence));
        Signal {
            severity,
            needs_help,
            can_help_others,
            location_confirmed,
            confidence,
            visibility,
            gps: None,
            resource_type: None,
        }
    }

    pub fn panic() -> Signal {
        Signal {
            severity: 5,
            needs_help: true,
            can_help_others: false,
            location_confirmed: true,
            confidence: 5,
            visibility: Visibility::Direct,
            gps: None,
            resource_type: None,
        }
    }

    pub fn with_gps(mut self, lat: f64, lon: f64) -> Signal {
        self.gps = Some(GpsCoord::new(lat, lon));
        self.location_confirmed = true;
        self
    }

    pub fn with_resource(mut self, resource: &str) -> Signal {
        self.resource_type = Some(resource.to_string());
        self
    }
}

// ─── Timestamp helper ─────────────────────────────────────────────────────────

pub fn now_ts() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── Message ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub origin: String,
    pub signal: Signal,
    pub note: Option<String>,
    pub message_type: MessageType,
    pub origin_active: bool,
    #[allow(dead_code)]
    pub signature: Option<Signature>,
    // Session 14 additions
    #[allow(dead_code)]
    pub created_at: u64,
    #[allow(dead_code)]
    pub ttl_seconds: u64, // 0 = never expires
    pub priority: MessagePriority,
    #[allow(dead_code)]
    pub hop_count: u8,
    pub seq: u64,
}
impl Message {
    pub fn new(id: &str, origin: &str, signal: Signal, note: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            origin: origin.to_string(),
            signal,
            note: note.map(|s| s.to_string()),
            message_type: MessageType::Normal,
            origin_active: true,
            signature: None,
            created_at: now_ts(),
            ttl_seconds: 600,
            priority: MessagePriority::Normal,
            hop_count: 0,
            seq: 0,
        }
    }

    pub fn rescue(id: &str, origin: &str, note: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            origin: origin.to_string(),
            signal: Signal::panic(),
            note: note.map(|s| s.to_string()),
            message_type: MessageType::RescueRequest,
            origin_active: true,
            signature: None,
            created_at: now_ts(),
            ttl_seconds: 0, // rescue requests never expire
            priority: MessagePriority::Critical,
            hop_count: 0,
            seq: 0,
        }
    }

    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        if self.ttl_seconds == 0 {
            return false;
        }
        let age = now_ts().saturating_sub(self.created_at);
        age > self.ttl_seconds
    }

    #[allow(dead_code)]
    pub fn age_seconds(&self) -> u64 {
        now_ts().saturating_sub(self.created_at)
    }

    #[allow(dead_code)]
    pub fn signable_payload(&self) -> Vec<u8> {
        format!("{}:{}:{}", self.id, self.origin, self.signal.severity).into_bytes()
    }
}
