# CM Challenge

A friends' league inside a network game: a button under the menu bar opens a game screen in cmui's
skin, with the content of the CM Challenge site (Dashboard, Seizoen, RAD, Stats, Historie, Regels).
The game's menu bar stays: a click on it leaves the page and does what it always does, the page's
own button shuts it. It shows with two or more human managers, or on a client. Code: `src\challenge\` (`mod.rs` the league, `track.rs` the RAD
tracking, `draw.rs` the page, `rules.tsv` points, ranks and RAD list).

## Who does what

The host's game works everything out; a client asks for the page (key `E`) and only reads it.

| What | How |
|---|---|
| Results and points | from each manager's history in the save (`facts_of`, `prizes`, `moves`); nothing is typed |
| RAD spin | the host, **DRAAI** |
| 23 RAD challenges | the game reads them on every human turn (`track.rs`) and settles them when the season turns |
| The other 35 | the host clicks the mark (all "in basiself" ones and Keeper 10+ clean sheets: no such stat) |
| A wrong verdict | the host clicks the mark: his word stands, the game leaves that row alone. The cross removes a row. |

The RAD tab: at the left every manager with his Actief / Gehaald / Mislukt counts for the season
shown (a click shows his), the wheel always for the running season and the chosen manager; at the
right his challenges of one season, grouped by where they stand; `<` and `>` change the season. A
new one says "Nieuw" in the list. Behaald means reached, the points come when the season turns:
points count when `done` is set, at the season's turn or by the host.

Choices: winter is 1 December to 1 March; fees are the sentence's number whatever the currency sign;
age at the purchase; limits (Most, Least) settle once passed, staff and league leaders (End) on the
last reading of the season; "coaching attributes" are the four of `assistant::SPECIALITIES`.

## The league file

`cmchallenge_<number>.tsv` next to the host's exe, tab-separated, rewritten on every change. It is
not in the save: results come from the save, this file keeps what the save has not: seasons typed on
the old site (`S`), each season's top scorer (`S`), RAD rows with their state (`R`), the best player
on 1 January (`B`). Back it up with the save. `<number>` comes from the first manager and how he
started, so every game has its own league and a new game starts empty, without any setup.
`cmchallenge.tsv` (from `..\challenge_import.py`) is taken over by the first game with one of its
managers and renamed. The same managers at the same clubs from the same day would share a file.

## Dropping it

Switch it off: `challenge=0` in `cmui.ini`, or untick CM Challenge in `cmui_config.bat`.

Remove it: delete `src\challenge\` and `challenge_import.py`, then take these out:

| File | What |
|---|---|
| `src\lib.rs` | `mod challenge;`, `"challenge"` in `FEATURES` (size back to 29), `const CHALLENGE`, the `has("challenge")` icon line, the `challenge::bar_item` block in `sidebar_item`, the block in `poll_input`, the block in the flip pass, the `b'E'` line in `host_answer` |
| `make_exe.py`, `make_patches.py`, `install.py`, `cmui_config.ps1` | the switch |
| `docs\` | this file, its row in `INDEX.md` and `switches.md`, key `E` in `network.md` |

The `pub`s it added in `history.rs`, `comp.rs` and `assistant.rs` and `history::sentence` do no harm.
