# CRCI Threat Model (STRIDE)

This document provides a formal STRIDE-based threat analysis of the CRCI peer-to-peer network. It evaluates adversarial capabilities and the specific cryptographic or architectural mechanisms CRCI uses to mitigate them.

## 1. Spoofing
**Threat:** An adversary attempts to impersonate another legitimate node on the network to inject false telemetry or sabotage the victim's reputation.
**Mitigation:** 
- **Identity Keys:** Every CRCI node identity is bound to an Ed25519 public key.
- **Message Signing:** The `rust-libp2p` network layer requires all gossip payloads to be cryptographically signed by the originating node's private key. 
- **Peer Handshake:** Connections use the Noise protocol framework, ensuring that any peer claiming a specific `PeerId` must prove ownership of the corresponding private key during the initial handshake, completely preventing spoofing of in-flight messages or peer identities.

## 2. Tampering
**Threat:** An adversary or man-in-the-middle (MITM) intercepts and modifies in-flight state transitions or gossip messages before they reach the rest of the network.
**Mitigation:**
- **Encrypted Transport:** The Noise protocol handshake establishes an encrypted, authenticated tunnel for all TCP traffic, preventing MITM tampering.
- **Payload Verification:** Even if a malicious node receives a valid message, tampers with the payload, and re-gossips it, the cryptographic signature validation performed by the receiving peer's local gossip validation layer will fail, causing the tampered message to be dropped and the malicious forwarder to incur a reputation penalty.

## 3. Repudiation
**Threat:** A malicious node sends a destructive or invalid message and later denies having sent it to avoid a reputation penalty.
**Mitigation:**
- **Cryptographic Non-Repudiation:** Because all payloads are signed with Ed25519 keys, the signature itself serves as unforgeable proof of origin. If node A sends an invalid payload to node B, node B can cryptographically verify the authorship and locally penalize the transport peer that delivered it.

## 4. Information Disclosure
**Threat:** An unauthorized entity observes network traffic to discover the topology or read the contents of disaster telemetry (e.g., location data).
**Mitigation:**
- **In-Flight Encryption:** All point-to-point connections are encrypted.
- **Data at Rest:** Persistent state is stored locally using an encrypted RocksDB instance.
- *Note:* While the payload contents are protected from passive external observers, CRCI is a permissioned-capable P2P network. Any node that successfully completes the handshake and joins the mesh can decrypt the gossip payloads.

## 5. Denial of Service (DoS)
**Threat:** A malicious node floods the network with garbage messages, high-frequency state updates, or replay attacks to exhaust network bandwidth and CPU resources.
**Mitigation:**
- **Replay Protection:** The system enforces strict sequence number validation using a sliding-window mechanism. Replayed messages are dropped immediately and explicitly penalize the transmitting neighbor.
- **Reputation-based Eviction:** Nodes that spam invalid messages or flood the mempool are dynamically penalized by the reputation engine. Once a node's reputation drops below the threshold, honest peers sever the TCP connection and drop all future packets from that `PeerId`.
- **TTL Expiry:** Messages carry a Time-to-Live (TTL). The `rust-libp2p` gossipsub implementation prunes expired messages from the network to prevent infinite routing loops.

## 6. Elevation of Privilege
**Threat:** A standard node attempts to act as an administrator, forcing state rollbacks or dictating state to the network.
**Mitigation:**
- **Decentralized Validation:** CRCI lacks a central authority or "admin" role by design. The validation rules are symmetric and strictly local. A node cannot elevate its privilege because the network does not rely on global state consensus. A single node attempting to dictate state by flooding malformed traffic will simply be ignored and isolated locally by the honest peers it connects to.
