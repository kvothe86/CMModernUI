"""challenge_rad_review.py: the RAD list of 2026-11-06 into src\\challenge\\rules.tsv (cp1252).

Only challenges the game ticks off itself stay on the wheel; the rest get a fifth field 1
(archived): kept, so old seasons' rows still read, but never drawn again. Points: the transfers and
staff ones as they were (the census cannot simulate deals or staff); the others by how often a
first-division club met them in 2030/31 without trying (`challenge_sim.py` on Glory_v2): 50%+ 3,
25%+ 6, 10%+ 9, 3%+ 12, below 20 (the user, 2026-11-07: 15 raised to 20). The site's extra challenges (renames.tsv) are added the same way.
Run it again after a change here; it rewrites only the C lines."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
RULES = os.path.join(HERE, 'src', 'challenge', 'rules.tsv')

# (category, text, points, archived); the euro sign is written as EURO, cp1252 makes it byte 0x80
EURO = '€'
LIST = [
    ('Staff', 'No staff', 6, 0),
    ('Staff', 'Assistant manager <10', 3, 0),
    ('Staff', 'Max 2 staff members in total', 3, 0),
    ('Staff', 'Dutch staff only', 3, 0),
    ('Staff', 'No physio and no scout', 3, 0),
    ('Staff', 'All coaches with coaching attributes below 5', 6, 0),
    # players and the team: 2030/31 rates in the comments
    ('Players', 'Keeper 10+ clean sheets', 3, 0),                     # 97%
    ('Players', 'Player scores 20+ goals', 6, 0),                     # 27%
    ('Players', 'At least 3 players with 10+ goals', 6, 0),           # 31%
    ('Players', 'At least 3 players with 5+ assists', 3, 0),          # 74%
    ('Players', 'Team gets no more than 1 red card', 9, 0),           # 10%
    ('Players', 'Team gets no more than 3 red cards', 6, 0),          # 42%
    ('Players', 'Team gets no more than 5 red cards', 3, 0),          # 71%
    ('Players', 'Player becomes top scorer OR top assists OR highest average rating', 9, 0),  # 14%
    ('Players', 'Player gets two of three (top scorer/assists/average rating)', 20, 0),      # 1%
    ('Players', 'Player becomes top scorer, top assists and highest average rating', 20, 0),  # 0%
    # age (starting XI: the 11 with most league starts; first team: 10+ league games)
    ('Age', '3 players 35+ in the starting XI', 20, 0),          # 0%
    ('Age', '5 players 30+ in the starting XI', 9, 0),           # 24%
    ('Age', '5 players 32+ in the starting XI', 20, 0),          # 2%
    ('Age', '5 players under 21 in the starting XI', 20, 0),     # 0%
    ('Age', '5 players under 23 in the starting XI', 20, 0),     # 2%
    ('Age', 'Average age starting XI <=23', 20, 0),              # 1%
    ('Age', 'Average age starting XI >=30', 20, 0),              # 2%
    ('Age', 'No player younger than 18 in the starting XI', 3, 1),  # 98%: no challenge
    ('Age', 'First team average age <=20', 20, 0),               # 0%
    ('Age', 'First team average age <=23', 20, 0),               # 1%
    ('Age', 'First team average age >=30', 20, 0),               # 1%
    ('Age', 'First team average age >=32', 20, 0),               # 0%
    # squad
    ('Squad', '5 left-footed players in the starting XI', 12, 0),   # 8%
    ('Squad', '5 right-footed players in the starting XI', 3, 1),   # 100%: no challenge
    ('Squad', 'Keeper + 4 defenders all 28+', 12, 0),               # 7%
    ('Squad', 'At least 3 homegrown players in the starting XI', 6, 0),   # joined the club at 18 or younger
    ('Squad', 'No player worth more than ' + EURO + '10M in the starting XI', 6, 0),
    ('Squad', 'At least 2 youth players promoted to the starting XI', 6, 0),  # 21 or younger, joined at 18 or younger
    ('Squad', 'At least 3 players with international caps this season', 3, 0),  # 65%
    # transfers (as they were)
    ('Transfers', 'Max 1 purchase', 9, 0),
    ('Transfers', 'Max 3 purchases', 6, 0),
    ('Transfers', 'No winter transfers', 3, 0),
    ('Transfers', 'No purchases > ' + EURO + '2M', 6, 0),
    ('Transfers', 'No sales in winter', 3, 0),
    ('Transfers', 'Only buy players <23', 6, 0),
    ('Transfers', 'Biggest purchase < ' + EURO + '5M', 6, 0),
    ('Transfers', 'Only Bosman/free transfers', 6, 0),
    ('Transfers', 'Sell your best player (avg rating 1 January)', 6, 0),
    ('Transfers', 'Release five players from your squad (no fee)', 6, 0),
    # nationality (first team unless the starting XI is named)
    ('Nationality', '5 players from Africa', 12, 0),            # 9%
    ('Nationality', '5 players from South America', 12, 0),     # 7%
    ('Nationality', '5 players from Eastern Europe', 20, 0),     # 1%
    ('Nationality', '5 players from Scandinavia', 20, 0),        # 1%
    ('Nationality', '5 foreigners in the starting XI', 3, 0),    # 74%
    ('Nationality', '5 players from NED/BEL/POR/TUR', 6, 0),     # 40%
    ('Nationality', '5 players from ENG/SPA/GER/ITA/FRA', 3, 0), # 52%
    ('Nationality', '3 players from the same country (not your own)', 6, 0),  # 25%
    ('Nationality', 'No player from your own country in the starting XI (min. 15 times)', 12, 0),  # 3%
    ('Nationality', 'At least 5 different nationalities in the starting XI', 3, 0),  # 83%
    ('Nationality', 'Team of 1 nationality from NED/BEL/POR/TUR', 20, 0),     # 1%
    ('Nationality', 'Team of 1 nationality from ENG/SPA/GER/ITA/FRA', 20, 0), # 0%
    ('Nationality', 'Team of 1 nationality from the other countries', 20, 0), # 0%
    ('Nationality', 'Fatbike kids challenge (African players only)', 20, 0),
    ('Nationality', 'Favela challenge (South American players only)', 20, 0),
    ('Nationality', 'Murica challenge (North American players only)', 20, 0),
    ('Nationality', 'Prawn cracker challenge (Asian players only)', 20, 0),
    ('Nationality', 'Down Under challenge (Oceanian players only)', 20, 0),
    ('Nationality', 'No players from your own country (competition)', 9, 1),  # what the site meant is unclear
    # tactics: the game keeps no match's formation, so cmui's match-day tracker notes it
    # (track::watch_tactics, 2026-11-07); park the bus is team instructions, not tracked
    ('Tactics', 'Play with a sweeper in the starting XI', 6, 0),
    ('Tactics', 'Play without wing backs', 6, 0),
    ('Tactics', 'Play with at least three attackers', 3, 0),
    ('Tactics', 'Always 4-4-2 (no other formation)', 3, 0),
    ('Tactics', 'Play with at least five defenders', 3, 0),
    ('Tactics', 'Play with at least five midfielders', 3, 0),
    ('Tactics', 'Play without attacking wingers', 6, 0),
    ('Tactics', 'Play without a striker (min. 15 competitive matches)', 6, 0),
    ('Tactics', 'Play park the bus, defensive, long ball, counter', 6, 1),
    ('Tactics', 'Play with 1 defender (Kamikaze)', 6, 0),
]


def main():
    lines = open(RULES, 'rb').read().decode('cp1252').splitlines()
    kept = [l for l in lines if not l.startswith('C\t')]
    new = [f'C\t{c}\t{t}\t{p}' + ('\t1' if a else '') for c, t, p, a in LIST]
    at = next((i for i, l in enumerate(lines) if l.startswith('C\t')), len(kept))
    out = kept[:at] + new + kept[at:]
    # (encoded first: a text that cannot be written must not leave the file empty)
    data = ('\n'.join(out) + '\n').encode('cp1252')
    open(RULES, 'wb').write(data)
    print(f'{len(new)} challenges, {sum(1 for x in LIST if x[3])} archived')


if __name__ == '__main__':
    main()
