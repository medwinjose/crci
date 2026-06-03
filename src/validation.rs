//! Input validation and rate limiting for CRCI.
//!
//! Defends against:
//!   - NaN / Infinity / out-of-range severity values
//!   - Panic button spam (same node, same zone, within 60 rounds)
//!   - Message flooding (>10 messages from one node per round)
//!   - Oversized payloads (>255 bytes for LoRa compat)
//!
//! All validation is stateless-friendly: ValidationGuard tracks
//! per-node counters for the current round only.

use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────

/// Max messages one node may originate per round.
const MAX_MSGS_PER_NODE_PER_ROUND: usize = 10;

/// Min rounds between panic button presses from the same node.
const PANIC_COOLDOWN_ROUNDS: u64 = 60;

/// Max payload bytes (LoRa constraint).
const MAX_PAYLOAD_BYTES: usize = 255;

/// Valid severity range.
const MIN_SEVERITY: u8 = 1;
const MAX_SEVERITY: u8 = 5;

// ── Validation errors ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    SeverityOutOfRange(f32),
    NanOrInfinity,
    RateLimitExceeded {
        node_id: String,
        count: usize,
    },
    PanicButtonCooldown {
        node_id: String,
        rounds_remaining: u64,
    },
    PayloadTooLarge {
        size: usize,
        max: usize,
    },
    InvalidNodeId,
    SeqOverflow,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::SeverityOutOfRange(v) => write!(f, "severity {v} out of range [1,5]"),
            ValidationError::NanOrInfinity => write!(f, "NaN or Infinity in numeric field"),
            ValidationError::RateLimitExceeded { node_id, count } => {
                write!(f, "rate limit: {node_id} sent {count} msgs this round")
            }
            ValidationError::PanicButtonCooldown {
                node_id,
                rounds_remaining,
            } => write!(
                f,
                "panic cooldown: {node_id} must wait {rounds_remaining} rounds"
            ),
            ValidationError::PayloadTooLarge { size, max } => {
                write!(f, "payload {size}B exceeds max {max}B")
            }
            ValidationError::InvalidNodeId => write!(f, "empty or malformed node ID"),
            ValidationError::SeqOverflow => write!(f, "sequence number overflow — wrap detected"),
        }
    }
}

// ── Severity validation ───────────────────────────────────────────

/// Validate a severity value. Returns Err if out of range or NaN/Inf.
pub fn validate_severity(severity: f32) -> Result<u8, ValidationError> {
    if severity.is_nan() || severity.is_infinite() {
        return Err(ValidationError::NanOrInfinity);
    }
    let s = severity as u8;
    if !(MIN_SEVERITY..=MAX_SEVERITY).contains(&s) {
        return Err(ValidationError::SeverityOutOfRange(severity));
    }
    Ok(s)
}

/// Validate an integer severity directly.
pub fn validate_severity_u8(severity: u8) -> Result<u8, ValidationError> {
    if !(MIN_SEVERITY..=MAX_SEVERITY).contains(&severity) {
        return Err(ValidationError::SeverityOutOfRange(severity as f32));
    }
    Ok(severity)
}

// ── Payload size validation ───────────────────────────────────────

pub fn validate_payload_size(bytes: &[u8]) -> Result<(), ValidationError> {
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(ValidationError::PayloadTooLarge {
            size: bytes.len(),
            max: MAX_PAYLOAD_BYTES,
        });
    }
    Ok(())
}

// ── Node ID validation ────────────────────────────────────────────

pub fn validate_node_id(id: &str) -> Result<(), ValidationError> {
    if id.is_empty() || id.len() > 64 {
        return Err(ValidationError::InvalidNodeId);
    }
    Ok(())
}

// ── Seq overflow protection ───────────────────────────────────────

/// u64 seq numbers wrap at u64::MAX - 1000 to prevent overflow.
/// Returns Err if the seq number is dangerously close to wrapping.
pub fn validate_seq(seq: u64) -> Result<(), ValidationError> {
    if seq > u64::MAX - 1000 {
        return Err(ValidationError::SeqOverflow);
    }
    Ok(())
}

// ── Rate limiter ──────────────────────────────────────────────────

/// Per-round rate limiter. Call reset() at the start of each round.
#[derive(Default)]
pub struct RateLimiter {
    /// node_id → message count this round
    counts: HashMap<String, usize>,
    /// node_id → last round when panic button was pressed
    panic_last_round: HashMap<String, u64>,
}

impl RateLimiter {
    pub fn new() -> Self {
        RateLimiter::default()
    }

    /// Reset per-round counters. Call at start of each gossip round.
    pub fn reset_round(&mut self) {
        self.counts.clear();
    }

    /// Record a message from node_id. Returns Err if rate limit exceeded.
    pub fn check_and_record(&mut self, node_id: &str) -> Result<(), ValidationError> {
        let count = self.counts.entry(node_id.to_string()).or_insert(0);
        *count += 1;
        if *count > MAX_MSGS_PER_NODE_PER_ROUND {
            return Err(ValidationError::RateLimitExceeded {
                node_id: node_id.to_string(),
                count: *count,
            });
        }
        Ok(())
    }

    /// Check panic button cooldown. Returns Err if within cooldown window.
    pub fn check_panic_cooldown(
        &mut self,
        node_id: &str,
        current_round: u64,
    ) -> Result<(), ValidationError> {
        if let Some(&last) = self.panic_last_round.get(node_id) {
            let elapsed = current_round.saturating_sub(last);
            if elapsed < PANIC_COOLDOWN_ROUNDS {
                return Err(ValidationError::PanicButtonCooldown {
                    node_id: node_id.to_string(),
                    rounds_remaining: PANIC_COOLDOWN_ROUNDS - elapsed,
                });
            }
        }
        self.panic_last_round
            .insert(node_id.to_string(), current_round);
        Ok(())
    }

    /// How many messages has this node sent this round?
    pub fn round_count(&self, node_id: &str) -> usize {
        self.counts.get(node_id).copied().unwrap_or(0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_nan_rejected() {
        assert!(validate_severity(f32::NAN).is_err());
    }

    #[test]
    fn test_severity_infinity_rejected() {
        assert!(validate_severity(f32::INFINITY).is_err());
    }

    #[test]
    fn test_severity_out_of_range_rejected() {
        assert!(validate_severity(0.0).is_err());
        assert!(validate_severity(6.0).is_err());
        assert!(validate_severity(-1.0).is_err());
    }

    #[test]
    fn test_severity_valid_range_accepted() {
        for s in 1..=5 {
            assert!(validate_severity(s as f32).is_ok());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_at_limit() {
        let mut limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.check_and_record("node-x").is_ok());
        }
        // 11th message should fail
        assert!(limiter.check_and_record("node-x").is_err());
    }

    #[test]
    fn test_rate_limiter_resets_per_round() {
        let mut limiter = RateLimiter::new();
        for _ in 0..10 {
            limiter.check_and_record("node-x").ok();
        }
        limiter.reset_round();
        // After reset, should accept again
        assert!(limiter.check_and_record("node-x").is_ok());
    }

    #[test]
    fn test_panic_cooldown_enforced() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.check_panic_cooldown("node-y", 1).is_ok());
        // Immediately after: blocked
        assert!(limiter.check_panic_cooldown("node-y", 2).is_err());
        // After cooldown: allowed
        assert!(limiter.check_panic_cooldown("node-y", 62).is_ok());
    }

    #[test]
    fn test_payload_size_enforced() {
        let small = vec![0u8; 100];
        let large = vec![0u8; 300];
        assert!(validate_payload_size(&small).is_ok());
        assert!(validate_payload_size(&large).is_err());
    }

    #[test]
    fn test_seq_overflow_detected() {
        assert!(validate_seq(u64::MAX - 500).is_err());
        assert!(validate_seq(1000).is_ok());
    }

    #[test]
    fn test_empty_node_id_rejected() {
        assert!(validate_node_id("").is_err());
    }
}
