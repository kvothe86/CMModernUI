"""challenge_sim.py <game folder> [report.md]: how often each RAD challenge is met by a first-division
club that does not try, from the cm25 save's own data (the RAD review of 2026-11-06), and the points
that rarity suggests.

Input, written by cmui's census (`challenge::track::census`, `cmui.census` in the game folder):
  challenge_census.tsv   every player: nation, continent, age now, caps, feet, value, the age he
                         joined his club, positions, this season's league / international / club
                         records (32 bytes hex) and his chain in the history file
  challenge_clubs.tsv    every club by index: name, league, played first division, country
  challenge_history.bin  a copy of `player stats history.tmp`: 47-byte nodes, a 43-byte line
                         (+0 year, +2 club index, +6 the 32-byte record, +0x2a record number << 3)
                         and the next node's offset
A 32-byte record (0x7c2750, checked against the squad screen): +0 apps, +1 of them as sub,
+2 goals, +3 clean sheets, +6 assists, +0xb yellow, +0xc red cards, +0xd man of the match,
+0xe the rating sum (a short; / apps = the average).
Record numbers: 2 league, 5 international, 6 all the club's games.

A season is judged on what the game has after it: the starting XI is the 11 with most league
starts, the first team who played 10+ league games. Past seasons only (2025..2030, full)."""
import collections
import statistics
import sys

GAME = sys.argv[1] if len(sys.argv) > 1 else r'E:\cm25'
REPORT = sys.argv[2] if len(sys.argv) > 2 else None
NOW = 2031  # the census's year (Glory_v2, October 2031)

EAST = {'Albania', 'Armenia', 'Azerbaijan', 'Belarus', 'Bosnia', 'Bosnia-Herzegovina', 'Bulgaria', 'Croatia',
        'Czech Republic', 'Czechia', 'Estonia', 'Georgia', 'Hungary', 'Kosovo', 'Latvia', 'Lithuania', 'Moldova',
        'Montenegro', 'North Macedonia', 'Macedonia', 'FYR Macedonia', 'Poland', 'Romania', 'Russia', 'Serbia',
        'Slovakia', 'Slovenia', 'Ukraine'}
SCANDINAVIA = {'Denmark', 'Norway', 'Sweden', 'Finland', 'Iceland', 'Faroe Islands'}
NBPT = {'Holland', 'Netherlands', 'Belgium', 'Portugal', 'Turkey', 'Türkiye'}
BIG5 = {'England', 'Spain', 'Germany', 'Italy', 'France'}


def record(raw):
    b = raw if len(raw) == 32 else bytes(32)
    return {'apps': b[0], 'subs': b[1], 'goals': b[2], 'clean': b[3], 'assists': b[6], 'yellow': b[0xb],
            'red': b[0xc], 'mom': b[0xd], 'rating': int.from_bytes(b[0xe:0x10], 'little', signed=True)}


def load():
    clubs = {}
    for line in open(f'{GAME}\\challenge_clubs.tsv', encoding='latin-1').read().splitlines()[1:]:
        k, name, league, first, country = (line.split('\t') + [''] * 5)[:5]
        clubs[int(k)] = {'name': name, 'league': league, 'first': first == '1', 'country': country}
    players = []
    for line in open(f'{GAME}\\challenge_census.tsv', encoding='latin-1').read().splitlines()[1:]:
        p = line.split('\t')
        if len(p) < 18:
            continue
        players.append({'club_now': int(p[1]), 'name': p[2], 'nation': p[3], 'continent': p[4], 'age': int(p[6]),
                        'left': int(p[8]), 'right': int(p[9]), 'value': int(p[10]), 'joined_age': int(p[11]),
                        'positions': set(p[12].split('/')), 'first': int(p[16]), 'last': int(p[17])})
    history = open(f'{GAME}\\challenge_history.bin', 'rb').read()
    return clubs, players, history


def lines_of(history, first, last):
    """A player's past lines (year, club, record number, record), as 0x58f120 walks his chain."""
    out, here, seen = [], first, 0
    if last == -1:
        return out
    while 0 <= here and here + 47 <= len(history) and seen < 400:
        node = history[here:here + 47]
        out.append((int.from_bytes(node[0:2], 'little'), int.from_bytes(node[2:6], 'little', signed=True),
                    node[0x2a] >> 3, record(node[6:0x26])))
        nxt = int.from_bytes(node[43:47], 'little', signed=True)
        if here == last or nxt == -1:
            break
        here, seen = nxt, seen + 1
    return out


def seasons(clubs, players, history):
    """(club, year) -> the men who played there: their league record, club record, caps that season."""
    table = collections.defaultdict(dict)
    for p in players:
        for year, club, n, r in lines_of(history, p['first'], p['last']):
            if year >= NOW or club not in clubs or not clubs[club]['first']:
                continue
            row = table[(club, year)].setdefault(id(p), {'p': p, 'league': None, 'club': None, 'caps': 0})
            if n == 2:
                row['league'] = r
            elif n == 6:
                row['club'] = r
            elif n == 5:
                row['caps'] = r['apps']
    return table


def league_leaders(table, clubs):
    """(league, year) -> the three leaders' ids (top scorer, assists, average with 10+ games)."""
    by = collections.defaultdict(list)
    for (club, year), men in table.items():
        for m in men.values():
            if m['league'] and m['league']['apps'] > 0:
                by[(clubs[club]['league'], year)].append(m)
    out = {}
    for key, ms in by.items():
        top = max(ms, key=lambda m: m['league']['goals'])
        ast = max(ms, key=lambda m: m['league']['assists'])
        rated = [m for m in ms if m['league']['apps'] >= 10]
        best = max(rated, key=lambda m: m['league']['rating'] / m['league']['apps']) if rated else None
        out[key] = [id(m['p']) for m in (top, ast, best) if m]
    return out


def judge(club, year, men, leaders):
    """Every challenge this season: met or not; None when the season has too few men to judge."""
    played = [m for m in men.values() if m['league'] and m['league']['apps'] > 0]
    if len(played) < 14:
        return None
    starts = lambda m: m['league']['apps'] - m['league']['subs']
    xi = sorted(played, key=starts, reverse=True)[:11]
    first = [m for m in played if m['league']['apps'] >= 10]
    age = lambda m: m['p']['age'] - (NOW - year)
    totals = [m['club'] or m['league'] for m in men.values() if (m['club'] or m['league'])]
    country = club['country']
    nat = lambda m: m['p']['nation']
    cont = lambda m: m['p']['continent']
    count = lambda ms, f: sum(1 for m in ms if f(m))
    avg = lambda ms, f: statistics.mean(f(m) for m in ms) if ms else 0
    nations_xi = collections.Counter(nat(m) for m in xi)
    only = lambda group: len(nations_xi) == 1 and next(iter(nations_xi)) in group
    keepers = [m for m in played if 'GK' in m['p']['positions']]
    held = max((sum(1 for l in leaders if l == id(m['p'])) for m in played), default=0)
    gk = [m for m in xi if 'GK' in m['p']['positions']]
    backs = [m for m in xi if 'D' in m['p']['positions'] and 'GK' not in m['p']['positions']]
    return {
        # players and the team (all the club's games)
        'Keeper 10+ clean sheets': any(m['league']['clean'] >= 10 for m in keepers),
        'Player scores 20+ goals': any(t['goals'] >= 20 for t in totals),
        'At least 3 players with 10+ goals': sum(t['goals'] >= 10 for t in totals) >= 3,
        'At least 3 players with 5+ assists': sum(t['assists'] >= 5 for t in totals) >= 3,
        'Team gets no more than 1 red card': sum(t['red'] for t in totals) <= 1,
        'Team gets no more than 3 red cards': sum(t['red'] for t in totals) <= 3,
        'Team gets no more than 5 red cards': sum(t['red'] for t in totals) <= 5,
        'Player becomes top scorer OR top assists OR highest average rating': held >= 1,
        'Player gets two of three (top scorer/assists/average rating)': held >= 2,
        'Player becomes top scorer, top assists and highest average rating': held >= 3,
        # age, starting XI
        '3 players 35+ in the starting XI': count(xi, lambda m: age(m) >= 35) >= 3,
        '5 players 30+ in the starting XI': count(xi, lambda m: age(m) >= 30) >= 5,
        '5 players 32+ in the starting XI': count(xi, lambda m: age(m) >= 32) >= 5,
        '5 players under 21 in the starting XI': count(xi, lambda m: age(m) < 21) >= 5,
        '5 players under 23 in the starting XI': count(xi, lambda m: age(m) < 23) >= 5,
        'Average age starting XI <=23': avg(xi, age) <= 23,
        'Average age starting XI >=30': avg(xi, age) >= 30,
        'No player younger than 18 in the starting XI': count(xi, lambda m: age(m) < 18) == 0,
        'First team average age <=20': avg(first, age) <= 20,
        'First team average age <=23': avg(first, age) <= 23,
        'First team average age >=30': avg(first, age) >= 30,
        'First team average age >=32': avg(first, age) >= 32,
        # squad
        '5 left-footed players in the starting XI': count(xi, lambda m: m['p']['left'] > m['p']['right']) >= 5,
        '5 right-footed players in the starting XI': count(xi, lambda m: m['p']['right'] > m['p']['left']) >= 5,
        'Keeper + 4 defenders all 28+': len(gk) >= 1 and len(backs) >= 4 and all(age(m) >= 28 for m in gk[:1] + backs[:4]),
        'At least 3 players with international caps this season': count(first, lambda m: m['caps'] > 0) >= 3,
        # nationality
        '5 players from Africa': count(first, lambda m: cont(m) == 'Africa') >= 5,
        '5 players from South America': count(first, lambda m: cont(m) == 'South America') >= 5,
        '5 players from Eastern Europe': count(first, lambda m: nat(m) in EAST) >= 5,
        '5 players from Scandinavia': count(first, lambda m: nat(m) in SCANDINAVIA) >= 5,
        '5 foreigners in the starting XI': count(xi, lambda m: nat(m) != country) >= 5,
        '5 players from NED/BEL/POR/TUR': count(first, lambda m: nat(m) in NBPT) >= 5,
        '5 players from ENG/SPA/GER/ITA/FRA': count(first, lambda m: nat(m) in BIG5) >= 5,
        '3 players from the same country (not your own)': any(n != country and c >= 3 for n, c in collections.Counter(nat(m) for m in first).items()),
        'No player from your own country in the starting XI (min. 15 times)': count(xi, lambda m: nat(m) == country) == 0,
        'At least 5 different nationalities in the starting XI': len(nations_xi) >= 5,
        'Team of 1 nationality from NED/BEL/POR/TUR': only(NBPT),
        'Team of 1 nationality from ENG/SPA/GER/ITA/FRA': only(BIG5),
        'Team of 1 nationality from the other countries': len(nations_xi) == 1 and next(iter(nations_xi)) not in NBPT | BIG5,
        'Fatbike kids challenge (African players only)': all(cont(m) == 'Africa' for m in xi),
        'Favela challenge (South American players only)': all(cont(m) == 'South America' for m in xi),
        'Murica challenge (North American players only)': all(cont(m) in ('North America', 'Central America') for m in xi),
        'Prawn cracker challenge (Asian players only)': all(cont(m) == 'Asia' for m in xi),
        'Down Under challenge (Oceanian players only)': all(cont(m) == 'Oceania' for m in xi),
    }


def points(rate):
    """Rarer is worth more: the tiers the site uses (3, 6, 9) and two above for the rare ones (12,
    and 20: raised from 15 by the user, 2026-11-07)."""
    if rate >= 0.5:
        return 3
    if rate >= 0.25:
        return 6
    if rate >= 0.10:
        return 9
    if rate >= 0.03:
        return 12
    return 20


def main():
    clubs, players, history = load()
    table = seasons(clubs, players, history)
    leaders = league_leaders(table, clubs)
    # (all seasons, and the last one alone: the database has lost the men who retired since, so the
    # older a season the fewer of its old players; the last season's rate is the one to go by)
    met, asked, last_met, last_asked = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter()
    samples, last_samples = 0, 0
    for (club, year), men in sorted(table.items()):
        r = judge(clubs[club], year, men, leaders.get((clubs[club]['league'], year), []))
        if r is None:
            continue
        samples += 1
        last_samples += year == NOW - 1
        for name, ok in r.items():
            asked[name] += 1
            met[name] += bool(ok)
            if year == NOW - 1:
                last_asked[name] += 1
                last_met[name] += bool(ok)
    rate = lambda n: last_met[n] / last_asked[n] if last_asked[n] else 0
    rows = sorted(((rate(n), n) for n in asked), reverse=True)
    out = ['# RAD challenges: how often a first-division club meets them without trying\n',
           f'{last_samples} club seasons of 2030/31 ({samples} with 2025/26..2029/30) of the cm25 save Glory_v2, {len(players)} players.\n',
           '| Challenge | Met 2030/31 | Met all seasons | Points by rarity |', '|---|---|---|---|']
    for r, name in rows:
        out.append(f'| {name} | {r:.1%} | {met[name] / asked[name]:.1%} | {points(r)} |')
    text = '\n'.join(out) + '\n'
    print(text)
    if REPORT:
        open(REPORT, 'w', encoding='utf-8').write(text)


if __name__ == '__main__':
    main()
