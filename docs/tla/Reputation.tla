----------------------------- MODULE Reputation -----------------------------
(***************************************************************************)
(* One observer's view of one peer, against every strike schedule on the   *)
(* time grid. A banned peer's frames are dropped before they are judged    *)
(* (NodeRuntime::process_inbox_delivered), so it cannot be struck.         *)
(*                                                                         *)
(* Pace = 0: the adversary strikes whenever it likes, at most MaxStrikes   *)
(* times in total. Pace > 0: it leaves at least Pace Ticks between         *)
(* strikes, the first after Pace Ticks. With Forced it strikes exactly     *)
(* every Pace Ticks, as the paced replayer of recovery_bench does.         *)
(***************************************************************************)
EXTENDS CRCITypes

CONSTANTS Pace, Forced, MaxStrikes, TickSeconds

VARIABLES
    peer,      \* the observer's record of the peer
    struck,    \* strikes so far (Pace = 0), capped by MaxStrikes
    burst,     \* strikes in the current Tick
    since,     \* Ticks since the last strike, capped at Pace (Pace > 0)
    elapsed    \* seconds since the start; a history variable, hidden from state identity

vars == <<peer, struck, burst, since, elapsed>>

\* Everything but the clock, so that a run with no ban still has finitely many states.
View == <<peer, struck, burst, since>>

Init ==
    /\ peer = NewPeer
    /\ struck = 0
    /\ burst = 0
    /\ since = 0
    /\ elapsed = 0

StrikeAllowed == Pace > 0 /\ since = Pace /\ ~peer.banned

Strike ==
    /\ ~peer.banned
    /\ IF Pace = 0 THEN struck < MaxStrikes ELSE StrikeAllowed
    /\ peer' = Penalise(peer)
    /\ struck' = IF Pace = 0 THEN struck + 1 ELSE struck
    /\ burst' = burst + 1
    /\ since' = 0
    /\ UNCHANGED elapsed

Tick ==
    /\ ~(Forced /\ StrikeAllowed)
    /\ peer' = Recover(peer)
    /\ burst' = 0
    /\ since' = Min(since + 1, Pace)
    /\ elapsed' = elapsed + TickSeconds
    /\ UNCHANGED struck

Next == Strike \/ Tick

Spec == Init /\ [][Next]_vars /\ WF_vars(Tick)

(***************************************************************************)
(* Safety                                                                  *)
(***************************************************************************)
ScoreInRange == peer.score \in 0..InitialScore

\* Inactivity never lowers a score or bans: only strikes do.
NoBanWithoutStrike == (struck = 0 /\ Pace = 0) => AtRest(peer)

\* Three strikes in a row ban from any state, whatever credit was recovered before.
BurstOfThreeBans == burst >= 3 => peer.banned

\* Readmission has hysteresis: a banned peer stays below the readmission score.
BannedBelowUnban == peer.banned => peer.score < UnbanThreshold

\* The check run with Pace >= the forgiveness period: such an adversary is never banned.
NeverBanned == ~peer.banned

(***************************************************************************)
(* Liveness (needs WF(Tick): time passes; strikes need no fairness)        *)
(***************************************************************************)
\* After finitely many strikes every peer is back at the initial score, unbanned.
EventuallyForgiven == <>[]AtRest(peer)

=============================================================================
