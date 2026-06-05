# ADR 002: Reputation-Weighted Quorum over PBFT

## Date: 2026-06-05
## Status: Accepted

## Context
Byzantine Fault Tolerance (BFT) systems must reach consensus in the presence of malicious or failing actors. The classic Practical Byzantine Fault Tolerance (PBFT) algorithm guarantees safety and liveness provided less than 1/3 of the nodes are malicious.

However, PBFT requires a multi-phase commit protocol (pre-prepare, prepare, commit) that necessitates O(n²) messages exchanged per round. In infrastructure-denied environments running on low-bandwidth radios like LoRa (often < 50 kbps), a 40-node network generating 1,600 messages per consensus round would immediately saturate the spectrum and collapse the network.

## Decision
We decided to implement a localized, Reputation-Weighted Quorum instead of full global PBFT. 

In this system, nodes independently observe the behavior of their peers (e.g., monitoring the median severity reported for a zone). Peers that deviate significantly from the consensus are penalized locally, lowering their reputation score. When aggregating state (like declaring a Mass Casualty Event), the system tallies the *reputation weight* of reporting nodes, not a strict majority vote. 

## Tradeoffs
**Lost:**
- Formal mathematical safety proofs guarantee that all honest nodes agree on a perfectly ordered sequence of events.
- Absolute global consistency in real-time.

**Gained:**
- Bandwidth feasibility: The system relies entirely on O(n) asynchronous gossip, making it viable for LoRa and BLE mesh networks.
- Partition tolerance: Nodes can independently penalize malicious actors and continue functioning even when isolated from the wider network.

## Threshold Maintained
Despite dropping PBFT, we maintain the BFT security threshold constraint: the system requires 2/3 reputation weight agreement to trust critical decisions. If the collective reputation weight of malicious actors exceeds 1/3, the network gracefully degrades, allowing honest nodes to form their own partition rather than capitulating to the adversary.
