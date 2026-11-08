--------------------------- MODULE MC ---------------------------
(* Topologies for the configurations. a is the adversary. *)
EXTENDS CRCI

\* s - h - r, with the adversary a next to h and r.
mc_Kite == { {"s", "h"}, {"h", "r"}, {"a", "h"}, {"a", "r"} }

\* s - h - r, adversary a next to r only: the honest path is the only way in.
mc_Line == { {"s", "h"}, {"h", "r"}, {"a", "r"} }

\* s - h - r without an adversary.
mc_Honest == { {"s", "h"}, {"h", "r"} }

\* s, h and r all linked: r hears s directly and through h.
mc_Triangle == { {"s", "h"}, {"h", "r"}, {"s", "r"} }
=================================================================
