# CM Challenge

A career tracker inside the game: a button under the menu bar opens a game screen in cmui's skin,
with the content of the CM Challenge site (Dashboard, Season, RAD, Stats, History, Rules). The
game's menu bar stays: a click on it leaves the page and does what it always does, the page's own
button shuts it. All of it is English.

It shows in every game with a human manager: network (host and client) and single player. With one
manager the Dashboard is his career (rank, a short standing card, his facts in two rows of tiles)
instead of a table; with two or more it is the friends' league. In single player the player is the
host: he spins and ticks. RAD is optional there, a way to set yourself challenges (nothing is
asked of him; with no rows the tab says "Spin to set yourself a challenge"). Code: `src\challenge\`
(`mod.rs` the league, `track.rs` the RAD tracking, `draw.rs` the page, `rules.tsv` points, ranks and
RAD list, `renames.tsv`, `comps_*.tsv`).

## Who does what

The host's game works everything out; a client asks for the page (key `E`) and only reads it.

| What | How |
|---|---|
| Results and points | from each manager's history in the save (`facts_of`, `prizes`, `moves`); nothing is typed |
| RAD spin | the host (a single player too), **Spin for ...** |
| RAD challenges | since 2026-11-07: 70 to draw, all read by the game (`track.rs`, squad readings, the match-day tactics tracker), 4 archived (rules.tsv C lines, flag 1); settled when the season turns; an "at least" one reached shows its box ticked and counts in "points in" at once |
| Releases | "Release five players": from the transfer manager's past transfers (`releases`: kind 3, the club left, no club joined, not on 1 July / 1 January, when contracts run out); a release never reaches the manager's history |
| A wrong verdict | the host clicks the row: done, then failed, then the game's again; his word stands. The cross removes a row. |

The RAD tab: at the left every manager with his Active / Done / Failed counts for the season
shown (a click shows his), the wheel always for the running season and the chosen manager; at the
right his challenges of one season, grouped by where they stand; `<` and `>` change the season. A
new one says "New" in the list. Reached means reached, the points come when the season turns:
points count when `done` is set, at the season's turn or by the host.

Seasons: the running season (page, spins, RAD rows, readings) turns on 1 July (`RUNNING_FROM`), so
a game started in July has 2025/26 from its first day. Results from the game's history are placed
by `SEASON_FROM` (day 200), because supercups fall around it. The button stands under the lowest
menu bar item that is not "You Have News" / "Player Waiting" (those sit at the foot of the bar and
only limit the room). The top scorer is the league row (2) of the stats table, kept and shown by
the first word of the name; a file with full names still loads and is written in the short form.

Choices: winter is 1 December to 1 March; fees are the sentence's number whatever the currency sign;
age at the purchase; limits (Most, Least) settle once passed, staff and league leaders (End) on the
last reading of the season; "coaching attributes" are the four of `assistant::SPECIALITIES`.

## Career goals (switch 30 `career_goals`)

A Goals tab after RAD (`goals.rs`, the `Q` lines of `rules.tsv`): 48 goals for a whole career, worth
10 to 150 points (weighed against a season: about 16 to 22 points on the friends' league). The host
picks **ten** for each manager (a click picks or drops one), and a country for the three that ask
"a country of your choice" (the chip opens the list of nations whose league the game plays: only the leagues loaded at the
start, `track::league_object`). Picks and
countries can change only in the season of the manager's first pick. A goal counts what happens
from the season it was picked, once; its points go into the career total (Dashboard, rank, Stats'
rank), not into a season's.

The game follows 46 of the 48 (`goals::holds`); the host ticks the European Golden Boot and the
Champions League top scorer (neither is an award in the databases, and the players' stats have one
"Continental" row for all European cups). A tick also stands for a goal the game follows.

| From | Goals | How |
|---|---|---|
| the history | titles, cups, supercups, promotions, European and world cups, countries and clubs worked at, trophies by country and club, lowest division up | by the competition's nation, the clubs by season (`career_of`, `club_in`), the club's nation for European cups (big five by the game's nation names), the Conference League by its name; "lowest playable division": the league of the lowest reputation in its nation, the "<nation> Lower Division" catch-all left out (`lowest_of`); "ten seasons": played out |
| the league's leaders | top scorer, assists, average rating, top scorer three in a row | `track::league_leaders`, the stats' league row (2), average from 10 matches, only in the nation's first division (`first_division`); a season's last reading stands (`E` lines) |
| the league table | unbeaten, 10+ points behind at halfway, derbies | `track::standing`: League object `[0xadadfc][competition id]` (+0x3e teams, +0xb1 rows of 0x3b: +0x11 played, +0x14 lost, +0x19 points, +5 head-to-head); unbeaten: lost 0 with every match of a double round (2 x (teams - 1)) played; halfway: the gap to the leader at the first reading with half played; derbies: wins over the rival in the head-to-head entries (+5 won), added up over seasons |
| awards | Ballon d'Or, three in the Team of the Year | the squad's staff history, kinds 0x34 (+0xd the award's row in 0xae23e0, +8 the club then, +0x19 the place: 1 winner, 2 runner up, 3 third) and 0x32; the Ballon d'Or of the running season at the club, Team of the Year lines of the last 30 days |
| club records | a 10+ goal win | the club's all-time biggest win (`biggest_win`, Club_Records 0xad9dac: set +0x7d), the winner's goals, at a season the manager was there |

Read from the moment the switch is on: seasons before that have no table, leader or award readings.
Head-to-head records exist only in a league whose rules use them as a tie-break (rule 4 of
League+0xc5): in Restructured the Premier League, Ligue 1, Bundesliga, Eredivisie, La Liga (and
more; Serie A and Scotland not seen); in the CM25 data only Serie A, La Liga, Liga Portugal and the
Super Lig. Elsewhere a derby stays the host's to tick. A trace lists them (`challenge league`).

File: `W<TAB>manager<TAB>goal<TAB>season picked<TAB>country<TAB>season ticked (0)`, `E<TAB>manager<TAB>season<TAB>scorer<TAB>assists<TAB>rating` (0/1), `V<TAB>manager<TAB>season<TAB>key<TAB>value` (`G` in the first build, read as such by its five fields) (played, lost, full, gap, derby0..6, ballon, toty). Page: `Q` (the
goals, with whether the game follows each), `M` (a manager: can his goals still change), `W` (his
picks: the season reached, country, ticked), `N` (nations, host only). Off: no Goals tab, no goal
points; the marks stay in the file.

## The league file

`cmchallenge_<number>.tsv` next to the host's exe, tab-separated, rewritten on every change. It is
not in the save: results come from the save, this file keeps what the save has not: seasons typed on
the old site (`S`), each season's top scorer (`S`), RAD rows with their state (`R`), the best player
on 1 January (`B`). Back it up with the save. `<number>` comes from the first manager and how he
started, so every game, single player or network, has its own file and a new game starts empty,
without any setup.

A game without a file (the first opening of a new game, a save made before CM Challenge, a save
copied to another PC without the file): no error, the rules alone. Every season's results are read
from the save's own history, so the career is whole at once; only what the save does not hold starts
now: RAD rows, the top scorer of past seasons, the best player of 1 January. The file appears at the
first change (a top scorer, a spin, a tick).

`cmchallenge.tsv` (from `..\challenge_import.py`) is taken over by the first network game (two or
more human managers) with one of its managers and renamed; a single player game never takes it. The
same managers at the same clubs from the same day would share a file.

### English names and old files

The first version, and the friends' league typed in it, used Dutch names for the prizes and RAD
texts. `renames.tsv` (`old<TAB>new`) maps them: a league file is read through it (items of `S`
lines, the text and the unit of the note of `R` lines) and written back in English, so an old file
gives the same totals (test `a_dutch_file_counts_as_its_english_one`). `challenge_import.py` maps
the site's names through the same file. A new name goes in `rules.tsv` only; a renamed one needs its
line in `renames.tsv`.

The fonts have no euro sign: `page_bytes` writes "EUR " on both ways to the page. Rules tab: three
columns, two below 720 wide; a rule that does not fit a size down goes on on the next row.

## Competitions per database

`family()` (which scoring family a cup is in: Champions League, UEFA Cup, ...) goes by the game's own
names. `comps_original.tsv` (the original data, also the net folder), `comps_cm25.tsv` and
`comps_restructured.tsv` list every international competition and every national supercup-like name
with its family (0 CL, 1 UEFA Cup, 2 European Supercup, 3 club world cup, 4 World Cup, 5 continental
tournament, 7 national supercup, `-` nothing). When a game is read, the table that knows most of its
competition names is taken (`best_table`). A name no table lists (an edited database) goes by words,
as before. A nation's main cup is not listed: it counts by being the nation's top cup.

Made by `comp_names.py <Data folder> <out>` from `club_comp.dat` + `nation_comp.dat`: the old word
rules plus its `fix` list (qualifying and play-off tournaments count for nothing, another
confederation's champions league too, the accent of Copa America, ...). Run it again for a new
database, read the result by eye, add the table to `TABLES`.

## Dropping it

Switch it off: `challenge=0` in `cmui.ini`, or untick CM Challenge in `cmui_config.bat`.

Remove it: delete `src\challenge\`, `challenge_import.py` and `comp_names.py`, then take these out:

| File | What |
|---|---|
| `src\lib.rs` | `mod challenge;`, `"challenge"` and `"career_goals"` in `FEATURES` (size back to 29), `const CHALLENGE`, `const CAREER_GOALS`, the `has("challenge")` icon line, the `challenge::bar_item` block in `sidebar_item`, the block in `poll_input`, the block in the flip pass, the `b'E'` line in `host_answer` |
| `make_exe.py`, `make_patches.py`, `install.py`, `cmui_config.ps1` | the switch |
| `docs\` | this file, its row in `INDEX.md` and `switches.md`, key `E` in `network.md` |

The `pub`s it added in `history.rs`, `comp.rs` and `assistant.rs` and `history::sentence` do no harm.
