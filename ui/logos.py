"""Club crests for cmui from football-logos.cc (fan use; logos belong to the clubs).

usage: python logos.py index    read the site's country pages into ..\\logos\\index.tsv
       python logos.py match    pair them with Data\\club.dat clubs by country and name
                                -> ..\\logos\\match.tsv, prints the misses of known clubs
       python logos.py why Celtic   the candidates for clubs whose name contains the text
       python logos.py fetch    download the matched 256px PNGs into ..\\logos\\png (skips those there)
       python logos.py comps    competitions from the hand table comp_logos.tsv -> ..\\logos\\comp_match.tsv
       python logos.py flags    nation flags from flagcdn.com -> ..\\logos\\png\\flag-*.png, ..\\logos\\flag_match.tsv
       python logos.py pack     write ..\\Data\\cmui_crests.bin for the DLL (clubs and competitions)
       python logos.py badges <pack folder> [names.csv]   a badge pack on disk (an FM logo pack:
                                pictures by FM ID, names from names.csv as id,name[,nation]) matched
                                to this database -> badge_match.tsv; then pack (its pictures go first)
       python logos.py find Wednesday   the pack's names containing the text
       python logos.py swap "Sheffield Wednesday" owl.png   that crest replaced in the pack
                                (first swap backs the pack up to cmui_crests.bin.bak)
       python logos.py restore  the pack back from cmui_crests.bin.bak
"""
import csv
import difflib
import html
import os
import re
import struct
import sys
import time
import unicodedata
import urllib.request

SITE, OUT = 'https://football-logos.cc', '../logos'
# the game folder whose database is matched and packed (env CMUI_GAME, default this one); the
# site index and the downloaded pictures are shared, the match lists are per game
GAME = os.environ.get('CMUI_GAME', '..')
TSV = OUT if GAME == '..' else os.path.join(GAME, 'cmui_logos')
NOT_COUNTRIES = {'all', 'collections', 'license', 'search', 'about', 'contact', 'privacy', 'leagues', 'tags'}
# CM nation name -> site country slug, where the plain spelling differs
NATION_ALIAS = {'holland': 'netherlands', 'united states': 'usa', 'korea republic': 'south-korea',
                'republic of ireland': 'republic-of-ireland', 'eire': 'republic-of-ireland', 'czech republic': 'czech-republic',
                'ivory coast': 'cote-d-ivoire', 'democratic republic of congo': 'congo-dr', 'dr congo': 'congo-dr',
                'congo dr': 'congo-dr', 'zaire': 'congo-dr', 'united arab emirates': 'uae', 'cape verde islands': 'cabo-verde',
                'cape verde': 'cabo-verde', 'surinam': 'suriname', 'bosnia-herzegovina': 'bosnia-and-herzegovina',
                'bosnia': 'bosnia-and-herzegovina', 'netherlands antilles': 'curacao', 'swaziland': 'eswatini',
                'fyr macedonia': 'north-macedonia', 'fyrom': 'north-macedonia', 'macedonia': 'north-macedonia',
                'korea dpr': 'north-korea', 'china pr': 'china', 'fyr of macedonia': 'north-macedonia',
                'st vincent & the grenadines': 'saint-vincent-and-the-grenadines',
                'st. vincent & grenadines': 'saint-vincent-and-the-grenadines', 'chinese taipei': 'taiwan',
                'brunei darussalam': 'brunei', 'timor': 'timor-leste', 'east timor': 'timor-leste'}
NOISE = {'fc', 'afc', 'cf', 'sc', 'ac', 'fk', 'sk', 'cd', 'ud', 'club', 'de', 'the', 'kv', 'rc', 'as',
         'ss', 'us', 'calcio', 'football', 'futebol', 'clube', 'cp', 'sv', 'bk', 'if', 'ff', 'nk', 'logo',
         'bv', 'fsv', 'tsv', 'vfl', 'vfb', 'spvgg', '1', 'r', 'cs'}
# CM 2001 spelling -> the site's (English or current) word
WORD = {'munchen': 'munich', 'koln': 'cologne', 'kobenhavn': 'copenhagen', 'kiev': 'kyiv',
        'dinamo': 'dynamo', 'prague': 'praha', 'zenith': 'zenit', 'vienna': 'wien', 'olympiakos': 'olympiacos',
        'lyonnais': 'lyon'}  # (2025-26's "Olympique Lyon" = the site's "Olympique Lyonnais")
# words that do not tell clubs apart: a fuzzy match must share another word
GENERIC = {'atletico', 'deportivo', 'desportiva', 'desportivo', 'sporting', 'sport', 'sports', 'real',
           'united', 'city', 'town', 'athletic', 'union', 'racing', 'olympic', 'dynamo', 'spor', 'esporte',
           'associacao', 'sociedad', 'royal', 'rovers', 'wanderers', 'county', 'national', 'team', 'esportiva',
           'recreativo', 'universidad', 'estudiantes', 'nacional', 'olimpia', 'juventud', 'independiente',
           'rangers', 'city', 'b', 'ii', 'reserves', 'amateure', 'jong', 'youth', 'u21', 'u23', 'u19'}
SPELL = str.maketrans({'ø': 'o', 'Ø': 'O', 'æ': 'ae', 'Æ': 'AE', 'ß': 'ss', 'đ': 'd', 'ł': 'l', 'Ł': 'L'})


def get(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (cmui logo index)'})
    return urllib.request.urlopen(req, timeout=60).read().decode('utf-8', 'replace')


def norm(s):
    s = unicodedata.normalize('NFKD', s.translate(SPELL)).encode('ascii', 'ignore').decode().lower()
    s = re.sub(r'\b(?:[a-z]\.){2,}', lambda m: m.group(0).replace('.', ''), s)  # N.E.C. -> nec
    return [WORD.get(t, t) for t in re.sub(r'[^a-z0-9]+', ' ', s).split() if t not in NOISE]


def file_key(url):
    """A picture's file name as one word: .../fcsb.8cb78585.png -> fcsb."""
    return re.sub(r'[^a-z0-9]', '', re.split(r'[/\\]', url)[-1].split('.')[0].lower())


def score(keys, site):
    """How well a site name fits a CM club's names (1 = same words as the full name; a fit of the
    short name counts a little less, so "Frankfurt" does not tie FSV with Eintracht)."""
    best = 0
    for k, weight in zip(keys, (1, 0.97)):
        if k == site or ''.join(k) == ''.join(site):
            fit = 1
        elif (set(k) <= set(site) or (set(site) <= set(k) and (len(site) > 1 or set(k) - set(site) <= GENERIC))) \
                and (any(len(t) >= 3 and t not in GENERIC for t in set(k) & set(site)) or (len(k) == 1 and site[:1] == k)):
            # "Berwick Rangers" is not "Rangers", "Bayern Munich Amateure" is "Bayern Munich"; a one-word
            # name that starts the site's is ("AZ" -> "AZ Alkmaar")
            fit = 0.9
        elif any(len(t) >= 4 and t not in GENERIC for t in set(k) & set(site)):
            fit = difflib.SequenceMatcher(None, ' '.join(k), ' '.join(site)).ratio()
        else:
            fit = 0
        best = max(best, fit * weight)
    return best


def index():
    os.makedirs(OUT, exist_ok=True)
    slugs = sorted(set(re.findall(r'href="(?:https://football-logos\.cc)?/([a-z0-9-]+)/"', get(SITE + '/all/'))) - NOT_COUNTRIES)
    rows = []
    for slug in slugs:
        page = get(f'{SITE}/{slug}/')
        # the federations (FIFA, UEFA, CAF...) on the tournaments page sit in the general folder
        folder = f'(?:{slug}/)?' if slug == 'tournaments' else f'{slug}/'
        found = re.findall(rf'<img src="(https://assets\.football-logos\.cc/logos/{folder}256x256/[^"]+)"[^>]*alt="([^"]+?) logo"', page)
        rows += [(slug, html.unescape(name), url) for url, name in dict(found).items()]
        print(f'{slug}: {len(found)}', flush=True)
        time.sleep(0.3)  # be gentle with the site
    with open(f'{OUT}/index.tsv', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(rows)
    print(len(rows), 'logos in', len(slugs), 'pages')


def records(path, size):
    data = open(path, 'rb').read()
    return [data[i:i + size] for i in range(0, len(data), size)]


def text(b):
    return b.split(b'\0', 1)[0].decode('latin1')


def slug_of_nation(name):
    """The site's country page of a CM nation: "Trinidad & Tobago" -> trinidad-and-tobago, "The
    Gambia" -> gambia, "Curaçao" -> curacao, the aliases where the names differ."""
    key = name.lower()
    if key in NATION_ALIAS:
        return NATION_ALIAS[key]
    key = unicodedata.normalize('NFKD', key).encode('ascii', 'ignore').decode().replace('&', ' and ')
    key = re.sub(r'^the\s+', '', key)
    return NATION_ALIAS.get(key, re.sub(r'[^a-z0-9]+', '-', key).strip('-'))


def mutual_best(rows, score_at):
    """Of match rows, those whose picture (column 5) fits no other club or competition better: a
    badge pack has no picture for most clubs, so a club must not take its nearest one (SSD
    Atletico Ascoli the picture of Ascoli)."""
    best = {}
    for r in rows:
        best[r[5]] = max(best.get(r[5], 0), float(r[score_at]))
    return [r for r in rows if float(r[score_at]) >= best[r[5]] - 0.02]


# (club, nation): the site's logo, where the words tie (the database's plain "Excelsior")
# (the last three: from Wikipedia, rows of their own in index.tsv, not on the site)
CHOSEN = {('Excelsior', 'Holland'): 'Excelsior Rotterdam', ('Haarlem', 'Holland'): 'HFC Haarlem (Wikipedia)',
          ('RBC Roosendaal', 'Holland'): 'RBC Roosendaal (Wikipedia)', ('Veendam', 'Holland'): 'SC Veendam (Wikipedia)'}


def match(why=None, logos=None, out='match.tsv'):
    """`logos`: {country slug: [(name, url or path, norm(name))]}, '*' for logos of no known
    country (tried for every club); the site's index by default. Another source (a badge pack)
    keeps only the mutual best fits."""
    if logos is None:
        logos = {}
        for slug, name, url in csv.reader(open(f'{OUT}/index.tsv', encoding='utf-8'), delimiter='\t'):
            logos.setdefault(slug, []).append((name, url, norm(name)))
    nations = {struct.unpack_from('<i', r)[0]: text(r[4:55]) for r in records(f'{GAME}/Data/nation.dat', 290)}
    slug_of = {}
    for nid, name in nations.items():
        slug = slug_of_nation(name)
        if slug in logos:
            slug_of[nid] = slug
    # logos of no known country only meet the clubs they share a word with (a full pack is big)
    anywhere = {}
    for c in logos.get('*', []):
        for t in set(c[2]):
            anywhere.setdefault(t, []).append(c)
    rows, misses = [], []
    for r in records(f'{GAME}/Data/club.dat', 581):
        cid, nation = struct.unpack_from('<i', r)[0], struct.unpack_from('<i', r, 83)[0]
        name, short, rep = text(r[4:55]), text(r[56:82]), struct.unpack_from('<H', r, 128)[0]
        keys = [norm(name), norm(short) or norm(name)]
        near = {id(c): c for t in set(keys[0]) | set(keys[1]) for c in anywhere.get(t, [])}
        cands = logos.get(slug_of.get(nation), []) + list(near.values())
        # (the picture's file name counts as a name too: the site calls FCSB "FC Steaua București"
        # but files it as fcsb.png; a little less than the name, so SD Logroñés's logrones.png does not
        # tie with UD Logroñés)
        ranked = sorted(((max(score(keys, c[2]), 0.97 if file_key(c[1]) == ''.join(keys[0]) else 0), c) for c in cands if c[2]), key=lambda t: -t[0])
        chosen = [c for c in cands if CHOSEN.get((name, nations.get(nation))) == c[0]]
        if chosen:
            rows.append((cid, name, nations.get(nation, '?'), rep, chosen[0][0], chosen[0][1], '1.00', short))
            continue
        top = ranked[0][0] if ranked else 0
        tied = sum(1 for s, _ in ranked if s >= top - 0.02)
        if why and why.lower() in name.lower():
            print(cid, name, '|', short, '|', nations.get(nation), slug_of.get(nation), keys, [(round(s, 2), c[0]) for s, c in ranked[:4]])
        if top >= 0.8 and tied == 1:
            c = ranked[0][1]
            rows.append((cid, name, nations.get(nation, '?'), rep, c[0], c[1], f'{top:.2f}', short))
        elif cands:
            misses.append((rep, name, nations.get(nation, '?'), [c[0] for _, c in ranked[:tied]][:3]))
    if out != 'match.tsv':
        rows = mutual_best(rows, 6)
    else:
        # national teams (nat_club.dat, the same records): the site's "<Country> National Team"
        # badge of their nation's page, the plain one (not "... Women's National Team"); else
        # the federation's ("Portuguese Football Federation")
        for r in records(f'{GAME}/Data/nat_club.dat', 581):
            cid, nation = struct.unpack_from('<i', r)[0], struct.unpack_from('<i', r, 83)[0]
            name, short, rep = text(r[4:55]), text(r[56:82]), struct.unpack_from('<H', r, 128)[0]
            page = logos.get(slug_of.get(nation), [])
            teams = sorted((c for c in page if c[0].lower().endswith(' national team')), key=lambda c: len(c[0])) \
                or sorted((c for c in page if re.search(r'(?i)federation|football association', c[0])), key=lambda c: len(c[0]))
            if teams:
                rows.append((cid, name, nations.get(nation, '?'), rep, teams[0][0], teams[0][1], '1.00', short))
    os.makedirs(TSV, exist_ok=True)
    with open(f'{TSV}/{out}', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(rows)
    print(len(rows), 'clubs matched;', sum(1 for r in rows if r[6] != '1.00'), 'of them not word for word')
    print('countries without a CM nation:', sorted(set(logos) - set(slug_of.values()) - {'*'})[:60])
    print('best-known misses (reputation, club, nation, candidates):')
    for m in sorted(misses, reverse=True)[:40]:
        print('  ', m)


ADJECTIVES = {'dutch', 'swiss', 'greek', 'english', 'scottish', 'welsh', 'irish', 'danish', 'turkish', 'spanish',
              'french', 'german', 'polish', 'swedish', 'norwegian', 'finnish', 'belgian', 'czech', 'croatian',
              'hungarian', 'american', 'argentine', 'brazilian', 'japanese', 'chinese', 'korean', 'thai', 'saudi',
              'qatari', 'iranian', 'egyptian', 'australian', 'austrian', 'russian', 'ukrainian', 'romanian',
              'bulgarian', 'cypriot', 'serbian', 'italian', 'portuguese', 'mexican', 'chilean', 'uruguayan',
              'colombian', 'peruvian', 'slovak', 'slovenian', 'israeli', 'moroccan', 'tunisian', 'algerian'}
# words that make a different competition of the same country (a cup is not the league)
KIND = {'cup', 'copa', 'coppa', 'pokal', 'taca', 'beker', 'kupa', 'kupasi', 'puchar', 'super', 'supercopa',
        'supercoppa', 'trophy', 'shield', 'schaal', 'league', 'liga', 'lig', 'championship'}


def cnorm(s):
    """A competition name's words: like norm, but digits and single letters are kept (tiers and
    groups: "Polish 1 Liga", "Serie C/A")."""
    s = unicodedata.normalize('NFKD', s.translate(SPELL)).encode('ascii', 'ignore').decode().lower()
    s = re.sub(r'\b(?:[a-z]\.){2,}', lambda m: m.group(0).replace('.', ''), s)
    return re.sub(r'[^a-z0-9]+', ' ', s).split() or ['-']


def same_comp(a, b):
    """Two competition names for the same competition: extra words on one side only ("PKO BP
    Ekstraklasa"), and none of them a kind of competition, a tier or a group."""
    a, b = set(a), set(b)
    if not (a <= b or b <= a):
        return False  # a word swapped: "Recopa Sudamericana" is not "Copa Sudamericana"
    extra = a ^ b
    digits_a, digits_b = {t for t in a if t.isdigit()}, {t for t in b if t.isdigit()}
    return not (extra & KIND
                or any(len(t) == 1 and t.isalpha() for t in extra)
                or any(t[0].isalpha() and any(ch.isdigit() for ch in t) for t in extra)
                or (digits_a and digits_b and digits_a != digits_b))


def rings():
    """The Olympic rings, not on the site: drawn, so the package holds no picture of them. The
    black ring is white, or it is lost on the dark screens."""
    from PIL import Image, ImageDraw
    path = f'{OUT}/png/olympic-rings.png'
    if os.path.exists(path):
        return
    os.makedirs(f'{OUT}/png', exist_ok=True)
    big = Image.new('RGBA', (1024, 1024))
    draw = ImageDraw.Draw(big)
    for x, y, colour in ((224, 430, '#0081C8'), (512, 430, '#FFFFFF'), (800, 430, '#EE334E'), (368, 578, '#FCB131'), (656, 578, '#00A651')):
        draw.ellipse((x - 130, y - 130, x + 130, y + 130), outline=colour, width=30)
    big.resize((256, 256), Image.LANCZOS).save(path)


def comps():
    """Competitions: the hand-made table comp_logos.tsv (CM names are generic 2001 English names,
    "Dutch Premier Division", that no name matching gets right) -> ..\\logos\\comp_match.tsv,
    same columns as match.tsv, short name left empty (shared: "First Division")."""
    site = {(slug, name): url for slug, name, url in csv.reader(open(f'{OUT}/index.tsv', encoding='utf-8'), delimiter='\t')}
    cm = {}
    for f in ('club_comp.dat', 'nation_comp.dat'):
        for r in records(f'{GAME}/Data/{f}', 107):
            cm[text(r[4:55])] = (struct.unpack_from('<i', r)[0], struct.unpack_from('<h', r, 105)[0])
    rings()
    rows = []
    for line in open('comp_logos.tsv', encoding='utf-8'):
        if line.startswith('#') or not line.strip():
            continue
        name, slug, logo = line.rstrip('\n').split('\t')
        url = logo if slug == 'drawn' else site.get((slug, logo))  # (drawn: a file in png, made here)
        if name not in cm:
            print('not in this database:', name)
        elif not url:
            print('not on the site:', slug, logo)
        else:
            rows.append((f'c{cm[name][0]}', name, slug, cm[name][1], logo, url, 'table', ''))
    # the rest by name (newer databases use the real names: "Dutch Eredivisie", "UEFA Europa
    # League"): the nation's adjective dropped, matched against the league-like logos of its
    # country, or of the tournaments page for a competition without a nation
    league = re.compile(r'(?i)liga|league|division|divisie|serie|bundesliga|cup|pokal|coupe|copa|coppa|ligue|premiership|'
                        r'allsvenskan|eliteserien|superliga|lig\b|eredivisie|championship|trophy|euro|world|libertadores|'
                        r'super|veikkaus|ekstraklasa|hnl|primeira|brasileiro|j1|j2|mls|a-league|k league|schaal|taça|kupa|puchar')
    by_slug = {}
    for (slug, name), url in site.items():
        if league.search(name):
            by_slug.setdefault(slug, []).append((name, url, norm(name)))
    auto = comps_by_name(by_slug, {r[1] for r in rows})
    rows += auto
    print(len(auto), 'more by name')
    os.makedirs(TSV, exist_ok=True)
    with open(f'{TSV}/comp_match.tsv', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(rows)
    print(len(rows), 'competitions with a logo')


def comps_by_name(by_slug, done):
    """CM competitions not in `done` matched by name to logos {country slug: [(name, url or path,
    words)]} ('tournaments' for those without a nation, '*' tried for all): match.tsv rows."""
    nations = {struct.unpack_from('<i', r)[0]: text(r[4:55]) for r in records(f'{GAME}/Data/nation.dat', 290)}
    slug_of = {nid: slug_of_nation(n) for nid, n in nations.items()}
    rows, done = [], set(done)
    for f in ('club_comp.dat', 'nation_comp.dat'):
        for r in records(f'{GAME}/Data/{f}', 107):
            name, cid, rep = text(r[4:55]), struct.unpack_from('<i', r)[0], struct.unpack_from('<h', r, 105)[0]
            nation = struct.unpack_from('<i', r, 93)[0]
            if name in done:
                continue
            slug = slug_of.get(nation, 'tournaments')
            words = cnorm(name)
            keys = [words]
            if len(words) > 1 and (words[0] in ADJECTIVES or words[0][:4] == cnorm(nations.get(nation, '-'))[0][:4]):
                keys.insert(0, words[1:])  # "Dutch Eredivisie" -> "Eredivisie"
            ranked = []
            # (a logo of no known country takes the whole name: "Welsh Cup" is not "AFC Asian Cup")
            for c, ks in [(c, keys) for c in by_slug.get(slug, [])] + [(c, [words]) for c in by_slug.get('*', [])]:
                site_words = cnorm(c[0])
                if any(same_comp(k, site_words) for k in ks):
                    fit = max(difflib.SequenceMatcher(None, ' '.join(k), ' '.join(site_words)).ratio() for k in ks)
                    ranked.append((fit, c))
            ranked.sort(key=lambda t: -t[0])
            if ranked and (len(ranked) == 1 or ranked[1][0] < ranked[0][0] - 0.02):
                c = ranked[0][1]
                rows.append((f'c{cid}', name, slug, rep, c[0], c[1], f'{ranked[0][0]:.2f}', ''))
                done.add(name)
    return rows


def badges(folder, names_csv=None):
    """A badge pack on disk -> badge_match.tsv and badge_comp_match.tsv, which pack prefers to
    the site's logos. FM packs name each picture by FM's unique ID (clubs\\normal\\1234.png,
    competitions\\normal\\56.png): their names come from `names_csv`, rows of id,name[,nation]
    (comma, semicolon or tab; exported from FM's editor or a search view). Pictures named after
    the club or competition ("Ajax.png", "1234 - Ajax.png") need no list. Of the same ID the
    biggest picture is taken (normal over small); nations' flags are left to flags."""
    names = {}
    if names_csv:
        text_ = open(names_csv, encoding='utf-8-sig').read()
        dialect = csv.Sniffer().sniff(text_[:4096], delimiters=',;\t')
        for row in csv.reader(text_.splitlines(), dialect):
            if row and row[0].strip().isdigit():
                names[row[0].strip()] = (row[1].strip(), row[2].strip() if len(row) > 2 else '')
    best = {}  # (kind, key) -> (pixels, path)
    for root, _, files in os.walk(folder):
        kind = 'comp' if 'competition' in root.lower() else 'nation' if 'nation' in root.lower() else 'club'
        for f in files:
            stem, ext = os.path.splitext(f)
            if ext.lower() not in ('.png', '.jpg', '.jpeg') or kind == 'nation':
                continue
            path = os.path.abspath(os.path.join(root, f))
            size = os.path.getsize(path)
            if best.get((kind, stem), (0,))[0] < size:
                best[kind, stem] = (size, path)
    clubs, comps_ = {}, {}
    unnamed = 0
    for (kind, stem), (_, path) in best.items():
        if stem in names:
            name, nation = names[stem]
        elif not stem.isdigit():
            name, nation = re.sub(r'^\d+\s*-\s*', '', stem), ''
        else:
            unnamed += 1
            continue
        slug = slug_of_nation(nation) if nation else '*'
        (comps_ if kind == 'comp' else clubs).setdefault(slug, []).append((name, path, norm(name)))
    print(sum(map(len, clubs.values())), 'club and', sum(map(len, comps_.values())), 'competition pictures named;',
          unnamed, 'pictures by ID without a name (give the names list)')
    match(logos=clubs, out='badge_match.tsv')
    rows = mutual_best(comps_by_name(comps_, set()), 6)
    with open(f'{TSV}/badge_comp_match.tsv', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(rows)
    print(len(rows), 'competitions matched')


def comp_names():
    """Lists for writing comp_logos.tsv by hand: CM competitions of a reputation of 5 and up,
    and the index names that look like leagues, cups or tournaments."""
    nations = {struct.unpack_from('<i', r)[0]: text(r[4:55]) for r in records(f'{GAME}/Data/nation.dat', 290)}
    for f in ('club_comp.dat', 'nation_comp.dat'):
        for r in records(f'{GAME}/Data/{f}', 107):
            rep = struct.unpack_from('<h', r, 105)[0]
            if rep >= 5:
                print('CM', rep, nations.get(struct.unpack_from('<i', r, 93)[0], '-'), '|', text(r[4:55]))
    league = re.compile(r'(?i)liga|league|division|divisie|serie|bundesliga|cup|pokal|coupe|copa|coppa|ligue|premiership|'
                        r'allsvenskan|eliteserien|superliga|lig\b|eredivisie|championship|trophy|euro|world|olymp|libertadores|super')
    for slug, name, url in csv.reader(open(f'{OUT}/index.tsv', encoding='utf-8'), delimiter='\t'):
        if league.search(name):
            print('SITE', slug, '|', name)


def fetch():
    os.makedirs(f'{OUT}/png', exist_ok=True)
    urls = sorted({row[5] for f in ('match.tsv', 'comp_match.tsv') if os.path.exists(f'{TSV}/{f}')
                   for row in csv.reader(open(f'{TSV}/{f}', encoding='utf-8'), delimiter='\t')})
    got = 0
    for i, url in enumerate(urls):
        path = f'{OUT}/png/{url.rsplit("/", 1)[-1]}'
        if os.path.exists(path):
            continue
        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (cmui logo fetch)'})
        try:
            data = urllib.request.urlopen(req, timeout=60).read()
        except OSError as e:
            print('failed', url, e, flush=True)
            continue
        open(path, 'wb').write(data)
        got += 1
        if got % 200 == 0:
            print(f'{i + 1}/{len(urls)}', flush=True)
        time.sleep(0.1)  # be gentle with the site
    print(got, 'downloaded,', len(os.listdir(f'{OUT}/png')), 'in', f'{OUT}/png')


FLAGS = 'https://flagcdn.com'
# CM nation name -> flagcdn code, where the names differ (CM 2001 names, UK home nations)
FLAG_ALIAS = {'holland': 'nl', 'england': 'gb-eng', 'scotland': 'gb-sct', 'wales': 'gb-wls',
              'northern ireland': 'gb-nir', 'republic of ireland': 'ie', 'eire': 'ie', 'usa': 'us',
              'korea republic': 'kr', 'south korea': 'kr', 'north korea': 'kp', 'korea dpr': 'kp',
              'czech republic': 'cz', 'ivory coast': 'ci', "cote d'ivoire": 'ci', 'fyr macedonia': 'mk',
              'macedonia': 'mk', 'bosnia': 'ba', 'bosnia-herzegovina': 'ba', 'china pr': 'cn', 'china': 'cn',
              'congo dr': 'cd', 'dr congo': 'cd', 'zaire': 'cd', 'congo': 'cg', 'russia': 'ru', 'iran': 'ir',
              'syria': 'sy', 'moldova': 'md', 'cape verde': 'cv', 'cape verde islands': 'cv', 'laos': 'la',
              'vietnam': 'vn', 'taiwan': 'tw', 'chinese taipei': 'tw', 'brunei': 'bn', 'tanzania': 'tz',
              'st. kitts & nevis': 'kn', 'st. lucia': 'lc', 'st. vincent & grenadines': 'vc', 'gambia': 'gm',
              'trinidad & tobago': 'tt', 'antigua & barbuda': 'ag', 'sao tome & principe': 'st',
              'turks & caicos islands': 'tc', 'us virgin islands': 'vi', 'british virgin islands': 'vg',
              'faroe islands': 'fo', 'kyrgyzstan': 'kg', 'bolivia': 'bo', 'venezuela': 've',
              'east timor': 'tl', 'swaziland': 'sz', 'burma': 'mm', 'myanmar': 'mm', 'palestine': 'ps',
              'serbia & montenegro': 'rs', 'yugoslavia': 'rs', 'netherlands antilles': 'cw', 'tahiti': 'pf',
              'brunei darussalam': 'bn', 'the congo': 'cg', 'fyr of macedonia': 'mk', 'timor': 'tl',
              'democratic republic of congo': 'cd', 'st kitts & nevis': 'kn', 'west germany': 'de',
              'great britain': 'gb', 'bonaire': 'bq', 'eswatini': 'sz'}


def flags():
    """The flag of every CM nation from flagcdn.com (public domain) into ..\\logos\\png\\flag640-xx.png,
    and ..\\logos\\flag_match.tsv for pack (keyed "flag:<nation name>"). Prints the nations without one."""
    import json
    codes = json.loads(get(f'{FLAGS}/en/codes.json'))
    by_name = {' '.join(norm(n)): c for c, n in codes.items()}
    rows, misses = [], []
    for r in records(f'{GAME}/Data/nation.dat', 290):
        nid, name = struct.unpack_from('<i', r)[0], text(r[4:55])
        code = FLAG_ALIAS.get(name.lower()) or by_name.get(' '.join(norm(name)))
        if not code:
            close = difflib.get_close_matches(' '.join(norm(name)), by_name, 1, 0.85)
            code = by_name[close[0]] if close else None
        if not code:
            misses.append(name)
            continue
        # flagpedia's 640px bitmaps (flagcdn is its CDN), packed at FLAG pixels for the big
        # national team flag, and filtered down for the small ones
        path = f'{OUT}/png/flag640-{code}.png'
        if not os.path.exists(path):
            req = urllib.request.Request(f'{FLAGS}/w640/{code}.png', headers={'User-Agent': 'Mozilla/5.0 (cmui flags)'})
            open(path, 'wb').write(urllib.request.urlopen(req, timeout=60).read())
            time.sleep(0.05)
        rows.append((nid, f'flag:{name}', name, 0, codes[code], f'{FLAGS}/flag640-{code}.png', '1.00', ''))
    os.makedirs(TSV, exist_ok=True)
    with open(f'{TSV}/flag_match.tsv', 'w', newline='', encoding='utf-8') as f:
        csv.writer(f, delimiter='\t').writerows(rows)
    print(len(rows), 'flags; no flag for', misses)


CREST = 80  # pixels, fits the title bar of the 1680x1050 exe
FLAG = 160  # flags: big beside a national team's badge


def pack():
    """..\\Data\\cmui_crests.bin: 'CMU2', count, 0, then count x (name[52] latin1, offset, side)
    and the pictures, side x side x (RGB565, alpha): CREST for crests and logos, FLAG for the
    flags ("flag:Holland"). Keys are CM full and short names; a short name
    two clubs share goes to the better-known club; of the same club, a badge pack's picture
    (badges) goes before the site's."""
    from PIL import Image
    owner = {}
    rows = [(row, f.startswith('badge')) for f in ('match.tsv', 'comp_match.tsv', 'flag_match.tsv', 'badge_match.tsv', 'badge_comp_match.tsv')
            if os.path.exists(f'{TSV}/{f}') for row in csv.reader(open(f'{TSV}/{f}', encoding='utf-8'), delimiter='\t')]
    for (cid, name, nation, rep, site, url, fit, short), from_pack in rows:
        path = url if os.path.isabs(url) else f'{OUT}/png/{url.rsplit("/", 1)[-1]}'
        if not os.path.exists(path):
            continue
        for key in {name, short} - {''}:
            if key not in owner or (int(rep), from_pack) > owner[key][:2]:
                owner[key] = (int(rep), from_pack, path)
    blobs, offsets, done = [], {}, 0
    for key, (_, _, path) in owner.items():
        if path not in offsets:
            side = FLAG if key.startswith('flag:') else CREST
            img = Image.open(path).convert('RGBA')
            img.thumbnail((side, side), Image.LANCZOS)
            canvas = Image.new('RGBA', (side, side))
            canvas.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
            px = bytearray()
            for r, g, b, a in canvas.getdata():
                px += struct.pack('<HB', (r >> 3) << 11 | (g >> 2) << 5 | b >> 3, a)
            offsets[path] = (done, side)
            done += len(px)
            blobs.append(bytes(px))
    keys = sorted(owner)
    start = 12 + 60 * len(keys)
    with open(f'{GAME}/Data/cmui_crests.bin', 'wb') as f:
        f.write(b'CMU2' + struct.pack('<II', len(keys), 0))
        for key in keys:
            at, side = offsets[owner[key][2]]
            f.write(key.encode('latin1', 'replace')[:51].ljust(52, b'\0') + struct.pack('<II', start + at, side))
        f.write(b''.join(blobs))
    print(len(keys), 'names,', len(blobs), 'pictures,', start + done, 'bytes')


PACK = f'{GAME}/Data/cmui_crests.bin'


def pack_index():
    """The pack's names: {name: (offset, side)}."""
    with open(PACK, 'rb') as f:
        magic, count, _ = struct.unpack('<4sII', f.read(12))
        if magic != b'CMU2':
            sys.exit(f'{PACK}: not a CMU2 pack (run logos.py pack)')
        index = {}
        for _ in range(count):
            e = f.read(60)
            index[e[:52].split(b'\0')[0].decode('latin1')] = struct.unpack_from('<II', e, 52)
    return index


def plain(s):
    return ' '.join(cnorm(s))


def find(text_):
    """The pack's names containing the text (case and accents ignored), with the names that
    share each picture (a club's short name, say)."""
    index = pack_index()
    for name, at in sorted(index.items()):
        if plain(text_) in plain(name):
            also = [n for n, a in index.items() if a == at and n != name]
            print(name, f'(also: {", ".join(also)})' if also else '')


def fit(image, side):
    """An image centred in a transparent side x side square, as pack fits it."""
    from PIL import Image
    img = Image.open(image).convert('RGBA')
    img.thumbnail((side, side), Image.LANCZOS)
    canvas = Image.new('RGBA', (side, side))
    canvas.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
    return canvas


def picture(at, side):
    """A picture of the pack as an RGBA image."""
    from PIL import Image
    with open(PACK, 'rb') as f:
        f.seek(at)
        px = f.read(side * side * 3)
    img = Image.new('RGBA', (side, side))
    img.putdata([((v >> 11) << 3, (v >> 5 & 63) << 2, (v & 31) << 3, a)
                 for v, a in struct.iter_unpack('<HB', px)])
    return img


def swap(name, image):
    """One picture of the pack replaced in place by an image (PNG, JPG, ...): every name that
    shares it (the club's full and short name) shows the new one. The first swap copies the
    pack to cmui_crests.bin.bak (restore puts it back). A later `pack` builds the pack anew from
    the match lists, without the swaps."""
    from PIL import Image
    import shutil
    index = pack_index()
    if name not in index:
        close = [n for n in index if plain(name) in plain(n)][:20]
        sys.exit(f'"{name}" is not in the pack' + ('; did you mean: ' + ' | '.join(close) if close else ''))
    at, side = index[name]
    if not os.path.exists(PACK + '.bak'):
        shutil.copyfile(PACK, PACK + '.bak')
    px = b''.join(struct.pack('<HB', (r >> 3) << 11 | (g >> 2) << 5 | b >> 3, a) for r, g, b, a in fit(image, side).getdata())
    with open(PACK, 'r+b') as f:
        f.seek(at)
        f.write(px)
    print('swapped', ', '.join(n for n, a in index.items() if a == (at, side)), f'({side}px) in {PACK}')


def restore():
    import shutil
    if not os.path.exists(PACK + '.bak'):
        sys.exit(f'no {PACK}.bak: nothing swapped yet')
    shutil.copyfile(PACK + '.bak', PACK)
    print('restored', PACK, 'from the backup')


def gui():
    """A window over find / swap / restore: the game folder, a search of the pack's names, the
    crest the pack has beside the image chosen, Save to game and Restore original."""
    import tkinter as tk
    from tkinter import filedialog, messagebox
    from PIL import Image, ImageTk
    root = tk.Tk()
    root.title('cmui crest swap')
    folder, query, status = tk.StringVar(value=os.path.abspath(GAME)), tk.StringVar(), tk.StringVar()
    st = {'index': {}, 'names': [], 'image': None, 'photos': {}}

    def show(where, img):
        img = img.resize((160, 160)) if img else Image.new('RGBA', (160, 160))  # 80px crests at twice the size
        st['photos'][where] = ImageTk.PhotoImage(img)
        (old if where == 'old' else new).config(image=st['photos'][where])

    def chosen():
        sel = names.curselection()
        return names.get(sel[0]) if sel else None

    def load():
        global PACK
        PACK = os.path.join(folder.get(), 'Data', 'cmui_crests.bin')
        try:
            st['index'] = pack_index()
            status.set(f'{len(st["index"])} names in {PACK}')
        except (OSError, SystemExit) as e:
            st['index'] = {}
            status.set(f'no crest pack: {e}')
        st['names'] = [(n, plain(n)) for n in sorted(st['index'])]
        search()

    def search(*_):
        q = plain(query.get()) if query.get().strip() else ''
        names.delete(0, 'end')
        names.insert('end', *[n for n, p in st['names'] if q in p][:3000])
        select()

    def select(*_):
        name = chosen()
        show('old', picture(*st['index'][name]) if name else None)
        show('new', fit(st['image'], st['index'][name][1]) if name and st['image'] else None)
        if name:
            at = st['index'][name]
            also = [n for n, a in st['index'].items() if a == at and n != name]
            status.set(name + (f'  (also shown for: {", ".join(also)})' if also else ''))

    def pick_folder():
        path = filedialog.askdirectory(initialdir=folder.get(), title='The game folder (with Data\\)')
        if path:
            folder.set(path)
            load()

    def pick_image():
        path = filedialog.askopenfilename(title='The new crest',
                                          filetypes=[('Images', '*.png *.jpg *.jpeg *.gif *.bmp *.webp'), ('All', '*.*')])
        if path:
            try:
                fit(path, 1)
            except OSError as e:
                return messagebox.showerror('cmui crest swap', f'not a picture the tool can read (PNG, JPG...): {e}')
            st['image'] = path
            select()

    def save():
        name = chosen()
        if not (name and st['image']):
            return status.set('pick a name and an image first')
        try:
            swap(name, st['image'])
        except (OSError, SystemExit) as e:
            return messagebox.showerror('cmui crest swap', str(e))
        select()
        status.set(f'saved {name}: the game shows it from its next start (backup: cmui_crests.bin.bak)')

    def undo():
        if not os.path.exists(PACK + '.bak'):
            return status.set('nothing swapped yet in this game')
        if messagebox.askyesno('cmui crest swap', 'Put every crest back as it was before the first swap?'):
            restore()
            select()
            status.set('restored the original pack')

    top = tk.Frame(root, padx=8, pady=8)
    top.pack(fill='x')
    tk.Entry(top, textvariable=folder).pack(side='left', fill='x', expand=True)
    tk.Button(top, text='Game folder...', command=pick_folder).pack(side='left', padx=(6, 0))
    left = tk.Frame(root, padx=8)
    left.pack(side='left', fill='both', expand=True)
    tk.Label(left, text='Search a club, competition or flag:').pack(anchor='w')
    find_box = tk.Entry(left, textvariable=query)
    find_box.pack(fill='x')
    query.trace_add('write', search)
    names = tk.Listbox(left, width=40, height=20, exportselection=False)
    names.pack(fill='both', expand=True, pady=(4, 8))
    names.bind('<<ListboxSelect>>', select)
    right = tk.Frame(root, padx=8)
    right.pack(side='left', fill='y')
    tk.Label(right, text='Now in the game').grid(row=0, column=0)
    tk.Label(right, text='New').grid(row=0, column=1)
    old, new = tk.Label(right, bg='#202020'), tk.Label(right, bg='#202020')
    old.grid(row=1, column=0, padx=4)
    new.grid(row=1, column=1, padx=4)
    tk.Button(right, text='Choose image...', command=pick_image).grid(row=2, column=0, columnspan=2, sticky='we', pady=(8, 0))
    tk.Button(right, text='Save to game', command=save).grid(row=3, column=0, columnspan=2, sticky='we', pady=(4, 0))
    tk.Button(right, text='Restore original', command=undo).grid(row=4, column=0, columnspan=2, sticky='we', pady=(4, 0))
    tk.Label(root, textvariable=status, anchor='w', padx=8).pack(side='bottom', fill='x', before=top)
    load()
    find_box.focus()
    root.mainloop()


sys.stdout.reconfigure(encoding='utf-8')
{'index': index, 'match': match, 'why': lambda: match(sys.argv[2]), 'fetch': fetch, 'pack': pack, 'comps': comps, 'comp_names': comp_names,
 'flags': flags, 'badges': lambda: badges(*sys.argv[2:4]), 'find': lambda: find(sys.argv[2]),
 'swap': lambda: swap(*sys.argv[2:4]), 'restore': restore, 'gui': gui}[(sys.argv[1:] or ['gui'])[0]]()
