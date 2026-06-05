# ADR 003: Merkle-Chained State over CRDTs

## Date: 2026-06-05
## Status: Accepted

## Context
In a highly partitioned, mobile mesh network, data must be eventually consistent across disconnected segments. Conflict-Free Replicated Data Types (CRDTs) are the standard solution for decentralized state synchronization, as they allow concurrent updates without coordination and mathematically guarantee conflict resolution upon merging.

However, CRCI operates in a hostile, Byzantine environment. We must prove the provenance and sequence of critical events (like rescue requests and hazard reports). If an adversarial node tampers with data or attempts a replay attack, the network needs an auditable trail to prove the manipulation and apply reputation penalties. CRDTs focus on state convergence, but they inherently discard the strict causal ordering and cryptographic history required for an adversarial audit.

## Decision
We decided to use a Merkle-Chained Signed State history instead of CRDTs. 

Each critical state update is cryptographically signed, assigned a Lamport sequence number, and linked to the previous state via a SHA3-256 hash, forming a local, tamper-evident blockchain for each node.

## Tradeoffs
**Lost:**
- Easy state merges: CRDTs merge automatically. Merkle chains require replaying the missing history when a network partition heals.
- Storage efficiency: Maintaining the full cryptographic chain is more memory-intensive than a compacted CRDT state map.

**Gained:**
- Cryptographic Auditability: Every state transition is mathematically bound to the originator's signature. Adversaries cannot rewrite history or inject forged past events without invalidating the chain.
- Accountability: The strict ordering provides the necessary evidence for the reputation engine to securely penalize Byzantine behavior, which is impossible with standard CRDT convergence.
