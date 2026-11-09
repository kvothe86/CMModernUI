"""Checks for footy.py's pure parts (no network, no game folder): python test_footy.py
The URL rule of a player's picture must stay the site's own (its script player-png-download-path),
and the name matching must fold accents and Nordic letters the way the game's database spells them."""
import io
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import footy  # noqa: E402
from PIL import Image  # noqa: E402


def check(name, got, want):
    assert got == want, f'{name}: {got!r} != {want!r}'
    print('ok', name)


# the site's file-name rule (accents dropped, anything else a hyphen)
check('slug accents', footy.slug_name('Martín Ødegaard'), 'martin-degaard')
check('slug season', footy.slug_name('2026/27'), '2026-27')
check('slug club', footy.slug_name('Arsenal FC'), 'arsenal-fc')

# a name as the game spells it and as the site spells it are the same words
check('words nordic', footy.words('Martin Odegaard'), footy.words('Martin Ødegaard'))
check('words accents', footy.words('Jurriën Timber'), footy.words('Jurrien Timber'))
check('words hyphen', footy.words('Myles Lewis-Skelly'), footy.words('Myles Lewis Skelly'))
check('words order free', footy.words('Gabriel Magalhães') == footy.words('Magalhaes Gabriel'), True)
check('words same set', footy.words('Gabriel Magalhães'), footy.words('Gabriel Magalhaes'))

# the picture kinds: headshots first, then portraits
check('headshots before portraits', footy.KINDS.index('ucl_head') < footy.KINDS.index('portrait'), True)
check('one size', footy.SIDE, 160)

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
