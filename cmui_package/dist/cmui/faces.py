"""Player pictures for cmui: <game>\\Data\\cmui_faces.bin, the same 'CMU2' pack as cmui_crests.bin,
keyed by the name a profile's title shows ("Paul van Boekel", "Tó Madeira"), plus "?": a
silhouette for players without a picture.
usage: python -I faces.py <game folder> [<picture folder> [names.csv]] [--defaults <folder>]
         pictures named after the player ("Paul van Boekel.png"), or by FM ID ("12345.png")
         with names.csv giving "id,name" per row (comma, semicolon or tab)
         --defaults: default faces (DF11's "Players - New colorways") packed as "?0", "?1", ...;
         a player without a picture (newgens, regens) gets one of them, the same one each time
       python -I faces.py <game folder> --mock names.txt
         drawn mock-up avatars for the names listed (one a line): a test without a face pack"""
import csv
import hashlib
import os
import struct
import sys

from PIL import Image, ImageDraw

SIDE = 160  # pixels in the pack; drawn at the title bar's crest height (80 at 1680x1050)


def avatar(name):
    """A plain drawn head and shoulders; looks picked from the name's hash (a stand-in, nothing
    read into the name)."""
    h = hashlib.md5(name.encode('utf-8')).digest()
    skins = [(247, 214, 186), (236, 192, 154), (214, 160, 118), (180, 124, 84), (141, 92, 60), (98, 64, 42)]
    hairs = [(30, 24, 20), (70, 46, 30), (120, 82, 50), (200, 160, 90), (40, 40, 40), None]
    shirts = [(200, 40, 40), (240, 200, 40), (40, 90, 180), (240, 240, 240), (30, 140, 80), (20, 20, 20)]
    skin, hair, shirt = skins[h[0] % len(skins)], hairs[h[1] % len(hairs)], shirts[h[2] % len(shirts)]
    s = SIDE * 4  # drawn large, scaled down for smooth edges
    im = Image.new('RGBA', (s, s))
    d = ImageDraw.Draw(im)
    d.ellipse((s * 0.12, s * 0.70, s * 0.88, s * 1.25), fill=shirt)                  # shoulders
    d.rectangle((s * 0.42, s * 0.56, s * 0.58, s * 0.76), fill=skin)                 # neck
    d.ellipse((s * 0.29, s * 0.18, s * 0.71, s * 0.66), fill=skin)                   # head
    if hair:
        d.chord((s * 0.27, s * 0.14, s * 0.73, s * 0.56), 180, 360, fill=hair)       # hair
    eye = (40, 30, 25)
    for ex in (0.42, 0.58):
        d.ellipse((s * (ex - 0.025), s * 0.41, s * (ex + 0.025), s * 0.45), fill=eye)
    return im.resize((SIDE, SIDE), Image.LANCZOS)


def silhouette():
    """The picture for players without one: a grey head and shoulders."""
    s = SIDE * 4
    im = Image.new('RGBA', (s, s))
    d = ImageDraw.Draw(im)
    grey = (96, 102, 112, 255)
    d.ellipse((s * 0.12, s * 0.70, s * 0.88, s * 1.25), fill=grey)
    d.ellipse((s * 0.30, s * 0.20, s * 0.70, s * 0.64), fill=grey)
    return im.resize((SIDE, SIDE), Image.LANCZOS)


def fitted(img):
    """Into the square, aspect kept, standing on its bottom edge (shoulders at the bottom)."""
    img = img.convert('RGBA')
    img.thumbnail((SIDE, SIDE), Image.LANCZOS)
    canvas = Image.new('RGBA', (SIDE, SIDE))
    canvas.paste(img, ((SIDE - img.width) // 2, SIDE - img.height))
    return canvas


def pack(game, pictures):
    """pictures: {name: Image}; writes <game>\\Data\\cmui_faces.bin."""
    keys = sorted(pictures)
    start = 12 + 60 * len(keys)
    blobs, entries, at = [], [], start
    for key in keys:
        px = bytearray()
        for r, g, b, a in fitted(pictures[key]).getdata():
            px += struct.pack('<HB', (r >> 3) << 11 | (g >> 2) << 5 | b >> 3, a)
        entries.append(key.encode('latin1', 'replace')[:51].ljust(52, b'\0') + struct.pack('<II', at, SIDE))
        blobs.append(bytes(px))
        at += len(px)
    out = os.path.join(game, 'Data', 'cmui_faces.bin')
    with open(out, 'wb') as f:
        f.write(b'CMU2' + struct.pack('<II', len(keys), 0) + b''.join(entries) + b''.join(blobs))
    print(out, len(keys), 'pictures,', at, 'bytes')


def main():
    args = sys.argv[1:]
    game = args.pop(0)
    pictures = {'?': silhouette()}
    if '--defaults' in args:
        at = args.index('--defaults')
        pool = sorted(f for f in os.listdir(args[at + 1]) if f.lower().endswith('.png'))
        for k, file in enumerate(pool):
            pictures[f'?{k}'] = Image.open(os.path.join(args[at + 1], file))
        del args[at:at + 2]
    if args[:1] == ['--mock']:
        with open(args[1], encoding='utf-8') as f:
            for name in filter(None, (line.strip() for line in f)):
                pictures[name] = avatar(name)
    elif args:
        folder, ids = args[0], {}
        if len(args) > 1:
            with open(args[1], encoding='utf-8', newline='') as f:
                text = f.read()
            dialect = csv.Sniffer().sniff(text[:2000], delimiters=',;\t')
            ids = {row[0].strip(): row[1].strip() for row in csv.reader(text.splitlines(), dialect) if len(row) > 1}
        for root, _, files in os.walk(folder):
            for file in files:
                stem, ext = os.path.splitext(file)
                if ext.lower() in ('.png', '.jpg', '.jpeg'):
                    name = ids.get(stem, stem if not ids else None)
                    if name:
                        pictures[name] = Image.open(os.path.join(root, file))
    pack(game, pictures)


if __name__ == '__main__':
    main()
