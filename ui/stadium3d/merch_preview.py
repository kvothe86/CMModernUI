"""ui\\src\\merch.bin painted as stadium.rs `merch_icon` paints it, for a few clubs' colours, on the
card's dark: one row per club (away shirt in swapped colours, scarf, name and number, kit, lock).
usage: python -I merch_preview.py -> merch/preview_icons.png"""
import numpy as np
from PIL import Image

N = 96
data = np.frombuffer(open("../src/merch.bin", "rb").read(), np.uint8).reshape(5, N, N, 3)
clubs = {"Accrington": [(255, 255, 255), (196, 0, 0)], "Chelsea": [(255, 255, 0), (0, 0, 200)], "Dortmund": [(0, 0, 0), (255, 255, 0)]}


def mosaic(c):
    """stadium.rs `mosaic`: the second colour darker or lighter when the two are alike."""
    if sum(abs(a - b) for a, b in zip(*c)) >= 90:
        return c
    lum = sum(c[0]) / 3
    return [c[0], tuple(int(v * 0.55) if lum > 128 else int(v + (255 - v) * 0.6) for v in c[0])]


def paint(icon, cols, tint=None):
    light, alpha, cls = (icon[:, :, i].astype(np.float32) for i in range(3))
    out = np.zeros((N, N, 3), np.float32)
    for i in range(3):
        if tint:
            out[:, :, i] = tint[i] * light / 255
            continue
        a = max(cols[0][i], 30) * light / 205
        b = max(cols[1][i], 30) * light / 205
        out[:, :, i] = np.where(cls == 1, a, np.where(cls == 2, b, light))
    bg = np.array([0x0c, 0x10, 0x16], np.float32)
    out = out.clip(0, 255) * (alpha[:, :, None] / 255) + bg * (1 - alpha[:, :, None] / 255)
    return Image.fromarray(out.astype(np.uint8)).resize((192, 192), Image.LANCZOS)


rows = []
for name, cols in clubs.items():
    c = mosaic(cols)
    s = [c[1], c[0]]  # (home items in the main colour, the game's second; stadium.rs)
    row = [paint(data[0], c), paint(data[1], s), paint(data[2], s), paint(data[3], s), paint(data[4], c, (0xd0, 0xd4, 0xda))]
    rows.append(row)
sheet = Image.new("RGB", (5 * 200, len(rows) * 200), (0x0c, 0x10, 0x16))
for r, row in enumerate(rows):
    for k, im in enumerate(row):
        sheet.paste(im, (k * 200 + 4, r * 200 + 4))
sheet.save("merch/preview_icons.png")
print("written merch/preview_icons.png")
