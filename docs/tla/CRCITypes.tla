--------------------------- MODULE CRCITypes ---------------------------
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Nodes,
    ByzantineNodes,
    MaxSeq,
    MinTrustedRep

ASSUME ByzantineNodes \subseteq Nodes

MessageKind == {"RESCUE", "NORMAL"}
BatteryTier == {"FULL", "LOW", "CRITICAL"}
RepRange    == 0..10
MsgIds      == {"m1", "m2", "m3", "m4", "m5"}
Zones       == {"zone-a", "zone-b"}

IsValidMessage(m) ==
    /\ "id" \in DOMAIN m /\ m.id \in MsgIds
    /\ "origin" \in DOMAIN m /\ m.origin \in Nodes
    /\ "kind" \in DOMAIN m /\ m.kind \in MessageKind
    /\ "seq" \in DOMAIN m /\ m.seq \in 0..MaxSeq
    /\ "zone" \in DOMAIN m /\ m.zone \in Zones

ValidMessages ==
    [id: MsgIds, origin: Nodes, kind: MessageKind, seq: 0..MaxSeq, zone: Zones]

=============================================================================
