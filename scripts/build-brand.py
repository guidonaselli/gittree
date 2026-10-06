#!/usr/bin/env python3
"""Genera los SVG de marca de GitTree en branding/ desde branding/source/concept.png.
Requiere: magick, y `pip install vtracer potracer`."""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from scipy import ndimage
import potrace
import vtracer

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "branding"
SRC = OUT / "source" / "concept.png"
SPLIT_Y = 408
SCALE = 4
NAVY = "#0F1F4D"
ICON_BG = ("#12338F", "#071A55")


def magick(*args, input=None):
    return subprocess.run(["magick", *map(str, args)], check=True, capture_output=True, input=input).stdout


def prepared(tmp, name, y0, y1, lighten):
    """Recorta la zona, la ajusta al fondo oscuro si corresponde y la amplía."""
    out = tmp / f"{name}.png"
    cmd = [SRC, "-crop", f"552x{y1-y0}+0+{y0}", "+repage", "-fuzz", "8%", "-trim", "+repage", "-fuzz", "7%", "-fill", "white", "-opaque", "white"]
    if lighten:
        cmd += ["-channel", "RGB", "-evaluate", "pow", str(lighten), "+channel"]
    magick(*cmd, "-filter", "Lanczos", "-resize", f"{SCALE*100}%", "-colorspace", "sRGB", out)
    return out


def size(png):
    w, h = magick(png, "-format", "%w %h", "info:").split()
    return int(w), int(h)


def trace_color(png, uid, keep_white=True):
    """Traza a color por capas. Las islas blancas internas pasan a ser huecos reales (máscara); con keep_white se conserva la mayor (el pájaro)."""
    w, h = size(png)
    dst = png.with_suffix(".svg")
    vtracer.convert_image_to_svg_py(
        str(png), str(dst), colormode="color", hierarchical="stacked", mode="spline",
        filter_speckle=30, color_precision=5, layer_difference=20, corner_threshold=60,
        length_threshold=6.0, splice_threshold=45, path_precision=1)
    body = re.search(r"<svg[^>]*>(.*)</svg>", dst.read_text(), re.S).group(1)
    paths = re.findall(r"<path[^>]*/>", body)[1:]
    white = [p for p in paths if re.search(r'fill="#(?:[E-F][0-9A-F]){3}"', p)]

    def area(p):
        n = [float(v) for v in re.findall(r"-?\d+\.?\d*", re.search(r' d="([^"]*)"', p).group(1))]
        xs, ys = n[0::2], n[1::2]
        return (max(xs) - min(xs)) * (max(ys) - min(ys))

    holes = [p for p in white if not (keep_white and p is max(white, key=area))]
    cut = "".join(re.sub(r'fill="[^"]*"', 'fill="#000"', p) for p in holes)
    shown = "".join(p for p in paths if not any(p is x for x in holes))
    mask = (f'<mask id="{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width="{w}" height="{h}">'
            f'<rect width="{w}" height="{h}" fill="#fff"/>{cut}</mask>')
    return f'{mask}<g mask="url(#{uid})">{shown}</g>', w, h


def trace_mono(png):
    """Silueta de un solo color (las zonas claras quedan como huecos)."""
    w, h = size(png)
    raw = magick(png, "-alpha", "off", "-colorspace", "Gray", "-depth", "8", "gray:-")
    bm = potrace.Bitmap(np.frombuffer(raw, np.uint8).reshape(h, w) >= 200)
    d = []
    for c in bm.trace(turdsize=30, alphamax=1.0, opttolerance=0.4):
        p = c.start_point
        d.append(f"M{p.x:.1f} {p.y:.1f}")
        for s in c.segments:
            if s.is_corner:
                d.append(f"L{s.c.x:.1f} {s.c.y:.1f}L{s.end_point.x:.1f} {s.end_point.y:.1f}")
            else:
                d.append(f"C{s.c1.x:.1f} {s.c1.y:.1f} {s.c2.x:.1f} {s.c2.y:.1f} {s.end_point.x:.1f} {s.end_point.y:.1f}")
        d.append("Z")
    assert d, "trace vacío"
    return f'<path fill-rule="evenodd" d="{"".join(d)}"/>', w, h


def small_mark(tmp, bird):
    """Silueta engrosada de la marca para tamaños chicos. bird=True re-corta el pájaro desde el original, agrandado."""
    crop = [SRC, "-crop", "552x404+0+0", "+repage", "-fuzz", "8%", "-trim", "+repage", "-filter", "Lanczos",
            "-resize", f"{SCALE*100}%", "-colorspace", "Gray", "-alpha", "off"]
    fine, bold = tmp / "fine.png", tmp / "bold.png"
    magick(*crop, "-threshold", "78%", fine)
    magick(fine, "-negate", "-morphology", "Dilate", "Disk:1.5", "-blur", "0x3", "-threshold", "50%", "-negate", bold)
    w, h = size(fine)
    load = lambda p: np.frombuffer(magick(p, "-alpha", "off", "-colorspace", "Gray", "-depth", "8", "gray:-"), np.uint8).reshape(h, w) < 128
    content, solid = load(fine), load(bold)
    lab, _ = ndimage.label(~content)
    sizes = np.bincount(lab.ravel())
    sizes[0] = 0
    sizes[lab[0, 0]] = 0
    window = lab == sizes.argmax()
    if bird:
        cut = ndimage.binary_dilation(window, iterations=6)
        solid &= ~cut
    else:
        solid |= window
    out = tmp / "small.png"
    magick("-size", f"{w}x{h}", "-depth", "8", "gray:-", out, input=np.where(solid, 0, 255).astype(np.uint8).tobytes())
    return trace_mono(out)


def doc(w, h, body, title="GitTree"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img">'
            f"<title>{title}</title>{body}</svg>\n")


def piece_doc(piece):
    return doc(piece[1], piece[2], piece[0])


def part(piece, x, y, width):
    body, w, h = piece
    return f'<svg x="{x:.0f}" y="{y:.0f}" width="{width:.0f}" height="{width*h/w:.0f}" viewBox="0 0 {w} {h}">{body}</svg>', width * h / w


def vertical(mark, word, bg=None):
    W, mw, ww, gap, pad = 1000, 640, 760, 56, 40
    m, mh = part(mark, (W - mw) / 2, pad, mw)
    t, th = part(word, (W - ww) / 2, pad + mh + gap, ww)
    return doc(W, round(pad * 2 + mh + gap + th), m + t)


def horizontal(mark, word):
    mh, gap, pad = 420, 70, 30
    mw = mh * mark[1] / mark[2]
    ww = 640
    m, _ = part(mark, pad, pad, mw)
    t, th = part(word, pad + mw + gap, pad + (mh - 640 * word[2] / word[1]) / 2, ww)
    return doc(round(pad * 2 + mw + gap + ww), mh + pad * 2, m + t)


def mono(piece, color, wrap=True):
    body, w, h = piece
    return body.replace("<path ", f'<path fill="{color}" ', 1), w, h


def bg_rect():
    return (f'<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{ICON_BG[0]}"/>'
            f'<stop offset="1" stop-color="{ICON_BG[1]}"/></linearGradient></defs><rect width="1024" height="1024" rx="228" fill="url(#bg)"/>')


def main():
    OUT.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        sets = {}
        for tag, light in (("", 0), ("-dark", 1)):
            word = trace_color(prepared(tmp, f"w{tag}", SPLIT_Y, 552, light and 0.28), f"hw{tag}", keep_white=False)
            sets[tag] = (trace_color(prepared(tmp, f"m{tag}", 0, SPLIT_Y, light and 0.42), f"hm{tag}"), word)
        small_mark_piece = small_mark(tmp, bird=True)
        tiny_mark_piece = small_mark(tmp, bird=False)
        flat = (trace_mono(prepared(tmp, "mm", 0, SPLIT_Y, 0)), trace_mono(prepared(tmp, "mw", SPLIT_Y, 552, 0)))

    for tag, (mk, wd) in sets.items():
        (OUT / f"mark{tag}.svg").write_text(doc(mk[1], mk[2], mk[0]))
        (OUT / f"wordmark{tag}.svg").write_text(doc(wd[1], wd[2], wd[0]))
        (OUT / f"logo{tag}.svg").write_text(vertical(mk, wd))
        (OUT / f"logo-horizontal{tag}.svg").write_text(horizontal(mk, wd))
    for tag, color in (("mono", NAVY), ("white", "#FFFFFF")):
        mk, wd = (mono(p, color) for p in flat)
        (OUT / f"mark-{tag}.svg").write_text(doc(mk[1], mk[2], mk[0]))
        (OUT / f"logo-{tag}.svg").write_text(vertical(mk, wd))
        (OUT / f"logo-horizontal-{tag}.svg").write_text(horizontal(mk, wd))

    for name, piece in (("small", small_mark_piece), ("tiny", tiny_mark_piece)):
        (OUT / f"mark-{name}.svg").write_text(piece_doc(mono(piece, NAVY)))
        (OUT / f"mark-{name}-dark.svg").write_text(piece_doc(mono(piece, "#7DB6FF")))
        icon, _ = part(mono(piece, "#FFFFFF"), 150, 130, 724)
        (OUT / f"app-icon-{name}.svg").write_text(doc(1024, 1024, bg_rect() + icon))
    body, w, h = tiny_mark_piece
    favicon = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><style>path{{fill:{NAVY}}}'
               f'@media (prefers-color-scheme:dark){{path{{fill:#7DB6FF}}}}</style>{body}</svg>\n')
    (OUT / "favicon.svg").write_text(favicon)

    mk = sets["-dark"][0]
    inner, _ = part(mk, 190, 150, 644)
    (OUT / "app-icon.svg").write_text(doc(1024, 1024, bg_rect() + inner))


if __name__ == "__main__":
    sys.exit(main())
