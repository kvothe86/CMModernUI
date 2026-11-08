# cmui: features to build

Ideas agreed with the user, not built yet. One section per feature: what, open questions,
where it goes. Move a feature to `docs\screens.md` / `docs\switches.md` once it is built.

**Built 2026-11-07, build 0EC7A5, installed in all six folders, not yet seen in the game:**
- History medals: `comp.rs` header branch + `medal`.
- Season card CM Challenge block: `challenge::summary`, `season.rs` `draw`.
- Scout % column: `scout.rs` `flip` (right end of the Name cell, head "Scout %"), Player Search /
  Short List rated by the session's sharpest scout; sort: the Rec head (key 1) -> `by_scout`
  in `get_results`. Open: the "Scout %" head itself is not clickable (its spot is the Name
  head's, key 4); the Short List may not go through `get_results` (check with a trace).
- Qualifiers story: `news.rs` `qualifier_blocks`; header picture = the first competition's
  logo (`story_crest`), not a Europe icon. Open: route lines ("league 1st-4th", "FA Cup").
- Match day card v2: `card.rs` `lineups`, `assistant::lineup` (own XI from the selection and
  active tactic; the other side's own pick when it has eleven, else the scout report's guess
  from its manager's preferred formation, tagged "Expected XI"). Middle column: injured only.
  Open: referee, stadium, capacity, weather, pitch (no pre-match offsets known), "Missing" with
  bans; falls back to the first card when the panel is too small.
- Saved line-ups: switch 37 `saved_lineups`, `lineups.rs`, `assistant::fill_team`. Changed from
  the design: the strip sits right of the view title (the assistant's note keeps the band's
  left, the title stays in the middle). Open: rename (no text input yet), Fill subs as an
  Assistant menu item. Network clients: done in night 8 (host and client; `H` codes 100+, key
  `l`).

## Test notes 0EC7A5 (2026-11-07, the user's live test; fix after the list is complete)

1. Saved line-ups strip: the save / store buttons are unclear next to "XI 1 / 2 / 3". (Seen in
   the shot too: the slot number is drawn twice, a small "1" at the left and "XI 1"; the
   floppy reads as a grey box, not as "save"; nothing says click = load. Shape shows
   "2-3-4-1" for GK DC DC DMC DMC DMC ML MR MC MC FC, which counts right by lines but may not be
   what the user calls it.)
2. Match day card v2: "Your XI" is empty (formation 2-3-4-1 and manager shown, no players),
   Carlisle's expected XI fine. Likely cause: `assistant::lineup` reads the human's eleven from
   club +0x1e0, but his selection lives in the Tactics store (GET_SLOT, as
   `assistant::selection` reads it for the saved line-ups): use that. (Also: both clubs "1st of
   24" on the first day of the season, everyone level; the place is meaningless then.)
3. Drop the "Ast" tags (and the In tags with them) from the squad list: not wanted. Only a
   well working XI 1 / 2 / 3 (save, load, Fill subs).
4. While Continue Game processes, the screen sometimes goes grey for a moment, as if the
   drawn screen disappears, then comes back; seen most on days with U21 international matches.
   Not yet known whether new in 0EC7A5 or older. Diagnose first (`cmui.trace` / `cmui.shot`
   on such a day; compare with 66DE08 / the build before), no guess fixes.

Status, night 7 (2026-11-07): 1: the strip redrawn as "Load XI n" (blue when full, the shape or
"N changed" under it) and a separate green "Save" ("Sure?" in red before overwriting a full
slot); "Fill subs" its own button; to judge in the game. 2: fixed (the human side from the
tactics' selection), seen by script. 3: Ast and In tags gone. 4: not fixed. Lead: the theme's
flat fill takes the place of the game's background picture at the start of every full frame;
a frame flipped before the panels are drawn shows bare grey (with the old picture it showed the
picture, hardly seen). Not proven: a `bare frame` log line now goes to cmui.log for such frames,
with the date and the screen node, to see whether they come on U21 days.

## Match day card v2: line-ups on "Proceed to the match ?" (2026-11-07)

Redesign of the existing match card (switch 34, `src\card.rs` `note`, `flip`); same switch.

Layout (top to bottom):
- One slim row: home crest, name, league place, form pills | centre: date and time,
  "v", last meetings | away crest, name, place, form.
- Three columns filling the screen:
  - **Left, home line-up:** formation as text above it, in big amber ("4-3-3", "4-2-3-1"),
    then 11 rows (shirt no., position, name, condition %), subs on one line, manager name
    at the bottom.
  - **Middle, match facts (narrow):** referee, stadium, capacity, weather, pitch,
    competition and round, then "Missing": injured or suspended players per side.
  - **Right, away line-up:** same as the left.
- Bottom bar: "No" | "Proceed to the match ?" | "Yes" (the question moves down into the bar).

Open questions, check before building:
- **AI line-up timing:** the AI may only pick its XI after Yes. If so, show an "expected XI"
  (its last starting XI from the last fixture) and label it that way; if it is already
  picked, show the real one.
- **Formation text:** derive it from the positions of the 11 (count D / DM / M / AM / F rows,
  e.g. 4-2-3-1), never from preset names (Restructured's lie, see the Team report notes).
- **Weather / pitch:** `weather.rs` (switch 32) already reads the weather; check whether it
  is set before kick-off. Pitch state: drop the row if the game has no such field.
- **Referee, stadium, capacity:** in the database; find the fixture's referee field
  (fixture record 0x4f bytes, see `docs\screens.md`).
- **Small screens:** at 800x600 the subs line goes and the middle column shrinks to referee
  and stadium only; 1680x1050 is the full layout; 1280x800 must fit.
- Network: the client gets the card too (host builds it, like the other screens).

## Saved line-ups 1/2/3 + "Fill subs" (2026-11-07)

**Design approved by the user 2026-11-07 ("seems perfect, lets do it that way").**

Problem: the Assistant's picks always overwrite the user's own XI.

Look: the dark band under the squad toolbar (View / Sort by / Assistant / Clear squad /
Filter) is NOT empty: the game writes the current view's title there, centred in amber
("Position(s)"). Share it: the view title moves to the left end of the band (same font and
colour), the slots and Fill subs sit right-aligned in the rest. At 800x600 the slots shrink to
three square "1 / 2 / 3" buttons (name and formation in a tooltip line under the band on
hover) so the title still fits. Three slots plus one button:
- Slot = number, name ("League XI", default "XI 1"), formation, floppy icon. Click the slot =
  load; floppy = save the current XI + bench there (asks before overwriting a full slot);
  right-click = rename / clear. Empty slot is grey, "save to store".
- The loaded slot keeps an amber border while the selection is unchanged; its second line
  says "2 changed" in amber when stand-ins went in. Any hand edit drops the border. No extra
  status row (the band has only one line).
- Stand-ins and filled subs are marked in the squad list's existing tag column (where "Req" /
  "Unh" sit): amber **In** (+ "for Bradshaw (inj)" after the name), blue **Ast** on a sub
  the assistant filled. Tags go at the next Clear / hand edit of that row.
- **Fill subs**: never touches the XI or a sub you placed; fills only the empty bench places,
  up to the competition's allowed number of subs (not always 9; see the 2026-10-29 bug: the
  Assistant picks 9 when 5 / 7 are allowed), with a spread (a keeper if there is none,
  then cover for the XI's lines). Also an item in the Assistant menu. Works with no slot.

Behaviour:
- A slot stores the 11 and the bench by the game's permanent person id (not the list index),
  each with its position slot (GK, DC, ..., SB1..), the active tactic / formation, the name.
- Stored beside the save in a cmui file, one per save + club (like `cmui_season.txt`); the
  save itself is never touched. Network: each manager has their own slots.
- Load, in order: (1) the slot's XI and bench go in; (2) a starter who can't play (injured,
  banned, gone, on international duty, condition under ~75 %) is replaced for that position
  only by the assistant's per-slot pick (`assistant.rs` `Eye::score`, as "Best team"),
  taking a saved sub first if he fits the slot; (3) a bench place left empty or freed is
  filled as in Fill subs. Extend `pick_for` with a locked set.

Open questions:
- Can the squad screen switch the tactic, or only the tactics screen? If not, loading a slot
  saved under another formation only warns ("slot 2 is 4-2-3-1, tactic is 4-3-3").
- The condition limit: fixed 75 % or a setting.
- Later, with the Coach (switch 31): pick a slot by competition (cup XI for cup games).

## News: "European qualifiers decided" as a table (2026-11-07, approved by the user)

Now: one long blue line ("Champions League: Arsenal, Man City, ... Conference League:
Everton."), the header crest is the first club's (Man City, looks random), and the chip row
runs out of room (Everton, the Conference League cut off).

Idea: parse the body's "<competition>: a, b, c." parts into one block per competition:
- left: competition logo (the crest pack's, as in the chips) + name + a grey route line
  ("league 1st-4th", "5th, 6th, FA Cup") when it can be told from the league table;
- right: a tile per club (crest, name, grey league place or "FA Cup" / "League Cup" when he
  got in by a cup); the human's own club gold. Tiles and competition names are the links.
- Header picture: a neutral Europe / continent icon (or the nation's flag), not a club crest.
- No bottom chip row for this story kind (the blocks are the links).
- Same for the other continents' versions of the story (Copa Libertadores ...), and the
  "qualified for" stories of a single club stay as they are.

Where: `news.rs` (`story`, `row`, `word_links`; the chips). Check: Restructured (more cups),
800x600 (tiles wrap), network client.

## Season review card: CM Challenge strip (2026-11-07, asked by the user)

Left out of the first card on purpose (said so 2026-11-07); the user missed it after the first
real season end. Add a block "CM CHALLENGE" to the card (switch 35, `season.rs` `draw`), in
the empty space between Stars / Signings and Last six:
- challenges completed this season (the RAD challenges ticked between 1 July and 30 June),
  each as a small green pill with its name, plus the points they gave;
- career goals (switch 30) reached this season, gold, and "x of 10 career goals" overall;
- the manager's place in the CM Challenge league table now and the change over the season.
Only when switch 29 / 30 is on and the game has a league file; otherwise the block is not
drawn. The snapshot (`cmui_season.txt`) gets the season start's challenge points so the
season's gain can be told. Check: a season end in a network game (host and client), and a
game without CM Challenge (no empty block).

## Scout rating % as its own column, everywhere, sortable (2026-11-07, asked by the user)

Now: the scout % sits after the name in orange ("Jankovic, Aleksa 68%"), on the "... Search"
screens only (scout tabs, `scout.rs`).
Wanted:
- Its own column with a header ("Scout %"), plain white text, right-aligned so the numbers
  line up; between Name and Club (narrow the Name cell, the names stay left).
- On Player Search, every Scout #n tab and the Short List; only for players the club knows
  (the scout has seen them / enough knowledge); unknown players show "-" (grey).
- Sortable: clicking the header (and an item "Scout %" in the Sort by menu) orders the list by
  it, best first; a second click turns it round; unknown at the bottom.

Open questions:
- Sorting: done before for the Ability view. `lib.rs` `get_results` (hook on the search's
  result list, `GET_RESULTS`) swaps the game's compare function by the chosen sort key
  (0x10000 / 0x20000 -> `by_current` / `by_potential`); the squad does the same at its qsort
  call (`squad_sort`, `SQUAD_SORT_CALL`). Same here: a `by_scout` compare on the % and borrow
  a sort key: the Rec head's (its stars are the same rating, so "sort by Rec" = by %, the
  stars ties broken by the %), and a Scout % head that sends the same key. Trace the key the
  Rec sort sends (`get_results` logs it with `cmui.trace`). Check the Short List goes through
  `get_results` too; if not, find its list's sort call as for the squad.
- The Name cell's width per screen (search, scout tabs, short list differ) and at 800x600.
- Network: the client asks the host for the % (as the stars now).

## History: medal icons in the column headers (2026-11-07)

On every competition History / Winners list: a gold trophy before the "Winners" header, a
silver medal before "Runners-up", a bronze medal before "Third Place" when that column is
there (not every competition has it). In the header only, not beside the club names.

Where: the history headers are drawn flat grey in `comp.rs` (the `HEADERS` / "Winners" /
"Runners-up" / "Third Place" branch, about line 710; text at `text_at(...)`, line 717). The
Honours list (`comp.rs` `honours`, about line 526) already draws a cup in the place's colour:
reuse that for the trophy, add a round medal for 2nd / 3rd, shift the header text right by
the icon's width. Check the club-history and nation-history variants of the screen too, and
800x600.
