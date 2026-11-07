#!/usr/bin/env python3
"""Run every TLC configuration and compare the outcome with the expected one.

Usage: check.py [--out DIR] [--quick] [NAME ...]
       check.py --export-transitions CSV | --export-traces JSONL

Each entry says which configuration of the protocol or reputation model to run, the
constants that differ from its base, and the outcome: "pass", or the name of the invariant
or property TLC must violate. A switch that must produce a counterexample shows the property
is not vacuous and the assumption is needed. Requires Java 11+; tla2tools.jar is in this folder.
"""
import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent

REPUTATION_20S = {
    "InitialScore": 50, "StrikePenalty": 15, "BanThreshold": 10, "UnbanThreshold": 25,
    "DampedCeiling": 18, "DampingStrikes": 2, "RecoveryPerTick": 1, "MemoryTicks": 15,
    "TickSeconds": 20, "Pace": 0, "Forced": False, "MaxStrikes": 6,
}
# 1 unit = 0.005, one Tick = 10 s: the grid on which 290 s and 300 s both fall.
REPUTATION_10S = {
    **REPUTATION_20S, "InitialScore": 100, "StrikePenalty": 30, "BanThreshold": 20,
    "UnbanThreshold": 50, "DampedCeiling": 36, "MemoryTicks": 30, "TickSeconds": 10,
    "MaxStrikes": 0,
}
# 1 unit = 0.01, one Tick = 100 s.
PROTOCOL = {
    "InitialScore": 50, "StrikePenalty": 15, "BanThreshold": 10, "UnbanThreshold": 25,
    "DampedCeiling": 18, "DampingStrikes": 2, "RecoveryPerTick": 5, "MemoryTicks": 3,
    "Nodes": {"s", "h", "r", "a"}, "Byzantine": {"a"}, "Edges": "mc_Kite", "Sources": {"s"},
    "MaxSeq": 1, "W": 2, "MaxFaults": 1, "MaxRequests": 2, "ByzFrames": 0, "ByzSeqs": 1,
    "PersistentReplayer": False, "TicksEnabled": False, "SeenMayClear": False,
    "OriginsMayBeForgotten": False,
    "FifoLinks": True, "LinksDuplicate": False, "SessionsMayExpire": False,
    "ExcuseAnswers": True, "WindowEnabled": True, "VerifySignatures": True,
    "SeenKeyedBySigner": True, "TickFair": False,
}
HONEST_LINE = {"Nodes": {"s", "h", "r"}, "Byzantine": set(), "Edges": "mc_Honest"}
SAFETY = ["HonestNeverStruck", "OnlyAdversariesBanned", "NoDoubleDelivery",
          "NoForgedDelivery", "WindowConsistent"]

# name: (module, base, overrides, invariants, properties, expected, expected trace value)
CHECKS = {
    # Reputation arithmetic.
    "rep-free": ("Reputation", REPUTATION_20S, {},
                 ["ScoreInRange", "NoBanWithoutStrike", "BurstOfThreeBans", "BannedBelowUnban"],
                 [], "pass", None),
    "rep-forgiven": ("Reputation", REPUTATION_20S, {"TickSeconds": 0}, [],
                     ["EventuallyForgiven"], "pass", None),
    "rep-spaced-300": ("Reputation", REPUTATION_10S, {"Pace": 30}, ["NeverBanned"], [],
                       "pass", None),
    "rep-paced-290": ("Reputation", REPUTATION_10S, {"Pace": 29, "Forced": True},
                      ["NeverBanned"], [], "NeverBanned", 10150),
    "rep-paced-120": ("Reputation", REPUTATION_10S, {"Pace": 12, "Forced": True},
                      ["NeverBanned"], [], "NeverBanned", 480),

    # Protocol: safety, and delivery once no honest step is left. Honest nodes only, then with
    # the adversary (which also rules out what its frames could do to honest delivery).
    "honest-loss-sync": ("MC", PROTOCOL,
                         {**HONEST_LINE, "MaxSeq": 2, "MaxFaults": 2, "SeenMayClear": True},
                         SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "adversary": ("MC", PROTOCOL, {"MaxSeq": 2, "ByzFrames": 4, "MaxRequests": 0, "MaxFaults": 0},
                  SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "adversary-sync": ("MC", PROTOCOL, {"ByzFrames": 1, "MaxRequests": 1, "MaxFaults": 0},
                       SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "adversary-loss": ("MC", PROTOCOL,
                       {"Edges": "mc_Line", "ByzFrames": 1, "MaxRequests": 1, "MaxFaults": 1},
                       SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "adversary-time": ("MC", PROTOCOL,
                       {"Edges": "mc_Line", "ByzFrames": 3, "TicksEnabled": True, "MaxFaults": 0,
                        "MaxRequests": 1},
                       SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "reordering-links": ("MC", PROTOCOL,
                         {**HONEST_LINE, "FifoLinks": False, "MaxSeq": 2, "MaxFaults": 1},
                         SAFETY + ["DeliveredWhenQuiescent"], [], "pass", None),
    "window-slides": ("MC", PROTOCOL, {**HONEST_LINE, "MaxSeq": 3, "W": 1, "MaxFaults": 0,
                                       "MaxRequests": 1},
                      SAFETY, [], "pass", None),

    # Protocol: liveness in temporal logic.
    "delivery": ("MC", PROTOCOL, {**HONEST_LINE, "MaxSeq": 2, "MaxFaults": 1}, [],
                 ["EventualDelivery"], "pass", None),
    "eviction": ("MC", PROTOCOL,
                 {"PersistentReplayer": True, "MaxFaults": 0, "MaxRequests": 0}, [],
                 ["EveryNeighbourBans"], "pass", None),

    # Each assumption or defence removed: TLC must produce a counterexample.
    "link-duplicates": ("MC", PROTOCOL, {**HONEST_LINE, "LinksDuplicate": True},
                        ["HonestNeverStruck"], [], "HonestNeverStruck", None),
    "session-expires": ("MC", PROTOCOL, {**HONEST_LINE, "SessionsMayExpire": True},
                        ["HonestNeverStruck"], [], "HonestNeverStruck", None),
    "no-excuse": ("MC", PROTOCOL, {**HONEST_LINE, "ExcuseAnswers": False},
                  ["HonestNeverStruck"], [], "HonestNeverStruck", None),
    "no-window": ("MC", PROTOCOL,
                  {"WindowEnabled": False, "SeenMayClear": True, "ByzFrames": 1, "MaxFaults": 0},
                  ["NoDoubleDelivery"], [], "NoDoubleDelivery", None),
    "no-signatures": ("MC", PROTOCOL, {"VerifySignatures": False, "ByzFrames": 1, "MaxFaults": 0},
                      ["NoForgedDelivery"], [], "NoForgedDelivery", None),
    "seen-by-id": ("MC", PROTOCOL,
                   {"SeenKeyedBySigner": False, "ByzFrames": 1, "MaxFaults": 0, "MaxRequests": 1},
                   ["DeliveredWhenQuiescent"], [], "DeliveredWhenQuiescent", None),
    "window-too-small": ("MC", PROTOCOL,
                         {**HONEST_LINE, "MaxSeq": 3, "W": 1, "MaxFaults": 1},
                         ["DeliveredWhenQuiescent"], [], "DeliveredWhenQuiescent", None),
    "origin-forgotten-delivery": ("MC", PROTOCOL,
                                  {"OriginsMayBeForgotten": True, "SeenMayClear": True,
                                   "ByzFrames": 1, "MaxFaults": 0, "MaxRequests": 0},
                                  ["NoDoubleDelivery"], [], "NoDoubleDelivery", None),
    "origin-forgotten-framing": ("MC", PROTOCOL,
                                 {"OriginsMayBeForgotten": True, "SeenMayClear": True,
                                  "ByzFrames": 1, "MaxFaults": 0, "MaxRequests": 0},
                                 ["HonestNeverStruck"], [], "HonestNeverStruck", None),
    "paced-replayer-spared": ("MC", PROTOCOL,
                              {"PersistentReplayer": True, "MaxFaults": 0, "MaxRequests": 0,
                               "TicksEnabled": True, "TickFair": True}, [],
                              ["EveryNeighbourBans"], "EveryNeighbourBans", None),
}


SLOW = {"honest-loss-sync", "adversary", "adversary-sync", "adversary-loss", "adversary-time",
        "reordering-links"}

# Random behaviours replayed through the real code (name, constants, seed, traces). Time moves in
# one set only, with no requests: sessions last 120 s, which the model leaves as an assumption.
TRACE_SETS = [
    ("window", {"Sources": {"s", "h"}, "MaxSeq": 3, "W": 1, "MaxFaults": 2, "ByzFrames": 4,
                "SeenMayClear": True}, 1, 60),
    ("strikes", {"MaxSeq": 2, "W": 2, "MaxFaults": 1, "ByzFrames": 6}, 2, 40),
    ("time", {"MaxSeq": 2, "W": 2, "MaxFaults": 0, "MaxRequests": 0, "ByzFrames": 8,
              "TicksEnabled": True}, 3, 20),
    ("honest", {**HONEST_LINE, "MaxSeq": 3, "W": 2, "MaxFaults": 2, "SeenMayClear": True}, 4, 20),
    ("stale", {"MaxSeq": 3, "W": 1, "MaxFaults": 0, "MaxRequests": 0, "ByzFrames": 10}, 5, 30),
    ("reorder", {"FifoLinks": False, "MaxSeq": 2, "W": 2, "MaxFaults": 1, "ByzFrames": 2}, 6, 30),
]


def tla_value(v):
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, (set, frozenset)):
        return "{" + ", ".join(f'"{x}"' for x in sorted(v)) + "}"
    return str(v)


def write_config(path, module, base, overrides, invariants, properties):
    constants = {**base, **overrides}
    if module == "MC" and not properties:
        spec = "SafetySpec"
    elif "EveryNeighbourBans" in properties:
        spec = "ReplayerSpec"
    else:
        spec = "Spec"
    lines = [f"SPECIFICATION {spec}"]
    if module == "Reputation" and not properties:
        lines.append("VIEW View")
    lines += [f"INVARIANT {i}" for i in invariants]
    lines += [f"PROPERTY {p}" for p in properties]
    lines.append("CONSTANTS")
    for name, value in constants.items():
        if isinstance(value, str):
            lines.append(f"  {name} <- {value}")
        else:
            lines.append(f"  {name} = {tla_value(value)}")
    path.write_text("\n".join(lines) + "\n", encoding="ascii")


def run(name, entry, out_dir):
    module, base, overrides, invariants, properties, expected, trace_value = entry
    work = Path(tempfile.mkdtemp(prefix=f"tlc-{name}-"))
    try:
        for f in HERE.glob("*.tla"):
            shutil.copy(f, work)
        write_config(work / f"{name}.cfg", module, base, overrides, invariants, properties)
        started = time.time()
        log = out_dir / f"{name}.txt"
        with open(log, "w", encoding="utf-8") as sink:
            subprocess.run(
                ["java", "-XX:+UseParallelGC", "-cp", str(HERE / "tla2tools.jar"), "tlc2.TLC",
                 "-workers", "auto", "-cleanup", "-deadlock", "-config", f"{name}.cfg",
                 f"{module}.tla"],
                cwd=work, stdout=sink, stderr=subprocess.STDOUT,
            )
        seconds = time.time() - started
    finally:
        shutil.rmtree(work, ignore_errors=True)
    output = log.read_text(encoding="utf-8", errors="replace")

    violated = re.search(r"Invariant (\w+) is violated", output)
    temporal = "Temporal properties were violated" in output
    clean = "Model checking completed. No error has been found." in output
    if clean:
        outcome = "pass"
    elif violated:
        outcome = violated.group(1)
    elif temporal:
        outcome = properties[0] if len(properties) == 1 else "temporal"
    else:
        outcome = "ERROR"

    ok = outcome == expected
    if ok and trace_value is not None:
        elapsed = [int(x) for x in re.findall(r"elapsed = (\d+)", output)]
        ok = bool(elapsed) and elapsed[-1] == trace_value
    counts = re.search(r"([\d,]+) states generated, ([\d,]+) distinct states found", output)
    depth = re.search(r"depth of the complete state graph search is (\d+)", output)
    print(f"{name:24} {'ok ' if ok else 'BAD'} expected={expected:22} outcome={outcome:22} "
          f"generated={counts.group(1) if counts else '-':>12} "
          f"distinct={counts.group(2) if counts else '-':>11} "
          f"depth={depth.group(1) if depth else '-':>4} {seconds:7.1f}s", flush=True)
    return ok


def export_transitions(path):
    """Write every distinct reputation transition of the rep-free state graph to `path`, for
    the Rust test that replays them through PeerReputation."""
    module, base, overrides, invariants, properties, _, _ = CHECKS["rep-free"]
    work = Path(tempfile.mkdtemp(prefix="tlc-dump-"))
    try:
        for f in HERE.glob("*.tla"):
            shutil.copy(f, work)
        write_config(work / "dump.cfg", module, base, overrides, invariants, properties)
        subprocess.run(
            ["java", "-XX:+UseParallelGC", "-cp", str(HERE / "tla2tools.jar"), "tlc2.TLC",
             "-workers", "1", "-cleanup", "-deadlock", "-dump", "dot,actionlabels", "graph",
             "-config", "dump.cfg", f"{module}.tla"],
            cwd=work, capture_output=True, text=True, check=True,
        )
        dot = (work / "graph.dot").read_text(encoding="utf-8")
    finally:
        shutil.rmtree(work, ignore_errors=True)

    def record(label):
        m = re.search(r"banned \|-> (\w+), score \|-> (\d+), ages \|-> <<([^>]*)>>", label)
        ages = [a.strip() for a in m.group(3).split(",") if a.strip()]
        return f"{m.group(2)},{' '.join(ages)},{m.group(1).lower()}"

    states = {n: record(label) for n, label in re.findall(r'^(-?\d+) \[label="(.*?)"', dot, re.M)}
    edges = re.findall(r'^(-?\d+) -> (-?\d+) \[label="(\w+)"', dot, re.M)
    rows = sorted({f"{states[a]},{action},{states[b]}" for a, b, action in edges})
    header = "score,ages,banned,action,next_score,next_ages,next_banned"
    with open(path, "w", encoding="ascii", newline="\n") as f:
        f.write("\n".join([header] + rows) + "\n")
    print(f"{len(rows)} transitions written to {path}")


class TlaParser:
    """The values TLC prints: sets and sequences (lists, tuples), records and functions (dicts)."""

    def __init__(self, text):
        self.text, self.pos = text, 0

    def skip(self):
        while self.pos < len(self.text) and self.text[self.pos].isspace():
            self.pos += 1

    def take(self, token):
        self.skip()
        if self.text.startswith(token, self.pos):
            self.pos += len(token)
            return True
        return False

    def items(self, close):
        out = []
        if self.take(close):
            return out
        while True:
            out.append(self.value())
            if self.take(close):
                return out
            if not self.take(","):
                raise ValueError(f"expected , at {self.pos}")

    def value(self):
        if self.take("<expression"):  # how simulation mode prints an unevaluated constant
            inner = self.value()
            self.take(">")
            return inner
        if self.take("{"):
            return self.items("}")
        if self.take("<<"):
            return tuple(self.items(">>"))
        if self.take("["):
            constant = re.compile(r"\s*\w+\s*\\in").match(self.text, self.pos)
            if constant:  # [p \in {...} |-> v]: TLC prints a constant function this way
                self.pos = constant.end()
                domain = self.value()
                self.take("|->")
                value = self.value()
                self.take("]")
                return {key: value for key in domain}
            record = {}
            while not self.take("]"):
                self.take(",")
                match = re.compile(r"\s*(\w+)\s*\|->").match(self.text, self.pos)
                self.pos = match.end()
                record[match.group(1)] = self.value()
            return record
        if self.take("("):
            function = {}
            while not self.take(")"):
                self.take("@@")
                key = self.value()
                self.take(":>")
                function[key] = self.value()
            return function
        if self.take("TRUE"):
            return True
        if self.take("FALSE"):
            return False
        match = re.compile(r'"([^"]*)"|(-?\d+)').match(self.text, self.pos)
        self.pos = match.end()
        return match.group(1) if match.group(1) is not None else int(match.group(2))


def parse_trace(text):
    states = []
    for block in re.split(r"^STATE_\d+ ==\s*$", text, flags=re.M)[1:]:
        block = block.split("=====")[0]
        parts = re.split(r"^/\\ (\w+) = ", block, flags=re.M)[1:]
        states.append({n: TlaParser(v).value() for n, v in zip(parts[0::2], parts[1::2])})
    return states


def message_of(m):
    identifier = m["id"]
    return {"id": f"{identifier[0]}/{identifier[1]}", "o": m["origin"], "k": m["seq"]}


def node_view(state, n):
    """What the replay compares with the code, for honest node `n`."""
    window = {o: [k, sorted({x[1] for x in state["deliveredBy"][n] if x[0] == o})]
              for o, k in state["seqMax"][n].items() if k > 0}
    return {
        "d": sorted(list(x) for x in state["delivered"][n]),
        "rep": {p: [r["score"], r["banned"]] for p, r in state["rep"][n].items()},
        "seen": sorted([x[0][0], f"{x[0][1][0]}/{x[0][1][1]}", x[1]] for x in state["seen"][n]),
        "win": window,
    }


def trace_events(states):
    """One event per step of a TLC behaviour, each with the views of the nodes it changed."""
    events = []
    for prev, cur in zip(states, states[1:]):
        grown = {p: cur["link"][p][len(prev["link"][p]):] for p in cur["link"]
                 if len(cur["link"][p]) > len(prev["link"][p])}
        shrunk = [p for p in cur["link"] if len(cur["link"][p]) < len(prev["link"][p])]
        gone = [q for q in prev["requests"] if q not in cur["requests"]]
        if cur["faults"] != prev["faults"] or cur["byzSent"] != prev["byzSent"]:
            continue  # the network lost something, or the adversary queued a frame
        if cur["counter"] != prev["counter"]:
            o = next(o for o in cur["counter"] if cur["counter"][o] != prev["counter"][o])
            event = {"a": "originate", "o": o, "k": cur["counter"][o], "to": sorted(p[1] for p in grown)}
        elif cur["asked"] != prev["asked"]:
            q = next(q for q in cur["requests"] if q not in prev["requests"])
            event = {"a": "request", "r": q["from"], "s": q["to"], "rid": q["rid"],
                     "held": sorted(list(x) for x in q["held"]),
                     "max": {o: k for o, k in q["max"].items() if k > 0}}
        elif gone:
            q = gone[0]
            if q["to"] not in cur["seen"]:
                continue  # the adversary ignored it
            frames = grown.get((q["to"], q["from"]), ())
            event = {"a": "answer", "r": q["from"], "s": q["to"], "rid": q["rid"],
                     "batch": sorted([f["msg"]["origin"], f["msg"]["seq"]] for f in frames)}
        elif shrunk:
            s, r = shrunk[0]
            if r not in cur["seen"]:
                continue  # the adversary read a frame
            before, after = prev["link"][(s, r)], cur["link"][(s, r)]
            taken = next(i for i in range(len(before)) if before[:i] + before[i + 1:] == after)
            frame = before[taken]
            event = {"a": "receive", "s": s, "r": r, **message_of(frame["msg"]),
                     "sig": frame["sig"], "tag": frame["tag"],
                     "to": sorted(p[1] for p in grown)}
        elif cur["rep"] != prev["rep"]:
            event = {"a": "tick"}
        else:
            n = next(n for n in cur["seen"] if cur["seen"][n] != prev["seen"][n])
            event = {"a": "clear", "n": n}
        changes = {}
        for n in cur["seen"]:
            before, after = node_view(prev, n), node_view(cur, n)
            delta = {field: after[field] for field in after if after[field] != before[field]}
            if delta:
                changes[n] = delta
        if changes:
            event["x"] = changes
        events.append(event)
    return events


def export_traces(path):
    """Write random behaviours of the protocol model to `path`, for the Rust test that replays
    them through NodeRuntime. TLC's simulation mode with a fixed seed and one worker."""
    with open(path, "w", encoding="ascii", newline="\n") as sink:
        for name, overrides, seed, count in TRACE_SETS:
            constants = {**PROTOCOL, **overrides}
            work = Path(tempfile.mkdtemp(prefix=f"tlc-sim-{name}-"))
            try:
                for f in HERE.glob("*.tla"):
                    shutil.copy(f, work)
                write_config(work / "sim.cfg", "MC", PROTOCOL, overrides, [], [])
                subprocess.run(
                    ["java", "-XX:+UseParallelGC", "-cp", str(HERE / "tla2tools.jar"), "tlc2.TLC",
                     "-simulate", f"file=tr,num={count}", "-depth", "60", "-seed", str(seed),
                     "-workers", "1", "-deadlock", "-config", "sim.cfg", "MC.tla"],
                    cwd=work, capture_output=True, text=True, check=True,
                )
                files = sorted(work.glob("tr_*"), key=lambda f: int(f.name.rsplit("_", 1)[1]))
                for f in files:
                    states = parse_trace(f.read_text(encoding="utf-8"))
                    edges = sorted(list(p) for p in states[0]["link"] if p[0] < p[1])
                    line = {"set": name, "w": constants["W"], "edges": edges,
                            "honest": sorted(states[0]["seen"]),
                            "events": trace_events(states)}
                    sink.write(json.dumps(line, separators=(",", ":")) + "\n")
                print(f"{name}: {len(files)} traces")
            finally:
                shutil.rmtree(work, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=None, help="folder for the raw TLC output")
    parser.add_argument("--export-transitions", default=None, metavar="CSV",
                        help="write the reputation transitions to CSV and stop")
    parser.add_argument("--export-traces", default=None, metavar="JSONL",
                        help="write behaviours of the protocol model to JSONL and stop")
    parser.add_argument("--quick", action="store_true",
                        help="leave out the checks that take minutes (for CI)")
    parser.add_argument("names", nargs="*", help="checks to run (default: all)")
    args = parser.parse_args()
    if args.export_transitions:
        export_transitions(args.export_transitions)
        return
    if args.export_traces:
        export_traces(args.export_traces)
        return
    out_dir = Path(args.out) if args.out else Path(tempfile.mkdtemp(prefix="tlc-out-"))
    out_dir.mkdir(parents=True, exist_ok=True)
    unknown = [n for n in args.names if n not in CHECKS]
    if unknown:
        sys.exit(f"unknown checks: {unknown}")
    names = args.names or [n for n in CHECKS if not (args.quick and n in SLOW)]
    results = [run(n, CHECKS[n], out_dir) for n in names]
    print(f"{sum(results)} of {len(results)} checks as expected; raw output in {out_dir}")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
