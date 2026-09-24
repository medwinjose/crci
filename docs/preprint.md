# CRCI: Local Reputation-Based Fault Isolation for Ad-Hoc Gossip Communication

**Medwin Jose**

Department of Computer Science and Engineering (SCOPE), Vellore Institute of Technology, Bhopal, India

---

## Abstract

Infrastructure failure during disasters eliminates the connectivity upon which emergency coordination depends. Ad-hoc mesh networking addresses physical-layer reachability but provides limited protection against Byzantine faults—nodes that inject false reports, suppress messages, or broadcast inconsistent state to partition the network. We present CRCI (Crisis Response Communication Infrastructure), a peer-to-peer mesh protocol implemented in Rust that integrates local misbehaving-peer isolation directly into the gossip layer. Each node maintains a local, continuously updated reputation score for every peer, derived from authenticated and protocol-verifiable behavioral evidence, specifically the transmission of invalid signatures or explicitly malicious direct message replays. The system enforces local reputation-based peer eviction using a deterministic three-strike threshold rule, independently isolating malicious nodes without requiring synchronized voting rounds or stable membership. An unevaluated prototype application, Emergency Decision Architecture (EDA), demonstrates how this verified state could support rule-based triage, automatically escalating zones with concentrated rescue requests and flagging suspected misinformation based on severity state-flips. Across 50 repeated benchmark runs on a loopback testbed, CRCI detects and evicts a replaying attacker after exactly three strikes without requiring voting rounds, achieving 50/50 survivability for honest peers that send fresh sequence numbers (corresponding to a two-sided 95% Clopper–Pearson upper bound of approximately 7.1% on the failure rate). These results provide empirical evidence that the proposed reputation-based fault-isolation mechanism can operate through fully local decisions, supporting the feasibility of local fault isolation as a component of decentralized crisis communication.

**Keywords:** Misbehaving-peer isolation, mesh networking, crisis communication, reputation systems, gossip protocol, disaster response, peer-to-peer systems, Ed25519 digital signatures

---

## 1. Introduction

Modern emergency coordination relies heavily on centralized telecommunications infrastructure that is among the first to fail during large-scale disasters. In 2005, Hurricane Katrina disrupted service for more than 3 million customer lines, leaving first responders unable to coordinate across affected zones [1]. The 2023 Turkey–Syria earthquakes produced similarly catastrophic blackouts, significantly delaying search-and-rescue operations [2]. Communication systems that depend on centralized infrastructure can lose service when that infrastructure is damaged or disconnected.

Ad-hoc mesh networking offers a structural alternative by allowing devices to communicate directly. However, mesh networks in crisis environments face Byzantine faults. A Byzantine node can inject fabricated emergency reports to divert scarce rescue resources, suppress legitimate messages, or broadcast inconsistent state.

Several systems address portions of this problem space. Meshtastic [3] provides long-range mesh communication over LoRa but offers no application-layer defense against Byzantine behavior. Briar [4] enables secure peer-to-peer messaging with end-to-end encryption via Tor onion services, Bluetooth, and Wi-Fi, targeting journalist safety rather than multi-party crisis coordination. PBFT [5] assumes a known replica set and coordinated communication rounds, making its consensus model fundamentally different from CRCI's local isolation mechanism. Reputation approaches like EigenTrust [6] compute global trust scores through iterative convergence, requiring network-wide communication rounds that are impractical when connectivity is intermittent. PeerReview [7], CONFIDANT [8], and Watchdog [16] address accountability or routing misbehavior detection through mechanisms that differ from CRCI's strictly local reputation and eviction model.

We present CRCI, a peer-to-peer mesh communication protocol that integrates local misbehaving-peer isolation directly into the gossip layer. Each node maintains a purely local reputation score for every peer, updated continuously from authenticated and protocol-verifiable behavioral evidence. CRCI binds this evidence to authenticated Ed25519 identities [9], preventing identity spoofing. The system enforces local reputation-based peer eviction via a threshold penalty system, isolating malicious nodes without requiring synchronized voting rounds or any form of global coordination. Built on top of this trust layer, an integrated Emergency Decision Architecture (EDA) prototype provides rule-based local triage logic. Existing decentralized communication systems address connectivity, confidentiality, routing, or accountability, while Byzantine consensus protocols address globally coordinated state agreement. There remains a narrower design space between these objectives: lightweight, locally enforceable isolation of demonstrably malformed or replayed traffic without requiring synchronized membership or consensus. CRCI investigates this local fault-isolation problem.

The contributions of this paper are as follows:
1. We propose a reputation-based misbehaving-peer isolation mechanism that operates as a fully local, asynchronous decision at each node.
2. We implement the proposed mechanism as a Rust-based gossip stack with authenticated message processing, local peer state, and deterministic reputation-based eviction.
3. We introduce EDA, a prototype application-layer architecture that leverages verified state to provide rule-based zone escalation and misinformation flagging.
4. We evaluate the system across independent benchmark trials, demonstrating eviction after exactly three strikes with zero false evictions of honest peers sending fresh sequence numbers.

The remainder of this paper is organized as follows. Section 2 reviews related work. Section 3 describes the methodology. Section 4 presents experimental results. Section 5 discusses implications, limitations, and ethical considerations. Section 6 details code availability and disclosures, and Section 7 concludes with directions for future work.

## 2. Related Work

### 2.1. Ad-Hoc and Delay-Tolerant Networking
Early approaches to network fragmentation focused on routing and reliable delivery. Epidemic algorithms (gossip protocols) [10] and bimodal multicast techniques [18] established that randomized pairwise communication could efficiently achieve eventual consistency. Early routing protocols for partially-connected environments [19] laid the groundwork for the formal Delay-Tolerant Networking (DTN) architecture [11], which introduced a store-and-forward overlay to handle intermittent connectivity [23]. Modern applications include Meshtastic [3], Briar [4], and Bridgefy, whose revised protocol was shown to retain vulnerabilities including user impersonation and denial-of-service attacks [25]. Meshtastic provides decentralized LoRa-based communication, but its documented architecture does not target the specific local Byzantine peer-isolation mechanism studied here. Briar targets journalist safety rather than multi-party crisis coordination. Generally, these systems assume benign network failures rather than actively adversarial Byzantine actors.

### 2.2. Byzantine Fault Tolerance (BFT)
The Byzantine Generals Problem [12] established theoretical foundations for consensus in the presence of malicious actors. PBFT [5] demonstrated state-machine replication could survive Byzantine faults, but classical approaches and even modern asynchronous variants like HoneyBadgerBFT [17] rely on a stable, known set of replica nodes or heavy voting phases.

### 2.3. Reputation Management in Peer-to-Peer Networks
EigenTrust [6] calculates a global trust value by aggregating local transaction histories using an iterative distributed algorithm. CONFIDANT [8], building on foundational mitigation strategies like Watchdog [16], detects routing misbehavior in mobile ad-hoc networks through a localized neighborhood watch mechanism.

### 2.4. How CRCI Differs
CRCI integrates a local misbehaving-peer isolation mechanism directly into an epidemic gossip protocol. Unlike PBFT, CRCI does not attempt to reach globally synchronized consensus. Unlike federated permissionless consensus models (e.g., Stellar SCP [21] or Ripple [22]) that rely on overlapping quorum slices, CRCI evaluates reputation strictly locally without any distributed voting. By binding reputation metrics to authenticated identities, CRCI enforces a dynamic fault-isolation threshold via a "3-strikes" penalty system without synchronous voting rounds.

## 3. Methodology

### 3.1. System Overview
CRCI operates as an overlay mesh network utilizing a gossip dissemination protocol. Nodes communicate via peer-to-peer transport. The Rust NodeRuntime maintains an inbox, outbox, and local state ledger. The system does not elect leaders or perform synchronized voting. Each node independently evaluates incoming messages and updates local reputation scores.

### 3.2. Threat Model
CRCI isolates adversaries within a local observation scope. We assume an adversary can replay messages, generate validly signed false reports, and flood the network. We assume the adversary cannot forge signatures corresponding to uncompromised public keys.

CRCI's eviction mechanism only bans for signature invalidation or repeated resends of the same message ID by one neighbor. Flooding is dropped by a token bucket rate limiter but does not trigger a ban, and validly signed false reports are only flagged as misinformation without causing an eviction. Additionally, because the protocol operates without a global PKI, a banned adversary can reset their ban by generating a fresh cryptographic identity.

Out-of-Scope: This model explicitly does not address Sybil attacks [13, 14, 15] (where an adversary generates unbounded cryptographic identities), coordinated collusion among multiple Byzantine nodes, or networks with a majority-Byzantine population. These attacks generally require additional identity-management or Sybil-resistance mechanisms, such as social-graph approaches [14, 15] or centralized PKI, which fall outside the strictly local scope of CRCI.

### 3.3. Cryptographic Primitives
Every node is identified by a public key (Ed25519). All gossiped messages are signed. Local persistent state is encrypted at rest (AES-256-GCM), and master keys are derived using Argon2id.

### 3.4. State Integrity and Replay Protection
A Byzantine adversary may replay old messages to exhaust resources or confuse the state. CRCI enforces bounded sequence freshness using a sliding-window mechanism. For each cryptographically verified author, the node tracks the maximum observed sequence number $\text{seq}_{\text{max}}$. An incoming message with sequence $\text{seq}$ is accepted if and only if:

1. $\text{seq} > \text{seq}_{\text{max}} - W$, where $W=64$ is the window size, and
2. $\text{seq}$ has not been previously recorded within the current window.

Out-of-order packets common in wireless routing are tolerated within $W$, while severely stale messages ($\text{seq} \le \text{seq}_{\text{max}} - W$) are rejected. Exact duplicates arriving from different transport peers are silently deduplicated to support normal gossip redundancy without penalizing honest relays; repeated delivery of the same message ID by the same transport peer is handled separately by the malicious-replay detector described in Section 3.5.

### 3.5. Local Misbehaving-Peer Isolation Mechanism
CRCI enforces fault-isolation directly in the gossip layer by separating transport-peer attribution from author-layer authentication. The immediate relaying neighbor is accountable for maliciously replayed traffic or invalid signatures, while the cryptographically claimed originator is used for signature and sequence verification. Reputation starts at $0.5$ for all peers.

When processing an incoming message, CRCI enforces the following strictly ordered checks:
1. Token-bucket rate limiting (dropping excess traffic).
2. Author-layer cryptographic signature verification (penalizing invalid signatures).
3. Silent deduplication (ignoring normal gossip redundancy from different paths or severely stale messages).
4. Explicitly malicious replay detection.

Specifically, an ID first received from neighbor $j$ and received again from $j$ is a strike; the same ID arriving from a different neighbor is treated as normal gossip redundancy and ignored. If a neighbor transmits a malicious replay or forwards an invalid signature, CRCI applies an explicitly malicious penalty ($p = -0.15$) against the transport-layer peer.

If a peer's reputation drops below the minimum acceptable threshold $\theta_{\text{ban}} = 0.1$, the peer is banned:
$$R_{i,j} < \theta_{\text{ban}} \implies \text{Ban}(j)$$

With the chosen initialization, penalty, and threshold parameters ($\theta_{\text{ban}} = 0.1$), the mechanism induces a three-strike eviction condition:
$$0.5 - 3(0.15) = 0.05 < 0.1$$

After 3 explicitly malicious acts attributable to the same transport peer, that peer identity is banned locally and subsequent connections using that identity are rejected. The transport layer tracks already_sent IDs per neighbor to prevent honest periodic re-gossiping from triggering replay strikes. Peers whose reputation is diminished but not banned slowly recover reputation over time through a decay_recover mechanism (e.g., $+0.05$ per hour). Rate limit violations result in dropped messages without reputation penalties.

### 3.6. Demonstration Application
EDA is included only as an architectural demonstration of how locally verified state could be consumed by an application layer; it is not evaluated and does not constitute a contribution to automated disaster triage. It operates locally on the ledger:
- **Zone Escalation:** High density of rescue requests ($\ge 3$ active requests within a 5-minute sliding window) from a specific zone triggers a `ZoneEscalated` flag.
- **Conflicting-State Flagging:** If reports from a source exhibit repeated severity-state transitions (e.g., severity rapidly flipping $\ge 3$ times within a 10-minute window), EDA tags the source with `MisinformationSuspect`, warning users without deleting potentially life-saving data. Misinformation tagging does not ban the peer, leaving human operators to evaluate the contested data.

## 4. Controlled Fault-Isolation Validation

### 4.1. Experimental Setup
The evaluation was conducted on a local loopback testbed utilizing a 3-node topology: an honest listener (Node A), an honest peer (Node H), and a Byzantine adversary (Node B). Node H periodically broadcasts legitimate sequence-incrementing telemetry, serving as the ground-truth control to verify honest-peer survivability. Simultaneously, the Byzantine adversary mounted a continuous replay attack against the honest listener by transmitting the exact same previously processed payload three times. The harness sends a total of four identical messages; the first is accepted normally, and the subsequent three identical replays trigger the three strikes required for eviction.

### 4.2. Eviction Latency and Survivability
We conducted $N=50$ independent trials under both baseline and a fixed 15 ms inter-message delay injected before each of the 3 replayed messages. The system successfully isolated the attacker after exactly 3 strikes in all trials. Because the procedure is deterministic, the 50 trials primarily serve to verify harness execution consistency. The results are summarized in Table 1.

| Condition | Strikes to Ban | Eviction Latency (Mean) | Min / Max Latency | Honest Survival Rate |
| :--- | :--- | :--- | :--- | :--- |
| Baseline | 3 | $< 1$ ms | $< 1$ ms | 50/50 |
| Fixed 15 ms Delay | 3 | 45.8 ms | 45 ms / 46 ms | 50/50 |

*Table 1: Strikes to eviction and processing latency across 50 trials.*

The deterministic 3-strike requirement forms the core eviction logic. The observed wall-clock latency (e.g., 45.8 ms with fixed delay) is dominated by the experimental injected delays rather than protocol overhead. The logical eviction condition is determined by the three-strike threshold, while observed wall-clock latency depends on message arrival and processing conditions.

### 4.3. Sliding-Window vs. Strict-Monotonic Rejection
We compared CRCI's sliding-window replay policy ($W=64$) against a naive strict-monotonic rule (`seq <= last_seq`). Both policies were evaluated over 50 trials using the same telemetry workload, reputation parameters, and controlled packet-reordering pattern (depth of 2 packets) for the honest peer:
- **Strict-Monotonic Baseline:** The out-of-order packet was immediately classified as a replay, resulting in a false-positive penalty against the honest peer. Repeated reordering led directly to false-positive bans (Honest Survival = 0/50), as expected by construction because three reordered packets trigger a ban under the strict-monotonic policy.
- **CRCI Sliding Window:** The out-of-order packet was correctly accepted. No penalty or eviction was observed for the honest peer (Honest Survival = 50/50).

## 5. Discussion, Limitations, and Ethical Considerations

### 5.1. Discussion
Under the evaluated replay and packet-reordering workloads, the results demonstrate that the local fault-isolation mechanism can perform deterministic peer isolation under the evaluated replay-attack conditions. By utilizing a strictly local computation, CRCI avoids the multi-phase communication and membership assumptions used by traditional BFT protocols. CRCI provides local isolation but offers no global consensus or safety guarantees.

### 5.2. Limitations
The experimental design evaluates a minimal $N=3$ topology against a single continuous replay attack vector, leaving larger and heterogeneous topologies untested. The evaluation does not establish network scalability overhead. The loopback evaluation does not capture radio interference, packet loss, MAC contention, asymmetric links, or duty-cycling constraints of LoRa [20]. The current implementation does not differentiate between malicious replays and honest periodic gossip re-sends or partition-heal syncing, potentially exposing honest peers to unintended bans. Additionally, while invalid signatures incur strikes, this vector was not empirically evaluated. Furthermore, validly signed false reports remain undetected by the reputation system. Finally, an adaptive attacker could pace their attacks to match the `decay_recover` function (e.g., $+0.05$ per hour), thus never reaching the ban threshold. This evasion tactic, along with the ability for banned identities to reset with a fresh key, highlights the limits of a purely local reputation model.

### 5.3. Ethical Considerations
EDA’s conflicting-state flagging strictly tags data (`MisinformationSuspect`) rather than silently discarding it. Ground truth in a disaster is inherently ambiguous, and automatic suppression of disputed reports could remove information that is subsequently determined to be valid; EDA is designed so that operators can review tagged reports and retain final judgment.

## 6. Data, Code Availability, and AI-Use Disclosure
The complete source code for the CRCI Rust implementation, EDA engine, and the benchmark harnesses used to generate the results in this paper are available open-source at [https://github.com/medwinjose/crci](https://github.com/medwinjose/crci). The benchmark harnesses and scripts in the repository reproduce the results.

**AI-Use Disclosure:** The author employed an AI coding assistant (Google Antigravity) to assist with drafting, code formatting, generating sliding-window logic, running benchmark scripts, and functioning as a review assistant for critique rounds. The author has personally reviewed, verified, and takes full responsibility for all code, methodology, and empirical claims in this manuscript.

## 7. Conclusion and Future Work
CRCI demonstrates a local fault-isolation mechanism for crisis communication networks that does not require fixed infrastructure or synchronized voting phases. Future work will focus on large-scale physical testing over LoRa and BLE to characterize MAC-layer collisions, and on invalid signature attacks, Sybil-resistant identity binding, and adaptive attack pacing.

## References

[1] Federal Communications Commission (FCC), "Recommendations of the Independent Panel Reviewing the Impact of Hurricane Katrina on Communications Networks," June 2006.

[2] United Nations Office for the Coordination of Humanitarian Affairs (UN OCHA), "Türkiye/Syria: Earthquakes - Flash Updates," February 2023.

[3] Meshtastic, "Meshtastic: Open Source LoRa Mesh," [Online]. Available: https://meshtastic.org.

[4] Briar Project, "Briar: Secure Messaging, Anywhere," [Online]. Available: https://briarproject.org.

[5] M. Castro and B. Liskov, "Practical Byzantine Fault Tolerance," in *Proceedings of the Third Symposium on Operating Systems Design and Implementation (OSDI)*, pp. 173–186, 1999.

[6] S. D. Kamvar, M. T. Schlosser, and H. Garcia-Molina, "The EigenTrust Algorithm for Reputation Management in P2P Networks," in *Proceedings of the 12th International Conference on World Wide Web*, pp. 640–651, 2003.

[7] A. Haeberlen et al., "PeerReview: Practical Accountability for Distributed Systems," in *Proceedings of the 21st ACM Symposium on Operating Systems Principles (SOSP)*, pp. 298–311, 2007.

[8] S. Buchegger and J.-Y. Le Boudec, "Performance Analysis of the CONFIDANT Protocol," in *Proceedings of the 3rd ACM International Symposium on Mobile Ad Hoc Networking & Computing (MobiHoc)*, pp. 226–236, 2002.

[9] D. J. Bernstein, N. Duif, T. Lange, P. Schwabe, and B. Yang, "High-speed high-security signatures," *Journal of Cryptographic Engineering*, vol. 2, pp. 77–89, 2012.

[10] A. Demers et al., "Epidemic Algorithms for Replicated Database Maintenance," in *PODC*, 1987.

[11] V. Cerf et al., "Delay-Tolerant Networking Architecture," *RFC 4838*, IETF, 2007.

[12] L. Lamport, R. Shostak, and M. Pease, "The Byzantine Generals Problem," *ACM Transactions on Programming Languages and Systems*, vol. 4, no. 3, pp. 382–401, 1982.

[13] J. R. Douceur, "The Sybil Attack," in *International Workshop on Peer-to-Peer Systems (IPTPS)*, Springer, pp. 251–260, 2002.

[14] H. Yu, M. Kaminsky, P. B. Gibbons, and A. Flaxman, "SybilGuard: Defending Against Sybil Attacks via Social Networks," in *Proceedings of the 2006 conference on Applications, technologies, architectures, and protocols for computer communications (SIGCOMM)*, pp. 267–278, 2006.

[15] H. Yu, P. B. Gibbons, M. Kaminsky, and F. Xiao, "SybilLimit: A Near-Optimal Social Network Defense against Sybil Attacks," *IEEE/ACM Transactions on Networking*, vol. 18, no. 3, pp. 885–898, 2010.

[16] S. Marti, T. J. Giuli, K. Lai, and M. Baker, "Mitigating routing misbehavior in mobile ad hoc networks," in *Proceedings of the 6th annual international conference on Mobile computing and networking (MobiCom)*, pp. 255–265, 2000.

[17] A. Miller, Y. Xia, K. Croman, E. Shi, and D. Song, "The Honey Badger of BFT Protocols," in *Proceedings of the 2016 ACM SIGSAC Conference on Computer and Communications Security (CCS)*, pp. 31–42, 2016.

[18] K. P. Birman, M. Hayden, O. Ozkasap, Z. Xiao, M. Budiu, and Y. Minsky, "Bimodal Multicast," *ACM Transactions on Computer Systems (TOCS)*, vol. 17, no. 2, pp. 41–88, 1999.

[19] A. Vahdat and D. Becker, "Epidemic Routing for Partially-Connected Ad Hoc Networks," Technical Report CS-2000-06, Duke University, 2000.

[20] F. Adelantado, X. Vilajosana, P. Tuset-Peiro, B. Martinez, J. Melia-Segui, and T. Watteyne, "Understanding the Limits of LoRaWAN," *IEEE Communications Magazine*, vol. 55, no. 9, pp. 34–40, 2017.

[21] D. Mazières, "The Stellar Consensus Protocol: A Federated Model for Internet-level Consensus," Stellar Development Foundation Web, 2015.

[22] D. Schwartz, N. Youngs, and A. Britto, "The Ripple Protocol Consensus Algorithm," Ripple Labs Inc Whitepaper, 2014.

[23] M. J. Khabbaz, C. M. Assi, and W. F. Fawaz, "Disruption-Tolerant Networking: A Comprehensive Survey on Recent Developments and Persisting Challenges," *IEEE Communications Surveys & Tutorials*, vol. 14, no. 2, pp. 607–640, 2012.

[24] M. Baert, J. Rossey, A. Shahid, and J. Hoebeke, "The Bluetooth Mesh Standard: An Overview and Experimental Evaluation," *Sensors*, vol. 18, no. 8, p. 2409, 2018.

[25] M. R. Albrecht, R. Eikenberg, and K. G. Paterson, "Breaking Bridgefy, again: Adopting libsignal is not enough," in *Proceedings of the 31st USENIX Security Symposium (USENIX Security 22)*, pp. 269–286, 2022.
