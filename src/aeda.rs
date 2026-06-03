//! Autonomous Emergency Decision Architecture (AEDA) for CRCI.
//!
//! Rule-based autonomous decisions — no external crates, no ML.
//! Designed to augment human coordinators, not replace them.
//! All decisions are logged and reversible.

use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────

const ESCALATION_THRESHOLD: usize = 3; // rescues in zone to auto-escalate
const ESCALATION_WINDOW: u64 = 5; // rounds window for escalation check
const FLIP_THRESHOLD: usize = 3; // sev flips to flag as misinformation
const SUSPICIOUS_DELTA: u8 = 3; // min sev change to count as a flip
const MAX_URGENCY: f32 = 25.0; // max possible urgency score

// ── Data types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RescueEvent {
    pub node_id: String,
    pub zone: String,
    pub severity: u8,
    pub round: u64,
    pub reputation: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AedaDecision {
    /// Zone has hit escalation threshold — alert coordinator
    ZoneEscalated {
        zone: String,
        rescue_count: usize,
        round: u64,
    },
    /// Node is suspected of reporting false all-clear
    SuspiciousAllClear { node_id: String, zone: String },
    /// Node may be running a misinformation campaign
    MisinformationSuspect { node_id: String, flip_count: usize },
    /// Urgency score for a rescue request
    UrgencyAssigned {
        node_id: String,
        zone: String,
        score: f32,
    },
}

impl std::fmt::Display for AedaDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AedaDecision::ZoneEscalated {
                zone,
                rescue_count,
                round,
            } => write!(
                f,
                "🚨 ESCALATE zone={zone} rescues={rescue_count} round={round}"
            ),
            AedaDecision::SuspiciousAllClear { node_id, zone } => {
                write!(f, "⚠ SUSPICIOUS all-clear from {node_id} in {zone}")
            }
            AedaDecision::MisinformationSuspect {
                node_id,
                flip_count,
            } => write!(f, "🔴 DISINFO suspect {node_id} flips={flip_count}"),
            AedaDecision::UrgencyAssigned {
                node_id,
                zone,
                score,
            } => write!(f, "📊 URGENCY {node_id}@{zone} score={score:.2}"),
        }
    }
}

// ── AEDA Engine ───────────────────────────────────────────────────

pub struct AedaEngine {
    /// All rescue events seen, keyed by zone
    zone_rescues: HashMap<String, Vec<RescueEvent>>,
    /// Per-node severity history for flip detection
    severity_history: HashMap<String, Vec<u8>>,
    /// All decisions made (append-only log)
    decisions: Vec<AedaDecision>,
}

impl AedaEngine {
    pub fn new() -> Self {
        AedaEngine {
            zone_rescues: HashMap::new(),
            severity_history: HashMap::new(),
            decisions: Vec::new(),
        }
    }

    /// Ingest a rescue event and run all AEDA rules.
    pub fn process(&mut self, event: RescueEvent, current_round: u64) {
        // 1. Escalation check
        self.check_escalation(&event, current_round);

        // 2. Urgency score
        let zone_density = self
            .zone_rescues
            .get(&event.zone)
            .map(|v| v.len())
            .unwrap_or(0) as f32
            + 1.0;
        let score = (zone_density * event.reputation * event.severity as f32).min(MAX_URGENCY);
        self.decisions.push(AedaDecision::UrgencyAssigned {
            node_id: event.node_id.clone(),
            zone: event.zone.clone(),
            score,
        });

        // 3. Track severity history for flip detection
        self.track_severity_flip(&event.node_id, event.severity);

        // Store event
        self.zone_rescues
            .entry(event.zone.clone())
            .or_default()
            .push(event);
    }

    /// Ingest a normal (non-rescue) status report.
    /// Used to detect suspicious all-clears while zone is in crisis.
    pub fn process_normal_report(&mut self, node_id: &str, zone: &str) {
        let zone_in_crisis = self
            .zone_rescues
            .get(zone)
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        if zone_in_crisis {
            self.decisions.push(AedaDecision::SuspiciousAllClear {
                node_id: node_id.to_string(),
                zone: zone.to_string(),
            });
        }
    }

    fn check_escalation(&mut self, event: &RescueEvent, current_round: u64) {
        let rescues = self.zone_rescues.entry(event.zone.clone()).or_default();

        let recent = rescues
            .iter()
            .filter(|r| current_round.saturating_sub(r.round) <= ESCALATION_WINDOW)
            .count()
            + 1; // +1 for current event not yet stored

        if recent >= ESCALATION_THRESHOLD {
            // Only escalate once per window (don't spam decisions)
            let already = self.decisions.iter().any(|d| {
                matches!(
                    d,
                    AedaDecision::ZoneEscalated { zone, round, .. }
                    if zone == &event.zone
                    && current_round.saturating_sub(*round) <= ESCALATION_WINDOW
                )
            });
            if !already {
                self.decisions.push(AedaDecision::ZoneEscalated {
                    zone: event.zone.clone(),
                    rescue_count: recent,
                    round: current_round,
                });
            }
        }
    }

    fn track_severity_flip(&mut self, node_id: &str, severity: u8) {
        let history = self
            .severity_history
            .entry(node_id.to_string())
            .or_default();
        history.push(severity);

        if history.len() < 2 {
            return;
        }

        // Count large swings in the last 6 reports
        let window = &history[history.len().saturating_sub(6)..];
        let flips = window
            .windows(2)
            .filter(|pair| {
                (pair[0] as i16 - pair[1] as i16).unsigned_abs() as u8 >= SUSPICIOUS_DELTA
            })
            .count();

        if flips >= FLIP_THRESHOLD {
            let already = self.decisions.iter().any(|d| {
                matches!(
                    d,
                    AedaDecision::MisinformationSuspect { node_id: nid, .. }
                    if nid == node_id
                )
            });
            if !already {
                self.decisions.push(AedaDecision::MisinformationSuspect {
                    node_id: node_id.to_string(),
                    flip_count: flips,
                });
            }
        }
    }

    /// All decisions made so far (immutable view).
    pub fn decisions(&self) -> &[AedaDecision] {
        &self.decisions
    }

    /// Count decisions of a specific type.
    pub fn count_escalations(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| matches!(d, AedaDecision::ZoneEscalated { .. }))
            .count()
    }

    pub fn count_suspicious(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| matches!(d, AedaDecision::SuspiciousAllClear { .. }))
            .count()
    }

    pub fn count_disinfo(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| matches!(d, AedaDecision::MisinformationSuspect { .. }))
            .count()
    }
}

impl Default for AedaEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(node: &str, zone: &str, sev: u8, round: u64) -> RescueEvent {
        RescueEvent {
            node_id: node.to_string(),
            zone: zone.to_string(),
            severity: sev,
            round,
            reputation: 1.0,
        }
    }

    #[test]
    fn test_escalation_triggers_at_threshold() {
        let mut engine = AedaEngine::new();
        for i in 0..3 {
            engine.process(make_event(&format!("node-{i}"), "zone-a", 4, 10), 10);
        }
        assert_eq!(engine.count_escalations(), 1);
    }

    #[test]
    fn test_escalation_no_duplicate_in_window() {
        let mut engine = AedaEngine::new();
        for i in 0..5 {
            engine.process(make_event(&format!("node-{i}"), "zone-a", 4, 10), 10);
        }
        assert_eq!(engine.count_escalations(), 1); // not 3
    }

    #[test]
    fn test_suspicious_all_clear_detected() {
        let mut engine = AedaEngine::new();
        engine.process(make_event("node-1", "zone-b", 5, 1), 1);
        engine.process_normal_report("node-2", "zone-b");
        assert_eq!(engine.count_suspicious(), 1);
    }

    #[test]
    fn test_misinformation_flip_detected() {
        let mut engine = AedaEngine::new();
        // Alternating sev 1 and 5 — large flips
        for round in 0..6u64 {
            let sev = if round % 2 == 0 { 1 } else { 5 };
            engine.process(make_event("bad-node", "zone-c", sev, round), round);
        }
        assert_eq!(engine.count_disinfo(), 1);
    }

    #[test]
    fn test_urgency_score_assigned() {
        let mut engine = AedaEngine::new();
        engine.process(make_event("node-1", "zone-d", 5, 1), 1);
        let urgency_decisions: Vec<_> = engine
            .decisions()
            .iter()
            .filter(|d| matches!(d, AedaDecision::UrgencyAssigned { .. }))
            .collect();
        assert!(!urgency_decisions.is_empty());
    }

    #[test]
    fn test_urgency_capped_at_max() {
        let mut engine = AedaEngine::new();
        // High density zone, high rep, high sev
        for i in 0..20 {
            engine.process(
                RescueEvent {
                    node_id: format!("n{i}"),
                    zone: "zone-e".to_string(),
                    severity: 5,
                    round: 1,
                    reputation: 2.0,
                },
                1,
            );
        }
        let over_max = engine.decisions().iter().any(|d| {
            if let AedaDecision::UrgencyAssigned { score, .. } = d {
                *score > MAX_URGENCY
            } else {
                false
            }
        });
        assert!(!over_max);
    }
}
