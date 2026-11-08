# cmui: a modern look for CM 01/02

A dark, flat theme in the style of mike_appguy's WIP mod (champman0102.co.uk, topic 11030),
with attribute bars on the player profile. It runs next to cmfix (the Display Fix: Alt+Tab,
window, crisp text). cmfix's own `dsound.dll` is renamed `cmfix_dsound.dll` where
`E:\CM0102\dsfix` is installed (a stand-in `dsound.dll` with all of dsound's exports, so a sound
driver's DLL such as C-Media's HsSrv.dll no longer stops the game).

**Find anything fast (screen -> code -> check -> fixes, network, testing, switches): `docs\INDEX.md`.**

**Bugs fixed, their causes and code: `FIXES.md`** (look there first when a fixed bug comes back).

## Files

| file | what |
|---|---|
| `src/lib.rs` | the DLL: palette swap, flat background, panel and text-box hooks, attribute bars |
| `install.py` | installs cmui into a game folder (below) |
| `make_exe.py` | makes `cm0102_ui.exe` from a game folder's `cm0102.exe`: entry stub that loads `cmui.dll` (in `.nick`), the mark `CMUI` and a switch byte per feature |
| `make_patches.py` | the same as misc patches for Nick's CM0102Patcher, into `patches\` (below) |
| `logos.py` | crests, competition logos and flags (football-logos.cc, flagcdn, or a badge pack you have) -> `Data\cmui_crests.bin` |
| `proof\` | the final shots per resolution (1680x1050, `f2526_800x600\`, `r1280x800\`); `sheets.py` makes the sheets |
| `shot.py` | screenshot of the game window's client area |
| `peek.py` | reads live game memory; fails with error 5 here (see below) |
| `run.ps1` | ends a running game (quit file), copies the fresh `cmui.dll` in, starts `cm0102_ui.exe`, prints `cmui.log` |
| `drive.ps1` | drives the running game through cmui's script file, turns its shots into PNGs |

Build: `cargo build --release --target i686-pc-windows-msvc`, copy `cmui.dll` to the game
folder. `cm0102.exe`, `g800.mbr` and `m800.mbr` are never changed.

## How it hooks

Every change checks the original bytes first and logs to `..\cmui.log`.
- Palette: the call to the GUI palette init 0x5e4560 inside graphics init (0x5e2c49) goes to
  cmui, which then rewrites the 30 palette slots (`THEME`). The game copies these into each
  widget, so this recolours every screen. The tab bars' and button rows' colour is not a slot:
  `gui_add_tab_bar` 0x5ed780 converts indigo once (flag byte 0xb0be34, colour 0xb0be36) and
  that came out purple at 800x600, olive at 1280x800, black at 1680x1050; cmui sets both
  (the sidebar colour) with the palette, so the game keeps it.
- Background: the background picture blit in `Display::draw_screen` (0x55f436) becomes a flat
  fill of the real screen size (0xaebe08 x 0xaebdd0; the fill draws nothing unless the
  rectangle is inside the screen).
- `gui_draw_panel` 0x5e5df0: entry hook; gradients become flat. cmfix hooks the same function
  first; cmui chains in front of its jump.
- `gui_draw_text_box` 0x5e6d80: entry hook. A number drawn in the box after an attribute label
  on the same row gets a bar and moves right of it. Labels come padded (`"  Acceleration"`);
  names are checked against the profile tab's table at 0xddf570 (31 x 0x65 bytes); labels with
  the same cell size count too (the hidden-attribute column of the patched exe).
- The left menu bar is not drawn code: it is the `.mbr` picture (made with piced.exe from a BMP).
- The same text-box hook turns form (`6-7-7-8-9`) into coloured boxes, and morale words and
  condition percentages into bars.
- Crests: a large text box inside the title bar (the wide top panel) names a club, either all of
  it or the part in the last brackets ("Name (Club)"); its crest is blended in at the bar's left
  through a DirectDraw lock of the back buffer (0xaebde4, vtable +0x64 Lock / +0x80 Unlock).
  A title that names no club but contains the human manager's surname (news, inbox) gets his
  club's crest: the current human is `[0xb74c26 + 0x300 * channel]` (channel i16 at 0xb77c7a),
  staff +0x39 the club (name at +4), +8 the second name record. Pointers are checked with
  VirtualQuery before use.
- CA/PA stars: the flip at the end of `Display::draw_screen` (0x55f5f9) goes to cmui, which
  first draws "CA" and "PA" with five stars each under the last attribute column when the frame
  had an attribute grid. Screen parameter 0 (`ScreenManager::get_screen_param` 0x7ff010 on
  0xb74450) is the profiled staff; staff +0x61 its player record; player +5 / +7 CA / PA.

## Player profile, mike_appguy's layout

The rows Form, Morale and Condition are taken out of the attribute grid (their values are kept);
under the grid three panels are drawn in the flip pass: reputation (player +13 world, +11 current,
+9 home; gold, green, red rings), player status (form tiles, morale and condition bars) and
positions (player +15..+26: ratings for GK, SW, D, DM, M, AM, F, WB, then sides R, L, C; a pitch
with a dot per spot). The ability block (CA green, PA gold stars, a Hide button that does not take
clicks yet) takes the freed rows of the third column. The season stats table: flat heads, gold
Apps/Gls/Asts/MoM, light for the rest, Av R as a pill (green from 7.5, gold from 7.0). Labels are
matched in English ("Form", "Apps"...).

Above the grid, mike's info strip replaces the line "Born 16.6.73 (Age 28). Greek (41 caps/12
goals).": flag, date of birth, age, nationality, wage, contract end, value, caps. Age and
nationality come from that line (the game's words); birth date staff +0x10, caps +0x22/+0x23, value
+0x52 (Cash::Cash 0x450a90, Cash::format_display_amount 0x450aa0), nation +0x1a (pointer, name at
+4); wage and end date from the main contract (state table [0xae2d0c] + id x 0x4f, +0 index into
the contract manager [[0xae1050]], 0x50 bytes each; wage via STAFF_CONTRACT::format_wage_text
0x85e750, end date +0x2d). Drawn in the flip pass across the panel. The positions pitch is drawn
as FM does it (grey lines, grey dots for spots not played, a gold ring on the best one).

Grid seams: the game leaves a 1-pixel gap between darkened cells; the darken hook fills one pixel
further. The band under the three panels is filled down to the lowest cell-sized darkened rect.

Other fixes for fonts chosen in Game Settings: "(" / ")" on the menu bar are arrows in the game's
own fonts and brackets in others, so cmui draws triangles; a single word wider than its box is not
drawn at all by the game (the menu bar's "Competitions"), so it gets the largest font seen that fits
(font_text_width 0x5e5ad0). The stats table's "*" / "+" become flat "-" / "+" buttons, and the heads
right of them are flat on every page of columns; goals green, apps / assists / MoM gold.

## Crests

`logos.py` (run from `ui\`): `index` reads football-logos.cc's 235 country pages into
`..\logos\index.tsv` (club and league logos, plus tournaments); `match` pairs them with
`Data\club.dat` by country and name (scores, word table for 2001 spellings, generic words);
`why <name>` shows a club's candidates; `fetch` downloads the matched 256px PNGs;
`pack` writes `Data\cmui_crests.bin` (80px RGB565 + alpha, keyed by CM full and short names,
so another database only needs `match` + `pack` again).

Competitions come from the hand table `comp_logos.tsv` (CM name, site page, site logo name):
CM's generic 2001 names do not match by name. General logos only, never a year version (the
user's rule), so World Cup, Euro, Copa América and Africa Cup of Nations have none for now.
`comps` turns the table into `..\logos\comp_match.tsv`, which `fetch` and `pack` include; a
competition is keyed by its full name only (short names like "First Division" are shared).

Flags: `flags` matches `Data\nation.dat` names with flagcdn.com's list (aliases in `FLAG_ALIAS`),
downloads `..\logos\png\flag-xx.png` and writes `..\logos\flag_match.tsv`; `pack` keys them
"flag:<nation name>". 208 of 213; none for the Soviet Union, CIS, Czechoslovakia, East Germany,
Pays Basque; Yugoslavia gets Serbia's.

## Squad, tactics, match, news, compare (night of 2026-10-27)

- Club header (club's own title, e.g. the squad): league logo left, crest right (left of a Print /
  Action dropdown), name lifted, "1st in Dutch First Division" under it in the title's colour.
  The line comes from the "1st in D1" button and a click on it is sent to that button. Club +0x57
  is its league (comp record: name +4, short name +0x38).
- Squad lists: positions gold ("D/DM C", `is_position`), the fit % on the row green / gold / red;
  in views without a condition column (Position(s)) the fit is drawn at the position cell's right,
  from the game's own condition (0x62c590 on the injury manager [0xb63d28], flag 1 = percent).
  Rows are matched to the club's squad (club +0xd7, 50 staff) by name.
- Tactics pitch: grass, stripes and lines recoloured (panel colours 0x0200, 0x01c0, 0x0400),
  player markers (filled circles) become shirts (keeper green), arcs drawn by cmui, numbers black,
  names in dark pills with a fit pill under them. Subs' number cells grey.
- Active tabs (panel flags 0x830): flat, gold underline, gold label.
- Match screen (sides: top boxes 35-50 % of the screen wide naming a club): crest left of each
  score square; header band (league logo tile, competition by its full name, ground, date; half
  time and full time); events with a ball (goals, green) or a red cross (injuries), minute right;
  referee / attendance / weather as small caps labels over gold values. The game draws the
  events before the sides, so a redraw is asked for when a match screen first shows; the state
  lasts while full redraws keep showing the sides. Not done: key stats and the last 5 minutes
  bar (need the match record).
- News: dates in a smaller font from the first draw; the story text moves down (text and its
  line panels inside the story panel), making room for a white crest tile, a larger headline,
  the date and a divider; a club row at the bottom of the story panel (not clickable yet).
- Compare ("A ~ B" title): drawn over the game's screen in the flip pass (the game shows only
  differences): both players in the header, the 31 attributes as P1 / P2 boxes coloured by level
  (the lower one dimmed; "-" where the game masks), a radar of eight groups (`RADAR`), positional
  suitability pitches with the best spots, the current season from the game's own rows. The
  players are found by name in the staff table ([0xae23bc], 0x6e bytes, count [0xae2364]);
  values from 0x543540 (staff, player, 1..31).
- Dropdown menus: dark rows, light text.

## Second round (2026-10-27, from the user's notes; helper reports `helper_*.md`)

- Player search "Ability" view: an 8th View item (call 0x80a774 to Display::add_item redirected
  to add it after Other; data bit 0x4000, set with Other's 0x20 by a hook on the input handler
  0x80eb00, which returns -0xb). The game builds the Other layout; its Nat. / Caps / Form / Morale
  cells become Club / Pos / Current / Potential stars (true CA / PA, hidden with the profile's
  Hide), I.Gls and Cond. stay empty. Rows are matched by name ("Second, First" or common name,
  an index of all players built once; the game's own formatter 0x53dba0 asserts on some
  records). Not done: sorting by the star columns (their headers still sort by Form / Morale).
- Match overview: key stats strip (shots, on target, pass %, tackles %, yellows; match record M:
  live `*(param0 + 0x5f)`, after the match screen parameter 0x20) and the last-5-minutes bar in
  the sides' colours (M+0x142); checked against the Match Stats tab. Goals get a football icon.
  Live: HALF TIME shows "-" until the first half is over (period M+0x4f: 0x1ef first half,
  0x3de half time, 0 / 1 full time) and the second score is labelled SCORE until full time (not
  seen live: a script cannot drive a live match, `poll_input` does not run then).
- News: competition stories (headline names a league, cup, award, draw...) get the competition's
  logo when the story names it in full; stories about the human's club without naming it (his
  name or a player of his) get its crest; the club row opens the club (a click stores the club,
  a hook on the news input 0x7892b0 calls open_club_squad_screen 0x453d60 and returns -4, as the
  game's club links do 0x7643d0).
- Crests that would vanish into their background (more than 45 % of their solid pixels close in
  brightness) get a soft halo along their outline, one pixel lower right.
- Tactics list: Av R, Tck, Drb (the profile's own text, PlayerStats::format_player_stat 0x7c25b0,
  category 6) between the name and "capt", small labels over the columns. Condition is the
  game's own column; no fitness of its own (the same number).
- Position(s) view: the condition % only for the human's own club (the game does not show
  other clubs' there).
- Squad list headers: Apps / Av R keep the game's header like the other columns (the flat style
  only for the profile's stats table, whose heads share the row of its - / + buttons).
- Menu bar: one font size for all items, the largest every word fits in (no smaller
  "Competitions"); a redraw when it changes.
- Popup arrows of dropdown buttons (the 4 bitmap glyphs in Guio::draw, via gui_draw_glyph
  0x5e3ce0) are smooth chevrons. Not done: the small arrows of the squad screen's bottom buttons
  (Tactics, Training...), drawn some other way.

## Third round (same day, the user's further notes; helper report `helper_live.md`)

- Ability view sorted by its star columns: the build's call to get_current_results (0x808e47)
  swaps in cmui's comparators (true CA / PA) when the sort key is Current / Potential.
- Bottom buttons' small two-tone arrows (not drawn through gui_draw_glyph): found by their pixels
  at a button's right edge in the flip pass and drawn over as chevrons.
- Live match: the competition from the fixture (+0x14: the competition record; the fixture is
  the match object +4 live, screen parameter 0x1f afterwards), so cups and European ties get
  their own logo; the ground from the bottom bar, the period after the date; red card icon for
  "sent off"; minutes like "19, 58" and "pen 82".
- In-match tactics (screen parameter 10 = 6, parameter 0 the Tactics_Screen, +4 the engine; asked
  only once its pitch is seen: asked during day processing the screen manager crashes the game):
  Pos and live Cond in the list next to the game's match rating ("Rat"), live condition in the
  pitch pills. Engine player rows: E + 0x4796 + k*0x1be, +0x69 Staff*, +0x4d condition (float).
- A screen's kept flags (match, tactics, search) are dropped when the screen manager's current
  node (+0x7e2) changes, so the next screen's first frame is not read as the last one's.
- The test script also runs from the flip pass while poll_input does not (it does run in a live
  match; the click targets just have to be right). `peek addr` logs a dword. Proof shots of
  everything: `proof\` (`INDEX.md`, `sheet_1..4.png`).

## Fourth round (user's notes; helper report `helper_squad.md`)

- Squad View "Ability" (switch `search_ability`): the call 0x456f7f (Selection's add_item) adds
  it after Selection; view bit 0x2000 in screen parameter 7, set with Selection's 0x1000 by a hook
  on the club screen's input 0x4671f0 (returns -11); other view items clear it. The Selection Info
  layout's Form / Morale / Cond. cells become Age / Current / Potential stars, Apps and Av R
  empty. Cells match header columns by their centre (the header row is a few pixels left).
- News about the own club: a player named by the name he goes by (common name, staff +0xc).
- Match labels: HALF TIME and FULL TIME always (as mike has them), "-" until reached; "Full Time"
  / "End of 90 Minutes" in the period text count as over.

## Scout stars (switch `scout_stars`, `src/scout.rs`; design `scout_stars.md`)

- The search's Rec stars (entry +0x29 current, +0x2a potential) come from Mad Scientist's CM
  Scout Intrinsic weights, ranked over every player in the game (5★ top 2 %, 4★ top 10 %, 3★ top
  30 %, 2★ top 70 %). Future uses CA moved toward PA by age (full at 20, none from 28). Each
  search's scout sees them with a fixed error of up to ±0.7·(20 − judging) score points (Judging
  Ability for current, Judging Potential for future; hash of the two staff ids).
- Hook: the first line of the `get_results` hook (0x808e47) rewrites the rated entries (either
  byte above 0) of all ten searches in the SearchEditSession before the game builds its sorted
  copies. Entries without a scout rating keep 0, and searches without a scout keep the game's
  stars. Who gets reported and the scout news are unchanged.
- `cargo test --release --target i686-pc-windows-msvc scout` checks the record offsets against
  the named fields of the rust crate over all of `..\VVV.sav`.
- 2026-10-29 (user's choice): the cut points are the scout's own club's best eleven (5★ = as
  good as its 2nd best, 4★ its 5th, 3★ its 11th, 2★ within 8 points), the whole game only as a
  fallback; see `scout_stars.md`.

## News, competition screens, speed, assistant (2026-10-28/29)

- News screen (`src/news.rs`) and competition screens (`src/comp.rs`): see memory
  `cmui-news-comp-round` for the full list; history title counts come from the competition
  history table 0xae23d4 (26 bytes a row).
- Speed: `cmui.timing` in the game folder logs cmui's time per frame (texts, flip, panels,
  glyphs, search results); `timing_summary.py` splits it per `mark` step of a script. `mapped`
  keeps readable regions for 50 ms, `club_named` has a name index, crest pictures are cached,
  star shapes are made once a size. Every screen is about 12 ms a frame or less.
- Sorting: the squad Ability view's heads sort by age / CA / PA (the qsort call 0x484865 in
  build_filtered_sorted_squad_list); search rows take their player from the name's link widget.
- Drag outline: over a slot smaller than the dragged box it takes the slot's size (the screen
  save call 0x561388 in Display::drag_widget).
- Assistant pick (switch `assistant_pick`, `src/assistant.rs`, design and facts in
  `assistant_pick.md`): an Assistant button left of Clear Squad (squad screen) and left of the
  top Last Match (pre-match tactics screen). Best team / Most fit team / Squad rotation / Clear
  team, as the club's assistant judges the players (0x6e86d0), plus captain, free kick and
  corner takers and the Penalty Takers list; a two-line note says what he did and why. The pick
  runs in the screen's input handler (club 0x4671f0, tactics 0x89f710), which returns -11.
- `set_switch.py folder index`: sets one switch byte in an existing cm0102_ui.exe (never rebuild
  an exe with make_exe.py to add a switch: that drops patches made to the exe itself).

## Installing (install.py)

    python install.py "<game folder>" --check          which features this exe allows
    python install.py "<game folder>" [--off a,b] [--crests] [--badges <pack> [names.csv]]
    python install.py "<game folder>" --uninstall      lists cmui's files, removes them on y
    python install.py x --list                         the feature names

It checks the exe at all 15 hook sites (count of 2026-10-27; the same checks the DLL makes at start; a site that does
not match leaves its feature off), writes `cm0102_ui.exe`, copies `cmui.dll` (the package's, else
the build's) and writes `cmui.ini` (every feature on unless in `--off`). `--crests` runs
`logos.py` match / comps / flags / fetch / pack for that game's database. `cm0102.exe` is never
changed. `make_exe.py` puts its loader in the patcher's `.nick` section, or, in an exe without one
(the untouched 2001 exe), in a small `.cmui` section it adds. Tested: the user's patched exe
(1680x1050), the Restructured 2025-26 exe (800x600, NickDLL), the untouched 2001 exe (all hooks
in; that exe then stops at its own CD check).

The menu bars: at start, with `theme` on, the DLL finds each bar picture name the exe holds once
(`game.mbr` / `match.mbr` as 555, a resolution patch's `g800.mbr` / `m800.mbr` as 565), writes
`Data\gcui.mbr` etc. in the original's size and swaps the name in memory (so no exe patch depends
on the resolution). The exe's own sections are mapped copy-on-write, which `readable()` leaves out:
read them directly.

## CM0102Patcher misc patches (make_patches.py, 2026-10-27)

    python make_patches.py "<cm0102.exe>"            writes patches\*.patch and *.txt
    python make_patches.py --test "<cm0102.exe>" "<out.exe>" [a,b]   applies them like the patcher

Nick's format (`Patcher.LoadPatchFile`): lines `FILEOFFSET: OLD NEW` (one byte each), commands
such as `EXPANDEXE` (adds `.nick` if missing), `#` comments; a patch counts as applied when every
NEW byte is there; unapply writes OLD back. `cmui Loader` sets the entry point (file 0x120) to the
stub at 0xdf0000 (file 0x6e5000; no community patch uses 0x6e4000-0x6e6000 or the entry field):
push "cmui.dll", call [LoadLibraryA], jmp 0x547a13, then "cmui.dll", the mark `CMUI` at +0x1c and
a switch byte per feature at +0x20 (38 since 2026-11-07, all 0). Each `cmui - <feature>` patch sets one switch to 1. The DLL
(`exe_switches`) turns a feature off where its byte is 0; `cmui.ini` can only switch off.
`make_exe.py` writes the same stub with all switches 1. cmui.dll still has to be copied into the
game folder (the patcher cannot add files); the misc patches would go into the patcher's
`MiscPatches.zip` (a "cmui Patches" folder) if Nick takes them.

Tested with the patcher's own code (`E:\CM0102\CM0102Patcher.exe` 2.0 loaded in PowerShell,
`Patcher.LoadPatchFile` / `ApplyPatch` / `DetectPatch` / `UnApplyPatch`): all 16 not detected (count of 2026-10-27)
before, detected after; the patched exe equals `make_exe.py`'s byte for byte (the patcher only
appends its log to the exe's end); unapplying all gives the original back. In the game (25-26,
800x600): loader + 5 features → the other 10 logged off, menu bar written by the DLL
(`proof\f2526_800x600\p01..p02`).

## 1280x800 (the patcher's recommended resolution, tested 2026-10-27)

A copy of the 25-26 folder made 1280x800 with the patcher's own functions (`to1280x800` and
`tapanispacemaker` patches, `ResolutionChanger.SetResolution`, `RGNConverter.RGN2RGN` for
`bkg1280_800.rgn`, the 126x800 `g800.mbr` / `m800.mbr` and the 60 pictures), then `install.py`:
15 of 15 sites, menu bars 126x800 (565), and start, fixtures with the CL logo, dropdown menu,
squad, player profile and tactics all fit (`proof\r1280x800\`). Not checked there: a live match,
news, search, compare.

Then (user's notes, same day):
- Profile: where the gap between the grid and the stats table holds the panels' five lines a
  little closer (gap / 5 at least 3/4 of a grid row: 1280x800), they go there in the next smaller
  font, with no More/Less button, and the stats table (Av R...) stays in view. 800x600 keeps the
  overlay; 1680x1050 is unchanged.
- Tactics: the labels over the list (Av R / Tck / Drb / Pos / Cond, in a match Pos / Cond / Rat)
  ran through the thin seam of page background between the tactic's name panel and the list.
  `label_row_inside` (flip pass) finds that seam above the first row and its panel's width, fills
  it with the panel's colour and draws it again just above the labels, so the labels are a row
  of the list. Checked at all three sizes (the in-match shots in `proof\` are from before).

## Flags and national teams (2026-10-27, user's notes)

- Flags come from flagpedia's 640px bitmaps (`flagcdn.com/w640`, the same public-domain set
  flagpedia.net/download offers; `..\logos\png\flag640-xx.png`) and are packed at 160 pixels;
  crests stay at 80. `cmui_crests.bin` is now 'CMU2': each entry has its picture's side (60-byte
  entries); the DLL still reads the old 'CMUC'.
- `draw_crest` scales down by averaging the source pixels under each screen pixel (colours by
  alpha), no longer the nearest one: thin lines and stripes keep their weight. Flags' edges stay
  sharp (alpha snapped), and a flag's background is sampled left of its middle (its square goes
  past the bar), so a white flag on a white bar (England) gets the soft edge.
  `proof\flags_old_vs_new.png`: the old and new drawing at 24 and 40 pixels.
- National teams (`nat_club.dat`, the game's `database_is_national_team` 0x53b900): `match`
  gives each the site's "<Country> National Team" badge (the shortest such name of its nation's
  page, so not the women's; else its federation's, "Portuguese Football Federation"); the title
  bar has the country's flag at the left (where a club has its league's logo, the user's wish)
  and the badge at the right, the flag as tall as the badge whatever its shape (`crest_rows`),
  no league line. Nation names go to the site's pages through `slug_of_nation` (accents, "&",
  "The ", aliases: Ivory Coast -> cote-d-ivoire, Republic of Ireland -> republic-of-ireland,
  which also brought the Irish clubs in). 25-26: 446 of 462 national teams with a badge, 452
  with a flag; 2001: 412 of 426 and 416; the rest are historic sides (USSR, Yugoslavia, both
  Germanys, Czechoslovakia, Basques, Great Britain). `proof\26_national_team_flag.png` (Holland,
  1680x1050), `f2526_800x600\13_national_team_flag.png` (England), `14_national_team_bosnia.png`.
  Opened via Nations & Clubs > Nations > Europe > the nation (scripted: hover Nations, click the
  continent). The title bar's ">" menu (other squads / nations) opens over the flag and over a
  club's league logo cleanly: while it is open neither is drawn (checked at 800x600, England and
  Dorking; Escape does not close that menu, its own button does).

## Badge packs (logos.py badges, install.py --badges)

    python install.py "<game folder>" --badges "<pack folder>" [names.csv]   (with or without --crests)
    set CMUI_GAME=<game folder> & python logos.py badges "<pack folder>" [names.csv] & python logos.py pack

FM logo packs (sortitoutsi, FMG...) name each picture by FM's unique ID
(`clubs\normal\1234.png`, `competitions\normal\56.png`), which CM's database does not know. The
names come from `names.csv`, rows of `id,name[,nation]` (comma, semicolon or tab; a search view
or the editor in FM exports them). Pictures named after the club need no list. Of one ID the
biggest picture is taken; nations are left to the flags. Clubs are matched like the site's
logos (within the nation where given; logos of no known nation meet only clubs sharing a word),
competitions like `comps` (no adjective dropped for logos of no known nation: "Welsh Cup" is not
"AFC Asian Cup"); a pack keeps only mutual best fits (a pack has no picture for most clubs, and
SSD Atletico Ascoli must not take Ascoli's). `pack` prefers the pack's picture to the site's for
the same club. Tested with a mock FM pack made from the 25-26 site matches (41 clubs and 3
competitions by ID, small and normal sizes, names as the site writes them): all 44 linked, 45 of
47 links as the site's matching made them; the other 2 (Hamburger SV and its U19 took the "SC
Condor Hamburg" picture) because the mock had no picture of Hamburger SV, which a real pack has;
the site's own match of that picture to Concordia Hamburg (0.84) is doubtful too. In the game the
pack's (tinted) Dorking crest showed in the squad header. No real FM pack tested (none here; FM's
names list needs FM).

## Swap one crest (logos.py gui / find / swap / restore)

    python logos.py          (or logos.py gui: a window; double-clicking logos.py opens it too)
    set CMUI_GAME=<game folder> & python logos.py find wednesday
    set CMUI_GAME=<game folder> & python logos.py swap "Sheffield Wednesday" owl.png
    set CMUI_GAME=<game folder> & python logos.py restore

`find` lists the pack's names containing the text, with the names sharing each picture (the
club's short name). `swap` writes the image (PNG, JPG; no SVG), fitted into the picture's square
like `pack` does, over that picture in `Data\cmui_crests.bin`: all names sharing it change. The
first swap copies the pack to `cmui_crests.bin.bak`; `restore` copies it back (undoing every swap
since). A later `pack` rebuilds from the match lists and drops the swaps. Works on a club,
competition or `flag:<nation>` name. Tested on copies of the CM0102 and 25-26 packs: only that
picture's bytes change, restore gives the pack back byte for byte. The window does the same: game
folder (default `CMUI_GAME`), search, the pack's crest beside the chosen image (80px shown at
160), Save to game, Restore original (asks first); needs Pillow, as `pack` does. Driven by a
script on a pack copy (search, pick, choose, save, restore): all as the commands.

## Other installs and resolutions (tested 2026-10-27 on CM0102 Restructured 2025-2026)

That install has the same 3.9.68 exe with other patcher options plus NickDLL, no resolution
patch (800x600), the original CM colours (the user's own install has a colour tool's dark mode)
and a 2025 database. All 20 hooked places (in the 15 functions install.py checks) are
byte-identical. Install into another game folder with `install.py "<game folder>" --crests`.

`CMUI_GAME` also points `run.ps1` / `drive.ps1` at that folder. The match lists for another
game go to `<game folder>\cmui_logos\`; the downloaded pictures stay shared in `..\logos\png`.
Competitions are matched by name too (newer databases use real names); `same_comp` allows extra
words on one side only, never a tier, group or kind of competition.

What made it portable (everything also checked again at 1680x1050):
- Pictures: the original `game.mbr` / `match.mbr` are read as 555, the resolution patch's
  `g800.mbr` / `m800.mbr` as 565 (`menu_bars`). Screens can be 565 or 555
  (`screen_format`); cmui keeps its colours in 565 and converts at the screen; the game's colour
  constants are compared with `is_colour` (the constant converted, not the pixel).
- Nothing is recognised by the user's own colours: dropdown rows by structure (a short filled
  panel whose text starts with five spaces); names drawn black by the original colours on a
  light background are drawn light.
- Sizes: minimum box sizes scale with the screen (`sx` / `sy`, given at 1680x1050); columns cmui
  adds are measured from their text (tactics list, key stats labels, info strip); texts that do
  not fit are cut (`clip_text`) or shortened.
- Profile: the info strip is two lines where the line is tall enough, else one line of values in
  one font that fits all; the three panels go under the grid where they fit above the stats
  table, else they are an overlay over it, shown with More / Less (cmui.ini profile_details).
- Fonts picked to fit only come from real game fonts (0xffff0001..): font 0, which empty texts
  pass, draws nothing (this hid the in-match names at 800x600).
- Game text is Latin-1 (the euro sign is 0x80): `latin1` / `to_latin1` both ways.

## Network games (2026-10-27)

A network client is a terminal: the host builds its screens and sends every area / widget rect;
the client's `Display::process_command_packet` (0x564080) adds them with `Display::add_area` /
`add_item`. A resolution patch hooks those two (`call scale; <first instruction>`, code cave at
0x601e98: x past the menu bar × 560/256 + 127, y × 450/256 at 1680x1050), so the client scaled
the host's already-scaled rects again: everything 1.75 times too big, "back to 800x600" to the
user. `client_rects_as_sent` (always on, any resolution patch of that shape, nothing at 800x600)
points the client's two calls at stubs that run the hook's own first instruction only. Tested
on one PC with two game copies (`E:\CM0102_net` host, `E:\CM0102_net2` client, folders
junctioned): rects identical on both sides, clicks hit. Fonts: a bigger-fonts patch in the
user's exe (`jmp` at 0x55fa14 to `mov edx, [esp+0x24c]; add edx, 1`, to 7 at most) also ran
twice; the add_item stub takes the font argument one down first where that patch is there. Font
ids then equal the host's. Checked again on a 1680x1050 copy of Restructured 25-26 made with the
patcher's own code (`F:\CM0102 - Restructured 2025-2026 - 1680x1050`, client copy beside it with
cnc-ddraw/cmfix, the single-instance jne at 0x68d3b6 made a jmp and a windowed game.cfg: test
only).

Game data for a client (`src\remote.rs`): the client has no game data and no screen state (both
live on the host), so cmui's game-data reads go through `readable` / `rd` / `data_bytes` /
`game_text`, which in a client (`remote::CLIENT`, set while ScreenManager::run_network_client
runs) read the host's memory in 256-byte chunks, and the heavy lookups and game functions go to
the host as keys (`ask_host` / `host_answer`: P screen parameter of the client's channel, C club
by name, G / M staff by name, N national team, F condition, S season stat, W / V wage and value
text, A attributes, O positions text, K open a club from the news row). The packets ride on the
game's own connection with opcode 0x7c, taken at the entry of Display::process_command_packet on
both sides (the host answers on its game thread, for the channel whose slot asked, checking every
pointer a client sent: staff in the staff table, the screen parameter only on a channel with a
screen). The client asks for what a frame missed at the flip, redraws when answers change things,
keeps answers 2 s (used while asked again; a new screen marks them all old). Tested on the E:
pair (host VVV, client "Test Client" at Ajax): squad header and condition %, profile (strip,
bars, CA/PA, panels, stats), tactics pitch and list, news (crests, club row opens the club), match
overview (band, events, key stats), search and squad Ability views, national team header.
Also checked on the client (2026-10-27, evening): compare (Action, hover Compare Players, then
"Set as 1st / 2nd player" in its submenu and Yes; the picks are kept per human on the host,
0x7f7780 / 0x7f7920, so a client's work like the host's; an earlier "Two players must be
selected" came from a test that clicked the parent entry) and a whole live match (kick off, half
time, in-match tactics, full time). A match is played without its manager when that manager's
client is not connected on the day; connected, the host waits for it ("Player Waiting").

Dropdowns on a client: the client redraws only the two rows the pointer leaves and enters (the
host redraws the whole list), so `menu_row` darkens the menu frame only right after the game drew
it, and a lone row takes its shade from its place in `MENU_ROWS`.

800x600 / 1280x800 network test (2026-10-27, evening; exes made with the patcher, see Testing):
every screen of `suite.ps1` on client and host, the live match on the client. Fixed on the way
(all sizes, single player too): attribute labels share one font, the one the longest name fits
(`attribute_font`; hidden attributes known by their grid box), so 800x600 no longer mixes sizes or
wraps "Off The Ball"; compare names in one font with a dot where still too long, radar values
placed by their width (whole numbers in a narrow column); in-match "capt" becomes "(c)" where a
long name leaves no room. Left as is: the squad's position cells have no room for condition %
at 800x600 (left out by design), the game's own smaller font for long names in tactics lists.

Clicks on a client: the click handler fires a widget while the button is held over it; only a
screen built in this process lets go of the button (ScreenManager::monitor clears 0xb67a34 /
0xb67a38), so on a client the host's next screen came in under a held button and fired again
(Tactics, then a player of the tactics list; Back twice; Add Manager's nationality step skipped).
`send_click` (the client loop's call of Display::send_widget_click) clears both after a click is
sent. Not verifiable by script: dragging (the script's drag does not swap on the host either).

Testing hints: the host binds only the last adapter of
its ipconfig parse (here the WSL one, 172.18.16.1), join by typing that address; the game writes
`~net.out` with ipconfig's output and in `E:\CM0102` that file is read-only from 2023, which
made "Unable To Create" there.

## Switches

`cmui.ini` in the game folder holds one line per feature switch, 38 of them (the full list with
what each covers: [docs/switches.md](docs/switches.md); the settings window `cmui_config.bat`
shows them all); `name=0` turns one off, missing names are written as `=1` at start. Read once at
start (restart after a change). `theme=0` leaves the game's palette and background picture.
An exe made by the CM0102Patcher misc patches also carries a switch per feature (on only where
its patch is applied); the ini can switch those off, not on.

## Testing

The game runs with higher rights than the shell: it cannot be ended or read from outside, it
ignores WM_CLOSE, and it does not see the shell's environment. So:
- create `..\cmui.quit` to make cmui end the game;
- create `..\cmui.trace` to log panel and text-box draws;
- create `..\cmui.peek` to log the first bytes of the hooked functions;
- `drive.ps1 "x y" "type VVV" "key 13" "move x y" "wait ms" "shot ui\s.bmp"` drives the game:
  cmui reads `..\cmui.click` in `poll_input` and posts the mouse / key messages to the game window
  (0xb67a2c); a click is a down, then an up on a later poll (the window procedure 0x68d630 keeps
  the button held). Shots come from the back buffer, so a covered or locked screen does not
  matter. Dropdown menus: open with a click, then click the entry and `move` the mouse a pixel
  (the menu acts on the next message). Loading the test save: `"536 423" "596 372" "type VVV"
  "1470 1003" "wait 20000"`.
- By label, at any resolution: `click <text>` / `hover <text>` act on the centre of the newest
  drawn text that is `<text>` (else contains it; `\n` for two-line buttons, `click Name|-60` 60
  px left of it, `click Name|-11.3%` a share of the screen width, `click Name|slot` the squad's selection cell
  on that name's row, found by the column of slot texts, so at any layout). Labels are recorded only once a script has come (the screen is redrawn then),
  and forgotten at each full redraw. Mouse points are scaled into a window larger than the game
  (cnc-ddraw stretching; the real size from GetWindowInfo, cnc-ddraw answers GetClientRect with
  the game's).
- Network pairs at other resolutions (2026-10-27): `net_res.ps1 W H` makes `E:\CM0102_net_WxH`
  (+ `_c`) from the user's exe with the patcher's own resolution change and the cmui misc
  patches (run with the 32-bit PowerShell: the patcher is x86); `net_pair.ps1 <host> <client>`
  starts both by label (host restores VVV as network game "test", client joins as Test Client
  at Ajax); `suite.ps1` shoots every cmui screen by label on either side. For tests, set
  `text4k=0` in cmfix.ini (its text is drawn over the window, not in the back buffer) and give
  ddraw.ini `fullscreen=false` with the game's width / height (a plain window, not stretched).
- **The full check (a tool; testing itself is on the user's go, see memory):** `powershell -File
  check.ps1 [800x600,1280x800,1680x1050] [part]` builds the DLL and, per resolution, starts the
  pair, runs `suite.ps1` on the client and the host (the host is single player), then the
  client's live match (part `match`), closes the pair and writes `check\<res>\`: the shots, six
  a sheet (`c_sheet1.png`, `h_sheet1.png`, ...), and `summary.txt` (labels not found, panics,
  shot counts). Pairs: `E:\CM0102_net_800x600(_c)`, `E:\CM0102_net_1280x800(_c)`, `E:\CM0102_net`
  + `E:\CM0102_net2` (1680x1050). A new screen can get its own part in `suite.ps1` (an `if ($part
  -in "all", "<name>")` block of label clicks and `shot`s). Scripts handle screens well; the
  game's flow (joining, match day, bans, a host waiting for its client) is where they stall.

## Known, not cmui

Game Credits crashes the user's patched exe (1680x1050) with or without cmui (checked
2026-10-26): the crash comes before any credits drawing reaches cmui's hooks.

## Shelved

Player pictures in the profile's title bar (2026-10-28, worked in single player and on a network
client at 1680x1050): `shelved\player_pictures\` holds `faces.py` and the code with how to put it
back. Shelved until a face pack by name (not FM ID) turns up.

## Not done

Getting the misc patches into the patcher's `MiscPatches.zip` (Nick's call), `match` taking another database
folder, general World Cup / Euro / AFCON logos from another source, the red card icon seen in a
match.
