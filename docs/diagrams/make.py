#!/usr/bin/env python3
"""The blog post's three figures, as plain SVG (renders the same on any blog, GitHub and
dev.to). Run: python3 make.py — writes flip.svg, lookup.svg and verdicts.svg beside it.
Ink and accent follow flint-docs' posters."""
import os

INK, SUB, MUTE, PAPER = "#14181D", "#3D4650", "#6B7683", "#FFFFFF"
CUR_F, CUR_L = "#E3F2EB", "#0F7A55"      # current generation
PREV_F, PREV_L = "#E4EEF9", "#2E6FB7"    # previous generation
DROP_F, DROP_L = "#F2F2F2", "#9AA3AD"    # dropped
WARN = "#8C2449"
FONT = "font-family='-apple-system,Segoe UI,Helvetica,Arial,sans-serif'"
HERE = os.path.dirname(os.path.abspath(__file__))


def svg(w, h, body, title):
    return (f"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w} {h}' width='{w}' height='{h}' "
            f"role='img' aria-label='{title}'>\n<title>{title}</title>\n"
            f"<rect width='{w}' height='{h}' fill='{PAPER}'/>\n"
            "<defs><marker id='a' viewBox='0 0 10 10' refX='9' refY='5' markerWidth='7' markerHeight='7' orient='auto'>"
            f"<path d='M0,0 L10,5 L0,10 z' fill='{SUB}'/></marker></defs>\n{body}</svg>\n")


def text(x, y, s, size=14, fill=INK, weight="normal", anchor="middle", italic=False):
    st = " font-style='italic'" if italic else ""
    return f"<text x='{x}' y='{y}' {FONT} font-size='{size}' fill='{fill}' font-weight='{weight}' text-anchor='{anchor}'{st}>{s}</text>\n"


def box(x, y, w, h, fill, line, dash=False, r=8):
    d = " stroke-dasharray='6 4'" if dash else ""
    return f"<rect x='{x}' y='{y}' width='{w}' height='{h}' rx='{r}' fill='{fill}' stroke='{line}' stroke-width='2'{d}/>\n"


def keys(x, y, ks, fill, line, strike=False):
    out = ""
    for i, k in enumerate(ks):
        kx = x + i * 38
        out += box(kx, y, 32, 32, PAPER, line, r=6) + text(kx + 16, y + 21, k, 16, INK if not strike else MUTE, "bold")
        if strike:
            out += f"<line x1='{kx+4}' y1='{y+28}' x2='{kx+28}' y2='{y+4}' stroke='{DROP_L}' stroke-width='2'/>\n"
    return out


def generation(x, y, label, ks, kind, slots=3):
    f, l = {"cur": (CUR_F, CUR_L), "prev": (PREV_F, PREV_L), "drop": (DROP_F, DROP_L)}[kind]
    w = 20 + slots * 38
    out = box(x, y, w, 66, f, l, dash=(kind == "drop"))
    out += text(x + w / 2, y - 8, label, 13, l if kind != "drop" else MUTE, "bold")
    out += keys(x + 12, y + 18, ks, f, l, strike=(kind == "drop"))
    return out


def flip():
    b = text(470, 32, "How a flip works (cap = 3)", 20, INK, "bold")
    cols = [(40, "1. The current generation is full"), (350, "2. put(d): it flips"), (660, "3. get(b): a promotion")]
    for x, t in cols:
        b += text(x + 125, 72, t, 15, SUB, "bold")
    # 1
    b += generation(98, 110, "current", ["a", "b", "c"], "cur") + generation(98, 230, "previous", ["x", "y", "z"], "prev")
    # 2
    b += generation(408, 110, "current", ["d"], "cur") + generation(408, 230, "previous (was current)", ["a", "b", "c"], "prev")
    b += generation(408, 350, "dropped", ["x", "y", "z"], "drop")
    # 3
    b += generation(718, 110, "current", ["d", "b"], "cur") + generation(718, 230, "previous", ["a", "c"], "prev")
    # b's path: out of the previous generation's right side, up into the current one's
    b += f"<path d='M 852 263 C 880 250, 880 190, 856 180' fill='none' stroke='{CUR_L}' stroke-width='2.5' marker-end='url(#a)'/>\n"
    b += text(894, 222, "b moved", 12, CUR_L, "bold", "start") + text(894, 238, "back", 12, CUR_L, "bold", "start")
    for x in (322, 632):
        b += f"<line x1='{x}' y1='190' x2='{x+30}' y2='190' stroke='{SUB}' stroke-width='2.5' marker-end='url(#a)'/>\n"
    b += text(470, 455, "The last cap keys used are always present; up to 2 x cap may be. Each flip is counted: that count is the sizing signal.", 13, SUB)
    return svg(970, 475, b, "How a flip works")


def lookup():
    b = text(430, 32, "One lookup: get(k) / get_or_insert_with(k, f)", 20, INK, "bold")
    def node(x, y, w, h, s, f, l, sub=None):
        o = box(x, y, w, h, f, l) + text(x + w / 2, y + (h / 2 + 5 if not sub else h / 2 - 3), s, 15, INK, "bold")
        if sub:
            o += text(x + w / 2, y + h / 2 + 15, sub, 12, SUB)
        return o
    def arrow(x1, y1, x2, y2, label=None, lx=None, ly=None):
        o = f"<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='{SUB}' stroke-width='2' marker-end='url(#a)'/>\n"
        if label:
            o += text(lx, ly, label, 12, SUB, "normal", "start", True)
        return o
    b += node(330, 60, 200, 46, "hash k once", PAPER, INK)
    b += arrow(430, 106, 430, 136)
    b += node(310, 138, 240, 56, "in current generation?", CUR_F, CUR_L)
    b += arrow(550, 166, 640, 166, "yes", 575, 158)
    b += node(642, 140, 190, 52, "hit", CUR_F, CUR_L, "stats: hits += 1")
    b += arrow(430, 194, 430, 230, "no", 438, 216)
    b += node(310, 232, 240, 56, "in previous generation?", PREV_F, PREV_L)
    b += arrow(550, 260, 640, 260, "yes", 575, 252)
    b += node(642, 230, 190, 62, "promotion", PREV_F, PREV_L, "move it back (may flip)")
    b += text(737, 312, "stats: promotions += 1", 12, SUB)
    b += arrow(430, 288, 430, 326, "no", 438, 310)
    b += node(330, 328, 200, 62, "miss", "#FBEFF3", WARN, "None, or insert f()")
    b += text(430, 410, "stats: misses += 1 (and inserts += 1 if f() was inserted; may flip)", 12, SUB)
    b += text(298, 170, "1st probe", 12, MUTE, "normal", "end", True) + text(298, 264, "2nd probe", 12, MUTE, "normal", "end", True)
    return svg(860, 430, b, "One lookup")


def verdicts():
    # calibration data: cap 10,000; hit ratio at C/2, C, 2C, 4C (fliplru 0.3, see tests/sizing.rs)
    series = [
        ("8,000 keys in rotation", "Oversized", [39.8, 99.2, 99.2, 99.2], "#0F7A55"),
        ("Zipf 0.99, 100,000 keys", "Fits", [68.2, 75.4, 82.6, 89.0], "#2E6FB7"),
        ("12,000 keys in rotation", "TooSmall", [0.0, 79.2, 98.8, 98.8], "#B7791F"),
        ("50,000 keys at random", "MuchTooSmall", [14.5, 28.0, 52.1, 86.7], "#8C2449"),
        ("200,000 keys at random", "Thrashing", [3.8, 7.4, 14.4, 27.5], "#6B7683"),
    ]
    X0, Y0, W, H = 90, 70, 520, 330
    xs = [X0 + i * W / 3 for i in range(4)]
    def y(v): return Y0 + H - v / 100 * H
    b = text(450, 32, "What each verdict looks like: hit ratio at other capacities", 20, INK, "bold")
    for v in range(0, 101, 25):
        b += f"<line x1='{X0}' y1='{y(v)}' x2='{X0+W}' y2='{y(v)}' stroke='#E6E9EC' stroke-width='1'/>\n" + text(X0 - 10, y(v) + 5, f"{v}%", 12, MUTE, anchor="end")
    for x, l in zip(xs, ["cap / 2", "cap (verdict here)", "2 x cap", "4 x cap"]):
        b += text(x, Y0 + H + 24, l, 13, INK if "verdict" in l else SUB, "bold" if "verdict" in l else "normal")
    b += f"<line x1='{xs[1]}' y1='{Y0}' x2='{xs[1]}' y2='{Y0+H}' stroke='{INK}' stroke-width='1.5' stroke-dasharray='4 4'/>\n"
    for i, (name, verdict, vals, col) in enumerate(series):
        pts = " ".join(f"{x},{y(v)}" for x, v in zip(xs, vals))
        b += f"<polyline points='{pts}' fill='none' stroke='{col}' stroke-width='3'/>\n"
        for x, v in zip(xs, vals):
            b += f"<circle cx='{x}' cy='{y(v)}' r='4.5' fill='{col}'/>\n"
        ly = 100 + i * 62
        b += f"<line x1='650' y1='{ly}' x2='676' y2='{ly}' stroke='{col}' stroke-width='3'/>\n"
        b += text(686, ly + 5, verdict, 15, col, "bold", "start") + text(686, ly + 23, name, 12, SUB, "normal", "start")
    b += text(450, Y0 + H + 52, "Flat to the right of cap: a bigger cache gains little (Oversized, Fits). Steep: it is too small. Low at every size: Thrashing.", 13, SUB)
    return svg(900, 480, b, "What each verdict looks like")


for name, fn in [("flip.svg", flip), ("lookup.svg", lookup), ("verdicts.svg", verdicts)]:
    open(os.path.join(HERE, name), "w").write(fn())
    print("wrote", name)
