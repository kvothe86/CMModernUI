"""Traditional (bitmap) fonts: put a euro sign in slot 0xA4 of the game's .fnt files.

Format (from gui_load_bitmap_font 0x5e4ba0): u32 line height, then for each char 0x20..0xFF four u32
(w = advance, bpr = bytes per row, a, b) and, when bpr != 0, height * bpr bytes of 4-bit alpha rows
(high nibble first; 0 = clear, 15 = solid colour, 1..14 blend).

usage: python fnt_euro.py [--dry] <font.fnt>...      (copies to <file>.before_euro once)
The euro is drawn with Arial (Arial Bold for the bold Trade fonts), scaled so its height equals the
font's digit '0' and set on the same rows, with the digit's a and b."""
import sys, os, shutil, struct
from PIL import Image, ImageDraw, ImageFont


def read(path):
    d = open(path, "rb").read()
    h = struct.unpack_from("<I", d, 0)[0]
    p, glyphs = 4, {}
    for c in range(0x20, 0x100):
        w, bpr, a, b = struct.unpack_from("<4I", d, p)
        p += 16
        data = d[p:p + h * bpr] if bpr else b""
        p += h * bpr if bpr else 0
        glyphs[c] = [w, bpr, a, b, data]
    assert p == len(d), f"{path}: parsed {p} of {len(d)} bytes"
    return h, glyphs


def write(path, h, glyphs):
    out = bytearray(struct.pack("<I", h))
    for c in range(0x20, 0x100):
        w, bpr, a, b, data = glyphs[c]
        out += struct.pack("<4I", w, bpr, a, b) + data
    open(path, "wb").write(out)


def rows(h, g):
    """The glyph as rows of 0..15 values."""
    w, bpr, a, b, data = g
    img = []
    for y in range(h):
        row = []
        for byte in data[y * bpr:(y + 1) * bpr]:
            row += [byte >> 4, byte & 15]
        img.append(row)
    return img


def euro_glyph(h, glyphs, bold):
    zero = rows(h, glyphs[ord("0")])
    ys = [y for y, r in enumerate(zero) if any(r)]
    top, bottom = ys[0], ys[-1]
    box = bottom - top + 1
    zw = glyphs[ord("0")][0]
    font_file = r"C:\Windows\Fonts\arialbd.ttf" if bold else r"C:\Windows\Fonts\arial.ttf"
    S = 8  # supersample
    big = ImageFont.truetype(font_file, 200)
    probe = Image.new("L", (400, 400), 0)
    ImageDraw.Draw(probe).text((20, 20), "\u20ac", font=big, fill=255)
    bb = probe.getbbox()
    glyph = probe.crop(bb)
    old = glyphs[0xA4]
    scale = box / glyph.height  # as high as the digits; the advance follows the width
    gw, gh = max(1, round(glyph.width * scale)), box
    small = glyph.resize((gw, gh), Image.LANCZOS)
    advance = gw + 1  # a pixel to the next letter
    width = advance + advance % 2
    px = [[0] * width for _ in range(h)]
    x0 = 0
    y0 = bottom - gh + 1  # sits on the digits' baseline
    for y in range(gh):
        for x in range(gw):
            v = small.getpixel((x, y))
            px[y0 + y][x0 + x] = min(15, (v * 15 + 127) // 255)
    bpr = width // 2
    data = bytearray()
    for row in px:
        row = row + [0] * (bpr * 2 - len(row))
        for i in range(0, len(row), 2):
            data.append(row[i] << 4 | row[i + 1])
    return [advance, bpr, old[2], old[3], bytes(data)]  # a and b as the old slot had them


def fix(path, dry):
    h, glyphs = read(path)
    bold = "bold" in os.path.basename(path).lower()
    old = glyphs[0xA4]
    glyphs[0xA4] = euro_glyph(h, glyphs, bold)
    if not dry:
        if not os.path.exists(path + ".before_euro"):
            shutil.copyfile(path, path + ".before_euro")
        write(path, h, glyphs)
    return f"slot 0xA4 was w={old[0]} bpr={old[1]}, now w={glyphs[0xA4][0]} bpr={glyphs[0xA4][1]} (height {h})"


if __name__ == "__main__":
    for p in [a for a in sys.argv[1:] if a != "--dry"]:
        print(os.path.basename(p), "->", fix(p, "--dry" in sys.argv))
