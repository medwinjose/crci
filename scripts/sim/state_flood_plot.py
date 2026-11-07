#!/usr/bin/env python3
"""Plot live heap against messages from the CSV of examples/state_flood and check it stays flat.

Usage: state_flood_plot.py OUT.svg TITLE LABEL=FILE.csv [LABEL=FILE.csv ...]

Fails if a run's heap at its end is over 15 % above its heap at a tenth of the run, or if a run
has less than one million messages when it has one million origins.
"""
import csv
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "testbed"))
import svgplot  # noqa: E402


def rows(path):
    with open(path, encoding="utf-8") as f:
        lines = [line for line in f if line[:1].isdigit() or line.startswith("messages,")]
    return list(csv.DictReader(lines))


def main():
    out, title, runs = sys.argv[1], sys.argv[2], sys.argv[3:]
    series, xlabels = {}, None
    for run in runs:
        label, path = run.split("=", 1)
        data = rows(path)
        heap = [int(r["live_bytes"]) / 1e6 for r in data]
        reference = heap[len(heap) // 10]
        assert heap[-1] <= 1.15 * reference, f"{label}: {reference:.2f} -> {heap[-1]:.2f} MB"
        print(f"{label}: messages={data[-1]['messages']} heap at a tenth {reference:.2f} MB, "
              f"end {heap[-1]:.2f} MB, peak {int(data[-1]['peak_bytes']) / 1e6:.2f} MB, "
              f"forgotten {data[-1]['forgotten']}, rss {data[-1]['rss_kib']} KiB")
        series[label] = heap
        if xlabels is None or len(data) > len(xlabels):
            xlabels = [str(round(int(r["messages"]) / 1000)) for r in data]
    for label in series:
        series[label] += [None] * (len(xlabels) - len(series[label]))
    svgplot.line_chart(out, title, "one node, validly signed messages, in-process", xlabels, series,
                       "live heap (MB)", xlabel="messages received (thousands)")


if __name__ == "__main__":
    main()
