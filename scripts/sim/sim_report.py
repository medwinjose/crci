#!/usr/bin/env python3
"""Tables, criteria checks and plots from a run_sim.py results.csv. Standard library only.

Writes into OUT: cross-check.csv (simulation vs testbed), tables.md, criteria-check.txt and
figures/*.svg. Runtime comes from OUT/timing.csv, a one-worker sweep, when it exists: runs that share
the CPU say little about cost. Exits non-zero if a criterion in docs/evidence/sim-2026-11-04/criteria.txt fails.

Usage: sim_report.py OUT [TESTBED_RESULTS.csv]
"""

import csv
import math
import os
import sys
from collections import defaultdict

DEFAULT_TESTBED = "docs/evidence/testbed-fast-recovery-2026-11-04/r4/results.csv"
TOPOLOGY_COLUMNS = {"mean_deg": "mean_deg", "max_deg": "max_deg", "diam": "diam", "aspl": "aspl",
                    "clust": "clust"}
EVICTION_TOLERANCE_MS = 200
SPECS = ["one", "0", "10", "20", "30", "45"]
SIZES = [100, 500, 1000, 2000, 5000]
COLORS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4"]
INK, MUTED, GRID, SURFACE = "#0b0b0b", "#52514e", "#e4e3df", "#fcfcfb"


def num(row, key):
    value = row[key]
    return float(value) if value not in ("", "None") else None


def all_of(fraction):
    got, total = fraction.split("/")
    return got == total


def cross_check(rows, testbed_path):
    """One row per cross-check run; the second value is the list of failures."""
    with open(testbed_path) as f:
        testbed = {r["run"]: r for r in csv.DictReader(f)}
    out, failures = [], []
    for sim in rows:
        name = f"{sim['kind']}-{sim['N']}-s{sim['seed']}"
        real = testbed.get(name)
        if real is None:
            failures.append(f"{name}: no testbed run")
            continue
        banned_sim = f"{sim['coverage_banned']}/{sim['coverage_links']}"
        evict_diff = abs(num(sim, "evict_sim_median_ms") - num(real, "evict_med_ms"))
        checks = {
            "topology": all(num(sim, a) == num(real, b) for a, b in TOPOLOGY_COLUMNS.items()),
            "banned_by": banned_sim == real["banned_by"] and all_of(banned_sim),
            "false_evictions": num(sim, "false_evictions") == 0 == num(real, "false_evict"),
            "delivery": num(sim, "delivery_pre") == num(sim, "delivery_post") == 1.0
            and num(real, "deliv_pre") == num(real, "deliv_post") == 1.0,
            "downstream": all(all_of(x) for x in (sim["down_pre"], sim["down_post"],
                                                  real["down_pre"], real["down_post"])),
            "eviction": evict_diff <= EVICTION_TOLERANCE_MS,
        }
        failures += [f"{name}: {k}" for k, ok in checks.items() if not ok]
        out.append({
            "run": name, "banned_by_sim": banned_sim, "banned_by_testbed": real["banned_by"],
            "false_evict_sim": sim["false_evictions"], "false_evict_testbed": real["false_evict"],
            "deliv_post_sim": sim["delivery_post"], "deliv_post_testbed": real["deliv_post"],
            "down_post_sim": sim["down_post"], "down_post_testbed": real["down_post"],
            "evict_ms_sim": sim["evict_sim_median_ms"], "evict_ms_testbed": real["evict_med_ms"],
            "frames_sim": sim["frames"], "topology_equal": checks["topology"],
        })
    return out, failures


def scale_failures(rows):
    failures = []
    for r in rows:
        name = r["run"]
        if num(r, "false_evictions") or num(r, "honest_strikes"):
            failures.append(f"{name}: honest false eviction or strike")
        if r["coverage_banned"] != r["coverage_links"]:
            failures.append(f"{name}: coverage {r['coverage_banned']}/{r['coverage_links']}")
        if num(r, "non_neighbour_records"):
            failures.append(f"{name}: records of non-neighbours")
        if num(r, "delivery_post_reachable_min") != 1.0:
            failures.append(f"{name}: delivery within the component below 1.0")
        if r["attackers_spec"] in ("one", "0"):
            if num(r, "rate_drops") or num(r, "origin_drops"):
                failures.append(f"{name}: budget drops")
            if num(r, "delivery_post") != 1.0:
                failures.append(f"{name}: delivery below 1.0")
    return failures


def long_failures(rows):
    failures = []
    for r in rows:
        name = r["run"]
        if num(r, "false_evictions") or num(r, "honest_strikes"):
            failures.append(f"{name}: honest false eviction or strike")
        if r["attackers_spec"] == "0" and num(r, "delivery_post") != 1.0:
            failures.append(f"{name}: delivery below 1.0")
        if r["coverage_banned"] != r["coverage_links"]:
            failures.append(f"{name}: coverage {r['coverage_banned']}/{r['coverage_links']}")
    return failures


def mean(values):
    return sum(values) / len(values)


def group_by(rows, *keys):
    groups = defaultdict(list)
    for r in rows:
        groups[tuple(r[k] for k in keys)].append(r)
    return groups


def scale_table(rows):
    lines = [
        "| Topology | N | Attackers | Runs | False evictions | Strikes on honest | Attacker bans / "
        "honest neighbours | Non-neighbour records | Runs partitioned | Largest component, mean | "
        "Delivery after bans, all honest, mean (min) | Same component, min | Frames, mean |",
        "|" + "---|" * 13,
    ]
    groups = group_by(rows, "kind", "N", "attackers_spec")
    for (kind, n, spec), g in sorted(groups.items(), key=lambda kv: (
            kv[0][0] != "rgg", int(kv[0][1]), SPECS.index(kv[0][2]))):
        banned = sum(int(r["coverage_banned"]) for r in g)
        links = sum(int(r["coverage_links"]) for r in g)
        label = "one" if spec == "one" else f"{spec} %"
        post = [num(r, "delivery_post") for r in g]
        post_min = [num(r, "delivery_post_min") for r in g]
        lines.append(
            f"| {kind} | {n} | {label} | {len(g)} "
            f"| {sum(int(r['false_evictions']) for r in g)} "
            f"| {sum(int(r['honest_strikes']) for r in g)} | {banned}/{links} "
            f"| {sum(int(r['non_neighbour_records']) for r in g)} "
            f"| {sum(1 for r in g if int(r['honest_components']) > 1)}/{len(g)} "
            f"| {mean([num(r, 'largest_component_frac') for r in g]):.4f} "
            f"| {mean(post):.4f} ({min(post_min):.4f}) "
            f"| {min(num(r, 'delivery_post_reachable_min') for r in g):.4f} "
            f"| {mean([num(r, 'frames') for r in g]):,.0f} |")
    return lines


def cost_table(rows, timing):
    lines = [
        "| Topology | N | Mean degree | Runtime alone, s, mean (max) | Peak memory, MiB (max) | "
        "Memory per node, MiB | Seen IDs per node, mean | Sender records per node, mean | "
        "Kept messages per node, mean | Known keys per node, mean |",
        "|" + "---|" * 10,
    ]
    groups = group_by(rows, "kind", "N")
    for (kind, n), g in sorted(groups.items(), key=lambda kv: (kv[0][0] != "rgg", int(kv[0][1]))):
        rss = max(num(r, "peak_rss_kib") for r in g) / 1024
        alone = [num(r, "runtime_s") for r in timing if r["kind"] == kind and r["N"] == n]
        lines.append(
            f"| {kind} | {n} | {mean([num(r, 'mean_deg') for r in g]):.2f} "
            f"| {mean(alone):.1f} ({max(alone):.1f}) | {rss:,.0f} | {rss / int(n):.2f} "
            f"| {mean([num(r, 'state_seen_ids_mean') for r in g]):.0f} "
            f"| {mean([num(r, 'state_seen_senders_mean') for r in g]):.0f} "
            f"| {mean([num(r, 'state_recent_msgs_mean') for r in g]):.0f} "
            f"| {mean([num(r, 'state_known_keys_mean') for r in g]):.0f} |")
    return lines


def long_table(rows):
    lines = [
        "| Topology | Attackers | Seed | Messages | False evictions | Strikes on honest | Bans / "
        "honest neighbours | Readmissions | Delivery after bans, mean (min) | Rate drops | "
        "Origin drops | Seen-ID cache clears | Seen IDs per node, mean (max) | Kept messages per "
        "node, mean (max) | Known keys, mean (max) | Runtime, s | Peak memory, MiB |",
        "|" + "---|" * 17,
    ]
    for r in sorted(rows, key=lambda r: (r["kind"] != "rgg", int(r["attackers_spec"]), r["seed"])):
        lines.append(
            f"| {r['kind']} | {r['attackers_spec']} % | {r['seed']} | {r['messages']} "
            f"| {r['false_evictions']} | {r['honest_strikes']} "
            f"| {r['coverage_banned']}/{r['coverage_links']} | {r['readmissions']} "
            f"| {num(r, 'delivery_post'):.4f} ({num(r, 'delivery_post_min'):.4f}) "
            f"| {r['rate_drops']} | {r['origin_drops']} | {r['seen_cache_clears']} "
            f"| {num(r, 'state_seen_ids_mean'):.0f} ({r['state_seen_ids_max']}) "
            f"| {num(r, 'state_recent_msgs_mean'):.0f} ({r['state_recent_msgs_max']}) "
            f"| {num(r, 'state_known_keys_mean'):.0f} ({r['state_known_keys_max']}) "
            f"| {num(r, 'runtime_s'):.0f} | {num(r, 'peak_rss_kib') / 1024:,.0f} |")
    return lines


def line_panel(x0, title, xs, series, x_label, y_label, y_range, x_format=str, log_x=False,
               width=380, height=250):
    """One panel of a figure, as SVG at horizontal offset x0. `series` is a list of
    (label, color, {x: (mean, low, high)}): lines through the means, whiskers low to high."""
    left, top, right, bottom = 56, 46, 74, 44
    plot_w, plot_h = width - left - right, height - top - bottom
    pos = [math.log10(x) for x in xs] if log_x else list(xs)

    def px(x):
        value = math.log10(x) if log_x else x
        return x0 + left + plot_w * (value - pos[0]) / (pos[-1] - pos[0])

    def py(y):
        return top + plot_h * (1 - (y - y_range[0]) / (y_range[1] - y_range[0]))

    parts = [f'<text x="{x0 + left}" y="18" class="title">{title}</text>']
    for i in range(5):
        y = y_range[0] + (y_range[1] - y_range[0]) * i / 4
        parts.append(f'<line x1="{x0 + left}" x2="{x0 + left + plot_w}" y1="{py(y):.1f}" '
                     f'y2="{py(y):.1f}" stroke="{GRID}" stroke-width="1"/>')
        parts.append(f'<text x="{x0 + left - 6}" y="{py(y) + 4:.1f}" text-anchor="end" '
                     f'class="tick">{y:g}</text>')
    for x in xs:
        parts.append(f'<text x="{px(x):.1f}" y="{top + plot_h + 16}" text-anchor="middle" '
                     f'class="tick">{x_format(x)}</text>')
    parts.append(f'<text x="{x0 + left + plot_w / 2}" y="{height - 8}" text-anchor="middle" '
                 f'class="tick">{x_label}</text>')
    parts.append(f'<text transform="translate({x0 + 13},{top + plot_h / 2}) rotate(-90)" '
                 f'text-anchor="middle" class="tick">{y_label}</text>')
    legend_x = x0 + left
    for label, color, _ in series:
        parts.append(f'<circle cx="{legend_x + 4}" cy="27" r="4" fill="{color}"/>'
                     f'<text x="{legend_x + 12}" y="31" class="label">{label}</text>')
        legend_x += 18 + 7 * len(label)
    label_ys = []
    for label, color, points in series:
        coords = [(px(x), py(points[x][0])) for x in xs if x in points]
        path = " ".join(f"{a:.1f},{b:.1f}" for a, b in coords)
        parts.append(f'<polyline points="{path}" fill="none" stroke="{color}" stroke-width="2" '
                     'stroke-linejoin="round"/>')
        for x in xs:
            if x not in points:
                continue
            m, low, high = points[x]
            parts.append(f'<line x1="{px(x):.1f}" x2="{px(x):.1f}" y1="{py(low):.1f}" '
                         f'y2="{py(high):.1f}" stroke="{color}" stroke-width="1"/>')
            parts.append(
                f'<circle cx="{px(x):.1f}" cy="{py(m):.1f}" r="4" fill="{color}" '
                f'stroke="{SURFACE}" stroke-width="2"><title>{label}, {x_format(x)}: {m:.4g} '
                f'(min {low:.4g}, max {high:.4g})</title></circle>')
        # Direct label at the line's end; with many close lines the legend alone reads better.
        if len(series) > 4:
            continue
        y = coords[-1][1] + 4
        while any(abs(y - other) < 12 for other in label_ys):
            y += 12
        label_ys.append(y)
        parts.append(f'<text x="{coords[-1][0] + 8:.1f}" y="{y:.1f}" class="label">{label}</text>')
    return "".join(parts)


def write_figure(path, panels, subtitle, width=780, height=272):
    style = (f"text{{font-family:system-ui,sans-serif;fill:{MUTED}}}"
             f".title{{font-size:13px;font-weight:600;fill:{INK}}}"
             ".tick{font-size:11px}.label{font-size:11px}")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
                f'width="{width}" height="{height}"><style>{style}</style>'
                f'<rect width="{width}" height="{height}" fill="{SURFACE}"/>'
                + "".join(panels) +
                f'<text x="56" y="{height - 4}" class="tick">{subtitle}</text></svg>\n')


def by_fraction(rows, kind, key):
    """Per N: {f: (mean, min, max)} of `key` over the seeds."""
    series = []
    for color, n in zip(COLORS, SIZES):
        points = {}
        for f in (0, 10, 20, 30, 45):
            values = [num(r, key) for r in rows if r["kind"] == kind and int(r["N"]) == n
                      and r["attackers_spec"] == str(f) and num(r, key) is not None]
            if values:
                points[f] = (mean(values), min(values), max(values))
        if points:
            series.append((f"N={n}", color, points))
    return series


def by_size(rows, key, scale=1.0):
    series = []
    for color, kind in zip(COLORS, ("rgg", "mesh")):
        points = {}
        for n in SIZES:
            values = [num(r, key) * scale for r in rows if r["kind"] == kind and int(r["N"]) == n]
            if values:
                points[n] = (mean(values), min(values), max(values))
        if points:
            series.append((kind, color, points))
    return series


def figures(out, rows, timing):
    os.makedirs(f"{out}/figures", exist_ok=True)
    fractions = [0, 10, 20, 30, 45]
    note = "In-process simulation (simulated time, no sockets, no loss). Lines: mean of 3 seeds; whiskers: min to max."
    pct = lambda f: f"{f}"  # noqa: E731
    write_figure(f"{out}/figures/delivery-vs-attackers.svg", [
        line_panel(0, "rgg: delivery after the bans", fractions, by_fraction(rows, "rgg", "delivery_post"),
                   "attackers, % of nodes", "share of honest nodes", (0, 1), pct),
        line_panel(390, "mesh: delivery after the bans", fractions, by_fraction(rows, "mesh", "delivery_post"),
                   "attackers, % of nodes", "share of honest nodes", (0, 1), pct),
    ], note)
    write_figure(f"{out}/figures/largest-component-vs-attackers.svg", [
        line_panel(0, "rgg: largest honest component", fractions,
                   by_fraction(rows, "rgg", "largest_component_frac"),
                   "attackers, % of nodes", "share of honest nodes", (0, 1), pct),
        line_panel(390, "mesh: largest honest component", fractions,
                   by_fraction(rows, "mesh", "largest_component_frac"),
                   "attackers, % of nodes", "share of honest nodes", (0, 1), pct),
    ], note)
    runtime_max = max(num(r, "runtime_s") for r in timing)
    rss_max = max(num(r, "peak_rss_kib") for r in rows) / 1024 / 1024
    write_figure(f"{out}/figures/cost-vs-n.svg", [
        line_panel(0, "Runtime of one run", SIZES, by_size(timing, "runtime_s"), "nodes (log scale)",
                   "seconds", (0, math.ceil(runtime_max / 20) * 20), str, log_x=True),
        line_panel(390, "Peak memory of one run", SIZES, by_size(rows, "peak_rss_kib", 1 / 1024 / 1024),
                   "nodes (log scale)", "GiB", (0, math.ceil(rss_max / 4) * 4), str, log_x=True),
    ], "One process, one thread, 60 simulated seconds at 1 message/s, all attacker settings. Lines: mean; whiskers: min to max.")


def main():
    if len(sys.argv) not in (2, 3):
        raise SystemExit(__doc__)
    out = sys.argv[1]
    testbed = sys.argv[2] if len(sys.argv) == 3 else DEFAULT_TESTBED
    with open(f"{out}/results.csv") as f:
        rows = list(csv.DictReader(f))
    cross = [r for r in rows if r["group"] == "cross"]
    scale = [r for r in rows if r["group"] == "scale"]
    long_runs = [r for r in rows if r["group"] == "long"]

    timing = scale
    if os.path.exists(f"{out}/timing.csv"):
        with open(f"{out}/timing.csv") as f:
            timing = list(csv.DictReader(f))

    cross_rows, cross_failed = cross_check(cross, testbed)
    with open(f"{out}/cross-check.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(cross_rows[0]), lineterminator="\n")
        w.writeheader()
        w.writerows(cross_rows)
    with open(f"{out}/tables.md", "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(["### Scale", ""] + scale_table(scale) + ["", "### Cost and state", ""]
                          + cost_table(scale, timing) + ["", "### Long runs", ""] + long_table(long_runs))
                + "\n")
    figures(out, scale, timing)

    failures = (cross_failed + scale_failures(scale) + long_failures(long_runs))
    report = [f"cross-check runs: {len(cross)}, failed criteria: {len(cross_failed)}",
              f"scale runs: {len(scale)}, failed criteria: {len(scale_failures(scale))}",
              f"long runs: {len(long_runs)}, failed criteria: {len(long_failures(long_runs))}"]
    report += [f"FAIL {x}" for x in failures]
    with open(f"{out}/criteria-check.txt", "w", newline="\n") as f:
        f.write("\n".join(report) + "\n")
    print("\n".join(report))
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
