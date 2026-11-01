"""challenge_import.py <site json> [rules tsv] [league tsv]: turns the CM Challenge site's data
(its Export, or https://api.npoint.io/<bin>) into cmui's files, in the game's own text (cp1252):
the rules (src\\challenge\\rules.tsv, built into cmui.dll) and the league (cmchallenge.tsv, next to
the host's exe: the seasons played so far and the RAD rows). Lines, tab-separated:
  P category item points trophy      a prize
  G rank points                      a rank's lower bound
  C category text points             a RAD challenge
  S name year club scorer goals rating item|item...   a season as entered on the site
  R id name year points done text    a RAD row
"""
import json
import sys


def clean(text):
    return str(text).replace('\t', ' ').replace('\n', ' ').replace('≠', 'niet').strip()


def write(path, lines):
    with open(path, 'w', encoding='cp1252', errors='replace', newline='\n') as f:
        f.write(''.join('\t'.join(clean(c) for c in line) + '\n' for line in lines))


def start_year(year):
    return int(str(year).split('/')[0])


def main():
    site = json.load(open(sys.argv[1], encoding='utf-8'))
    db, cfg = site['db'], site['cfg']
    rules_path = sys.argv[2] if len(sys.argv) > 2 else 'src/challenge/rules.tsv'
    league_path = sys.argv[3] if len(sys.argv) > 3 else 'cmchallenge.tsv'
    rules = [['P', cat, item, v['v'], int(v['t'])] for cat, items in cfg['points'].items() for item, v in items.items()]
    rules += [['G', rank, points] for rank, points in cfg['goals'].items()]
    rules += [['C', cat, text, points] for cat, tasks in cfg['rad_challenges'].items() for text, points in tasks.items()]
    write(rules_path, rules)
    league = []
    for n, player in enumerate(db['players']):
        for s in player['seasons']:
            league.append(['S', player['name'], start_year(s['year']), s.get('club', ''), s.get('tsName', ''), s.get('tsGoals', ''), s.get('rating', ''), '|'.join(s.get('items', []))])
        for k, r in enumerate(player['rads']):
            league.append(['R', n * 1000 + k, player['name'], start_year(r['s']), r['pts'], int(bool(r['done'])), r['text']])
    write(league_path, league)
    print(len(rules), 'rules,', len(league), 'league rows')
    # the site's own totals, to hold cmui's against
    for player in db['players']:
        points = {item: v['v'] for items in cfg['points'].values() for item, v in items.items()}
        total = sum(points.get(i, 0) for s in player['seasons'] for i in s.get('items', []))
        total += sum(r['pts'] for r in player['rads'] if r['done'])
        print(player['name'], total)


if __name__ == '__main__':
    main()
