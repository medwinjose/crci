# ADR 003: Hash-Chained State over CRDTs

- **Date:** 2026-06-05
- **Status:** Accepted

## Context

CRDTs give automatic convergence of concurrently updated state, but they deliberately forget the order and
origin of updates. For CRCI, the order and origin of critical events (rescue requests, hazard reports) are
what a node needs in order to audit a neighbour's behaviour.

## Decision

Each node keeps an append-only chain of state snapshots (`crates/crci-core/src/merkle.rs`). Each block
stores a sequence number, the previous block's hash and a snapshot, and is identified by a SHA-256 hash
over those fields. Nodes can compare chain heads to detect divergence (`DivergenceAlert`).

Authenticity comes from the Ed25519 signatures on individual messages, not from the chain: chain blocks
themselves are not signed.

## Consequences

**Lost:**
- Automatic merging. After a partition, missing messages are exchanged by the digest-based anti-entropy
  protocol; the chains themselves are not exchanged or merged (only their heads are compared).
- Compactness: the chain gains a block every 50 delivered messages; in memory only the newest 512 to
  1 023 blocks are kept, so very old heads can no longer be compared.

**Gained:**
- A local, tamper-evident record of what the node has seen, in order.
