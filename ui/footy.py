"""Club logos and player headshots from footylogos.com for cmui (fan use; the marks belong to their
owners, the site's terms say its files are reference files and ask for a credit to FootyLogos.com).
Nothing downloaded here goes into the public repo: each user runs this for their own game folder.

usage: python footy.py index      the site's logo list and player catalogue -> ..\\logos\\footy\\
       python footy.py logos      match the site's club logos to <game>\\Data\\club.dat, download only
                                  the matched ones, -> <logos>\\badge_match.tsv (then: python logos.py pack)
       python footy.py faces [--dry]  match the catalogue's players to <game>\\Data\\staff.dat by name,
                                  nation and club, download each one's headshot to <game>\\cmui_faces\\png\\
                                  <name the game shows>.png, then: python -I shelved\\player_pictures\\faces.py
                                  <game> <that folder>   (players without one get faces.py's generic "?")
Game folder: env CMUI_GAME like logos.py (default .., the main folder)."""
import csv
import json
import os
import re
import struct
import sys
import time
import unicodedata
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import logos as L  # noqa: E402

L.NATION_ALIAS.update({'ireland': 'republic-of-ireland', 'czechia': 'czech-republic', 'turkiye': 'turkey', 'korea republic': 'south-korea'})
SITE = 'https://www.footylogos.com'
CACHE = os.path.join(L.OUT, 'footy')
FACES = os.path.join(L.GAME, 'cmui_faces', 'png')
# photo kinds, best first: the site's headshots (the user's choice), then its portraits
KINDS = ['ucl_head', 'portugal_head', 'head', 'portrait', 'eredivisie_portrait', 'belgium_portrait', 'mls_portrait', 'spl_portrait']
SIDE = 160  # faces.py packs 160 px


PREVIEW = {}  # logo slug -> the site's preview picture (a fallback)


def fetch(url, path=None):
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (cmui footylogos fetch)'})
    data = urllib.request.urlopen(req, timeout=60).read()
    if path:
        open(path, 'wb').write(data)
    return data


def index():
    os.makedirs(CACHE, exist_ok=True)
    home = fetch(SITE + '/player-pngs').decode('utf8', 'replace')
    cat = re.search(r'/data/player-pngs/catalogue\.[0-9a-f]+\.json', home).group(0)
    fetch(SITE + cat, f'{CACHE}/catalogue.json')
    fetch(SITE + '/data/search-index.json', f'{CACHE}/search-index.json')
    print('saved catalogue.json and search-index.json in', CACHE)


def slug_name(s):
    s = unicodedata.normalize('NFD', str(s or ''))
    s = re.sub('[̀-ͯ]', '', s).lower()
    return re.sub(r'^-|-$', '', re.sub(r'[^a-z0-9]+', '-', s))


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


def words(s):
    s = unicodedata.normalize('NFKD', s.translate(L.SPELL)).encode('ascii', 'ignore').decode().lower()
    return frozenset(re.sub(r"[^a-z0-9]+", ' ', s).split())


def db_players():
    """{nation slug: [(shown name, {name variants as word sets}, club keys, staff id)]} of every player."""
    game = L.GAME
    # index.dat: 67-byte entries (file[51], table, count, offset, version); staff.dat holds several tables
    where = {}
    raw = open(f'{game}/Data/index.dat', 'rb').read()[8:]  # (8-byte header)
    for i in range(0, len(raw) - 66, 67):
        where[struct.unpack_from('<i', raw, i + 51)[0]] = struct.unpack_from('<ii', raw, i + 55)

    def table(file, tid, size):
        count, offset = where[tid]
        data = open(f'{game}/Data/{file}.dat', 'rb').read()
        return [data[offset + i * size:offset + (i + 1) * size] for i in range(count)]
    names = {f: [L.text(r[:51]) for r in table(f, tid, 60)] for f, tid in (('first_names', 13), ('second_names', 14), ('common_names', 15))}
    nations = {struct.unpack_from('<i', r)[0]: L.text(r[4:55]) for r in L.records(f'{game}/Data/nation.dat', 290)}
    clubs = {}
    for r in L.records(f'{game}/Data/club.dat', 581):
        name, short = L.text(r[4:55]), L.text(r[56:82])
        clubs[struct.unpack_from('<i', r)[0]] = [L.norm(name), L.norm(short) or L.norm(name)]
    out = {}
    pick = lambda table, i: names[table][i] if 0 <= i < len(names[table]) else ''
    for r in table('staff', 6, 110):
        sid, first, second, common = struct.unpack_from('<iiii', r)
        nat, nat2, club, player = *struct.unpack_from('<ii', r, 26), *struct.unpack_from('<i', r, 57), *struct.unpack_from('<i', r, 97)
        if player < 0:
            continue
        f, s, c = pick('first_names', first), pick('second_names', second), pick('common_names', common)
        shown = c or f'{f} {s}'.strip()
        variants = {words(shown), words(f'{f} {s}')} - {frozenset()}
        for nid in {nat, nat2}:
            if nid in nations:
                out.setdefault(L.slug_of_nation(nations[nid]), []).append((shown, variants, clubs.get(club), sid))
    return out


def club_fits(cm_keys, site_clubs):
    return cm_keys is not None and any(L.score(cm_keys, L.norm(c)) >= 0.8 for c in site_clubs)


def faces(dry=False):
    cat = json.load(open(f'{CACHE}/catalogue.json', encoding='utf8'))['players']
    db = db_players()
    hits, none = {}, 0  # shown name -> (player, how, staff id)
    for p in cat:
        pool = db.get(L.slug_of_nation(p.get('nat') or ''), [])
        wanted = {words(p['n'])} | {words(a) for a in p.get('alias', [])}
        wanted -= {frozenset()}
        sites = {m['c'] for m in p['m'][:3]}
        exact = [c for c in pool if wanted & c[1]]
        near = [c for c in pool if any(w < v or v < w for w in wanted for v in c[1]) and club_fits(c[2], sites)]
        for how, cands in (('exact', exact), ('near', near)):
            if how == 'exact' and len(cands) > 1:
                cands = [c for c in cands if club_fits(c[2], sites)]
            if len({c[3] for c in cands}) == 1:
                hits.setdefault(cands[0][0], (p, how, cands[0][3]))
                break
        else:
            none += 1
    tiers = {}
    for _, how, _ in hits.values():
        tiers[how] = tiers.get(how, 0) + 1
    print(len(cat), 'catalogue players;', len(hits), 'matched to this database;', none, 'without a match; by tier', tiers)
    if dry:
        got = {id(h[0]) for h in hits.values()}
        for lg in ('premier-league', 'serie-a'):
            mine = [p for p in cat if p['m'][0]['g'] == lg and p['m'][0]['s'] == '2026/27']
            print(lg, 'current players:', len(mine), 'matched:', sum(id(p) in got for p in mine))
            for p in [p for p in mine if id(p) not in got][:12]:
                print('   miss', p['n'], '|', p.get('nat'), '|', p['m'][0]['c'])
        return
    os.makedirs(FACES, exist_ok=True)
    got = 0
    for i, (shown, (p, how, _)) in enumerate(sorted(hits.items())):
        path = os.path.join(FACES, re.sub(r'[<>:"/\\|?*]', '', shown) + '.png')
        if os.path.exists(path):
            continue
        shots = [a for a in p['a'] if a['f'] in KINDS and not a.get('archive')] or [a for a in p['a'] if a['f'] in KINDS]
        if not shots:
            continue
        a = min(shots, key=lambda a: (KINDS.index(a['f']), -int(a['s'][:4])))
        fn = '-'.join(x for x in map(slug_name, [p['n'], a['c'], a.get('photoSeason') or a['s'], a['f']]) if x)[:175].rstrip('-')
        try:
            shrink(fetch(f'{SITE}/downloads/player/{a["k"]}/{fn}-footylogos.png'), path, SIDE)
            got += 1
        except OSError as e:
            print('failed', shown, e)
        if got % 100 == 99:
            print(f'{i + 1}/{len(hits)}', flush=True)
        time.sleep(0.15)
    print(got, 'downloaded into', FACES, '\nnow: python -I shelved\\player_pictures\\faces.py', L.GAME, FACES)


if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else ''
    if cmd == 'index':
        index()
    elif cmd == 'logos':
        logos()
    elif cmd == 'faces':
        faces('--dry' in sys.argv)
    else:
        print(__doc__)
