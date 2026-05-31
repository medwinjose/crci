use ed25519_dalek::Signature;

// ─── MessageType ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum MessageType {
    Normal,
    RescueRequest,
    MassCasualtyEvent,
}

// ─── Visibility ──────────────────────────────────────────────────────────────
// In real life: how clearly can the person actually observe the situation?
// Direct = eyes on. Indirect = can hear/smell. Unknown = indoors/blocked.

#[derive(Clone, Debug)]
pub enum Visibility {
    Direct,
    Indirect,
    Unknown,
}

// ─── Signal ──────────────────────────────────────────────────────────────────
// In real life: the structured 4-button form on someone's phone.
// These are the only fields the anomaly engine judges — no text, no spelling.

#[derive(Clone, Debug)]
pub struct Signal {
    pub severity: u8,
    pub needs_help: bool,
    pub can_help_others: bool,
    pub location_confirmed: bool,
    pub confidence: u8,
    pub visibility: Visibility,
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
        assert!(severity >= 1 && severity <= 5, "Severity must be 1-5");
        assert!(confidence >= 1 && confidence <= 5, "Confidence must be 1-5");
        Signal { severity, needs_help, can_help_others, location_confirmed, confidence, visibility }
    }

    // Panic button — one tap, maximum emergency, no choices needed
    pub fn panic() -> Signal {
        Signal {
            severity: 5,
            needs_help: true,
            can_help_others: false,
            location_confirmed: true,
            confidence: 5,
            visibility: Visibility::Direct,
        }
    }
}

// ─── Message ─────────────────────────────────────────────────────────────────
// In real life: the envelope carrying someone's report through the network.
// Now includes a cryptographic signature — proof the stated origin actually
// sent this. A forged message without a valid signature is dropped instantly.

#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub origin: String,
    pub signal: Signal,
    pub note: Option<String>,
    pub message_type: MessageType,
    pub origin_active: bool,
    pub signature: Option<Signature>, // None = unsigned (legacy/system messages)
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
        }
    }

    // The bytes that get signed — id + origin + severity combined
    // In real life: this is the exact content that proves authenticity.
    // Changing any field after signing invalidates the signature.
    pub fn signable_payload(&self) -> Vec<u8> {
        format!("{}:{}:{}", self.id, self.origin, self.signal.severity)
            .into_bytes()
    }
}