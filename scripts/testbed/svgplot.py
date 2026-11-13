"""Line charts as standalone SVG, standard library only (the project has no plotting dependency).

line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0, ymax=None) draws one polyline
per series over categorical x positions; a None value leaves a gap. Colours are fixed and the
legend sits under the plot, so the figure reads the same in a light or a dark viewer. Every series
also has its own marker (and an optional dash), so the figure survives a greyscale print.
"""

import math

COLOURS = ["#4477aa", "#cc6677", "#228833", "#ccbb44", "#aa3377", "#66ccee"]
MARKERS = ["circle", "square", "triangle", "diamond"]

# Default layout: a standalone figure with a title and a subtitle. compact() switches to a canvas of
# printed size (1 unit = 1 pt at column width) without them, for a figure explained by its caption.
LAYOUT = {"width": 640, "height": 380, "left": 70, "right": 20, "top": 52, "bottom": 96,
          "legend_dx": 190, "legend_dy": 16, "font": 11, "marker": 3.5, "line": 2.0, "titles": True,
          "dodge": 0.0, "tick_y": 18, "label_y": 32, "legend_y": 36, "family": "Helvetica, Arial, sans-serif"}


def compact():
    LAYOUT.update(width=248, height=172, left=36, right=8, top=17, bottom=48, legend_dx=70, legend_dy=10,
                  font=7.5, marker=2.4, line=1.1, titles=False, dodge=2.4, tick_y=10, label_y=20,
                  legend_y=20, family="Times New Roman, Times, serif")


def _fmt(v):
    return f"{v:.3g}"


def nice_ticks(lo, hi, most=5):
    """Round tick values in [lo, hi]: step 1, 2 or 5 times a power of ten, at most `most` intervals."""
    span = hi - lo
    e = math.floor(math.log10(span))
    step = next(m * 10 ** k for k in range(e - 3, e + 2) for m in (1, 2, 5) if span / (m * 10 ** k) <= most)
    first = math.ceil(lo / step - 1e-9)
    return [round(k * step, 10) for k in range(first, math.floor(hi / step + 1e-9) + 1)]


def _marker(kind, x, y, r, colour):
    if kind == "square":
        return f'<rect x="{x - r:.1f}" y="{y - r:.1f}" width="{2 * r:.1f}" height="{2 * r:.1f}" fill="{colour}"/>'
    if kind == "triangle":
        return (f'<polygon points="{x:.1f},{y - 1.25 * r:.1f} {x - 1.15 * r:.1f},{y + r:.1f} '
                f'{x + 1.15 * r:.1f},{y + r:.1f}" fill="{colour}"/>')
    if kind == "diamond":
        return (f'<polygon points="{x:.1f},{y - 1.3 * r:.1f} {x + 1.3 * r:.1f},{y:.1f} {x:.1f},{y + 1.3 * r:.1f} '
                f'{x - 1.3 * r:.1f},{y:.1f}" fill="{colour}"/>')
    return f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.1f}" fill="{colour}"/>'


def _legend(out, n, name, colour, marker, dash, height, left):
    lx = left + (n % 3) * LAYOUT["legend_dx"]
    ly = height - LAYOUT["legend_y"] + (n // 3) * LAYOUT["legend_dy"]
    dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
    mark = _marker(marker, lx + 7, ly - 3.5, LAYOUT["marker"], colour) if marker else ""
    out.append(f'<line x1="{lx}" y1="{ly - 3.5}" x2="{lx + 14}" y2="{ly - 3.5}" stroke="{colour}" '
               f'stroke-width="{LAYOUT["line"]}"{dash_attr}/>' + mark + f'<text x="{lx + 18}" y="{ly}">{name}</text>')


def _head(out_width, out_height):
    return [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {out_width} {out_height}" '
            f'font-family="{LAYOUT["family"]}" font-size="{LAYOUT["font"]}">'
            f'<rect width="{out_width}" height="{out_height}" fill="white"/>']


def _titles(out, title, subtitle, panel, left):
    if LAYOUT["titles"]:
        out += [f'<text x="{left}" y="18" font-size="13" font-weight="bold">{title}</text>',
                f'<text x="{left}" y="34" fill="#555">{subtitle}</text>']
    elif panel:
        out.append(f'<text x="{left}" y="9" font-weight="bold">{panel}</text>')


def line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0.0, ymax=None, xlabel="", errors=None,
               panel="", dashes=None, yticks=None):
    """errors: optional {series name: [half-height or None per point]}, drawn as vertical bars with caps.
    dashes: optional {series name: stroke-dasharray}; panel: a short label drawn above a compact plot."""
    errors, dashes = errors or {}, dashes or {}
    width, height = LAYOUT["width"], LAYOUT["height"]
    left, right, top, bottom = LAYOUT["left"], LAYOUT["right"], LAYOUT["top"], LAYOUT["bottom"]
    values = [v + (errors.get(name, [None] * len(ys))[i] or 0)
              for name, ys in series.items() for i, v in enumerate(ys) if v is not None]
    hi = ymax if ymax is not None else (max(values) * 1.1 if values else 1.0)
    hi = hi if hi > ymin else ymin + 1.0
    plot_w, plot_h = width - left - right, height - top - bottom
    shift = LAYOUT["dodge"]

    def x_at(i, n):
        return left + (plot_w * (i + 0.5) / len(xlabels)) + (n - (len(series) - 1) / 2) * shift

    def y_at(v):
        return top + plot_h * (1 - (v - ymin) / (hi - ymin))

    out = _head(width, height)
    _titles(out, title, subtitle, panel, left)
    for v in (yticks if yticks is not None else nice_ticks(ymin, hi)):
        y = y_at(v)
        out.append(f'<line x1="{left}" y1="{y:.1f}" x2="{width - right}" y2="{y:.1f}" stroke="#ddd" stroke-width="0.6"/>')
        out.append(f'<text x="{left - 4}" y="{y + LAYOUT["font"] * 0.35:.1f}" text-anchor="end">{_fmt(v)}</text>')
    cy = top + plot_h / 2
    out.append(f'<text x="{LAYOUT["font"]}" y="{cy:.0f}" transform="rotate(-90 {LAYOUT["font"]} {cy:.0f})" '
               f'text-anchor="middle">{ylabel}</text>')
    for i, label in enumerate(xlabels):
        out.append(f'<text x="{left + plot_w * (i + 0.5) / len(xlabels):.1f}" y="{top + plot_h + LAYOUT["tick_y"]}" '
                   f'text-anchor="middle">{label}</text>')
    if xlabel:
        out.append(f'<text x="{left + plot_w / 2:.0f}" y="{top + plot_h + LAYOUT["label_y"]}" '
                   f'text-anchor="middle">{xlabel}</text>')
    out.append(f'<line x1="{left}" y1="{top + plot_h}" x2="{width - right}" y2="{top + plot_h}" stroke="black"/>'
               f'<line x1="{left}" y1="{top}" x2="{left}" y2="{top + plot_h}" stroke="black" stroke-width="0.6"/>')
    for n, (name, ys) in enumerate(series.items()):
        colour = COLOURS[n % len(COLOURS)] if not dashes else COLOURS[(n % 3) % len(COLOURS)]
        marker = MARKERS[n % 3 % len(MARKERS)] if dashes else MARKERS[n % len(MARKERS)]
        dash = dashes.get(name)
        points = [(x_at(i, n), y_at(v)) for i, v in enumerate(ys) if v is not None]
        if len(points) > 1:
            dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
            out.append('<polyline fill="none" stroke="%s" stroke-width="%s"%s points="%s"/>'
                       % (colour, LAYOUT["line"], dash_attr, " ".join(f"{x:.1f},{y:.1f}" for x, y in points)))
        for i, v in enumerate(ys):
            e = errors.get(name, [None] * len(ys))[i]
            if v is None or not e:
                continue
            x = x_at(i, n)
            y0, y1 = y_at(min(max(v - e, ymin), hi)), y_at(min(v + e, hi))
            out.append(f'<line x1="{x:.1f}" y1="{y0:.1f}" x2="{x:.1f}" y2="{y1:.1f}" stroke="{colour}" stroke-width="0.8"/>'
                       f'<line x1="{x - 1.8:.1f}" y1="{y0:.1f}" x2="{x + 1.8:.1f}" y2="{y0:.1f}" stroke="{colour}" stroke-width="0.8"/>'
                       f'<line x1="{x - 1.8:.1f}" y1="{y1:.1f}" x2="{x + 1.8:.1f}" y2="{y1:.1f}" stroke="{colour}" stroke-width="0.8"/>')
        for x, y in points:
            out.append(_marker(marker, x, y, LAYOUT["marker"], colour))
        _legend(out, n, name, colour, marker, dash, height, left)
    out.append("</svg>")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")


def xy_chart(path, title, subtitle, series, xlabel, ylabel, xmax, ymin=0.0, ymax=1.0, hlines=None, panel=""):
    """One polyline per series of (x, y) points on a numeric x axis from 0 to xmax; hlines: [(y, label)]."""
    width, height = LAYOUT["width"], LAYOUT["height"]
    left, right, top, bottom = LAYOUT["left"], LAYOUT["right"], LAYOUT["top"], LAYOUT["bottom"]
    plot_w, plot_h = width - left - right, height - top - bottom

    def x_at(v):
        return left + plot_w * v / xmax

    def y_at(v):
        return top + plot_h * (1 - (v - ymin) / (ymax - ymin))

    out = _head(width, height)
    _titles(out, title, subtitle, panel, left)
    for v in nice_ticks(ymin, ymax):
        out.append(f'<line x1="{left}" y1="{y_at(v):.1f}" x2="{width - right}" y2="{y_at(v):.1f}" stroke="#ddd" stroke-width="0.6"/>')
        out.append(f'<text x="{left - 4}" y="{y_at(v) + LAYOUT["font"] * 0.35:.1f}" text-anchor="end">{_fmt(v)}</text>')
    for xv in nice_ticks(0, xmax):
        out.append(f'<text x="{x_at(xv):.1f}" y="{top + plot_h + LAYOUT["tick_y"]}" text-anchor="middle">{_fmt(xv)}</text>')
    cy = top + plot_h / 2
    out.append(f'<text x="{LAYOUT["font"]}" y="{cy:.0f}" transform="rotate(-90 {LAYOUT["font"]} {cy:.0f})" '
               f'text-anchor="middle">{ylabel}</text>')
    out.append(f'<text x="{left + plot_w / 2:.0f}" y="{top + plot_h + LAYOUT["label_y"]}" text-anchor="middle">{xlabel}</text>')
    out.append(f'<line x1="{left}" y1="{top + plot_h}" x2="{width - right}" y2="{top + plot_h}" stroke="black"/>'
               f'<line x1="{left}" y1="{top}" x2="{left}" y2="{top + plot_h}" stroke="black" stroke-width="0.6"/>')
    for y, label in hlines or []:
        out.append(f'<line x1="{left}" y1="{y_at(y):.1f}" x2="{width - right}" y2="{y_at(y):.1f}" '
                   f'stroke="#888" stroke-dasharray="4 3"/><text x="{width - right - 2}" y="{y_at(y) - 3:.1f}" '
                   f'text-anchor="end" fill="#555" stroke="white" stroke-width="3" paint-order="stroke">{label}</text>')
    for n, (name, pts) in enumerate(series.items()):
        colour = COLOURS[n % len(COLOURS)]
        out.append('<polyline fill="none" stroke="%s" stroke-width="%s" points="%s"/>'
                   % (colour, LAYOUT["line"], " ".join(f"{x_at(x):.1f},{y_at(y):.1f}" for x, y in pts)))
        _legend(out, n, name, colour, None, None, height, left)
    out.append("</svg>")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
