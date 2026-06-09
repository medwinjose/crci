# Byzantine Fault Tolerance

In crisis scenarios, networks are susceptible not only to physical partition and degradation but also to malicious actors. Adversaries may attempt to inject false emergency messages, spoof identities, or flood the network with garbage bytes to execute denial-of-service (DoS) attacks. CRCI is engineered from the ground up to operate reliably in these hostile conditions.

## The PBFT Threshold

CRCI employs a localized, reputation-weighted variant of Practical Byzantine Fault Tolerance (PBFT). For the network to reach consensus on the state of a crisis (e.g., escalating a severe event into an automated Major Crisis Event via AEDA), the network must satisfy the Byzantine threshold of `f < n/3`. This means that as long as strictly less than one-third of the participating peer nodes in a localized zone are actively malicious or compromised, the network will correctly converge on the legitimate state and isolate the bad actors. The core safety and liveness properties of this consensus mechanism are formally specified in TLA+. Running the TLC model checker on a bounded model (N=4, F=1) verified these properties across 81 distinct states with zero errors.

## Gossip Mechanics and State Propagation

State is propagated asynchronously via a K-bucket structured gossip protocol. When an emergency message is generated, it is cryptographically signed using the origin node's Ed25519 private key. 

As the message propagates:
1. **Signature Verification**: Every receiving node independently verifies the cryptographic signature before relaying.
2. **Replay Protection**: The sequence number and message ID are checked against an active replay filter. Duplicates are silently dropped.
3. **Reputation Weighting**: The origin's reputation score is applied. Nodes with high reputation have their messages weighted more heavily by the receiving consensus engine, while nodes with zero reputation are entirely ignored.

## Surviving Adversarial Input

A core strength of the CRCI `NodeRuntime` is its resilience against raw, unparseable TCP-level garbage designed to crash the application. 

When a Byzantine peer attempts to connect and transmit malformed data (such as the attacks successfully tested in our automated integration suites):
- The `TcpTransport` cleanly captures the serialization failure at the socket boundary without panicking.
- The invalid connection is immediately dropped.
- The peer's invalid data never reaches the inner gossip pipeline, ensuring that the node's memory and peer tables remain completely uncorrupted.
- Legitimate, concurrent connections remain entirely unaffected. Honest peers continue to exchange data, demonstrating true partition-resilience.

## STRIDE Threat Model Mitigation

CRCI mitigates classical security threats according to the STRIDE model:
- **Spoofing**: Defeated via strict Ed25519 cryptographic identity verification on every message.
- **Tampering**: Blocked by Merkle-chained state hashes and signature invalidation if payloads are modified in transit.
- **Repudiation**: Prevented through secure, append-only local audit logs that track message origins.
- **Information Disclosure**: Mitigated via AES-256-GCM encrypted local storage for sensitive key material and records.
- **Denial of Service**: Handled autonomously by battery-aware rate limiting, token buckets, and aggressive eviction of non-compliant Byzantine peers.
- **Elevation of Privilege**: Stopped natively by the decentralized PBFT consensus; no single node inherently possesses "admin" privileges to override the network state.
