-------------------------------- MODULE CRCI --------------------------------
(***************************************************************************)
(* The isolation protocol as NodeRuntime::process_inbox_delivered runs it: *)
(* seen cache, sliding replay window, strikes, bans, recovery, signed      *)
(* messages, anti-entropy with tagged answers, and relaying to every       *)
(* neighbour, against an adversary that replays, forges, reuses message    *)
(* IDs and omits.                                                          *)
(*                                                                         *)
(* Each link is a FIFO queue that can lose frames, as one relay task per   *)
(* peer gives on TCP. Dropping a frame for a neighbour's or an origin's    *)
(* token bucket has the same effect as losing it, so budgets are loss.     *)
(* Not modelled: key pinning, frame pacing, the shun, the limits on batch  *)
(* and origin counts in a digest, the session limit per peer (MaxRequests  *)
(* stays below it). docs/formal-verification.md lists what this leaves out.*)
(*                                                                         *)
(* The switches at the end of the constants remove one assumption or one   *)
(* defence each. A check that passes with every switch at its default and  *)
(* fails with the switch flipped shows that the property is not vacuous    *)
(* and that the assumption is needed.                                      *)
(***************************************************************************)
EXTENDS CRCITypes, FiniteSets, TLC

CONSTANTS
    Nodes, Byzantine,
    Edges,               \* set of two-element sets of nodes
    Sources,             \* honest nodes that originate messages
    MaxSeq,              \* messages per source
    W,                   \* replay window
    MaxFaults,           \* frames or requests lost or duplicated in one behaviour
    MaxRequests,         \* sync requests per ordered pair of nodes
    ByzFrames,           \* frames an adversary sends in one behaviour
    ByzSeqs,             \* sequence numbers an adversary signs with its own key
    PersistentReplayer,  \* the adversary keeps replaying its own message
    TicksEnabled,
    SeenMayClear,        \* the seen cache can be cleared (its limit of 1 792 entries)
    OriginsMayBeForgotten, \* a node can forget an origin whole (its limit on tracked origins)
    FifoLinks,           \* FALSE: a frame can overtake another on a link
    LinksDuplicate,      \* TRUE: the network can duplicate a frame
    SessionsMayExpire,   \* TRUE: a sync session can end before its answers arrive
    ExcuseAnswers,       \* FALSE: tagged answers get no excuse
    WindowEnabled,       \* FALSE: no replay window behind the seen cache
    VerifySignatures,    \* FALSE: invalid signatures are accepted
    SeenKeyedBySigner,   \* FALSE: the seen cache is keyed by message ID alone
    TickFair             \* TRUE: time keeps passing, whatever the adversary does

Honest == Nodes \ Byzantine
Nbrs(n) == {p \in Nodes : {n, p} \in Edges}
Pairs == {<<s, r>> \in Nodes \X Nodes : {s, r} \in Edges}

ASSUME /\ Byzantine \subseteq Nodes
       /\ Sources \subseteq Honest
       /\ ByzSeqs <= MaxSeq

Seqs == 1..MaxSeq

\* A message: the ID is chosen by its signer, the origin and sequence number are signed.
Authored(o, k) == [id |-> <<o, k>>, origin |-> o, seq |-> k]
AdversaryMsg(a, k, id) == [id |-> id, origin |-> a, seq |-> k]

\* A frame on a link. sig: the origin signature verifies. tag: the sync request (numbered
\* per ordered pair) that this frame answers, 0 for gossip. Frames are numbered by the
\* transport and authenticated, so the sender of a frame cannot be forged.
Frame(m, sig, tag) == [msg |-> m, sig |-> sig, tag |-> tag]

VARIABLES
    counter,    \* source -> messages originated
    link,       \* ordered pair -> sequence of frames in flight
    requests,   \* sync requests in flight (one-shot connections: no order)
    seen,       \* honest node -> set of <<key, sender>>
    seqMax,     \* honest node -> origin -> highest accepted sequence number (0: none)
    deliveredBy,\* honest node -> set of <<origin, seq, sender>> inside the window
    rep,        \* honest node -> neighbour -> reputation record
    sessions,   \* honest node -> sync sessions opened
    recent,     \* honest node -> messages kept for answering sync requests
    delivered,  \* honest node -> set of <<origin, seq>> accepted
    twice,      \* set of <<node, origin, seq>> accepted a second time (must stay empty)
    known,      \* adversary -> messages it has received
    asked,      \* ordered pair -> sync requests sent
    faults,     \* faults used so far
    byzSent     \* adversary frames sent so far

vars == <<counter, link, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
          delivered, twice, known, asked, faults, byzSent>>

Init ==
    /\ counter     = [o \in Sources |-> 0]
    /\ link        = TLCEval([p \in Pairs |-> <<>>])
    /\ requests    = {}
    /\ seen        = [n \in Honest |-> {}]
    /\ seqMax      = [n \in Honest |-> [o \in Nodes |-> 0]]
    /\ deliveredBy = [n \in Honest |-> {}]
    /\ rep         = [n \in Honest |-> [p \in Nbrs(n) |-> NewPeer]]
    /\ sessions    = [n \in Honest |-> {}]
    /\ recent      = [n \in Honest |-> {}]
    /\ delivered   = [n \in Honest |-> {}]
    /\ twice       = {}
    /\ known       = [a \in Byzantine |-> {}]
    /\ asked       = TLCEval([p \in Pairs |-> 0])
    /\ faults      = 0
    /\ byzSent     = 0

----------------------------------------------------------------------------
(* Helpers *)

\* message_key: the signing key is the origin here (keys are not modelled apart from origins).
Key(m) == IF SeenKeyedBySigner THEN <<m.origin, m.id>> ELSE m.id

\* A batch of frames, each origin's messages oldest first.
RECURSIVE ToSeq(_)
ToSeq(S) ==
    IF S = {} THEN <<>>
    ELSE LET x == CHOOSE y \in S :
                     \A z \in S : z.msg.origin = y.msg.origin => y.msg.seq <= z.msg.seq
         IN  <<x>> \o ToSeq(S \ {x})

RemoveAt(s, i) == SubSeq(s, 1, i - 1) \o SubSeq(s, i + 1, Len(s))

Floor(n, o) == IF seqMax[n][o] > W THEN seqMax[n][o] - W ELSE 0
OutOfWindow(n, o, k) == WindowEnabled /\ k < Floor(n, o)

Held(n) == {<<x[1], x[2]>> : x \in deliveredBy[n]}

\* What a node relays to its neighbours. A node does not send a message to its origin.
RelayTo(lk, n, m) ==
    TLCEval([p \in Pairs |->
        IF p[1] = n /\ p[2] # m.origin
        THEN Append(lk[p], Frame(m, TRUE, 0))
        ELSE lk[p]])

\* The last W + 1 sequence numbers kept per origin (remember_recent).
Trim(S) ==
    {m \in S : \A x \in S : x.origin = m.origin => m.seq + W >= x.seq}

----------------------------------------------------------------------------
(* Judging a frame: process_inbox_delivered, in its order of checks *)

Excused(r, s, f) ==
    LET key == <<f.msg.origin, f.msg.seq>> IN
    /\ ExcuseAnswers
    /\ \E ses \in sessions[r] :
        /\ ses.peer = s
        /\ IF f.tag # 0 THEN ses.rid = f.tag ELSE key \in ses.answered
        /\ key \notin ses.held
        /\ key \notin ses.exempted

Exempt(r, s, f) ==
    LET key == <<f.msg.origin, f.msg.seq>> IN
    {IF /\ ses.peer = s
        /\ IF f.tag # 0 THEN ses.rid = f.tag ELSE key \in ses.answered
     THEN [ses EXCEPT !.exempted = @ \cup {key}]
     ELSE ses : ses \in sessions[r]}

\* note_answered: an accepted answer earns one excuse for the gossip copy that may follow it.
Answered(r, s, f) ==
    {IF ses.peer = s /\ ses.rid = f.tag
     THEN [ses EXCEPT !.answered = @ \cup {<<f.msg.origin, f.msg.seq>>}]
     ELSE ses : ses \in sessions[r]}

\* ReplayFilter::check_and_record, as a verdict.
Filter(r, m, s) ==
    IF ~WindowEnabled THEN "accept"
    ELSE IF m.seq > seqMax[r][m.origin] THEN "accept"
    ELSE IF m.seq < Floor(r, m.origin) THEN "stale"
    ELSE LET by == {x[3] : x \in {y \in deliveredBy[r] : y[1] = m.origin /\ y[2] = m.seq}}
         IN  IF by = {} THEN "accept"
             ELSE IF s \in by THEN "replayed"
             ELSE "redundant"

Verdict(r, s, f) ==
    LET m == f.msg
        senders == {x[2] : x \in {y \in seen[r] : y[1] = Key(m)}}
    IN
    IF rep[r][s].banned THEN "drop"
    ELSE IF senders # {} THEN
        IF s \notin senders THEN "copy"
        ELSE IF OutOfWindow(r, m.origin, m.seq) THEN "drop"
        ELSE IF Excused(r, s, f) THEN "excuse"
        ELSE "strike"
    ELSE IF VerifySignatures /\ ~f.sig THEN "strike"
    ELSE IF m.origin = r THEN "own"
    ELSE LET v == Filter(r, m, s) IN
         IF v = "replayed" THEN (IF Excused(r, s, f) THEN "first-excuse" ELSE "first-strike")
         ELSE IF v = "stale" THEN "first-drop"
         ELSE v

\* An accepted sequence number slides the window; records below the floor are forgotten.
AcceptWindow(r, m, s) ==
    LET newMax == Max(seqMax[r][m.origin], m.seq)
        floor  == IF newMax > W THEN newMax - W ELSE 0
    IN  [max |-> newMax,
         by  |-> {x \in deliveredBy[r] : x[1] # m.origin \/ x[2] >= floor}
                 \cup {<<m.origin, m.seq, s>>}]

Receive(s, r) ==
    /\ r \in Honest
    /\ link[<<s, r>>] # <<>>
    /\ \E i \in IF FifoLinks THEN {1} ELSE 1..Len(link[<<s, r>>]) :
        LET f == link[<<s, r>>][i]
            m == f.msg
            v == Verdict(r, s, f)
            popped == [link EXCEPT ![<<s, r>>] = RemoveAt(@, i)]
            seenAdded == {"copy", "own", "accept", "redundant", "first-excuse",
                          "first-strike", "first-drop"}
            win == AcceptWindow(r, m, s)
        IN
        /\ seen' = IF v \in seenAdded
                   THEN [seen EXCEPT ![r] = @ \cup {<<Key(m), s>>}]
                   ELSE seen
        /\ rep' = IF v \in {"strike", "first-strike"}
                  THEN [rep EXCEPT ![r][s] = Penalise(@)]
                  ELSE rep
        /\ sessions' = CASE v \in {"excuse", "first-excuse"} ->
                                 [sessions EXCEPT ![r] = Exempt(r, s, f)]
                         [] v = "accept" /\ f.tag # 0 ->
                                 [sessions EXCEPT ![r] = Answered(r, s, f)]
                         [] OTHER -> sessions
        /\ seqMax' = IF v = "accept"
                     THEN [seqMax EXCEPT ![r][m.origin] = win.max]
                     ELSE seqMax
        /\ deliveredBy' = CASE v = "accept" -> [deliveredBy EXCEPT ![r] = win.by]
                            [] v = "redundant" ->
                                 [deliveredBy EXCEPT ![r] = @ \cup {<<m.origin, m.seq, s>>}]
                            [] OTHER -> deliveredBy
        /\ IF v = "accept"
           THEN /\ delivered' = [delivered EXCEPT ![r] = @ \cup {<<m.origin, m.seq>>}]
                /\ twice' = IF <<m.origin, m.seq>> \in delivered[r]
                            THEN twice \cup {<<r, m.origin, m.seq>>}
                            ELSE twice
                /\ recent' = [recent EXCEPT ![r] = Trim(@ \cup {m})]
                /\ link' = RelayTo(popped, r, m)
           ELSE /\ link' = popped
                /\ UNCHANGED <<delivered, twice, recent>>
    /\ UNCHANGED <<counter, requests, known, asked, faults, byzSent>>

----------------------------------------------------------------------------
(* Honest nodes *)

Originate(o) ==
    /\ counter[o] < MaxSeq
    /\ LET m == Authored(o, counter[o] + 1) IN
        /\ counter' = [counter EXCEPT ![o] = @ + 1]
        /\ link' = RelayTo(link, o, m)
        /\ seen' = [seen EXCEPT ![o] = @ \cup {<<Key(m), o>>}]
        /\ recent' = [recent EXCEPT ![o] = Trim(@ \cup {m})]
        /\ delivered' = [delivered EXCEPT ![o] = @ \cup {<<o, m.seq>>}]
    /\ UNCHANGED <<requests, seqMax, deliveredBy, rep, sessions, twice, known, asked,
                   faults, byzSent>>

\* A neighbour holds a message this node has not accepted. The node asks periodically in
\* the real protocol; a request with nothing to answer changes nothing but the cooldown,
\* so only requests with something to answer are modelled.
Lacks(r, s) ==
    \E m \in recent[s] : m.origin # r /\ <<m.origin, m.seq>> \notin delivered[r]

\* MaxRequests bounds the requests that race frames still in flight. The real node keeps asking
\* for ever, so each fault also earns one request on a quiet link: without it a behaviour could
\* spend its budget early and a later loss would look unrecoverable. MaxRequests + MaxFaults
\* stays within the four sessions a node keeps per peer.
CanAsk(r, s) ==
    IF s \in Byzantine
    THEN asked[<<r, s>>] < MaxRequests
    ELSE /\ Lacks(r, s)
         /\ \/ asked[<<r, s>>] < MaxRequests
            \/ /\ asked[<<r, s>>] < MaxRequests + faults
               /\ link[<<s, r>>] = <<>>
               /\ ~\E q \in requests : q.from = r /\ q.to = s

RequestSync(r, s) ==
    /\ r \in Honest
    /\ s \in Nbrs(r)
    /\ CanAsk(r, s)
    /\ LET rid == asked[<<r, s>>] + 1 IN
        /\ asked' = [asked EXCEPT ![<<r, s>>] = rid]
        /\ requests' = requests \cup
             {[from |-> r, to |-> s, rid |-> rid, held |-> Held(r), max |-> seqMax[r]]}
        /\ sessions' = [sessions EXCEPT ![r] = @ \cup
             {[peer |-> s, rid |-> rid, held |-> Held(r), exempted |-> {}, answered |-> {}]}]
    /\ UNCHANGED <<counter, link, seen, seqMax, deliveredBy, rep, recent, delivered,
                   twice, known, faults, byzSent>>

\* answer_sync: every kept message the digest lacks, tagged, oldest first as one batch.
Answer(q) ==
    /\ q \in requests
    /\ q.to \in Honest
    /\ requests' = requests \ {q}
    /\ LET wanted == {m \in recent[q.to] :
                        /\ m.origin # q.from
                        /\ \/ q.max[m.origin] = 0
                           \/ /\ m.seq + W >= q.max[m.origin]
                              /\ <<m.origin, m.seq>> \notin q.held}
           batch == ToSeq({Frame(m, TRUE, q.rid) : m \in wanted})
           pair == <<q.to, q.from>>
       IN  link' = [link EXCEPT ![pair] = @ \o batch]
    /\ UNCHANGED <<counter, seen, seqMax, deliveredBy, rep, sessions, recent, delivered,
                   twice, known, asked, faults, byzSent>>

\* The seen cache is cleared whole at its limit of 1 792 entries.
ClearSeen(n) ==
    /\ SeenMayClear
    /\ seen[n] # {}
    /\ seen' = [seen EXCEPT ![n] = {}]
    /\ UNCHANGED <<counter, link, requests, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, faults, byzSent>>

\* Above its limit on tracked origins a node forgets an origin's window and recent messages. Not
\* part of the checks that must hold: the two checks with this switch on must break, which shows
\* what bounded memory costs while identities are free.
ForgetOrigin(n, o) ==
    /\ OriginsMayBeForgotten
    /\ seqMax[n][o] # 0
    /\ seqMax' = [seqMax EXCEPT ![n][o] = 0]
    /\ deliveredBy' = [deliveredBy EXCEPT ![n] = {x \in @ : x[1] # o}]
    /\ recent' = [recent EXCEPT ![n] = {m \in @ : m.origin # o}]
    /\ UNCHANGED <<counter, link, requests, seen, rep, sessions, delivered, twice, known, asked,
                   faults, byzSent>>

Tick ==
    /\ TicksEnabled
    /\ \E n \in Honest, p \in Nodes : p \in Nbrs(n) /\ ~AtRest(rep[n][p])
    /\ rep' = TLCEval([n \in Honest |-> [p \in Nbrs(n) |-> Recover(rep[n][p])]])
    /\ UNCHANGED <<counter, link, requests, seen, seqMax, deliveredBy, sessions, recent,
                   delivered, twice, known, asked, faults, byzSent>>

----------------------------------------------------------------------------
(* Faults of the network *)

LoseFrame(s, r) ==
    /\ faults < MaxFaults
    /\ link[<<s, r>>] # <<>>
    /\ \E i \in 1..Len(link[<<s, r>>]) :
        link' = [link EXCEPT ![<<s, r>>] = RemoveAt(@, i)]
    /\ faults' = faults + 1
    /\ UNCHANGED <<counter, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, byzSent>>

LoseRequest(q) ==
    /\ faults < MaxFaults
    /\ q \in requests
    /\ requests' = requests \ {q}
    /\ faults' = faults + 1
    /\ UNCHANGED <<counter, link, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, byzSent>>

DuplicateFrame(s, r) ==
    /\ LinksDuplicate
    /\ s \in Honest
    /\ faults < MaxFaults
    /\ link[<<s, r>>] # <<>>
    /\ link' = [link EXCEPT ![<<s, r>>] = Append(@, Head(@))]
    /\ faults' = faults + 1
    /\ UNCHANGED <<counter, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, byzSent>>

\* An answer that outlives its session (120 s, or the four newest per peer).
ExpireSession(n, ses) ==
    /\ SessionsMayExpire
    /\ ses \in sessions[n]
    /\ sessions' = [sessions EXCEPT ![n] = @ \ {ses}]
    /\ UNCHANGED <<counter, link, requests, seen, seqMax, deliveredBy, rep, recent,
                   delivered, twice, known, asked, faults, byzSent>>

----------------------------------------------------------------------------
(* The adversary: replays what it has received with any tag, forges frames whose        *)
(* signature does not verify, signs its own messages under any message ID it has seen,  *)
(* and otherwise stays silent. It holds no honest key and cannot pose as another sender.*)

ByzReceive(s, a) ==
    /\ a \in Byzantine
    /\ link[<<s, a>>] # <<>>
    /\ known' = [known EXCEPT ![a] = @ \cup {Head(link[<<s, a>>]).msg}]
    /\ link' = [link EXCEPT ![<<s, a>>] = Tail(@)]
    /\ UNCHANGED <<counter, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, asked, faults, byzSent>>

ByzIgnoreRequest(q) ==
    /\ q \in requests
    /\ q.to \in Byzantine
    /\ requests' = requests \ {q}
    /\ UNCHANGED <<counter, link, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, faults, byzSent>>

Tags == 0..MaxRequests

AdversaryFrames(a) ==
    LET ids == {m.id : m \in known[a]} \cup {<<a, k>> : k \in 1..ByzSeqs}
        own == {AdversaryMsg(a, k, id) : k \in 1..ByzSeqs, id \in ids}
    IN  {Frame(m, TRUE, t) : m \in known[a] \cup own, t \in Tags}
        \cup {Frame(Authored(o, k), FALSE, 0) : o \in Sources, k \in Seqs}

ByzSend(a, r) ==
    /\ a \in Byzantine
    /\ r \in Honest
    /\ {a, r} \in Edges
    /\ byzSent < ByzFrames
    /\ byzSent' = byzSent + 1
    /\ \E f \in AdversaryFrames(a) :
        link' = [link EXCEPT ![<<a, r>>] = Append(@, f)]
    /\ UNCHANGED <<counter, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, faults>>

\* Replays its own message without end, as fast as the link takes it.
ByzPersist(a, r) ==
    /\ PersistentReplayer
    /\ a \in Byzantine
    /\ r \in Honest
    /\ {a, r} \in Edges
    /\ link[<<a, r>>] = <<>>
    /\ link' = [link EXCEPT ![<<a, r>>] = <<Frame(AdversaryMsg(a, 1, <<a, 1>>), TRUE, 0)>>]
    /\ UNCHANGED <<counter, requests, seen, seqMax, deliveredBy, rep, sessions, recent,
                   delivered, twice, known, asked, faults, byzSent>>

----------------------------------------------------------------------------

Next ==
    \/ \E p \in Pairs : Receive(p[1], p[2])
    \/ \E o \in Sources : Originate(o)
    \/ \E p \in Pairs : RequestSync(p[1], p[2])
    \/ \E q \in requests : Answer(q) \/ LoseRequest(q) \/ ByzIgnoreRequest(q)
    \/ \E n \in Honest : ClearSeen(n) \/ \E ses \in sessions[n] : ExpireSession(n, ses)
    \/ \E n \in Honest, o \in Nodes : ForgetOrigin(n, o)
    \/ Tick
    \/ \E p \in Pairs : LoseFrame(p[1], p[2]) \/ DuplicateFrame(p[1], p[2])
    \/ \E p \in Pairs : ByzReceive(p[1], p[2]) \/ ByzSend(p[1], p[2]) \/ ByzPersist(p[1], p[2])

\* Honest nodes keep working and frames in flight are eventually handled. Loss, duplicates,
\* the clock and the adversary get no fairness. One fairness condition covers all honest steps:
\* with finitely many frames, losses and requests there are finitely many such steps, so none
\* can be starved for ever.
HonestStep ==
    \/ \E p \in Pairs : Receive(p[1], p[2]) \/ RequestSync(p[1], p[2])
    \/ \E q \in requests : Answer(q)

Spec == Init /\ [][Next]_vars /\ WF_vars(HonestStep)

\* A persistent replayer makes frames without end, so here each link needs fairness of its own.
ReplayerSpec ==
    /\ Init /\ [][Next]_vars
    /\ \A p \in Pairs : WF_vars(Receive(p[1], p[2])) /\ WF_vars(ByzPersist(p[1], p[2]))
    /\ TickFair => WF_vars(Tick)

\* The same behaviours with no liveness obligation: faster for the safety configurations.
SafetySpec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* Safety *)

\* No honest node ever loses reputation for what an honest neighbour sent.
HonestNeverStruck ==
    \A r \in Honest, s \in Honest : s \in Nbrs(r) => AtRest(rep[r][s])

\* An observer bans only an adversary.
OnlyAdversariesBanned ==
    \A r \in Honest, s \in Nodes : s \in Nbrs(r) /\ rep[r][s].banned => s \in Byzantine

\* A node accepts each (origin, sequence number) at most once.
NoDoubleDelivery == twice = {}

\* A node accepts for an honest origin only what that origin signed.
NoForgedDelivery ==
    \A r \in Honest, o \in Sources, k \in Seqs :
        <<o, k>> \in delivered[r] => k <= counter[o]

\* What the window remembers is inside the window, and within what was accepted.
WindowConsistent ==
    \A r \in Honest : \A x \in deliveredBy[r] :
        /\ x[2] <= seqMax[r][x[1]]
        /\ WindowEnabled => x[2] >= Floor(r, x[1])

\* No honest step is left: nothing in flight to an honest node, and no honest neighbour holds
\* anything this node could still ask for.
HonestQuiescent ==
    /\ \A p \in Pairs : p[2] \in Honest => link[p] = <<>>
    /\ \A q \in requests : q.to \notin Honest
    /\ \A r \in Honest, s \in Honest : s \in Nbrs(r) => ~CanAsk(r, s)

\* Delivery as a state predicate. Honest steps are finite here, so under WF(HonestStep) every
\* behaviour reaches a state in which none is enabled, and by then every authored message must
\* have reached every honest node. Implies EventualDelivery for these bounded models, and is
\* far cheaper for TLC than the temporal form, which the "delivery" check also runs.
DeliveredWhenQuiescent ==
    HonestQuiescent =>
        \A o \in Sources, r \in Honest, k \in Seqs :
            k <= counter[o] => <<o, k>> \in delivered[r]

----------------------------------------------------------------------------
(* Liveness *)

\* Every message of an honest source reaches every honest node, given the fairness above,
\* finitely many losses, and an honest graph that stays connected.
EventualDelivery ==
    \A o \in Sources, k \in Seqs, r \in Honest :
        (counter[o] >= k) ~> (<<o, k>> \in delivered[r])

\* A replayer that does not stop is banned by every honest neighbour.
EveryNeighbourBans ==
    \A a \in Byzantine, r \in Honest : {a, r} \in Edges => <>(rep[r][a].banned)

=============================================================================
