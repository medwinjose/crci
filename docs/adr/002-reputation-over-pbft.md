# ADR 002: Local Reputation over PBFT

- **Date:** 2026-06-05
- **Status:** Accepted

## Context

PBFT tolerates fewer than one third Byzantine replicas, but each agreement round needs three phases with
O(n²) messages and a stable, known membership. A crisis mesh has neither the bandwidth (LoRa links run at
a few kbit/s) nor stable membership.

## Decision

Do not run a consensus protocol. Each node keeps a **local** reputation score per neighbour, lowers it on
locally verifiable misbehaviour (replays, invalid signatures, key changes) and stops accepting traffic from
a neighbour whose score falls below the ban threshold. Dissemination is plain gossip, O(n) per message.

Separately, the demonstration application layer (AEDA and the mesh simulator in `mesh.rs`) aggregates
severity reports per zone, weighted by the reporters' reputation. That aggregation is an application
feature, not an agreement protocol.

## Consequences

**Lost:**
- Any guarantee that honest nodes agree on the same state or order of events.
- Any network-wide safety or liveness threshold: each node's view is its own.

**Gained:**
- No voting rounds and no membership requirement; nodes keep working while partitioned.
- Bandwidth per message is linear in the number of peers.
