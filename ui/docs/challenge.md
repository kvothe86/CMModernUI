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
| 23 RAD challenges | the game reads them on every human turn (`track.rs`) and settles them when the season turns |
| The other 35 | the host clicks the mark (all "in the starting XI" ones and Keeper 10+ clean sheets: no such stat) |
| A wrong verdict | the host clicks the mark: his word stands, the game leaves that row alone. The cross removes a row. |

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
| `src\lib.rs` | `mod challenge;`, `"challenge"` in `FEATURES` (size back to 29), `const CHALLENGE`, the `has("challenge")` icon line, the `challenge::bar_item` block in `sidebar_item`, the block in `poll_input`, the block in the flip pass, the `b'E'` line in `host_answer` |
| `make_exe.py`, `make_patches.py`, `install.py`, `cmui_config.ps1` | the switch |
| `docs\` | this file, its row in `INDEX.md` and `switches.md`, key `E` in `network.md` |

The `pub`s it added in `history.rs`, `comp.rs` and `assistant.rs` and `history::sentence` do no harm.
