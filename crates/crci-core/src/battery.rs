//! Battery-aware gossip throttling for CRCI.
//!
//! Nodes in low-battery state reduce gossip frequency and payload
//! size to extend survival time. Rescue requests are ALWAYS forwarded
//! regardless of battery state — no rescue is ever silenced.
//!
//! Three tiers:
//!   Full    (>20%): normal operation
//!   Low     (5-20%): halve gossip rate, drop non-rescue forwards
//!   Critical (<5%): beacon-only mode — only panic/rescue traffic

use std::collections::VecDeque;

// ── Constants ─────────────────────────────────────────────────────

const BATTERY_LOW_THRESHOLD: u8 = 20;
const BATTERY_CRITICAL_THRESHOLD: u8 = 5;

/// Gossip interval multiplier per tier (relative to base interval).
const FULL_INTERVAL_MULTIPLIER: u32 = 1;
const LOW_INTERVAL_MULTIPLIER: u32 = 3;
const CRITICAL_INTERVAL_MULTIPLIER: u32 = 10;

// ── Battery tier ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryTier {
    Full,
    Low,
    Critical,
}

impl BatteryTier {
    pub fn from_percent(pct: u8) -> Self {
        if pct <= BATTERY_CRITICAL_THRESHOLD {
            BatteryTier::Critical
        } else if pct <= BATTERY_LOW_THRESHOLD {
            BatteryTier::Low
        } else {
            BatteryTier::Full
        }
    }

    pub fn interval_multiplier(self) -> u32 {
        match self {
            BatteryTier::Full => FULL_INTERVAL_MULTIPLIER,
            BatteryTier::Low => LOW_INTERVAL_MULTIPLIER,
            BatteryTier::Critical => CRITICAL_INTERVAL_MULTIPLIER,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BatteryTier::Full => "FULL",
            BatteryTier::Low => "LOW",
            BatteryTier::Critical => "CRITICAL",
        }
    }
}

// ── Battery state ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BatteryState {
    pub node_id: String,
    pub percent: u8,
    pub tier: BatteryTier,
    /// Rolling drain rate samples (last 5 readings)
    drain_samples: VecDeque<u8>,
    /// Total messages forwarded (for stats)
    pub forwarded: u64,
    /// Total messages suppressed due to battery (non-rescue only)
    pub suppressed: u64,
}

impl BatteryState {
    pub fn new(node_id: &str, initial_percent: u8) -> Self {
        BatteryState {
            node_id: node_id.to_string(),
            percent: initial_percent,
            tier: BatteryTier::from_percent(initial_percent),
            drain_samples: VecDeque::new(),
            forwarded: 0,
            suppressed: 0,
        }
    }

    /// Update battery level. Recalculates tier.
    pub fn update(&mut self, new_percent: u8) {
        if self.percent > new_percent {
            self.drain_samples.push_back(self.percent - new_percent);
            if self.drain_samples.len() > 5 {
                self.drain_samples.pop_front();
            }
        }
        self.percent = new_percent;
        self.tier = BatteryTier::from_percent(new_percent);
    }

    /// Estimated drain rate per round (average of last 5 samples).
    pub fn drain_rate_per_round(&self) -> f32 {
        if self.drain_samples.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.drain_samples.iter().map(|&x| x as u32).sum();
        sum as f32 / self.drain_samples.len() as f32
    }

    /// Estimated rounds remaining at current drain rate.
    pub fn estimated_rounds_remaining(&self) -> Option<u32> {
        let rate = self.drain_rate_per_round();
        if rate <= 0.0 {
            return None;
        }
        Some((self.percent as f32 / rate) as u32)
    }

    /// Should this node forward a message?
    /// is_rescue: rescue/panic messages are ALWAYS forwarded.
    pub fn should_forward(&mut self, is_rescue: bool) -> bool {
        if is_rescue {
            self.forwarded += 1;
            return true; // rescue always forwarded
        }
        let forward = match self.tier {
            BatteryTier::Full => true,
            BatteryTier::Low => {
                // Forward every 3rd round equivalent: use forwarded count
                self.forwarded.is_multiple_of(3)
            }
            BatteryTier::Critical => false, // only rescue (handled above)
        };
        if forward {
            self.forwarded += 1;
        } else {
            self.suppressed += 1;
        }
        forward
    }
}

// ── Network-level battery summary ─────────────────────────────────

/// Summary of battery state across all nodes.
#[derive(Debug)]
pub struct NetworkBatterySummary {
    pub total_nodes: usize,
    pub full_count: usize,
    pub low_count: usize,
    pub critical_count: usize,
    pub min_percent: u8,
    pub avg_percent: f32,
}

impl NetworkBatterySummary {
    pub fn from_states(states: &[BatteryState]) -> Self {
        let total = states.len();
        let full = states
            .iter()
            .filter(|s| s.tier == BatteryTier::Full)
            .count();
        let low = states.iter().filter(|s| s.tier == BatteryTier::Low).count();
        let critical = states
            .iter()
            .filter(|s| s.tier == BatteryTier::Critical)
            .count();
        let min = states.iter().map(|s| s.percent).min().unwrap_or(0);
        let avg = if total == 0 {
            0.0
        } else {
            states.iter().map(|s| s.percent as f32).sum::<f32>() / total as f32
        };
        NetworkBatterySummary {
            total_nodes: total,
            full_count: full,
            low_count: low,
            critical_count: critical,
            min_percent: min,
            avg_percent: avg,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_classification() {
        assert_eq!(BatteryTier::from_percent(100), BatteryTier::Full);
        assert_eq!(BatteryTier::from_percent(20), BatteryTier::Low);
        assert_eq!(BatteryTier::from_percent(19), BatteryTier::Low);
        assert_eq!(BatteryTier::from_percent(5), BatteryTier::Critical);
        assert_eq!(BatteryTier::from_percent(4), BatteryTier::Critical);
        assert_eq!(BatteryTier::from_percent(0), BatteryTier::Critical);
    }

    #[test]
    fn test_rescue_always_forwarded_in_critical() {
        let mut state = BatteryState::new("node-x", 2);
        assert_eq!(state.tier, BatteryTier::Critical);
        // Rescue must always go through
        assert!(state.should_forward(true));
        assert!(state.should_forward(true));
        // Normal message suppressed in critical
        assert!(!state.should_forward(false));
    }

    #[test]
    fn test_normal_suppressed_in_critical() {
        let mut state = BatteryState::new("node-y", 3);
        for _ in 0..10 {
            state.should_forward(false);
        }
        assert_eq!(state.suppressed, 10);
        assert_eq!(state.forwarded, 0);
    }

    #[test]
    fn test_drain_rate_estimated() {
        let mut state = BatteryState::new("node-z", 100);
        state.update(95); // -5
        state.update(90); // -5
        state.update(85); // -5
        let rate = state.drain_rate_per_round();
        assert!((rate - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_estimated_rounds_remaining() {
        let mut state = BatteryState::new("node-w", 50);
        state.update(45); // -5 per round
        let remaining = state.estimated_rounds_remaining();
        assert!(remaining.is_some());
        assert_eq!(remaining.unwrap(), 9); // 45 / 5 = 9
    }

    #[test]
    fn test_network_summary() {
        let states = vec![
            BatteryState::new("a", 80),
            BatteryState::new("b", 15),
            BatteryState::new("c", 3),
        ];
        let summary = NetworkBatterySummary::from_states(&states);
        assert_eq!(summary.full_count, 1);
        assert_eq!(summary.low_count, 1);
        assert_eq!(summary.critical_count, 1);
        assert_eq!(summary.min_percent, 3);
    }
}
