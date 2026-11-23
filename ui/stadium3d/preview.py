"""A look at a rendered ground as the page shows it, without the game: the seats in two club
colours (or grey where standing), the roof, works, shop or megastore over it, as stadium.rs
`paint` / `compose` do. usage:
    python -I preview.py <folder> <size s|m|l|x> [roof] [works] [shop|mega] [red] [seated 0..1]
-> <folder>/preview_<size>.png ('red': red and white, the reference's club; else Heracles)"""
import sys
import numpy as np
from PIL import Image

folder, size = sys.argv[1], sys.argv[2]
flags = sys.argv[3:]
seated = next((float(f) for f in flags if f.replace(".", "").isdigit()), 0.8)
cols = np.array([[255, 255, 255], [196, 0, 0]] if "red" in flags else [[255, 255, 255], [20, 20, 20]], dtype=np.float32)


def paint(rgba, ids):
    """stadium.rs `paint` over a whole picture: class 1/2 seats, 3 trim, standing segments grey."""
    cls = np.clip((ids[:, :, 0] + 40) // 80, 0, 3)
    stand = np.clip((ids[:, :, 1] + 40) // 80, 0, 3)
    seg = np.clip((ids[:, :, 2] + 8) // 16, 0, 15)
    cls = np.where(ids[:, :, 3] > 0, cls, 0)
    s = 1.0 - seated
    share = np.where(stand >= 2, min(s / 0.3, 1.0), max((s - 0.3) / 0.7, 0.0))
    standing = np.abs(seg - 7.5) < share * 8.0
    lum = (rgba[:, :, 0] * 3 + rgba[:, :, 1] * 6 + rgba[:, :, 2]) / 10
    out = rgba.copy()
    for c in (1, 2, 3):
        colour = cols[0] if c == 1 else cols[1]  # (stadium.rs paint: trim in the game's second colour)
        for i in range(3):
            v = np.maximum(colour[i], 30) * lum / 205
            if c != 3:
                v = np.where(standing, 0x70 * lum / 205, v)
            out[:, :, i] = np.where(cls == c, np.minimum(v, 255), out[:, :, i])
    return out


load = lambda name: np.asarray(Image.open(f"{folder}/{name}_{size}.png").convert("RGBA")).astype(np.float32)
img = Image.fromarray(paint(load("base"), load("id").astype(np.int32)).astype(np.uint8))
for kind in ("roof", "works", "shop", "mega"):
    if kind in flags:
        layer = load(kind)
        if kind in ("shop", "mega"):
            layer = paint(layer, load("id" + kind).astype(np.int32))
        layer = Image.fromarray(layer.astype(np.uint8))
        img.paste(layer, (0, 0), layer)
bg = Image.new("RGBA", img.size, (30, 32, 36, 255))  # (the page's dark behind what is see-through)
bg.alpha_composite(img)
bg.convert("RGB").save(f"{folder}/preview_{size}.png")
print("written", f"{folder}/preview_{size}.png")
