"""A line chart as a standalone SVG, standard library only (the project has no plotting dependency).

line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0, ymax=None) draws one polyline
per series over categorical x positions; a None value leaves a gap. Colours are fixed and the
legend sits under the plot, so the figure reads the same in a light or a dark viewer.
"""

COLOURS = ["#4477aa", "#cc6677", "#228833", "#ccbb44", "#aa3377", "#66ccee"]


def _fmt(v):
    return f"{v:.3g}"


def line_chart(path, title, subtitle, xlabels, series, ylabel, ymin=0.0, ymax=None, xlabel=""):
    width, height = 640, 380
    left, right, top, bottom = 70, 20, 52, 96
    values = [v for ys in series.values() for v in ys if v is not None]
    hi = ymax if ymax is not None else (max(values) * 1.1 if values else 1.0)
    hi = hi if hi > ymin else ymin + 1.0
    plot_w, plot_h = width - left - right, height - top - bottom

    def x_at(i):
        return left + (plot_w * (i + 0.5) / len(xlabels))

    def y_at(v):
        return top + plot_h * (1 - (v - ymin) / (hi - ymin))

    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
           f'font-family="sans-serif" font-size="11"><rect width="{width}" height="{height}" fill="white"/>',
           f'<text x="{left}" y="18" font-size="13" font-weight="bold">{title}</text>',
           f'<text x="{left}" y="34" fill="#555">{subtitle}</text>']
    for t in range(5):
        v = ymin + (hi - ymin) * t / 4
        y = y_at(v)
        out.append(f'<line x1="{left}" y1="{y:.0f}" x2="{width - right}" y2="{y:.0f}" stroke="#ddd"/>')
        out.append(f'<text x="{left - 6}" y="{y + 4:.0f}" text-anchor="end">{_fmt(v)}</text>')
    out.append(f'<text x="14" y="{top + plot_h / 2:.0f}" transform="rotate(-90 14 {top + plot_h / 2:.0f})" '
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
        for x, y in points:
            out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="3.5" fill="{colour}"/>')
        lx = left + (n % 3) * 190
        ly = height - 36 + (n // 3) * 16
        out.append(f'<rect x="{lx}" y="{ly - 9}" width="10" height="10" fill="{colour}"/>'
                   f'<text x="{lx + 14}" y="{ly}">{name}</text>')
    out.append("</svg>")
    with open(path, "w") as f:
        f.write("\n".join(out) + "\n")
