--------------------------- MODULE CRCITypes ---------------------------
(***************************************************************************)
(* Reputation arithmetic shared by the models. It transcribes              *)
(* PeerReputation in crates/crci-core/src/sybil.rs on a time grid: scores  *)
(* are integers and one Tick is the time in which recovery restores        *)
(* RecoveryPerTick units. The configurations state the grid they use.      *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS
    InitialScore,     \* INITIAL_SCORE: score of a new peer, and the recovery cap
    StrikePenalty,    \* STRIKE_PENALTY: score lost per strike
    BanThreshold,     \* BAN_THRESHOLD: a score below this bans the peer
    UnbanThreshold,   \* UNBAN_THRESHOLD: a banned peer at or above this is readmitted
    DampedCeiling,    \* DAMPED_CEILING: recovery cap while damping applies
    DampingStrikes,   \* DAMPING_STRIKES: remembered strikes from which damping applies
    RecoveryPerTick,  \* RECOVERY_PER_SEC times the length of a Tick
    MemoryTicks       \* STRIKE_MEMORY in Ticks

ASSUME /\ BanThreshold < UnbanThreshold
       /\ UnbanThreshold <= InitialScore
       /\ MemoryTicks > 0

Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a > b THEN a ELSE b

\* A peer as an observer sees it. ages lists the remembered strikes, oldest first.
NewPeer == [score |-> InitialScore, ages |-> <<>>, banned |-> FALSE]

\* A peer with no remembered strikes at the recovery cap: time changes nothing.
AtRest(p) == p.score = InitialScore /\ p.ages = <<>> /\ ~p.banned

\* PeerReputation::penalise_at.
Penalise(p) ==
    LET new == IF p.score > StrikePenalty THEN p.score - StrikePenalty ELSE 0
    IN  [score  |-> new,
         ages   |-> Append(p.ages, 0),
         banned |-> p.banned \/ new < BanThreshold]

\* PeerReputation::recover_at over one Tick. Remembered strikes are younger than
\* MemoryTicks, so the cap holds for the whole Tick, and a strike that reaches
\* MemoryTicks at its end is forgotten. Recovery caps the increase and never lowers.
RECURSIVE Older(_)
Older(s) ==
    IF s = <<>> THEN <<>>
    ELSE IF Head(s) + 1 >= MemoryTicks THEN Older(Tail(s))
    ELSE <<Head(s) + 1>> \o Older(Tail(s))

Recover(p) ==
    LET ceiling == IF Len(p.ages) >= DampingStrikes THEN DampedCeiling ELSE InitialScore
        new     == Max(p.score, Min(p.score + RecoveryPerTick, ceiling))
    IN  [score  |-> new,
         ages   |-> Older(p.ages),
         banned |-> p.banned /\ new < UnbanThreshold]

=============================================================================
