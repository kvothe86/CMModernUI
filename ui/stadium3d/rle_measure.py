"""How much run-length coding of 4-byte pixels saves on a cmui_stadium.bin (a measurement only).
usage: python -I rle_measure.py <pack>"""
import struct, sys

d = open(sys.argv[1], "rb").read()
n = struct.unpack_from("<I", d, 4)[0]
raw = packed = 0
for i in range(n):
    e = d[8 + 64 * i: 8 + 64 * (i + 1)]
    w, h = struct.unpack_from("<HH", e, 24)
    at = struct.unpack_from("<I", e, 28)[0]
    px = d[at: at + w * h * 4]
    runs, k = 0, 0
    while k < len(px):
        j = k + 4
        while j < len(px) and px[j:j + 4] == px[k:k + 4] and (j - k) // 4 < 255:
            j += 4
        runs += 1
        k = j
    raw += len(px)
    packed += runs * 5
print(f"{n} pictures: {raw // 1024} KB raw, {packed // 1024} KB as runs ({packed * 100 // max(raw, 1)}%)")
