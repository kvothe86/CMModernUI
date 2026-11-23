"""A look at a rendered ground as the page shows it, without the game: the seats in two club
colours (or grey where standing), the roof and the works over it, as stadium.rs `paint` /
`compose` do. usage: python -I preview.py <folder> <size s|m|l|x> [roof] [works] [seated 0..1]
-> <folder>/preview_<size>.png"""
import sys
import numpy as np
from PIL import Image

folder, size = sys.argv[1], sys.argv[2]
flags = sys.argv[3:]
roof, works = "roof" in flags, "works" in flags
seated = next((float(f) for f in flags if f.replace(".", "").isdigit()), 0.8)
cols = np.array([[255, 255, 255], [20, 20, 20]], dtype=np.float32)  # Heracles: white and black

base = np.asarray(Image.open(f"{folder}/base_{size}.png").convert("RGBA")).astype(np.float32)
ids = np.asarray(Image.open(f"{folder}/id_{size}.png").convert("RGBA")).astype(np.int32)
cls = np.clip((ids[:, :, 0] + 40) // 80, 0, 3)
stand = np.clip((ids[:, :, 1] + 40) // 80, 0, 3)
seg = np.clip((ids[:, :, 2] + 8) // 16, 0, 15)
cls = np.where(ids[:, :, 3] > 0, cls, 0)
s = 1.0 - seated
share = np.where(stand >= 2, min(s / 0.3, 1.0), max((s - 0.3) / 0.7, 0.0))
standing = np.abs(seg - 7.5) < share * 8.0
lum = (base[:, :, 0] * 3 + base[:, :, 1] * 6 + base[:, :, 2]) / 10
out = base[:, :, :3].copy()
for c in (1, 2, 3):
    colour = cols[0] if c in (1, 3) else cols[1]
    for i in range(3):
        v = np.maximum(colour[i], 30) * lum / 205
        if c != 3:
            v = np.where(standing, 0x70 * lum / 205, v)
        out[:, :, i] = np.where(cls == c, np.minimum(v, 255), out[:, :, i])
img = Image.fromarray(out.astype(np.uint8))
for kind, on in (("roof", roof), ("works", works)):
    if on:
        layer = Image.open(f"{folder}/{kind}_{size}.png").convert("RGBA")
        img.paste(layer, (0, 0), layer)
img.save(f"{folder}/preview_{size}.png")
print("written", f"{folder}/preview_{size}.png")
