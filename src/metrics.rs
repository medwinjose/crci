use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Clone, Default)]
pub struct PeerState {
    pub zone: String,
    pub reputation: f64,
    pub messages: u64,
    pub rescues: u64,
    pub is_online: bool,
}

pub struct CrciMetrics {
    pub messages_accepted: AtomicU64,
    pub messages_rejected: AtomicU64,
    pub messages_throttled: AtomicU64,
    pub byzantine_detected: AtomicU64,
    pub rescue_requests_held: AtomicU64,
    pub reputation_penalties: AtomicU64,
    pub peer_count: AtomicU64,
    pub consensus_rounds: AtomicU64,
    pub peers: Mutex<HashMap<String, PeerState>>,
}

impl Default for CrciMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl CrciMetrics {
    pub fn new() -> Self {
        CrciMetrics {
            messages_accepted: AtomicU64::new(0),
            messages_rejected: AtomicU64::new(0),
            messages_throttled: AtomicU64::new(0),
            byzantine_detected: AtomicU64::new(0),
            rescue_requests_held: AtomicU64::new(0),
            reputation_penalties: AtomicU64::new(0),
            peer_count: AtomicU64::new(0),
            consensus_rounds: AtomicU64::new(0),
            peers: Mutex::new(HashMap::new()),
        }
    }

    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();

        out.push_str("# HELP crci_messages_accepted Total accepted messages\n");
        out.push_str("# TYPE crci_messages_accepted counter\n");
        out.push_str(&format!(
            "crci_messages_accepted {}\n",
            self.messages_accepted.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_messages_rejected Total rejected messages\n");
        out.push_str("# TYPE crci_messages_rejected counter\n");
        out.push_str(&format!(
            "crci_messages_rejected {}\n",
            self.messages_rejected.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_messages_throttled Total throttled messages\n");
        out.push_str("# TYPE crci_messages_throttled counter\n");
        out.push_str(&format!(
            "crci_messages_throttled {}\n",
            self.messages_throttled.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_byzantine_detected Total byzantine nodes detected\n");
        out.push_str("# TYPE crci_byzantine_detected counter\n");
        out.push_str(&format!(
            "crci_byzantine_detected {}\n",
            self.byzantine_detected.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_rescue_requests_held Current active rescue requests held\n");
        out.push_str("# TYPE crci_rescue_requests_held gauge\n");
        out.push_str(&format!(
            "crci_rescue_requests_held {}\n",
            self.rescue_requests_held.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_reputation_penalties Total reputation penalties applied\n");
        out.push_str("# TYPE crci_reputation_penalties counter\n");
        out.push_str(&format!(
            "crci_reputation_penalties {}\n",
            self.reputation_penalties.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_peer_count Current active peer connections\n");
        out.push_str("# TYPE crci_peer_count gauge\n");
        out.push_str(&format!(
            "crci_peer_count {}\n",
            self.peer_count.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP crci_consensus_rounds Total consensus rounds completed\n");
        out.push_str("# TYPE crci_consensus_rounds counter\n");
        out.push_str(&format!(
            "crci_consensus_rounds {}\n",
            self.consensus_rounds.load(Ordering::Relaxed)
        ));

        if let Ok(peers) = self.peers.lock() {
            out.push_str("# HELP crci_peer_reputation Peer reputation\n");
            out.push_str("# TYPE crci_peer_reputation gauge\n");
            for (id, state) in peers.iter() {
                out.push_str(&format!(
                    "crci_peer_reputation{{node_id=\"{}\",zone=\"{}\",status=\"{}\"}} {:.2}\n",
                    id,
                    state.zone,
                    if state.is_online { "online" } else { "offline" },
                    state.reputation
                ));
            }

            out.push_str("# HELP crci_peer_messages Peer message count\n");
            out.push_str("# TYPE crci_peer_messages counter\n");
            for (id, state) in peers.iter() {
                out.push_str(&format!(
                    "crci_peer_messages{{node_id=\"{}\",zone=\"{}\"}} {}\n",
                    id, state.zone, state.messages
                ));
            }

            out.push_str("# HELP crci_peer_rescues Peer rescues count\n");
            out.push_str("# TYPE crci_peer_rescues counter\n");
            for (id, state) in peers.iter() {
                out.push_str(&format!(
                    "crci_peer_rescues{{node_id=\"{}\",zone=\"{}\"}} {}\n",
                    id, state.zone, state.rescues
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_format() {
        let metrics = CrciMetrics::new();
        metrics.messages_accepted.store(5, Ordering::Relaxed);
        metrics.byzantine_detected.store(2, Ordering::Relaxed);

        let out = metrics.to_prometheus();

        assert!(out.contains("crci_messages_accepted 5"));
        assert!(out.contains("# TYPE crci_messages_accepted counter"));

        assert!(out.contains("crci_byzantine_detected 2"));
        assert!(out.contains("# TYPE crci_byzantine_detected counter"));
    }
}
