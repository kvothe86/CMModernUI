"""Fit 800x600-born background pictures (stretched to 1680x1050 with the 90 px menu bar baked in,
so a 189 px bar) to the 126 px bar of 1680x1050: bar column = its own colour per row, badge in it
scaled down keeping its shape, the rest stretched over x 126..1679.
usage: python bkg_fix.py scan <Pictures dir>          bar width found per file (no writes)
       python bkg_fix.py preview <file.rgn> <out.png> before / after side by side
       python bkg_fix.py fix <Pictures dir> <out dir>  writes fixed copies (same subfolders)"""
import os, sys
import numpy as np
from PIL import Image
import rgn

NEW_BAR = 126  # g800.mbr width at 1680x1050


def bar_width(img):
    """x where the baked bar ends (0 = none): first column from x 150 on that leaves the bar colour."""
    h, w = img.shape[:2]
    if w != 1680 or h != 1050:
        return 0
    ref = np.median(img[:, 2:8].astype(int), axis=1)  # bar colour per row
    # bar must be one colour along its left edge for most rows
    if (np.abs(img[:, 2:8].astype(int) - ref[:, None]).max(axis=2) > 24).mean() > 0.2:
        return 0
    for x in range(150, 240):
        col = np.abs(img[:, x].astype(int) - ref).max(axis=1)
        if (col > 30).mean() > 0.5:
            return x
    return 0


def fix(img, bw):
    h, w = img.shape[:2]
    ref = np.median(img[:, 2:8].astype(int), axis=1).astype(np.uint8)
    out = np.empty_like(img)
    out[:, :NEW_BAR] = ref[:, None]
    rest = Image.fromarray(img[:, bw:]).resize((w - NEW_BAR, h), Image.LANCZOS)
    out[:, NEW_BAR:] = np.asarray(rest)
    # badge: pixels in the bar that are not the bar colour
    bar = img[:, :bw].astype(int)
    mask = np.abs(bar - ref[:, None].astype(int)).max(axis=2) > 30
    ys, xs = np.nonzero(mask)
    if len(ys) > 50:
        y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
        s = min(1.0, (NEW_BAR - 12) / (x1 - x0))
        nw, nh = max(1, round((x1 - x0) * s)), max(1, round((y1 - y0) * s))
        badge = Image.fromarray(img[y0:y1, x0:x1]).resize((nw, nh), Image.LANCZOS)
        m = Image.fromarray((mask[y0:y1, x0:x1] * 255).astype(np.uint8)).resize((nw, nh), Image.LANCZOS)
        cy = (y0 + y1) // 2
        top, left = max(0, cy - nh // 2), (NEW_BAR - nw) // 2
        a = np.asarray(m)[:, :, None] / 255.0
        region = out[top:top + nh, left:left + nw].astype(float)
        out[top:top + nh, left:left + nw] = (np.asarray(badge) * a + region * (1 - a)).astype(np.uint8)
    return out


def files(root):
    for d, _, fs in os.walk(root):
        for f in fs:
            if f.lower().endswith(".rgn"):
                yield os.path.join(d, f)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "scan":
        hist = {}
        for p in files(sys.argv[2]):
            bw = bar_width(rgn.read(p))
            hist[bw] = hist.get(bw, 0) + 1
            if bw == 0 or not 180 <= bw <= 200:
                print(bw, os.path.relpath(p, sys.argv[2]))
        print("bar widths:", dict(sorted(hist.items())))
    elif cmd == "preview":
        img = rgn.read(sys.argv[2])
        bw = bar_width(img)
        after = fix(img, bw) if bw else img
        Image.fromarray(np.vstack([img, after])).resize((840, 1050)).save(sys.argv[3])
        print("bar", bw)
    elif cmd == "fix":
        src, dst = sys.argv[2], sys.argv[3]
        n = 0
        for p in files(src):
            img = rgn.read(p)
            bw = bar_width(img)
            if not 180 <= bw <= 200:  # ponytail: other "bars" found were dark photo edges in the root folder
                continue
            q = os.path.join(dst, os.path.relpath(p, src))
            os.makedirs(os.path.dirname(q), exist_ok=True)
            rgn.write(q, fix(img, bw))
            n += 1
        print("fixed", n)
