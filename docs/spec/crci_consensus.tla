----------------------- MODULE crci_consensus -----------------------
EXTENDS Integers, FiniteSets

CONSTANTS 
    Nodes,              \* Set of all node IDs
    ByzantineNodes,     \* Subset of Nodes that are Byzantine
    Threshold,          \* Quorum threshold (scaled to 100, e.g., 66)
    PenaltyDelta,       \* Reputation decrement on penalty (e.g., 10)
    IsolationThreshold  \* Reputation at which a node is excluded (e.g., 30)

VARIABLES 
    reputation,         \* Node -> Integer (0..100) representing [0.0, 1.0]
    network_msgs,       \* Set of all broadcasted messages
    votes,              \* Message -> Set of Nodes that agree with the message
    penalties_applied   \* Set of messages that have already triggered penalties

vars == <<reputation, network_msgs, votes, penalties_applied>>

\* Helper to sum reputation weights of a set of nodes
RECURSIVE SumRep(_)
SumRep(S) == 
    IF S = {} THEN 0 
    ELSE LET n == CHOOSE x \in S : TRUE 
         IN reputation[n] + SumRep(S \ {n})

\* Total active network weight (nodes above isolation threshold)
ActiveNodes == {n \in Nodes : reputation[n] >= IsolationThreshold}
TotalActiveWeight == SumRep(ActiveNodes)

\* Quorum achieved if agreeing active nodes have weight > Threshold % of TotalActiveWeight
QuorumReached(msg) == 
    LET agreeing_active == votes[msg] \cap ActiveNodes
    IN (SumRep(agreeing_active) * 100) > (TotalActiveWeight * Threshold)

\* True environmental severity (simplified to a single active event for checking)
TrueSeverity == 3

----------------------------------------------------------------------
\* INITIALIZATION
Init ==
    /\ reputation = [n \in Nodes |-> 100]
    /\ network_msgs = {}
    /\ votes = [m \in {} |-> {}]
    /\ penalties_applied = {}

----------------------------------------------------------------------
\* ACTIONS

\* Honest node originates a message with the true severity
HonestOriginate ==
    \E n \in (Nodes \ ByzantineNodes) \cap ActiveNodes :
        LET msg == [origin |-> n, severity |-> TrueSeverity, is_false |-> FALSE, seq |-> 1, valid_sig |-> TRUE] IN
        /\ msg \notin network_msgs
        /\ network_msgs' = network_msgs \cup {msg}
        /\ votes' = votes @@ (msg :> {n})
        /\ UNCHANGED <<reputation, penalties_applied>>

\* Byzantine node injects a message with a false severity
ByzantineInject ==
    \E n \in ByzantineNodes \cap ActiveNodes :
        \E bad_sev \in {1, 5} :
            LET msg == [origin |-> n, severity |-> bad_sev, is_false |-> TRUE, seq |-> 1, valid_sig |-> TRUE] IN
            /\ msg \notin network_msgs
            /\ network_msgs' = network_msgs \cup {msg}
            /\ votes' = votes @@ (msg :> {n})
            /\ UNCHANGED <<reputation, penalties_applied>>

\* Node votes for a message it agrees with
Vote ==
    \E n \in ActiveNodes :
        \E msg \in network_msgs :
            /\ n \notin votes[msg]
            \* Honest nodes only vote for true severity, Byzantine nodes might vote for false ones
            /\ (n \notin ByzantineNodes => msg.is_false = FALSE)
            /\ votes' = [votes EXCEPT ![msg] = @ \cup {n}]
            /\ UNCHANGED <<reputation, network_msgs, penalties_applied>>

\* Apply penalty to a message originator if the message fails to gather quorum after some condition
\* For simplification, any false message can be penalized if it hasn't reached quorum
ApplyPenalty ==
    \E msg \in network_msgs :
        /\ msg \notin penalties_applied
        /\ ~QuorumReached(msg)
        /\ msg.is_false = TRUE
        /\ penalties_applied' = penalties_applied \cup {msg}
        /\ reputation' = [reputation EXCEPT ![msg.origin] = 
                            IF @ >= PenaltyDelta THEN @ - PenaltyDelta ELSE 0]
        /\ UNCHANGED <<network_msgs, votes>>

Next == 
    \/ HonestOriginate
    \/ ByzantineInject
    \/ Vote
    \/ ApplyPenalty

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

----------------------------------------------------------------------
\* INVARIANTS & PROPERTIES

TypeInvariant ==
    /\ \A n \in Nodes : reputation[n] \in 0..100
    /\ \A msg \in network_msgs : msg.severity \in 1..5

\* SAFETY: A false message must never achieve quorum if Byzantine weight < 1/3
SafetyInvariant ==
    \A msg \in network_msgs :
        msg.is_false => ~QuorumReached(msg)

\* LIVENESS: A valid message from an honest node eventually achieves quorum
LivenessProperty ==
    \A msg \in network_msgs :
        (~msg.is_false) ~> QuorumReached(msg)

======================================================================
