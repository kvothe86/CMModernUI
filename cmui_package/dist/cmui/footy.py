"""Club logos from footylogos.com for cmui (fan use; the marks belong to their owners, the site's
terms say its files are reference files and ask for a credit to FootyLogos.com).
Nothing downloaded here goes into the public repo: each user runs this for their own game folder.

usage: python footy.py index      the site's logo list -> ..\\logos\\footy\\
       python footy.py logos      match the site's club logos to <game>\\Data\\club.dat, download only
                                  the matched ones, -> <logos>\\badge_match.tsv (then: python logos.py pack)
Game folder: env CMUI_GAME like logos.py (default .., the main folder).
(The player pictures this also fetched were scrapped by the user 2026-11-16.)"""
import csv
import json
import os
import re
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import logos as L  # noqa: E402

L.NATION_ALIAS.update({'ireland': 'republic-of-ireland', 'czechia': 'czech-republic', 'turkiye': 'turkey', 'korea republic': 'south-korea'})
SITE = 'https://www.footylogos.com'
CACHE = os.path.join(L.OUT, 'footy')


PREVIEW = {}  # logo slug -> the site's preview picture (a fallback)


def fetch(url, path=None):
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (cmui footylogos fetch)'})
    data = urllib.request.urlopen(req, timeout=60).read()
    if path:
        open(path, 'wb').write(data)
    return data


def index():
    os.makedirs(CACHE, exist_ok=True)
    fetch(SITE + '/data/search-index.json', f'{CACHE}/search-index.json')
    print('saved search-index.json in', CACHE)


def shrink(data, path, side):
    from PIL import Image
    import io
    img = Image.open(io.BytesIO(data)).convert('RGBA')
    img.thumbnail((side, side), Image.LANCZOS)
    img.save(path)


def logos():
    entries = json.load(open(f'{CACHE}/search-index.json', encoding='utf8'))
    by_country = {}
    for e in entries:
        m = re.match(r'Logo · (.+)$', e.get('meta', ''))
        if e.get('kind') != 'logo' or e.get('sportId') not in ('football', 'soccer') or not m or '/logos/' not in e['href']:
            continue
        slug = e['href'].rsplit('/', 1)[-1]
        PREVIEW[slug] = e.get('preview', '')
        path = os.path.abspath(f'{CACHE}/png/{slug}.png')
        by_country.setdefault(L.slug_of_nation(m.group(1)), []).append((e['label'], path, L.norm(e['label'])))
    print(sum(map(len, by_country.values())), 'football logos in', len(by_country), 'countries')
    L.match(logos=by_country, out='badge_match.tsv')
    os.makedirs(f'{CACHE}/png', exist_ok=True)
    rows = list(csv.reader(open(f'{L.TSV}/badge_match.tsv', encoding='utf-8'), delimiter='\t'))
    todo = sorted({r[5] for r in rows if not os.path.exists(r[5])})
    for i, path in enumerate(todo):
        slug = os.path.basename(path)[:-4]
        try:
            shrink(fetch(f'{SITE}/downloads/logo/{slug}-logo-footylogos.png'), path, 256)
        except OSError as e:
            # no PNG download for it on the site: its 320 px preview picture does for an 80 px crest
            pre = PREVIEW.get(slug, '')
            try:
                shrink(fetch(pre if pre.startswith('http') else SITE + pre), path, 256)
            except (OSError, ValueError):
                print('failed', slug, e)
        if i % 100 == 99:
            print(f'{i + 1}/{len(todo)}', flush=True)
        time.sleep(0.15)  # be gentle with the site
    print(len(todo), 'downloaded; now: python logos.py pack')


if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else ''
    if cmd == 'index':
        index()
    elif cmd == 'logos':
        logos()
    else:
        print(__doc__)
