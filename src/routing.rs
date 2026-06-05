use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RoutingEntry {
    pub peer_id: String,
    pub last_seen_ms: u64,
    pub hop_count: u8,
    pub freshness_ms: u64,
}

impl RoutingEntry {
    pub fn is_stale(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_seen_ms) > self.freshness_ms
    }
}

/// Priority score for outbound message scheduling.
/// Higher score = send first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessagePriority(pub u64);

pub fn compute_priority(severity: u8, created_at_ms: u64, now_ms: u64) -> MessagePriority {
    let age_ms = now_ms.saturating_sub(created_at_ms);
    let urgency_score = (severity as u64) * 10_000;
    let freshness_penalty = age_ms / 100;
    let priority = urgency_score.saturating_sub(freshness_penalty);
    MessagePriority(priority)
}

pub struct TemporalRouter {
    table: HashMap<String, RoutingEntry>,
}

impl TemporalRouter {
    pub fn new() -> Self {
        TemporalRouter {
            table: HashMap::new(),
        }
    }

    pub fn upsert(&mut self, entry: RoutingEntry) {
        if let Some(existing) = self.table.get(&entry.peer_id) {
            if entry.last_seen_ms >= existing.last_seen_ms {
                self.table.insert(entry.peer_id.clone(), entry);
            }
        } else {
            self.table.insert(entry.peer_id.clone(), entry);
        }
    }

    pub fn evict_stale(&mut self, now_ms: u64) -> usize {
        let initial_len = self.table.len();
        self.table.retain(|_, entry| !entry.is_stale(now_ms));
        initial_len - self.table.len()
    }

    pub fn best_next_hop(&self, exclude: &[&str], now_ms: u64) -> Option<&RoutingEntry> {
        self.table
            .values()
            .filter(|entry| !entry.is_stale(now_ms) && !exclude.contains(&entry.peer_id.as_str()))
            .min_by(|a, b| {
                a.hop_count
                    .cmp(&b.hop_count)
                    .then_with(|| b.last_seen_ms.cmp(&a.last_seen_ms)) // tie-break: most recent first
            })
    }

    pub fn active_peer_ids(&self, now_ms: u64) -> Vec<String> {
        self.table
            .values()
            .filter(|entry| !entry.is_stale(now_ms))
            .map(|entry| entry.peer_id.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

impl Default for TemporalRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_entry_eviction() {
        let mut router = TemporalRouter::new();
        router.upsert(RoutingEntry {
            peer_id: "peer1".to_string(),
            last_seen_ms: 0,
            hop_count: 1,
            freshness_ms: 1000,
        });

        assert_eq!(router.len(), 1);
        let evicted = router.evict_stale(2000);
        assert_eq!(evicted, 1);
        assert_eq!(router.len(), 0);
    }

    #[test]
    fn best_next_hop_prefers_low_hop_count() {
        let mut router = TemporalRouter::new();
        router.upsert(RoutingEntry {
            peer_id: "peer3".to_string(),
            last_seen_ms: 1000,
            hop_count: 3,
            freshness_ms: 5000,
        });
        router.upsert(RoutingEntry {
            peer_id: "peer1".to_string(),
            last_seen_ms: 1000,
            hop_count: 1,
            freshness_ms: 5000,
        });
        router.upsert(RoutingEntry {
            peer_id: "peer2".to_string(),
            last_seen_ms: 1000,
            hop_count: 2,
            freshness_ms: 5000,
        });

        let best = router.best_next_hop(&[], 2000).unwrap();
        assert_eq!(best.peer_id, "peer1");
        assert_eq!(best.hop_count, 1);
    }

    #[test]
    fn priority_decays_with_age() {
        let fresh_priority = compute_priority(9, 0, 0);
        let stale_priority = compute_priority(9, 0, 50_000);

        assert!(fresh_priority.0 > stale_priority.0);
    }

    #[test]
    fn high_severity_beats_fresh_low_severity() {
        // severity 9, age 5s
        let high_sev = compute_priority(9, 0, 5000);
        // severity 1, age 0s
        let low_sev = compute_priority(1, 5000, 5000);

        // 9 * 10000 - 50 = 89950
        // 1 * 10000 - 0 = 10000
        assert!(high_sev.0 > low_sev.0);
    }

    #[test]
    fn upsert_does_not_regress_last_seen() {
        let mut router = TemporalRouter::new();
        router.upsert(RoutingEntry {
            peer_id: "peerA".to_string(),
            last_seen_ms: 1000,
            hop_count: 1,
            freshness_ms: 5000,
        });

        // Upsert with older last_seen_ms
        router.upsert(RoutingEntry {
            peer_id: "peerA".to_string(),
            last_seen_ms: 500,
            hop_count: 2,
            freshness_ms: 5000,
        });

        let best = router.best_next_hop(&[], 1500).unwrap();
        // The last_seen_ms should still be 1000
        assert_eq!(best.last_seen_ms, 1000);
        assert_eq!(best.hop_count, 1);
    }
}
