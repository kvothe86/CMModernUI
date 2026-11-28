"""Player background pack (champman0102 t=11152, 800x600) -> F:\\CM2627 at 1680x1050.
usage: python pbg_new.py list <pack.zip> <Pictures dir>        new files (not in Pictures), sizes
       python pbg_new.py build <pack.zip> <Pictures dir> <out>  new files at 1680x1050 into out
       python pbg_new.py preview <pack.zip> <Pictures dir> <name in zip> <out.png>
New files are all player photos (p__*.RGN, no menu bar baked in): see scale()."""
import os, struct, sys, zipfile
import numpy as np
from PIL import Image
import rgn
from bkg_fix import NEW_BAR

W, H = 1680, 1050


def new_names(zpath, pics):
    have = set()
    for d, _, fs in os.walk(pics):
        for f in fs:
            have.add(os.path.relpath(os.path.join(d, f), pics).replace("\\", "/").lower())
    z = zipfile.ZipFile(zpath)
    out = []
    for n in z.namelist():
        rel = n[len("Pictures/"):]
        if rel.lower().endswith(".rgn") and rel.lower() not in have:
            out.append((n, rel))
    return z, out


def read_rgn(b):  # rgn.read on bytes
    w, h = struct.unpack_from("<II", b)
    px = np.frombuffer(b, "<u2", w * h, 48).reshape(h, w).astype(np.uint32)
    r, g, bl = (px >> 11) & 31, (px >> 5) & 63, px & 31
    return np.dstack([(r * 255 + 15) // 31, (g * 255 + 31) // 63, (bl * 255 + 15) // 31]).astype(np.uint8)


def scale(img):
    """Player photos have no bar baked in: the whole photo goes right of the 126 px menu bar, the bar
    strip gets the photo's own left-edge colour per row, darkened (like the club pictures' bar)."""
    out = np.empty((H, W, 3), np.uint8)
    out[:, NEW_BAR:] = np.asarray(Image.fromarray(img).resize((W - NEW_BAR, H), Image.LANCZOS))
    edge = np.median(out[:, NEW_BAR:NEW_BAR + 12].astype(int), axis=1) * 0.45
    smooth = np.asarray(Image.fromarray(edge.astype(np.uint8)[:, None]).resize((1, 64)).resize((1, H), Image.BILINEAR))
    out[:, :NEW_BAR] = smooth[:, :1]
    return out


if __name__ == "__main__":
    z, names = new_names(sys.argv[2], sys.argv[3])
    if sys.argv[1] == "list":
        for n, rel in names:
            img = read_rgn(z.read(n))
            print(img.shape[1], img.shape[0], rel)
    elif sys.argv[1] == "preview":  # preview <zip> <Pictures> <name in zip> <out.png>
        Image.fromarray(scale(read_rgn(z.read(sys.argv[4])))).save(sys.argv[5])
    elif sys.argv[1] == "build":
        out = sys.argv[4]
        for i, (n, rel) in enumerate(names):
            img = read_rgn(z.read(n))
            dst = os.path.join(out, rel)
            os.makedirs(os.path.dirname(dst) or out, exist_ok=True)
            rgn.write(dst, img if img.shape[:2] == (H, W) else scale(img))
            if i % 500 == 0:
                print(i, len(names), rel, flush=True)
        print("done", len(names))
