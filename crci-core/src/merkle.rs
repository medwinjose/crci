use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A snapshot of the subset of node state that matters for integrity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateSnapshot {
    pub peer_count: usize,
    pub reputation_floor: f64,
    pub messages_handled: u64,
    pub byzantine_events: u64,
    pub timestamp_ms: u64,
}

/// One link in the Merkle chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleBlock {
    pub sequence: u64,
    pub prev_hash: [u8; 32],
    pub snapshot: StateSnapshot,
    pub block_hash: [u8; 32],
}

/// The append-only chain kept by a node.
pub struct MerkleChain {
    blocks: Vec<MerkleBlock>,
}

impl MerkleChain {
    pub fn new() -> Self {
        let snapshot = StateSnapshot {
            peer_count: 0,
            reputation_floor: 0.0,
            messages_handled: 0,
            byzantine_events: 0,
            timestamp_ms: 0,
        };
        let prev_hash = [0; 32];
        let sequence = 0;
        let block_hash = Self::hash_block(sequence, &prev_hash, &snapshot);

        let genesis_block = MerkleBlock {
            sequence,
            prev_hash,
            snapshot,
            block_hash,
        };

        MerkleChain {
            blocks: vec![genesis_block],
        }
    }

    fn hash_block(sequence: u64, prev_hash: &[u8; 32], snapshot: &StateSnapshot) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(sequence.to_le_bytes());
        hasher.update(prev_hash);
        let snapshot_bytes = serde_json::to_vec(snapshot).unwrap();
        hasher.update(snapshot_bytes);
        hasher.finalize().into()
    }

    pub fn append(&mut self, snapshot: StateSnapshot) -> &MerkleBlock {
        let prev_block = self.blocks.last().unwrap();
        let sequence = prev_block.sequence + 1;
        let prev_hash = prev_block.block_hash;
        let block_hash = Self::hash_block(sequence, &prev_hash, &snapshot);

        let block = MerkleBlock {
            sequence,
            prev_hash,
            snapshot,
            block_hash,
        };

        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    pub fn head_hash(&self) -> [u8; 32] {
        self.blocks.last().unwrap().block_hash
    }

    pub fn head_sequence(&self) -> u64 {
        self.blocks.last().unwrap().sequence
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn verify_integrity(&self) -> bool {
        let mut expected_prev_hash = [0; 32];

        for block in &self.blocks {
            if block.prev_hash != expected_prev_hash {
                return false;
            }

            let expected_block_hash =
                Self::hash_block(block.sequence, &block.prev_hash, &block.snapshot);
            if block.block_hash != expected_block_hash {
                return false;
            }

            expected_prev_hash = block.block_hash;
        }

        true
    }

    pub fn diverges_from(&self, other_head: [u8; 32], other_seq: u64) -> bool {
        if other_seq > self.head_sequence() {
            // They are ahead of us, we can't tell if we diverged or just haven't caught up
            return false;
        }

        let idx = other_seq as usize;
        if idx >= self.blocks.len() {
            return false;
        }

        self.blocks[idx].block_hash != other_head
    }

    pub fn build_divergence_alert(
        &self,
        peer_id: &str,
        their_head_hash: [u8; 32],
        their_head_seq: u64,
    ) -> Option<DivergenceAlert> {
        if their_head_seq > self.head_sequence() {
            return None;
        }

        if self.diverges_from(their_head_hash, their_head_seq) {
            let idx = their_head_seq as usize;
            if idx < self.blocks.len() {
                let our_hash = self.blocks[idx].block_hash;
                let expected_head_hash = our_hash
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                let actual_head_hash = their_head_hash
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                return Some(DivergenceAlert {
                    peer_id: peer_id.to_string(),
                    expected_head_hash,
                    actual_head_hash,
                    divergence_seq: their_head_seq,
                    detected_at_ms: crate::message::now_ts(),
                });
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivergenceAlert {
    pub peer_id: String,
    pub expected_head_hash: String,
    pub actual_head_hash: String,
    pub divergence_seq: u64,
    pub detected_at_ms: u64,
}

impl Default for MerkleChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_block_is_valid() {
        let chain = MerkleChain::new();
        assert_eq!(chain.len(), 1);
        assert!(chain.verify_integrity());
        assert_eq!(chain.head_sequence(), 0);
    }

    #[test]
    fn append_and_verify() {
        let mut chain = MerkleChain::new();
        for i in 1..=10 {
            let snapshot = StateSnapshot {
                peer_count: i as usize,
                reputation_floor: 1.0,
                messages_handled: i * 10,
                byzantine_events: 0,
                timestamp_ms: 1000 + i,
            };
            chain.append(snapshot);
        }
        assert_eq!(chain.len(), 11); // genesis + 10 blocks
        assert!(chain.verify_integrity());
        assert_eq!(chain.head_sequence(), 10);
    }

    #[test]
    fn tamper_detection() {
        let mut chain = MerkleChain::new();
        for i in 1..=5 {
            let snapshot = StateSnapshot {
                peer_count: i as usize,
                reputation_floor: 1.0,
                messages_handled: i * 10,
                byzantine_events: 0,
                timestamp_ms: 1000 + i,
            };
            chain.append(snapshot);
        }

        // Tamper with block 2 (index 2)
        chain.blocks[2].snapshot.peer_count += 1;
        assert!(!chain.verify_integrity());
    }

    #[test]
    fn divergence_detection() {
        let mut chain_a = MerkleChain::new();
        let mut chain_b = MerkleChain::new(); // Same genesis

        let snapshot1 = StateSnapshot {
            peer_count: 1,
            reputation_floor: 1.0,
            messages_handled: 10,
            byzantine_events: 0,
            timestamp_ms: 1001,
        };
        chain_a.append(snapshot1.clone());
        chain_b.append(snapshot1);

        assert!(!chain_a.diverges_from(chain_b.head_hash(), chain_b.head_sequence()));

        // Now append different snapshots to each
        let snapshot_a = StateSnapshot {
            peer_count: 2,
            reputation_floor: 1.0,
            messages_handled: 20,
            byzantine_events: 0,
            timestamp_ms: 1002,
        };
        chain_a.append(snapshot_a);

        let snapshot_b = StateSnapshot {
            peer_count: 1,         // Different
            reputation_floor: 0.5, // Different
            messages_handled: 21,  // Different
            byzantine_events: 1,   // Different
            timestamp_ms: 1003,
        };
        chain_b.append(snapshot_b);

        // Chain A should detect divergence when seeing Chain B's head
        assert!(chain_a.diverges_from(chain_b.head_hash(), chain_b.head_sequence()));
        assert!(chain_b.diverges_from(chain_a.head_hash(), chain_a.head_sequence()));
    }
}
