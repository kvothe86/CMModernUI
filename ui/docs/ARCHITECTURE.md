# cmui architecture: read this first

One document for the whole picture: what cmui is, how it is built and loaded, what every source
file does, how network games work, where every other document lives and how far to trust it.
It points to the detailed docs and does not repeat them. Written 2026-11-07 from the code and
the docs as they stood then; where code and a doc disagree, the code wins (see section 6).

Paths start at the repository root unless a path starts with `ui\` or `..\` (then relative to
the `ui` folder). Addresses (0x5e5df0 ...) are virtual addresses in the 3.9.68 `cm0102.exe`.

Contents:

1. What cmui is, loading, building, testing, installing
2. Code map (every file in `ui\src`)
3. How a frame flows through the hooks
4. The switch table
5. Network model, host keys, blocks, prefetch
6. Where things are written down (and what is stale)
7. How to add a feature (checklist)

---

## 1. What cmui is

Two separate add-ons for Championship Manager 01/02 (3.9.68), both Rust 32-bit DLLs
(`i686-pc-windows-msvc`). Keep them apart: separate DLLs, separate packages.

| Project | Where | What |
|---|---|---|
| **cmui** ("CM Modern UI") | `ui\` | A DLL that hooks the running game: dark flat theme, crests, attribute bars, new overlays and screens, an Assistant, a Coach, CM Challenge. Every feature has its own switch. Works in single player and network games. |
| **CM Network** (cmnet) | `net\` | A stand-in `wsock32.dll`: network games over the internet (bind on all addresses, host names in the join box, UPnP, no Nagle delay, zstd packing of the wire). Passes everything else on to Windows. Needs no exe change. Docs: its `net\package\README CM Network.txt`, `net\src\lib.rs` top comment, and the "Network client" rows of `ui\FIXES.md`. |

cmui and CM Network only meet at the wire: cmui adds its own packets (opcode 0x7c, section 5)
to the game's connection; cmnet compresses whatever the game sends. cmui works without cmnet.
Also in the tree but outside `ui\`: `dsfix\` (a dsound stand-in so a sound driver's DLL loads,
see [switches.md](switches.md)), `cmui_package\` (what players download), `re\` and `rust\` (a
different project, the Rust rebuild of the game; not covered here).

### Loading

- `make_exe.py` writes `cm0102_ui.exe` from `cm0102.exe`: the entry point goes to a stub in the
  `.nick` (or `.cmui`) section that calls `LoadLibraryA("cmui.dll")` and jumps on to the original
  entry (0x547a13). The stub carries the mark `CMUI` and **one byte per feature switch** at
  stub+0x20 (1 = allowed, 0 = off).
- `make_patches.py` writes the same thing as misc patches for the CM0102Patcher (`patches\`:
  a loader patch, one patch per switch, an "Everything" patch). `install.py` checks the hook
  sites and writes exe, DLL and ini into a game folder.
- `DllMain` (`lib.rs`) runs `load_features` (exe bytes first, then `cmui.ini`: `name=0` turns a
  switch off; missing names are written as `=1`), then patches the hook sites. **Every hook
  compares the original bytes first** and leaves the site alone when another patch changed it.
- `cmui.ini` can switch off what the exe allows, never switch on. Read once at start.
- Details: [switches.md](switches.md), `..\README.md` ("How it hooks", "Installing",
  "CM0102Patcher misc patches").

### Building

```
cd ui
cargo build --release --target i686-pc-windows-msvc          # target\i686-pc-windows-msvc\release\cmui.dll
cargo test  --release --target i686-pc-windows-msvc          # unit tests (about 50)
```

`cargo test` includes `every_switch_list_agrees`: it fails when `make_exe.py`'s switch count or
a switch name in `make_patches.py`, `install.py` or `cmui_config.ps1` is missing. A feature
`save_check` exists for a local scout test against a save. Release profile: `panic = "abort"`,
`opt-level = "s"`. The DLL logs panics to `cmui.log` before the game aborts.

### Testing

The game ignores WM_CLOSE and cannot be read from outside, so all control goes through files in
the game folder that cmui polls. Full reference: [testing.md](testing.md).

| File / script | Use |
|---|---|
| `cmui.click` | script lines (`click <label>`, `x y`, `type`, `key`, `wait`, `shot f.bmp`, `mark`) read and deleted by `poll_input`; `script()` in `lib.rs` |
| `cmui.log`, `cmui.trace` | what cmui did; trace lines while the file exists |
| `cmui.quit`, `cmui.peek`, `cmui.noremote`, `cmui.stack`, `cmui.shot`, `cmui.timing` | quit the game; log first bytes of hooked code; silence a client's questions; hang diagnosis; back-buffer shot from a side thread; frame timing |
| `drive.ps1` | writes `cmui.click`, waits for its marker, prints new log lines, turns BMP shots into PNG |
| `run.ps1`, `retest.ps1` | quit, copy fresh DLL, start the game; retest also installs everywhere and restores a save |
| `suite.ps1`, `check.ps1` | every screen by label, shots per resolution, `summary_*.txt` |
| `net_pair.ps1`, `net_join.ps1`, `net_tes.ps1`, `net_rejoin.ps1`, `net_days.ps1`, `net_res.ps1` | start a host and a client, join, step days, build folders at other resolutions |

Rules that matter: one game driver at a time; never close a game someone is playing; the user
clicks and Claude watches the game window only; a scripted check is a tool, not a gate. The
scripts tied to one machine (`suite`, `check`, `retest`, `net_*`, `install_all`) are git-ignored.

### Installing

`cargo build`, then `install_all.ps1` (games closed): copies `cmui.dll` and the settings window
(`cmui_config.bat/.ps1`) into every game folder it lists, hash-checks each ("= build" or
"DIFFERENT"), then runs `check_switches.py <folder>`, which sets every switch byte `FEATURES`
names to 1 in that folder's exe (only those bytes change). Hard rule: the same build in every
folder, the main game folder included. To add one switch to an existing exe use
`set_switch.py <folder> <index>`, never `make_exe.py` (it drops other patches).

---

## 2. Code map

`ui\src\lib.rs` is about 7000 lines and holds most of the game-facing code (hooks, drawing,
squad / tactics / profile / compare / search, the script runner, network glue). The other files
are features that grew big enough to leave. Find a function with Grep `fn <name>` in `ui\src`;
screen -> function tables are in [INDEX.md](INDEX.md) and [screens.md](screens.md).

| File | Lines | What it does | Main entry points | Switch |
|---|---|---|---|---|
| `lib.rs` | ~7000 | The DLL: `DllMain`, feature table, hook installation, palette and panel hooks, the text-box hook (all per-text restyling: bars, form boxes, crests, info strip, squad cells, tactics, match screen), the flip pass, `poll_input` and the test script runner, packet / screen hooks for network, `host_answer`, `ask_host`, `key`, `rd` / `readable` / `data_bytes` / `game_text`. | `DllMain`, `load_features`, `on`, `text_box_hooked`, `panel`, `flip_hooked`, `poll_input`, `script`, `packet`, `screen_sent`, `display_reset`, `send_click`, `client_loop`, `host_answer`, `ask_host`, `get_results` | most (0-28 mainly) |
| `remote.rs` | 370 | Network transport for a client: wanted keys, cached answers, packet send/receive, "Waiting for the host" state, chunks of host memory. `network_game()`. | `network_game`, `get`, `chunk`, `ask`, `send_now`, `send`, `send_answers`, `received`, `new_screen`, `hold_over`, `clicked`, `host_silent` | - |
| `prefetch.rs` | 440 | Host side: learns what a client asked per screen kind and sends those answers with the next screen of the kind; keeps `cmui.prefetch`. | `asked`, `valid_club`, `send` | - |
| `assistant.rs` | 1900 | Assistant pick (Best / Most fit / Rotation / Clear) and note, team report and its formation choice, scout's report text, staff opinions on a compare pair, remote requests for clients (`H` / `J` / `T` / `B`), `fill_team` / `fill` (also used by `lineups.rs`). | `flip`, `input`, `run_pending`, `client_request`, `remote_request`, `remote_note`, `remote_report`, `remote_scout`, `staff_views`, `report`, `report_text`, `coaching` | `assistant_pick`, `team_report` |
| `scout.rs` | 570 | Scout stars (Rec column), CM intrinsic score, margin, `seen_score` (a judge's view with his fixed error), Scout % column, best three of a club, ratings for clients. | `best_three`, `remote_rated`, `seen_score`, `restar`, `flip` | `scout_stars` (+ `scout_screen`) |
| `comp.rs` | 1350 | Competition screens: league table, stats rankings (medals, pills, bars), history screens (crests, colours, medals), table icons, flags and crests on fixture lists, title counts, form block for clients. | `text`, `panel`, `flip`, `titles`, `history_block`, `form_block`, `client_height`, `viewer_height`, `nation_flag`, `tie_name`, `tie_flags` | `stats_table`, `honours`, `table_icons` |
| `news.rs` | 1330 | News screen: rows, unread dot, icons, crests, story layout, link boxes and chips, cards (transfer, record, media, mood), qualifiers story, scrollbars; the cmui block's link section for clients. | `story`, `row`, `word_links`, `link_list`, `link_names`, `screen_block`, `block_tail`, `mark_button`, `clicked`, `scrollbars` | `news` |
| `history.rs` | 210 | A player's last injuries and bans (Injuries & Bans) and moves (Transfer tab); clients ask key `I`. | `rows`, `flip`, `note` | `injury_history`, `transfer_history` |
| `report.rs` | 250 | Icons on the Match Report lines (goal, cards, sub, injury) identified by the game's own words in `Data\events_*.cfg`. | `flip`, `kind_of`, `note_text` | `match_screen` |
| `weather.rs` | 140 | Weather icon before the game's weather text and in the match screen's WEATHER box. | `text`, `before_right` | `weather_icons` |
| `form.rs` | 160 | A club's last results through the game's fixture manager; W/D/L pills in the club header; used by the match card. | `results`, `mini`, `played`, `today_fixture` | `form_timeline` |
| `card.rs` | 240 | The card on "Proceed to the match ?": both clubs, places, last five, this season's meetings, expected XIs. | `note`, `flip` | `match_card` |
| `season.rs` | 550 | Season review card on 1 July from a daily snapshot (`cmui_season.txt`); text for clients by key `s`. | `flip`, `input`, `remote` | `season_review` |
| `natpool.rs` | 390 | National pool for a national team manager: every player of the country by group, + to the game's Short List; key `n` for clients. | `open`, `draw`, `input`, `remote` | `national_pool` |
| `lineups.rs` | 330 | Saved line-ups 1 / 2 / 3 and Fill subs on the squad screen (`cmui_lineups.txt`); key `l` for clients. | `run`, `draw`, `input`, `remote_summary` | `saved_lineups` |
| `coach.rs` | 1350 | Claude as coach: a button and page with four tabs (briefing, analysis, team report, Ask). WinHTTP request on its own thread; 1680 wide and up; off without `claude_api_key` in `cmui.ini`; key `k` carries a client's game data. | `button_shown`, `active`, `bar_item`, `flip`, `input`, `remote_data` | `coach` |
| `challenge\mod.rs` | 1620 | CM Challenge: managers' league of a network game (or the one manager's career in single player): prizes from the staff history, `cmchallenge.tsv`, spin, seasons. Built to be droppable (remove the folder and the lines `challenge.md` lists). | `remote` (key `E`), `prizes`, `moves`, `seasons`, `page`, `update`, `league_place`, `summary` | `challenge` |
| `challenge\draw.rs` | 1340 | The page in cmui's skin: tabs (Dashboard, Season, RAD, Stats, History, Rules), buttons under the menu bar, overlay. | `bar_item`, `button_shown`, `flip`, `input`, `overlay` | `challenge` |
| `challenge\track.rs` | 1090 | RAD challenges the game can judge itself (squad stats, transfers, staff); read every human turn; host can override. | `rule`, `update`, `tick` | `challenge` |
| `challenge\goals.rs` | 280 | Career goals: ten per manager, 30 of 48 read from the manager's history. | (internal to `mod.rs`) | `career_goals` |
| `challenge\*.tsv` | - | Data: `rules.tsv` (RAD and goal rules), `comps_*.tsv` (competition tables per database: original, cm25, restructured), `renames.tsv`. | - | - |

Outside `src`, the tools around it:

| File | What |
|---|---|
| `make_exe.py`, `make_patches.py`, `install.py`, `set_switch.py`, `check_switches.py` | build the patched exe, the patcher misc patches, install into a folder, set one switch byte, enforce all switch bytes |
| `cmui_config.ps1` / `.bat` | settings window: a box per switch, Hide CA/PA, Profile details, background opacity, Coach key and model |
| `logos.py`, `comp_logos.tsv`, `comp_names.py` | build `Data\cmui_crests.bin` (crests, competition logos, flags, badge packs) |
| `challenge_import.py`, `challenge_rad_review.py`, `challenge_sim.py` | CM Challenge data import, RAD review, points simulation |
| `drive.ps1`, `run.ps1`, `suite.ps1`, `check.ps1`, `retest.ps1`, `install_all.ps1`, `net_*.ps1`, `shot.py`, `peek.py`, `stack.py`, `sheet.py`, `showcase.py`, `timing_summary.py` | test and install harness (section 1) |
| `patches\` | generated misc patches (`.patch` + `.txt` per switch) |
| `Network client resolution fix.patch`, `Network client font fix.patch`, `Discipline Player network fix.patch` | stand-alone patcher patches for network clients (no cmui needed) |

---

## 3. How a frame flows through the hooks

Everything is a call-site or entry hook; nothing replaces game logic. Three drawing places and
one input place:

```
game draws a screen
  |-- palette init call (0x5e4560 in graphics init)  -> palette_init / apply_palette   (theme)
  |-- Display::draw_screen background blit (0x55f436) -> background                    (flat fill / picture)
  |-- gui_draw_panel (0x5e5df0)                      -> panel                          (flat panels, tabs)
  |-- gui_draw_text_box (0x5e6d80), once per text    -> text_box_hooked                (the "text hook")
  |        decides by label / position / switch which per-screen module restyles it:
  |        squad_member, tactics_row, match_text, ability_text, compare_text,
  |        news::row / story, comp::text, info_strip, bars, crest ...
  |        a module may draw its own thing, or note the text for the flip pass (note_text, note)
  `-- end of Display::draw_screen (0x55f5f9)         -> flip_hooked  (the "flip pass", once a frame)
           draws what needs the whole frame: CA/PA stars, overlays, strips, pills, cards
           and pages: comp::flip, news, assistant::flip, history::flip, report::flip,
           scout::flip, card::flip, season::flip, lineups::draw, natpool::draw,
           coach::flip, challenge::draw::flip, options_flip ...
           then runs the test script if poll_input is not running (live match),
           and on a client: remote::ask() sends what this frame missed

input:  Display poll (0x55f620) -> poll_input: reads cmui.click (script), cmui.quit, redraw on
        remote::ARRIVED, and offers clicks / keys to the modules (assistant::input,
        news_input, club_input, tactics_input, search_input, lineups::input,
        natpool::input, season::input, coach::input, challenge::draw::input ...).
```

Rules of thumb:

- **Text hook = per text, flip pass = per frame.** A module that needs to know several texts of
  a frame (a row, a header) notes them in the text hook and draws in the flip pass. A list redrawn
  alone has no flip pass (known case: scrolling a Match Report).
- Sizes scale with `sx` / `sy` (designed at 1680x1050). `small_screen()` is true under 1680
  wide: 800x600 and 1280x800 must fit, 1680x1050 stays as designed. `bigger_text` and
  `squad_table` act at 1280 and up.
- Game data is read with `rd::<T>(addr)`, `readable`, `data_bytes`, `game_text`, which on a
  network client fetch host memory. A hook that touches game data must use these, not raw
  pointers, or it breaks on a client. Pointers from outside (a client's packet) are checked
  (`valid_staff`, `mapped`, `prefetch::valid_club`).
- Calling a game function directly (`game::<fn>(ADDR)(...)`) is for single player and the host
  only (section 5).
- Network hooks: `packet` (entry of `Display::process_command_packet` 0x564080, takes opcode
  0x7c), `screen_sent` (0x7fca5c in `SCREEN_MANAGER::monitor`, host), `display_reset`,
  `send_click`, `client_loop` (`ScreenManager::run_network_client` 0x7fdc00),
  `client_rects_as_sent`, `contacting_box_lower`.

---

## 4. The switch table

38 switches, index = bit in `ENABLED` (a u64) = byte at stub+0x20+index of the exe. Test with
`on(CONST)`. Constants and names are in `FEATURES` / the `const` list in `lib.rs` (about line
2646). The long "Covers" text per switch is in [switches.md](switches.md).

| # | Name | Const | Main code |
|---|---|---|---|
| 0 | `theme` | `THEME_ON` | `palette_init`, `background`, `menu_bars` (lib.rs) |
| 1 | `tabs` | `TABS` | `panel` |
| 2 | `attribute_bars` | `ATTRIBUTE_BARS` | text hook (profile) |
| 3 | `crests` | `CRESTS_ON` | `crest_pixels`, `draw_crest`, `club_named`; needs `Data\cmui_crests.bin` |
| 4 | `profile` | `PROFILE` | `info_strip`, `ca_pa`, panels |
| 5 | `stats_table` | `STATS_TABLE` | `comp.rs` (also gates `comp::flip`) |
| 6 | `news` | `NEWS` | `news.rs` |
| 7 | `club_header` | `CLUB_HEADER` | `league_line`, `header_club` |
| 8 | `squad_list` | `SQUAD_LIST` | `squad_member`, `fit_cells` |
| 9 | `tactics_pitch` | `TACTICS_PITCH` | `tactics_row`, `tactics_columns`, `live_*` |
| 10 | `match_screen` | `MATCH_SCREEN` | `match_text`, `match_overlays`, `report.rs` |
| 11 | `compare_screen` | `COMPARE_SCREEN` | `compare_players`, `compare_overlay` |
| 12 | `menu_fixes` | `MENU_FIXES` | `menu_row`, arrows, `contacting_box_lower` |
| 13 | `status_bars` | `STATUS_BARS` | morale / condition bars |
| 14 | `search_ability` | `SEARCH_ABILITY` | `ability_text`, `get_results` |
| 15 | `scout_stars` | `SCOUT_STARS` | `scout.rs` |
| 16 | `assistant_pick` | `ASSISTANT_PICK` | `assistant.rs` |
| 17 | `team_report` | `TEAM_REPORT` | `assistant.rs` (needs 16) |
| 18 | `honours` | `HONOURS` | `comp.rs` (needs 5) |
| 19 | `export_button` | `EXPORT_BUTTON` | `export_button` in `DllMain` |
| 20 | `injury_history` | `INJURY_HISTORY` | `history.rs` |
| 21 | `transfer_history` | `TRANSFER_HISTORY` | `history.rs` |
| 22 | `compare_report` | `COMPARE_REPORT` | `assistant::staff_views` (needs 11) |
| 23 | `scout_screen` | `SCOUT_SCREEN` | `assistant.rs` / `scout.rs` |
| 24 | `bigger_text` | `BIGGER_TEXT` | `font_up`, `roomier_font` |
| 25 | `squad_table` | `SQUAD_TABLE` | squad Position(s) table |
| 26 | `table_icons` | `TABLE_ICONS` | `comp::text` (needs 5) |
| 27 | `background_show` | `BACKGROUND_SHOW` | `keep_picture`, `let_picture_through` |
| 28 | `modern_buttons` | `MODERN_BUTTONS` | button / dropdown colours |
| 29 | `challenge` | `CHALLENGE` | `challenge\` |
| 30 | `career_goals` | `CAREER_GOALS` | `challenge\goals.rs` (needs 29) |
| 31 | `coach` | `COACH` | `coach.rs` |
| 32 | `weather_icons` | `WEATHER_ICONS` | `weather.rs` |
| 33 | `form_timeline` | `FORM_TIMELINE` | `form.rs`, header in lib.rs (off in network games) |
| 34 | `match_card` | `MATCH_CARD` | `card.rs` |
| 35 | `season_review` | `SEASON_REVIEW` | `season.rs` |
| 36 | `national_pool` | `NATIONAL_POOL` | `natpool.rs` |
| 37 | `saved_lineups` | `SAVED_LINEUPS` | `lineups.rs` |
| 38 | `flat_buttons` | `FLAT_BUTTONS` | `lib.rs` `panel` (+ `rounded`, `FLAT_CELL`), `news.rs` `flat_tabs` |
| 39 | `pl_report` | `PL_REPORT` | `finreport.rs` (+ `accountants_notes.tsv`) |

Club logos and player pictures from footylogos.com: `footy.py` (`index`, `logos`, `faces`) matches the
site's catalogue to a game folder's database and downloads only what matches (a polite 0.15 s per
file); the pictures never go into the repo. Logos feed `logos.py pack` (`badge_match.tsv`), faces
feed `shelved\player_pictures\faces.py` (`Data\cmui_faces.bin`, drawn by `draw_face` in `lib.rs`).
`test_footy.py` checks its pure parts.

Settings that are not switches live in the same ini (`setting` / `set_setting`): `hide_ca_pa`,
`profile_details`, `background_opacity`, `claude_api_key` and `claude_model` (never logged, never
copied into packages, never put in docs).

An exe made before a switch existed has its byte at 0 (off) until `set_switch.py` sets it; the
install script's `check_switches.py` does that for every folder.

---

## 5. Network model

Short version: **the host runs the game and builds every client's screens; the client is a
terminal.** cmui on a client has no game world. Detail and measurements: [network.md](network.md);
the original idea: `..\NETWORK_PLAN.md`.

Rules that follow:

1. On a client (`remote::CLIENT`, set while `client_loop` runs) the reads `rd`, `readable`,
   `data_bytes`, `game_text` ask the host for 256-byte chunks of its memory (key `R`). They
   return zeros until the answer comes; the screen redraws when it arrives (`remote::ARRIVED`).
2. **Never call a game function on a client.** It runs on the client's own empty world and
   crashes (seen at 0x5abf36, night 7). Whatever needs the game is worked out on the host and
   comes back as bytes through a key.
3. A feature that cmui draws from game memory needs a route to the client: a chunk read (free,
   but slow and flickery), a key (`ask_host(&key(b'X', &[args]))`), or a block sent with the
   screen.
4. An action the client starts on the host (Assistant pick, saved line-up, pool +) is a request
   key that the host keeps and carries out when its screen manager serves that client's channel:
   section "H path" below.
5. `remote::network_game()` is true on a client or when the session's Network object exists
   (host). A feature that must be off in network games tests it (today only the form strip,
   `lib.rs` ~4395; saved line-ups work in network games since night 8).

### Transport

Packets use the game's own connection (`Network::send_packet` 0x77ac30), opcode `0x7c`.
Client: `remote::get(key)` marks a key wanted and returns the cached answer (`Option<Option<..>>`:
not asked yet / asked, none / answer); `remote::ask()` at the end of `flip_hooked` sends the
wanted keys as `[0x7c,'q']`. Host: `packet` -> `remote::received` -> `host_answer(channel, key)`
replies `[0x7c,'a']`. `ask_host(key)` is the one entry for module code: on the host it calls
`host_answer(0, key)` directly, on a client it goes through `remote::get`. `key(kind, &[u32])`
builds a key: letter + little-endian u32 args. Timing constants (`FRESH` 2 s, `UNANSWERED` 15 s,
`SETTLED` / `REFRESH` 10 s, `HOLD_MAX` 1 s) are in `remote.rs` and network.md.

### Host keys (`host_answer` in `lib.rs`, about line 6580)

| Key | Args | Returns | Asked by |
|---|---|---|---|
| `R` | address | 256-byte memory chunk (mapped memory only) | `rd` / `data_bytes` (all modules); `prefetch.rs` sends them unasked |
| `P` | n | screen parameter n of the client's channel; only while that channel is served (`served_node`), else the game stops | `screen_param` (lib.rs ~5967) |
| `C` | name | club pointer | club lookup by name (lib.rs, `club_named`) |
| `N` | staff | national team? (1 byte) | lib.rs ~3981 |
| `F` | staff | condition | lib.rs ~4000 |
| `S` | staff, field | season stat | `season_stat` callers (lib.rs ~4049) |
| `W` / `V` | staff | wage / value text | `money_texts` (info strip, lib.rs ~3319) |
| `A` | staff | 31 attributes | lib.rs ~5150 |
| `O` | staff | positions text | lib.rs ~5526 |
| `M` | names | compare pair (two staff pointers) | compare screen |
| `G` | row name | staff by the row's name | search screens (`staff_by_row_name`) |
| `Y` | link text | a story's crest from its links | news (`story_link_crest`) |
| `L` | - | link list of the channel's story | `news::link_list` (old route) |
| `X` | cell | sets `news::PENDING_CELL`, the click that follows opens the link | `news::clicked` (lib.rs ~2922) |
| `Z` | kind, object | a link's name and crest key | `news.rs` ~1010 |
| `K` | club | sets `PENDING_CLUB`, the click that follows opens the club | news chips / club rows (lib.rs ~2923) |
| `Q` | - | sets `news::PENDING_MARK` (Mark all read) | lib.rs ~2908 |
| `p` | height | no answer; sent with every click; marks the host present and tells the client's screen height | `remote::clicked`, `comp::client_height` |
| `H` | mode, number, players not available, ... | empty; stores a request in `assistant::REMOTE_PENDING` | `assistant::client_request`; also `lineups.rs` (codes 100 Fill subs, 110+k load, 120+k save, 130+k clear) |
| `J` | n | the Assistant's note | `assistant.rs` ~367 |
| `T` | n, count, players | team report text; only while the channel is served | `assistant::report` (~493) |
| `B` | n, club | the scout's report on a club | `assistant.rs` ~549 |
| `I` | staff | a player's injuries, bans, moves | `history.rs` ~191 |
| `D` | staff, staff | staff opinions on a compare pair | lib.rs ~5429 |
| `U` | club | a club's three best players (staff pointers) | lib.rs ~2029 (`scout::best_three`) |
| `E` | n, year, a, b | CM Challenge page text, read only | `challenge\draw.rs` ~175 |
| `k` | n | the coach's game data for the client's manager | `coach.rs` ~799 |
| `l` | club | the three saved line-ups' shapes | `lineups.rs` ~211 |
| `r` | screen number | scout ratings of the client's search screen | `scout.rs` ~405 |
| `n` | team, number, adds... | a national team's pool as bytes; adds players to the Short List | `natpool.rs` ~75 |
| `s` | season (0 any) | the season review card's texts | `season.rs` ~350 |
| `m` | - | the match day card's bytes, worked out on the host; only while the channel is served | `card.rs` (`card::remote`) |
| `b` | (not a key) | packet `[0x7c,'b',block]`, host to client, unasked | `remote::BLOCK` |

Keys whose staff pointer comes from a client are checked with `valid_staff` (inside the staff
table, 0x6e-byte records, mapped). Action keys (`X Q H K J L Y Z T`) are never learned by the
prefetch.

### The "H" path (actions on a client's selection)

Client: module builds a request (`key(b'H', &words)`) and `remote::send_now`s it, then sends the
click. Host: `host_answer('H')` stores `(channel, code, unavailable, number)` in
`assistant::REMOTE_PENDING`. The host's squad or tactics input handler later calls
`assistant::run_pending(tactics)`, which runs only when the screen manager is serving that
client's channel and takes the request for it; it works on the client's club, selection and slot
labels (`fill_team` takes them as arguments for this reason). The note comes back by `J`. Saved
line-ups (codes 100 Fill subs, 110+k load, 120+k save, 130+k clear) and Fill subs use the same
route; the national pool's + goes by key `n`, which adds the players to the client's Short List.

### Screen blocks and prefetch (host -> client, with the screen)

- **cmui block `'b'`**: `screen_sent` hooks the end of the host's send of a client screen
  (`Display::log_screen_sent` 0x5622a0) and sends `[0x7c,'b'] + news::screen_block(display) +
  comp::history_block() (+ comp::form_block())`, 38,000 bytes at most. Sections: links of a news
  story (kind, object, cell, text, name, crest key), history title counts per winner row, league
  table forms. The client keeps it in `remote::BLOCK` and drops it in `display_reset`.
- **Prefetch**: right before the block, `prefetch::send` sends the answers the client asked for
  the last times it was on that kind of screen (squad, tactics, live tactics, profile, search),
  as `'a'` packets, cap 160,000 bytes a screen. The host learns from every question
  (`prefetch::asked`) per channel and screen kind (the input handler of the channel's screen node);
  `cmui.prefetch` in the game folder keeps the portable templates between sessions.
  The client holds its first questions after a new screen until the block is in (1 s at most).
- **Client-only state**: `CLIENT_SCREEN` (the node it draws), `comp::viewer_height`, `ARRIVED`.

### What works on a client

See "What works where" at the end of network.md (single player, host, client per switch; current
since night 8, 2026-11-08). Saved line-ups and Fill subs (37) work for host and client; only the
form strip (33) is off in network games. Plan of night 8: `night8\PLAN.md` (local only).

### Clicks on a client

An ordinary widget click goes through `send_click` (wraps `Display::send_widget_click` 0x568560,
clears 0xb67a34 / 0xb67a38 so a held button does not fire on the host's next screen). Clicks on
things cmui draws first send a key (`X`, `K`, `Q`, `H`), then the click. A click's `p` key tells
the host the client is there; no answer in 1.5 s shows "Waiting for the host ...".

### CM Network (cmnet)

Separate DLL, `net\src\lib.rs`. Game side facts: plain Winsock TCP, port 6667 (+6668 for the
compression handshake), UDP broadcast for LAN discovery, numeric IPs in the game's own box
(cmnet resolves names). cmnet also sends a packet's four length bytes with the packet and waits
with `select` on a busy socket (the night 4 match-day hang). Package: `net\package\`.
Nothing in cmui depends on it.

---

## 6. Where things are written down

Kind: **current** = kept up to date, use it; **log** = a night's or round's record, true on its
date, not a spec; **plan** = intent, may be done or dropped; **gitignored** = not in the public
repository (`.gitignore`: `ui/night*/`, `ui/check/`, `ui/round/`, `ui/proof/`, `ui/shelved/`,
`ui/save_check/`, `ui/NETWORK*_PLAN.md`, the machine-bound scripts).

| File | What it is | Status |
|---|---|---|
| `ui\docs\ARCHITECTURE.md` | this file | current (start here) |
| `ui\docs\INDEX.md` | screen -> doc -> main functions; plans list | current |
| `ui\docs\screens.md` | per screen: what is drawn, functions, hooks, how to check | current (reference) |
| `ui\docs\switches.md` | the switch table in full, exe bytes, patches, safeguard, dsound stand-in | current (reference) |
| `ui\docs\network.md` | network model, keys, block, prefetch, measurements, what works where | current (key table and "What works where" updated night 8) |
| `ui\docs\testing.md` | test files, script commands, harness scripts, saves, night roles, regression checklist | current |
| `ui\docs\challenge.md` | CM Challenge design, files, how to drop it | current |
| `ui\FIXES.md` | register of every fix and open item: symptom, cause, code, check, status | current; add a row with each fix |
| `ui\FEATURES.md` | features agreed or just built, with the user's test notes of 2026-11-07 | current working list; move items out when settled |
| `ui\README.md` | long history in dated rounds: hooks in depth, installing, patcher, resolutions, flags, badges, network | background; the "How it hooks" and "Installing" parts hold up, round sections are history |
| `ui\AGENTS.md` | rules for AI assistants working in the public repository | current |
| `ui\assistant_pick.md` | Assistant design and facts | current for design; feature built |
| `ui\scout_stars.md` | scout stars design and measurements | current; built 2026-10-29 |
| `ui\helper_search.md`, `helper_squad.md`, `helper_match.md`, `helper_live.md`, `helper_stats.md` | static findings from the exe: addresses, record offsets | reference, stable |
| `ui\NETWORK_PLAN.md` | the cmui block idea for clients, steps 1-6 | plan; steps 1-3 and 5 done; 4 and 6 superseded by the prefetch (`prefetch.rs`); gitignored |
| `ui\NETWORK_NIGHT_PLAN.md` | the 2026-10-30 network night (roles, checklist) | log; gitignored |
| `ui\night2\REPORT.md` | night of 2026-10-30: network screens, traffic baseline | log (Dutch and English mixed) |
| `ui\night3\` | prefetch runs C, D, E (data files only, no report) | log; findings are written into network.md |
| `ui\night4\PLAN.md`, `REPORT.md` | 2026-10-31: all new features in single player and network, match-day hang | log (report in Dutch; left as it is, historical) |
| `ui\night5\PLAN.md`, `REPORT.md` | 2026-11-01/02: the user's 19 live-test points | log; parts in Dutch (left as it is, historical); may contain real names (gitignored) |
| `ui\night6\PLAN.md`, `BUGS.md`, `REPORT.md`, `TESTER_BRIEF.md` | 2026-11-04/05 single player crawl: about 135 findings, fixed in D91FCA | log; open items ranked in REPORT, mirrored in FIXES "Open" |
| `ui\night7\PLAN.md`, `REPORT.md`, `RAD_REVIEW.md` | 2026-11-06/07: clean test setup, network step by step; RAD review and balance | log; step 7 of the plan is replaced by night 8 |
| `ui\night8\PLAN.md` | 2026-11-07/08: the new features in network games, the rule that decides how | plan; "Status" section is empty, no report yet |
| `ui\check\`, `ui\proof\`, `ui\save_check\`, `ui\shelved\` | shots, proof sets, save checks, parked code (player pictures) | generated / parked; gitignored |
| `ui\patches\` | generated patcher patches | generated by `make_patches.py` |
| `cmui_package\LICENCES.md`, `TEST.md` | what may be shared; test notes for the package | current |
| `net\package\README CM Network.txt` | player-facing CM Network text | current |

### Known doc inconsistencies (still open; the 2026-11-07 list was fixed on 2026-11-08)

1. **Night 8** has a plan but no status or report in `night8\`; `FIXES.md` and `network.md` hold
   what it did. Night 7's report is the last complete night record.
2. **`lineups.rs` header comment** still says "a network client has no strip"; the code and
   `network.md` say clients have it (key `l`, `H` codes 100+). Fix the comment at the next code
   change (docs-only job: not touched).
3. **`AGENTS.md`** names `net\` and `cmui_package\` paths that this document treats as outside
   `ui\`; harmless, left as it is.
4. The night 4 and night 5 reports are partly Dutch (historical logs, left as they are).

---

## 7. How to add a feature (checklist)

1. **Design in `FEATURES.md`** (what, open questions, where it goes). Check `FIXES.md` and
   `screens.md` for an existing piece of the same screen.
2. **Switch.** Append a name to `FEATURES` in `lib.rs`, add `const NAME: usize = N;`, then the
   other lists: `FEATURES = N+1` in `make_exe.py`, the name in `make_patches.py`, `install.py`
   and `cmui_config.ps1`. `cargo test` (`every_switch_list_agrees`) fails if one is missing.
   Add a row to [switches.md](switches.md) (and section 4 above). Never rebuild an exe with
   `make_exe.py` for it: `set_switch.py <folder> <index>`, or let `install_all.ps1` do it.
3. **Code.** New file `src\<name>.rs` with a `//!` header (what, which switch, which user wish),
   `mod name;` in `lib.rs`, calls from the text hook or flip pass and `poll_input`, each behind
   `on(NAME)`. Check original bytes before any new hook site; add the site to `install.py`'s list.
4. **Game data** through `rd` / `readable` / `data_bytes` / `game_text`; no raw pointers from
   packets; no game function calls that a client could reach.
5. **Network** (a feature is not done until it works in all three):
   - draws from game data: add a host key to `host_answer` (a letter not yet used; free letters
     are listed by reading the match arms), the client side with `ask_host(&key(b'x', &[..]))`
     and a redraw on `remote::ARRIVED`; keys that change game state or need a game call go the
     `H` way (request stored, `run_pending` on the host); add the key to the table in section 5
     and in `network.md`;
   - state kept per manager on the host (like `cmui_lineups.txt`, `cmui_season.txt`): key it by
     club index so clients' clubs have their own lines;
   - if it must be off in network games, test `remote::network_game()` and say so in
     `network.md`'s "What works where".
6. **Check** single player, then host, then client (one game driver at a time, the user's game
   never closed). Resolutions 800x600 and 1280x800 must fit (`small_screen()`); 1680x1050 stays
   as designed. Run `cargo test`. `suite.ps1` gets a part for a new screen.
7. **Install**: `install_all.ps1` (hash check, switch bytes), same build in every folder.
8. **Write down**: a `FIXES.md` row for every fix or built wish (symptom, cause, where, check,
   status), `switches.md`, `screens.md` rows for the screen, `network.md` for new keys, this
   file if a source file or key was added. Keep the public repository free of names, paths of
   people, keys (`cmui.ini` is never copied), and logos.
9. **Package** (`cmui_package\`): refresh only on the user's word; the zip scripts refuse files
   with an API key.
