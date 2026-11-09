"""A line chart as a standalone SVG, standard library only (the project has no plotting dependency).

line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0, ymax=None) draws one polyline
per series over categorical x positions; a None value leaves a gap. Colours are fixed and the
legend sits under the plot, so the figure reads the same in a light or a dark viewer.
"""

COLOURS = ["#4477aa", "#cc6677", "#228833", "#ccbb44", "#aa3377", "#66ccee"]

# Default layout: a standalone figure with a title and a subtitle. compact() switches to a small canvas
# without them, for a figure that is printed at column width and explained by its caption.
LAYOUT = {"width": 640, "height": 380, "left": 70, "right": 20, "top": 52, "bottom": 96,
          "legend_dx": 190, "titles": True}


def compact():
    LAYOUT.update(width=400, height=260, left=48, right=10, top=10, bottom=80, legend_dx=124, titles=False)


def _fmt(v):
    return f"{v:.3g}"


def line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0.0, ymax=None, xlabel="", errors=None):
    """errors: optional {series name: [half-height or None per point]}, drawn as vertical bars with caps."""
    errors = errors or {}
    width, height = LAYOUT["width"], LAYOUT["height"]
    left, right, top, bottom = LAYOUT["left"], LAYOUT["right"], LAYOUT["top"], LAYOUT["bottom"]
    values = [v + (errors.get(name, [None] * len(ys))[i] or 0)
              for name, ys in series.items() for i, v in enumerate(ys) if v is not None]
    hi = ymax if ymax is not None else (max(values) * 1.1 if values else 1.0)
    hi = hi if hi > ymin else ymin + 1.0
    plot_w, plot_h = width - left - right, height - top - bottom

    def x_at(i):
        return left + (plot_w * (i + 0.5) / len(xlabels))

    def y_at(v):
        return top + plot_h * (1 - (v - ymin) / (hi - ymin))

    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
           f'font-family="sans-serif" font-size="11"><rect width="{width}" height="{height}" fill="white"/>']
    if LAYOUT["titles"]:
        out += [f'<text x="{left}" y="18" font-size="13" font-weight="bold">{title}</text>',
                f'<text x="{left}" y="34" fill="#555">{subtitle}</text>']
    for t in range(5):
        v = ymin + (hi - ymin) * t / 4
        y = y_at(v)
        out.append(f'<line x1="{left}" y1="{y:.0f}" x2="{width - right}" y2="{y:.0f}" stroke="#ddd"/>')
        out.append(f'<text x="{left - 6}" y="{y + 4:.0f}" text-anchor="end">{_fmt(v)}</text>')
    out.append(f'<text x="12" y="{top + plot_h / 2:.0f}" transform="rotate(-90 12 {top + plot_h / 2:.0f})" '
               f'text-anchor="middle">{ylabel}</text>')
    for i, label in enumerate(xlabels):
        out.append(f'<text x="{x_at(i):.0f}" y="{top + plot_h + 16}" text-anchor="middle">{label}</text>')
    if xlabel:
        out.append(f'<text x="{left + plot_w / 2:.0f}" y="{top + plot_h + 32}" text-anchor="middle">{xlabel}</text>')
    out.append(f'<line x1="{left}" y1="{top + plot_h}" x2="{width - right}" y2="{top + plot_h}" stroke="black"/>')
    for n, (name, ys) in enumerate(series.items()):
        colour = COLOURS[n % len(COLOURS)]
        points = [(x_at(i), y_at(v)) for i, v in enumerate(ys) if v is not None]
        if len(points) > 1:
            out.append('<polyline fill="none" stroke="%s" stroke-width="2" points="%s"/>'
                       % (colour, " ".join(f"{x:.1f},{y:.1f}" for x, y in points)))
        for i, v in enumerate(ys):
            e = errors.get(name, [None] * len(ys))[i]
            if v is None or not e:
                continue
            x = x_at(i)
            y0, y1 = y_at(min(max(v - e, ymin), hi)), y_at(min(v + e, hi))
            out.append(f'<line x1="{x:.1f}" y1="{y0:.1f}" x2="{x:.1f}" y2="{y1:.1f}" stroke="{colour}" stroke-width="1.5"/>'
                       f'<line x1="{x - 4:.1f}" y1="{y0:.1f}" x2="{x + 4:.1f}" y2="{y0:.1f}" stroke="{colour}"/>'
                       f'<line x1="{x - 4:.1f}" y1="{y1:.1f}" x2="{x + 4:.1f}" y2="{y1:.1f}" stroke="{colour}"/>')
        for x, y in points:
            out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="3.5" fill="{colour}"/>')
        lx = left + (n % 3) * LAYOUT["legend_dx"]
        ly = height - 36 + (n // 3) * 16
        out.append(f'<rect x="{lx}" y="{ly - 9}" width="10" height="10" fill="{colour}"/>'
                   f'<text x="{lx + 14}" y="{ly}">{name}</text>')
    out.append("</svg>")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")


def xy_chart(path, title, subtitle, series, xlabel, ylabel, xmax, ymin=0.0, ymax=1.0, hlines=None):
    """One polyline per series of (x, y) points on a numeric x axis from 0 to xmax; hlines: [(y, label)]."""
    width, height = LAYOUT["width"], LAYOUT["height"]
    left, right, top, bottom = LAYOUT["left"], LAYOUT["right"], LAYOUT["top"], LAYOUT["bottom"]
    plot_w, plot_h = width - left - right, height - top - bottom

    def x_at(v):
        return left + plot_w * v / xmax

    def y_at(v):
        return top + plot_h * (1 - (v - ymin) / (ymax - ymin))

    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
           f'font-family="sans-serif" font-size="11"><rect width="{width}" height="{height}" fill="white"/>']
    if LAYOUT["titles"]:
        out += [f'<text x="{left}" y="18" font-size="13" font-weight="bold">{title}</text>',
                f'<text x="{left}" y="34" fill="#555">{subtitle}</text>']
    for t in range(5):
        v = ymin + (ymax - ymin) * t / 4
        out.append(f'<line x1="{left}" y1="{y_at(v):.0f}" x2="{width - right}" y2="{y_at(v):.0f}" stroke="#ddd"/>')
        out.append(f'<text x="{left - 6}" y="{y_at(v) + 4:.0f}" text-anchor="end">{_fmt(v)}</text>')
        xv = xmax * t / 4
        out.append(f'<text x="{x_at(xv):.0f}" y="{top + plot_h + 16}" text-anchor="middle">{_fmt(xv)}</text>')
    out.append(f'<text x="12" y="{top + plot_h / 2:.0f}" transform="rotate(-90 12 {top + plot_h / 2:.0f})" '
               f'text-anchor="middle">{ylabel}</text>')
    out.append(f'<text x="{left + plot_w / 2:.0f}" y="{top + plot_h + 32}" text-anchor="middle">{xlabel}</text>')
    out.append(f'<line x1="{left}" y1="{top + plot_h}" x2="{width - right}" y2="{top + plot_h}" stroke="black"/>')
    for y, label in hlines or []:
        out.append(f'<line x1="{left}" y1="{y_at(y):.1f}" x2="{width - right}" y2="{y_at(y):.1f}" '
                   f'stroke="#888" stroke-dasharray="4 3"/><text x="{width - right - 2}" y="{y_at(y) - 3:.1f}" '
                   f'text-anchor="end" fill="#555" stroke="white" stroke-width="3" paint-order="stroke">{label}</text>')
    for n, (name, pts) in enumerate(series.items()):
        colour = COLOURS[n % len(COLOURS)]
        out.append('<polyline fill="none" stroke="%s" stroke-width="1.5" points="%s"/>'
                   % (colour, " ".join(f"{x_at(x):.1f},{y_at(y):.1f}" for x, y in pts)))
        lx, ly = left + (n % 3) * LAYOUT["legend_dx"], height - 36 + (n // 3) * 16
        out.append(f'<rect x="{lx}" y="{ly - 9}" width="10" height="10" fill="{colour}"/>'
                   f'<text x="{lx + 14}" y="{ly}">{name}</text>')
    out.append("</svg>")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
