# CRCI Fault Model

This document outlines the failure modes and Byzantine attacks that the CRCI mesh network is designed to tolerate, along with the detection mechanisms and system response.

## 1. Crash Faults
**Description:** A node abruptly stops responding or powering on (e.g., destroyed hardware, dead battery, complete physical obstruction).
**CRCI Response:** Crash faults are detected via a lack of heartbeat/beacon messages over `N` consecutive rounds. The network removes the crashed node from the active peer set. Any messages uniquely held by the node are adopted by peers through the local replica exchange protocol before the crash, ensuring persistence.

## 2. Byzantine Fault Type 1 — Selective Gossip Poisoning
**Description:** An adversarial node sends valid state to some peers while intentionally sending invalid or corrupted state to others.
**CRCI Detection & Response:** Honest nodes cross-reference state hashes during peer exchange. Disagreement on the state hash triggers a reputation penalty. If the selective gossiper continues, its reputation drops below 0.5, and its subsequent messages and vouches are ignored.

## 3. Byzantine Fault Type 2 — Inconsistent State Broadcast
**Description:** A node broadcasts completely different state hashes to different peers for the exact same logical state sequence, attempting to partition the consensus.
**CRCI Detection & Response:** Quorum hash comparison quickly identifies the conflicting broadcasts. Honest nodes penalize the sender for protocol deviation. The reputation engine isolates the node, preventing it from successfully partitioning the network.

## 4. Byzantine Fault Type 3 — Delayed Message Injection
**Description:** A malicious node holds onto a valid emergency or normal message, intentionally delaying it, and injects it later after the network has already achieved partial convergence on a conflicting state.
**CRCI Detection & Response:** The system uses strict Lamport sequence number validation and Time-to-Live (TTL) expiries. Delayed messages with obsolete sequence numbers are immediately dropped by the replay filter. Expired messages are pruned and ignored, nullifying the delayed injection attack.

## Safety Threshold
**Threshold:** `< 1/3` Byzantine nodes by reputation weight.
As long as the total reputation weight of malicious nodes remains under 33%, the network guarantees safety and liveness. 
**Above Threshold:** If the Byzantine weight exceeds 1/3, honest nodes gracefully degrade by forming their own partition to maintain internal consistent state, rather than capitulating to the adversary.

## Out of Scope
The following physical and cryptographic layer attacks are outside the current threat model:
- **Sybil attacks at scale:** (Requires physical hardware limits or external identity binding)
- **Physical layer jamming:** (Wideband radio frequency jamming)
- **Side-channel attacks:** (Timing or power analysis on Ed25519 keypair operations)
