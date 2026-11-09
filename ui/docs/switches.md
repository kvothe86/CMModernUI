# Feature switches, exe patches, dsound stand-in

## The 38 switches (indices 0-37) (`FEATURES` in `src\lib.rs`, index = bit = exe switch byte)

`ENABLED` is a u64 since 2026-11-07 (switches 32 and up); an exe made before has bytes 32-35 = 0
(free space after the stub): off until `set_switch.py <folder> <index>` sets them.

| # | Name | Const | Covers |
|---|---|---|---|
| 0 | `theme` | `THEME_ON` | palette, flat background, menu bar pictures |
| 1 | `tabs` | `TABS` | flat tab buttons, active tab marked |
| 2 | `attribute_bars` | `ATTRIBUTE_BARS` | attribute bars |
| 3 | `crests` | `CRESTS_ON` | crests, competition logos, flags (needs `Data\cmui_crests.bin`) |
| 4 | `profile` | `PROFILE` | info strip, three panels, CA/PA |
| 5 | `stats_table` | `STATS_TABLE` | stats table pills; also gates `comp::flip` and competition text |
| 6 | `news` | `NEWS` | news restyle, links, Mark all read |
| 7 | `club_header` | `CLUB_HEADER` | club header band |
| 8 | `squad_list` | `SQUAD_LIST` | squad fit, form, headers |
| 9 | `tactics_pitch` | `TACTICS_PITCH` | pitch, list columns, live data |
| 10 | `match_screen` | `MATCH_SCREEN` | match overview |
| 11 | `compare_screen` | `COMPARE_SCREEN` | compare |
| 12 | `menu_fixes` | `MENU_FIXES` | menu bar icons, dark dropdowns, arrows, network "Contacting Server" box |
| 13 | `status_bars` | `STATUS_BARS` | morale / condition bars |
| 14 | `search_ability` | `SEARCH_ABILITY` | Ability view in search and squad |
| 15 | `scout_stars` | `SCOUT_STARS` | scout Rec stars; the CM intrinsic % after a rated player's name on the "... Search" screen, as the search's scout sees him (his error in it, no margin shown) |
| 16 | `assistant_pick` | `ASSISTANT_PICK` | Assistant button |
| 17 | `team_report` | `TEAM_REPORT` | Team report item in that button's menu (squad screen; needs `assistant_pick`); a % pill under each slot, the slot's intrinsic score as the assistant sees it |
| 18 | `honours` | `HONOURS` | cups and counts on a club's Honours list (needs `stats_table`) |
| 19 | `export_button` | `EXPORT_BUTTON` | Print called Export, clear of the crest, always to a file; the file also as an HTML dashboard (`export.rs`), opened in the browser in single player |
| 20 | `injury_history` | `INJURY_HISTORY` | last injuries and bans on Injuries & Bans |
| 21 | `transfer_history` | `TRANSFER_HISTORY` | moves on the Transfer tab |
| 22 | `compare_report` | `COMPARE_REPORT` | staff lines on Compare Players (needs `compare_screen`) |
| 23 | `scout_screen` | `SCOUT_SCREEN` | Full scout report chip and screen |
| 24 | `bigger_text` | `BIGGER_TEXT` | larger fonts where a text fits its box, 1280 wide or more (`font_up`, `roomier_font`) |
| 25 | `squad_table` | `SQUAD_TABLE` | squad's Position(s) view as a table in two wide columns, 1280 wide or more |
| 26 | `table_icons` | `TABLE_ICONS` | league table's C, P, Pl, R boxes as a cup and arrows; Q as a green tick, Pl on a continental table (nation column) as a blue PO pill (`comp::text`; needs `stats_table`) |
| 27 | `background_show` | `BACKGROUND_SHOW` | Background Changes on: panel-coloured pixels let the picture through at the flip (`keep_picture`, `let_picture_through`); how much in cmui.ini |
| 28 | `modern_buttons` | `MODERN_BUTTONS` | grey buttons, dropdowns and list cells in steel blue, column heads flat, no box around an empty stats cell |
| 29 | `challenge` | `CHALLENGE` | CM Challenge button and page in a network game ([challenge.md](challenge.md)) |
| 30 | `career_goals` | `CAREER_GOALS` | the Goals tab of CM Challenge: ten career goals a manager (needs `challenge`; the host's switch decides, a client shows what the host sends). An exe from before has the byte 0: `set_switch.py <folder> 30` |
| 31 | `coach` | `COACH` | Claude as coach (`src\coach.rs`): a Coach button under the menu bar (with CM Challenge's in Game Options' place; Game Options' widget moves under them, `options_flip`) opens a page right of the bar with Pre-match briefing, Post-match analysis, Team report (expanded) and Ask the coach (typed questions, a conversation). **Only at 1680 wide and up** (`small_screen()` false), single player or network host (off on a client), and **off when cmui.ini has no `claude_api_key=`** (read once at start; `claude_model=` overrides `claude-haiku-5-5`). One HTTPS request per tab per game day (WinHTTP, `anthropic-version: 2026-10-21`), on its own thread; Ask again repeats it. Sent along: the assistant's report and every player (age, positions, stars, condition, Av R, value, wage, contract), the next fixture as last seen on the squad / tactics screen with the scout's report on the opponent, our last match screen's texts (kept from the frames drawn) and the texts of the screen behind. Keys of the Ask tab go through a subclass of the game window (WM_KEYDOWN / WM_CHAR). The key never goes in the log; `cmui_package\make_zip.py` and `make_cm25_zip.py` drop the line and refuse any file with `sk-ant-`. An exe from before has the byte 0: `set_switch.py <folder> 31` |
| 32 | `weather_icons` | `WEATHER_ICONS` | a drawn icon (sun, rain, downpour, snow, wind lines) before the game's weather text, "Weather - Gusty, Wet, 10°C" or "Breezy, 7°C" (no rain word: dry), and in the match screen's WEATHER box (`weather.rs`) |
| 33 | `form_timeline` | `FORM_TIMELINE` | the club header's last five as small W / D / L pills under the league line (`form::mini`; results from FIXTURE_MANAGER::find_fixtures, `form.rs`) |
| 34 | `match_card` | `MATCH_CARD` | on "Proceed to the match ?": both clubs, league place, last five, this season's meetings (`card.rs`) |
| 35 | `season_review` | `SEASON_REVIEW` | on the first day of a new season a card over the screen until Back: place, record, goals, biggest win, points, stars, signings, last six (`season.rs`; a daily snapshot per human club in `cmui_season.txt`); any day from the squad screen's Assistant menu "Season review"; "Save image" writes the card as a PNG to `Screenshots\` (night 8); network: the host works the card out (key 's') |
| 36 | `national_pool` | `NATIONAL_POOL` | a "National pool" item in a national team's Assistant menu (squad screen, single player / host): every player whose first nationality is the country, by group (GK / DEF / MID / ATT), best 100 a group by the assistant's % (`scout::seen_score`, his error in it); + per row and "Add top 10 here" put players on the game's Short List (SCOUT_MANAGER `*0xb74448`, add_to_shortlist 0x7f75f0, is_player_shortlisted 0x7f7490); filters, pages, Back (`natpool.rs`) |
| 37 | `saved_lineups` | `SAVED_LINEUPS` | squad screen, single player / host: a strip right of the list's title (the assistant's note keeps the left): line-ups 1 / 2 / 3 (eleven + bench by staff id and the shape, `cmui_lineups.txt`), a floppy saves (a full slot asks with a second click), a click loads (a starter gone, injured, banned or under 75 % condition is replaced for his slot by the assistant's pick, `assistant::fill_team`), right click clears; Fill subs fills only the empty bench places up to the match's allowed number; In / Ast tags by the names (`lineups.rs`) |
| 38 | `flat_buttons` | `FLAT_BUTTONS` | (needs `modern_buttons`; user 2026-11-09, styles B + D of four mockups) the game's grey buttons drawn by cmui flat with 6 px corners, 2 px in at each side (gaps), no bevel, lighter when pressed (`panel`, `rounded`); the tab rows one dark bar with soft corners and the open tab a gold block with dark text (`news::flat_tabs`; a row with a disabled tab keeps the old look); a list's clickable 0x39e8 cells (positions, dates) without a box, the row's background, their text light blue (`FLAT_CELL`) |
| 39 | `pl_report` | `PL_REPORT` | (needs `crests`; user 2026-11-09) a "P&L report" button right of the View dropdown on the money views of a club's Finances & Info tab (Finances, Income, Expenditure): a full profit and loss account on a sheet of paper over the screen, signed by the made-up firm "Accountants & Co" (a logo drawn in code), notes picked by how the club is doing from `accountants_notes.tsv`; Back and Save image (a PNG in `Screenshots\`). The figures are the club's finance record (`finreport.rs`), money shown the game's way (its currency and rate, read off its own formatter). Single player and host only |

Check in code with `on(CONST)`. `load_features` (called from `DllMain`) turns a feature off where
the exe's byte is 0 (`exe_switches`), then reads `cmui.ini`: `name=0` off, missing names written
as `name=1`. So **the ini can only switch off what the exe allows**; read once at start. `theme=0`
keeps the game's palette and picture. Settings that are not switches in the same file:
`hide_ca_pa`, `profile_details`, `background_opacity` (0-100, % of the picture behind the panels, default 30; read every half second, in force from the next full frame) (`setting` / `set_setting`).

**`cmui_config.bat`** (with `cmui_config.ps1`, copied next to the DLL by `install_all.ps1`): a window with a box per switch, Hide CA/PA, Profile details, the opacity slider, the coach's Claude API key (a password box: empty keeps the saved key, only its last four shown; Remove drops it) and model (Haiku 5.5 / Sonnet 5.5, or what the ini has), saved in the `cmui.ini` beside it; lines it does not know stay. A new switch goes in its `$switches` list too. `-Ini <file> -Test` unticks Tabs, sets a test key and Sonnet, saves to that file and writes `<file>.png` of the window.

## The exe's switch bytes

`make_exe.py` writes `cm0102_ui.exe` from `cm0102.exe`: a loader stub (entry point -> push
"cmui.dll", `LoadLibraryA`, jmp 0x547a13) in the `.nick` section (or a `.cmui` section), the mark
`CMUI` at stub+0x1c and one byte per feature at stub+0x20 (`FEATURES = 38`, all 1).
`make_patches.py "<cm0102.exe>"` writes the same as CM0102Patcher misc patches into `patches\`
(`cmui Loader` + `cmui - <Title>` per switch + `cmui - Everything`); `--test exe out [a,b]` applies
them like the patcher. `install.py "<game>" [--check|--off a,b|--crests|--badges|--uninstall|--list]`
checks the hook sites, writes the exe, DLL and ini.

**Safeguard (2026-11-07, after Career Goals was off in three folders for days):**
`install_all.ps1` runs `check_switches.py <folder>` after each DLL copy: every switch byte that
`FEATURES` in `lib.rs` names is set to 1 in that folder's exe (only those bytes change, checked),
printing "= all N" or "TURNED ON <folder>: <switches>", and naming any `cmui.ini` switch-offs
(those stay: the player's choice). The test `every_switch_list_agrees` fails the build's tests
when `make_exe.py`'s count or a name in `make_patches.py`, `install.py` or `cmui_config.ps1`
is missing. So a new switch needs only its lists right; the install turns it on everywhere.

**Add a switch to an existing exe: `python set_switch.py "<folder>" <index> [0|1]`. Never rebuild
an exe with `make_exe.py` to add one** (it drops patches made to that exe). Adding a feature means
touching five places: `FEATURES` + const in `lib.rs`, `FEATURES` in `make_exe.py` (count),
`make_patches.py`, `install.py` and `cmui_config.ps1` lists, then `set_switch.py` on every game folder, then
`install_all.ps1` (the hard rule: main `E:\CM0102` too). `set_switch.py` keeps its first backup
per index: an older `cm0102_ui_before_switch<index>.exe` in the folder makes its "no other byte
changed" check fail after the byte is written (2026-10-30, F Restructured, a backup of
2026-10-27); run it again to see "already 1". The backups pile up (59 of them, 524 MB, went to
the Recycle Bin on the user's word, 2026-11-07): remove them once the switch is seen working.

## dsound stand-in (`E:\CM0102\dsfix`)

A C-Media sound driver DLL (HsSrv.dll) wants dsound ordinal 12, which cmfix's `dsound.dll` (only
`DirectSoundCreate`) lacks ("ordinal 12 not found in HsSrv.dll"). `dsfix` is a Rust DLL
(`dsfix\src\lib.rs`) exporting all of dsound: `DirectSoundCreate` goes to `cmfix_dsound.dll`
(cmfix's, renamed), the rest to Windows' own dsound loaded by full path. `dsfix\install.ps1`
renames cmfix's file (only if it contains "cmfix"), copies the stand-in as `dsound.dll` into the
same seven game folders as `install_all.ps1`, hash-checks each. FIXES "Network client".

## Inconsistencies noticed

- `..\README.md`'s switch byte count and switch list now point here; its "15 hook sites" /
  "all 16 not detected" are from the patcher test of 2026-10-27 (count of 2026-10-27) and were not re-run.
  `FEATURES` has 40 entries (indices 0-39) in `lib.rs`, `make_exe.py`, `make_patches.py`,
  `install.py` and `cmui_config.ps1` (the window: three columns of thirteen); the test
  `every_switch_list_agrees` keeps them in step.
