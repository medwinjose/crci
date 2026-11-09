#!/usr/bin/env python3
"""Compare repeat sets of the 18 eventual runs (20 % loss, 60 s drain) across node builds.

Usage: compare_sets.py OUT LABEL=DIR [LABEL=DIR ...], where DIR holds eventual/<run>/.
Writes OUT/sets.csv (one row per set) and OUT/per-run.csv (one row per run).
"""
import csv
import glob
import json
import os
import re
import sys


def pct(s, p):
    return s[min(len(s) - 1, int(round(p / 100 * len(s) + 0.5)) - 1)]


REQUEST = re.compile(r"received message from (n\d+)#[0-9a-f]+: id=\S+, type=sync_request")


def requests(run_dir, attacker):
    """Sync requests received: answered, refused for budget, ignored from the attacker, refused from honest nodes."""
    out = dict(received=0, answered=0, budget=0, unanswered_attacker=0, unanswered_honest=0)
    for path in glob.glob(f"{run_dir}/logs/*.log"):
        lines = open(path, errors="replace").read().splitlines()
        for i, line in enumerate(lines):
            m = REQUEST.search(line)
            if not m:
                continue
            out["received"] += 1
            result = None
            for later in lines[i + 1:i + 8]:
                if "received message from" in later:
                    break
                if "answering sync request" in later:
                    result = "answered"
                    break
                if "budget exhausted" in later:
                    result = "budget"
                    break
            if result is None:
                result = "unanswered_attacker" if m[1] == attacker else "unanswered_honest"
            out[result] += 1
    return out


def main():
    out, sets = sys.argv[1], [a.split("=", 1) for a in sys.argv[2:]]
    os.makedirs(out, exist_ok=True)
    set_rows, run_rows = [], []
    for label, base in sets:
        post, every, totals = [], [], {}
        for run_dir in sorted(glob.glob(f"{base}/eventual/*/")):
            run = os.path.basename(run_dir[:-1])
            rows = list(csv.DictReader(open(f"{run_dir}latency.csv")))
            r_all = sorted(int(r["latency_ms"]) for r in rows)
            r_post = sorted(int(r["latency_ms"]) for r in rows if r["period"] == "post")
            post += r_post
            every += r_all
            attacker = "n%03d" % json.load(open(f"{run_dir}meta.json"))["topology"]["roles"]["byzantine"]
            req = requests(run_dir, attacker)
            for k, v in req.items():
                totals[k] = totals.get(k, 0) + v
            run_rows.append(dict(set=label, run=run, post_deliveries=len(r_post), post_p99_ms=pct(r_post, 99),
                                 post_max_ms=r_post[-1], all_deliveries=len(r_all), all_p99_ms=pct(r_all, 99),
                                 all_max_ms=r_all[-1], all_over_5s=sum(v > 5000 for v in r_all), **req))
        post.sort()
        every.sort()
        set_rows.append(dict(set=label, post_deliveries=len(post), post_p50_ms=pct(post, 50), post_p99_ms=pct(post, 99),
                             post_max_ms=post[-1], all_deliveries=len(every), all_p50_ms=pct(every, 50),
                             all_p99_ms=pct(every, 99), all_max_ms=every[-1],
                             all_over_5s_pct=round(100 * sum(v > 5000 for v in every) / len(every), 2), **totals))
    for name, rows in (("sets.csv", set_rows), ("per-run.csv", run_rows)):
        with open(f"{out}/{name}", "w", newline="\n") as f:
            w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
            w.writeheader()
            w.writerows(rows)


if __name__ == "__main__":
    main()
