#!/usr/bin/env python3
"""Table and criteria for the adversary-host runs (run_host_attack.sh).

Writes OUT/host_attack.csv (one row per run) and OUT/host_attack.md (mean over seeds per
condition, then the pass/fail criteria and whether each holds).

The criteria were written before any run of this experiment existed; a criterion that fails is
reported as failed, never reworded. T is the attack's length: from its start to the end of the
drain (130 s for the forge and flood conditions).

The hold condition needed four designs, each changed after its first seed showed that the old
listener was not locked out (V2 failed), before any data of the final one: (1) the adversary from
5 s after T0 with 20 % of the links flapping: links opened before the attack stayed up and the mesh
routed around the flapping ones; (2) the adversary from 15 s before T0: the nodes open their relay
connections when they start, before the adversary's are in place, and nothing made them reconnect;
(3) as (2) plus a 10 s outage of every node: with every node silent almost nothing was relayed, so
few links carried a frame to time out, and links stayed up; (4) as (2) plus a 10 s outage of the
neighbours of B only: their neighbours keep relaying to them, so every connection into them aborts
and must be opened again. The criteria V2 and D1 stayed as written; V3 and the outage reference were
added with design (3), before its data.

Validity (without these the experiment shows nothing):
  V1. every run is faithful (the testbed's host criteria);
  V2. in every hold-off run at least one message misses at least one node by the end of the
      drain (idle connections lock honest neighbours out of a listener that counts connections);
  V3. in every outage run (no adversary) every message reaches every node by the end of the drain,
      so a loss in hold-on would be the adversary's doing.
With per-host limits (the -on runs):
  D1. hold-on: every message reaches every node by the end of the drain, in every run;
  D2. no node bans any node, in any run (on or off);
  D3. forge-on: each target checks at most 20 + 20 T broken signatures from the adversary host
      (the per-host budget; Theorem 6 in docs/formal-verification.md);
  D4. forge-on: mean delivery to all nodes at least 0.99, in every run;
  D5. flood-on: the target accepts at most 20 + 20 T messages from origins that are no node.
Reported, not judged: every -off number, the targets' CPU against ref, delivery under flood,
connections refused and closed by the listeners.

Usage: host_attack_table.py OUT
"""

import csv
import glob
import json
import os
import re
import sys
from collections import defaultdict

NAME = re.compile(r"^(?P<cond>[a-z-]+?)-(?P<kind>rgg|mesh)-(?P<n>\d+)-s(?P<seed>\d+)$")


def mean(v):
    v = [x for x in v if x is not None]
    return round(sum(v) / len(v), 3) if v else None


def load(out):
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        s = json.load(open(path))
        run = os.path.basename(os.path.dirname(path))
        m = NAME.match(run)
        if not m:
            continue
        cfg, flap, ha = s["config"], s.get("flap") or {}, s.get("host_attack") or {}
        start = cfg.get("host_attack_start") or 0
        attack_s = cfg["window"] + cfg["drain"] - start if ha else None
        # The adversary's own totals, as printed by examples/host_attack (numbers as text).
        att = {k: float(v) for k, v in (ha.get("attacker") or {}).items()
               if k != "mode" and v.replace(".", "", 1).isdigit()}
        delivery = (ha.get("delivery_all_nodes") or flap.get("delivery_all_nodes") or {}).get("mean")
        rows.append({
            "run": run,
            "cond": m["cond"],
            "seed": int(m["seed"]),
            "faithful": s["fidelity"]["pass"],
            "undelivered_pairs": ha.get("undelivered_pairs", flap.get("undelivered_pairs")),
            "delivery_all_nodes": delivery,
            "latency_p95_ms": (ha.get("latency_ms") or flap.get("latency_all_ms") or {}).get("p95"),
            "node_bans": ha.get("honest_bans", flap.get("bans_total")),
            "adversary_identities_banned": ha.get("adversary_identities_banned"),
            "invalid_signatures_checked": ha.get("invalid_signatures_checked"),
            "foreign_accepted_per_target_max": (ha.get("foreign_messages_accepted_per_target") or {}).get("max"),
            "foreign_accepted_total": ha.get("foreign_messages_accepted_total"),
            "targets": ha.get("targets"),
            "attack_s": attack_s,
            "listener_refused": ha.get("listener_refused"),
            "listener_evicted": ha.get("listener_evicted"),
            "attacker_connections": att.get("connections"),
            "attacker_refused": att.get("refused"),
            "attacker_identities": att.get("identities"),
            "attacker_frames": att.get("frames"),
            "cpu_s_targets_mean": (ha.get("cpu_s_targets") or {}).get("mean"),
            "cpu_s_others_mean": (ha.get("cpu_s_others") or {}).get("mean"),
            "cpu_busy_pct_mean": (s["host"]["cpu_busy_pct"] or {}).get("mean"),
        })
    return rows


def main():
    out = sys.argv[1]
    rows = load(out)
    if not rows:
        sys.exit(f"no runs in {out}")
    with open(f"{out}/host_attack.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)

    by = defaultdict(list)
    for r in rows:
        by[r["cond"]].append(r)
    order = ["ref", "outage", "hold-on", "hold-off", "forge-on", "forge-off", "flood-on",
             "flood-off"]
    cols = ["delivery_all_nodes", "undelivered_pairs", "latency_p95_ms", "node_bans",
            "adversary_identities_banned", "invalid_signatures_checked",
            "foreign_accepted_per_target_max", "listener_refused", "listener_evicted",
            "attacker_connections", "attacker_identities", "cpu_s_targets_mean",
            "cpu_s_others_mean", "cpu_busy_pct_mean"]
    lines = ["# Adversary host with free identities", "",
             "Mean over seeds (" + ", ".join(sorted({str(r['seed']) for r in rows})) + ").", "",
             "| condition | runs | faithful | " + " | ".join(cols) + " |",
             "|---" * (len(cols) + 3) + "|"]
    for cond in order + sorted(set(by) - set(order)):
        if cond not in by:
            continue
        rs = by[cond]
        cells = [str(mean([r[c] for r in rs])) for c in cols]
        lines.append(f"| {cond} | {len(rs)} | {sum(r['faithful'] for r in rs)} | " + " | ".join(cells) + " |")

    def bound(r):
        return 20 + 20 * r["attack_s"] if r["attack_s"] else None

    crit = [
        ("V1 every run faithful", all(r["faithful"] for r in rows)),
        ("V2 hold-off: a message misses a node in every run",
         bool(by["hold-off"]) and all((r["undelivered_pairs"] or 0) > 0 for r in by["hold-off"])),
        ("V3 outage: no undelivered pair in any run",
         bool(by["outage"]) and all(r["undelivered_pairs"] == 0 for r in by["outage"])),
        ("D1 hold-on: no undelivered pair in any run",
         bool(by["hold-on"]) and all(r["undelivered_pairs"] == 0 for r in by["hold-on"])),
        ("D2 no node banned in any run", all((r["node_bans"] or 0) == 0 for r in rows)),
        ("D3 forge-on: broken signatures checked per target <= 20 + 20 T",
         bool(by["forge-on"]) and all(r["invalid_signatures_checked"] is not None and r["targets"]
                                      and r["invalid_signatures_checked"] / r["targets"] <= bound(r)
                                      for r in by["forge-on"])),
        ("D4 forge-on: mean delivery to all nodes >= 0.99 in every run",
         bool(by["forge-on"]) and all((r["delivery_all_nodes"] or 0) >= 0.99 for r in by["forge-on"])),
        ("D5 flood-on: foreign messages accepted at the target <= 20 + 20 T",
         bool(by["flood-on"]) and all(r["foreign_accepted_per_target_max"] is not None
                                      and r["foreign_accepted_per_target_max"] <= bound(r)
                                      for r in by["flood-on"])),
    ]
    lines += ["", "## Criteria (fixed before the runs)", ""]
    lines += [f"- {name}: **{'holds' if ok else 'FAILS'}**" for name, ok in crit]
    with open(f"{out}/host_attack.md", "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
