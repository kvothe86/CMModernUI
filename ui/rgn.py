"""CM 01/02 .rgn pictures (48-byte header: w, h, size, fixed RGB565 masks; then w*h RGB565, top row
first), as in CM0102Patcher's RGNConverter.cs.  usage: python rgn.py topng in.rgn out.png"""
import struct, sys
import numpy as np
from PIL import Image

HEADER_TAIL = bytes.fromhex("144C68012000000040000000000000001000000000F80000E00700001F00000000000000")


def read(path):
    b = open(path, "rb").read()
    w, h = struct.unpack_from("<II", b)
    px = np.frombuffer(b, "<u2", w * h, 48).reshape(h, w).astype(np.uint32)
    r, g, bl = (px >> 11) & 31, (px >> 5) & 63, px & 31
    return np.dstack([(r * 255 + 15) // 31, (g * 255 + 31) // 63, (bl * 255 + 15) // 31]).astype(np.uint8)


def write(path, rgb):
    h, w = rgb.shape[:2]
    c = rgb.astype(np.uint32)
    px = ((c[..., 0] * 31 + 127) // 255 << 11) | ((c[..., 1] * 63 + 127) // 255 << 5) | ((c[..., 2] * 31 + 127) // 255)
    open(path, "wb").write(struct.pack("<III", w, h, w * h * 2) + HEADER_TAIL + px.astype("<u2").tobytes())


if __name__ == "__main__":
    if sys.argv[1] == "topng":
        Image.fromarray(read(sys.argv[2])).save(sys.argv[3])
    # round-trip self-check: read -> write -> read is stable
    a = np.random.randint(0, 256, (4, 5, 3), np.uint8)
    import tempfile, os
    t = os.path.join(tempfile.gettempdir(), "rgn_check.rgn")
    write(t, a); b = read(t); write(t, b)
    assert (read(t) == b).all()
