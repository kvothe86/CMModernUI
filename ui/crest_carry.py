"""Crests of an older database's matched clubs carried to the newer database's clubs that the logo
sites did not match, by name within the same nation (IDs are not stable between the Restructured
databases: of 11,786 shared IDs only 430 were the same club's nation). Picks up renamed clubs whose
old name the hand-made and site matches knew ("Athletic Bilbao" -> "Athletic Club de Bilbao").
Taken: the same full or short name, or a reserve side ("... II", "... U21") of a club so named.
usage: python crest_carry.py <old game folder> <new game folder> [--dry]
       python crest_carry.py hand <game folder>    the hand table club_logos.tsv -> hand_match.tsv
writes <new>\\cmui_logos\\carry_match.tsv (match.tsv columns; logos.py pack reads it)."""
import csv
import os
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import logos as L  # noqa: E402

TABLES = ('match.tsv', 'badge_match.tsv')
RESERVE = re.compile(r'\s+(?:II|B|C|2|U-?1[789]|U-?2[0-3]|Juniors|Reserves|Youth)$')


def clubs(game):
    """{id: (name, short, nation name, rep)} of clubs and national teams."""
    nations = {struct.unpack_from('<i', r)[0]: L.text(r[4:55]) for r in L.records(f'{game}/Data/nation.dat', 290)}
    out = {}
    for f in ('club.dat', 'nat_club.dat'):
        for r in L.records(f'{game}/Data/{f}', 581):
            out[struct.unpack_from('<i', r)[0]] = (L.text(r[4:55]), L.text(r[56:82]),
                                                   nations.get(struct.unpack_from('<i', r, 83)[0], '?'), struct.unpack_from('<H', r, 128)[0])
    return out


def rows(game):
    out = {}
    for f in TABLES:
        p = f'{game}/cmui_logos/{f}'
        if os.path.exists(p):
            for row in csv.reader(open(p, encoding='utf-8'), delimiter='\t'):
                if row and not row[0].startswith('c'):
                    out.setdefault(int(row[0]), row)
    return out


def picture(row):
    p = row[5] if os.path.isabs(row[5]) else os.path.abspath(f'{L.OUT}/png/{row[5].rsplit("/", 1)[-1]}')
    return p if os.path.exists(p) else None


def carry(old, new):
    a, b, have = clubs(old), clubs(new), rows(new)
    by_nation = {}
    # the new database's own matches too: a reserve side takes its parent's crest
    for db, table in ((a, rows(old)), (b, have)):
        for cid, row in table.items():
            if cid in db and (pic := picture(row)):
                name, short, nation, _ = db[cid]
                by_nation.setdefault(nation, []).append((name, short, row[4], pic))
    out, renamed = [], []
    for cid, (name, short, nation, rep) in b.items():
        if cid in have:
            continue
        cands = by_nation.get(nation, [])
        exact = [c for c in cands if name in (c[0], c[1]) or (short and short in (c[0], c[1]))]
        # ponytail: exact names only; a fuzzy fit took "United FC (UAE)" for the UAE national team
        parent = RESERVE.sub('', name).strip()
        if len({c[3] for c in exact}) == 1:
            best, how = exact[0], 'carry'
        elif parent != name and len({c[3] for c in cands if parent in (c[0], c[1])}) == 1:
            best, how = next(c for c in cands if parent in (c[0], c[1])), 'carry reserve'
        else:
            continue
        out.append((cid, name, nation, rep, best[2], best[3], how, short))
        if best[0] != name:
            renamed.append((rep, best[0], name, how))
    return out, renamed


def hand(game, table=os.path.join(os.path.dirname(os.path.abspath(__file__)), 'club_logos.tsv')):
    """club_logos.tsv (CM name, fl|footy, the site's logo name) -> <game>\\cmui_logos\\hand_match.tsv,
    downloading pictures not cached yet."""
    import json
    import footy
    fl = {name: url for _, name, url in csv.reader(open(f'{L.OUT}/index.tsv', encoding='utf-8'), delimiter='\t')}
    ft = {e['label']: e['href'].rsplit('/', 1)[-1] for e in json.load(open(f'{footy.CACHE}/search-index.json', encoding='utf8'))
          if e.get('kind') == 'logo' and '/logos/' in e.get('href', '')}
    by_name = {}
    for cid, (name, short, nation, rep) in clubs(game).items():
        by_name.setdefault(name, (cid, name, nation, rep, short))
    out = []
    for line in open(table, encoding='utf-8'):
        if line.startswith('#') or not line.strip():
            continue
        name, src, logo = line.rstrip('\n').split('\t')
        if name not in by_name:
            print('not in this database:', name)
            continue
        if src == 'fl' and logo in fl:
            path = os.path.abspath(f'{L.OUT}/png/{fl[logo].rsplit("/", 1)[-1]}')
            if not os.path.exists(path):
                footy.shrink(footy.fetch(fl[logo]), path, 256)
        elif src == 'footy' and logo in ft:
            path = os.path.abspath(f'{footy.CACHE}/png/{ft[logo]}.png')
            if not os.path.exists(path):
                footy.shrink(footy.fetch(f'{footy.SITE}/downloads/logo/{ft[logo]}-logo-footylogos.png'), path, 256)
        else:
            print('not on the site:', src, logo)
            continue
        cid, name, nation, rep, short = by_name[name]
        out.append((cid, name, nation, rep, logo, path, 'hand', short))
    with open(f'{game}/cmui_logos/hand_match.tsv', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(out)
    print(len(out), 'hand-picked crests')


if __name__ == '__main__':
    if sys.argv[1] == 'hand':
        hand(sys.argv[2])
        sys.exit()
    old, new = sys.argv[1], sys.argv[2]
    out, renamed = carry(old, new)
    print(f'{len(out)} crests carried by name, {len(renamed)} of them under another name:')
    for rep, was, now, how in sorted(renamed, reverse=True)[:40]:
        print(f'   {was}  ->  {now}   ({how})')
    if '--dry' not in sys.argv:
        os.makedirs(f'{new}/cmui_logos', exist_ok=True)
        with open(f'{new}/cmui_logos/carry_match.tsv', 'w', newline='', encoding='utf-8') as f:
            csv.writer(f, delimiter='\t').writerows(out)
