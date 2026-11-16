"""Checks for footy.py's pure part (no network, no game folder): python test_footy.py"""
import io
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import footy  # noqa: E402
from PIL import Image  # noqa: E402


def check(name, got, want):
    assert got == want, f'{name}: {got!r} != {want!r}'
    print('ok', name)


# shrink: any picture to at most side x side, transparent kept
src = io.BytesIO()
Image.new('RGBA', (500, 400), (200, 10, 10, 128)).save(src, 'PNG')
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'test_footy_out.png')
footy.shrink(src.getvalue(), out, 160)
with Image.open(out) as img:
    img.load()
    size, alpha = img.size, img.getpixel((10, 10))[3]
os.remove(out)
check('shrink size', size, (160, 128))
check('shrink keeps alpha', alpha, 128)
print('all ok')
