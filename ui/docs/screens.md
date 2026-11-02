# Screens: what cmui draws, where, how to check

All in `ui\src\lib.rs` unless a file is named. Drawing happens in three places: the text-box hook
(`text_box_hooked`, game 0x5e6d80, per text), the panel hook (`panel`, 0x5e5df0), and the flip pass
(`flip_hooked`, 0x5e2eb0 via 0x55f5f9, once a frame: overlays, strips, panels). Input: `poll_input`
(0x55f620). Checks: `suite.ps1 <game> <prefix> <manager> <club> <p1> <p2> [part]` parts are
menus, squad, profile, compare, news, search, match (see [testing.md](testing.md)); fixes are in
`..\FIXES.md`. Sizes scale with `sx` / `sy` (given at 1680x1050).

## Menu bar and theme

| Item | Functions | Hooks |
|---|---|---|
| Palette (30 slots), tab bar colour | `palette_init`, `apply_palette`, `THEME` | call 0x5e4560 in graphics init 0x5e2c49; tab flag 0xb0be34 / colour 0xb0be36 |
| Flat background | `background` | blit in `Display::draw_screen` 0x55f436 |
| Flat panels, active tabs | `panel` (+ `PANEL_ORIG` chain, cmfix hooks it first) | 0x5e5df0 |
| Menu bar icons / items | `sidebar_item`, `sidebar_icon`, `draw_icon`, `fit_font` | text hook |
| Menu bar picture | `menu_bars` (writes `Data\gcui.mbr` etc., swaps the name in memory) | exe names game.mbr / match.mbr / g800.mbr / m800.mbr |
| Dropdown rows | `menu_row` (+ `news::MENU_OPEN`) | text hook, structure-detected |
| Chevrons / arrows | `arrow`, `glyph` (0x5e3ce0 in Guio::draw 0x5ee590), `button_arrows` (bottom buttons, found by pixels, flip pass) | 5 calls in 0x5ee590 |
| Scrollbars | `news::scrollbars` | flip |

Check: `suite.ps1 ... menus` (hover News / Transfers, shot `_menu`). Fixes: FIXES "Typing / redraws".

## Squad

| Item | Functions |
|---|---|
| Club header: league logo, crest, "1st in D1" line | `league_line`, `league_position`, `header_club`, `draw_crest`, `crest_rows`, `club_named`, `note_title` |
| Positions gold, fit % (green/gold/red) | `is_position`, `fit_colour`, `fit_cells`, `condition` (0x62c590), `squad_member` |
| View "Position(s)" (the game's Traditional, the title says Position(s)): two wide columns with a table | `squad_two_columns`, `squad_table_row`, `squad_table_heads`, `TABLE_ENDS`, `POSITIONS_VIEW` |
| View "Ability" (Age / CA / PA stars) | `selection_item` (call 0x456f7f), `squad_ability_text`, `potential_span`, `squad_sort` (qsort call 0x484865); input `club_input` (0x4671f0, bit 0x2000 of screen param 7) |
| Form boxes | `form_colour`, text hook |
| Assistant button | see [Assistant](#assistant) |

Check: `suite.ps1 ... squad` (Selection, view menu, Traditional, Ability shots). Spec: `..\helper_squad.md`.

## Tactics and live tactics

| Item | Functions |
|---|---|
| Pitch: grass, shirts, names in pills, fit pill | `shirt`, `keeper_at`, `keeper_kit`, `pitch_member`, `circle`, text hook; panel colours 0x0200 / 0x01c0 / 0x0400 |
| List columns Av R / Tck / Drb, label row | `tactics_row`, `tactics_columns`, `tactics_labels`, `label_row_inside`, `season_stat` (0x7c25b0) |
| Live: engine rows E+0x4796+k*0x1be | `live_engine`, `live_player`, `live_condition`, `live_position`, `live_off_pitch` |
| In-match bench / sent-off rows | `tactics_row` live branch (see FIXES Match screen) |
| Tactics input | `tactics_input` (0x89f710, runs `assistant::run_pending(true)`) |

Check: `suite.ps1 ... squad` (the `click Tactics` step) and the `match` part (`_live_tactics`).
Specs: `..\helper_live.md`, `..\helper_stats.md`.

## Assistant

`assistant.rs` (switch `assistant_pick`): `flip` draws the button and menu, `input` handles clicks
from `poll_input`, `run_pending(tactics)` runs the pick in the screen's input handler, `pick_for`
does the choosing, `note_text` / `note_member` show the note. Network: `client_request`,
`remote_request` ('H'), `remote_note` ('J'). Spec: `..\assistant_pick.md`. Check: click Assistant on
squad and tactics, client too (night checklist 2, 3, 11).

**Team report** (switch `team_report`): a fifth menu item on the squad screen only (`menu_len`,
`SQUAD_HERE`). `open_report` makes it, `report_for` picks both elevens with the pick's own rating
(`candidates`, `eleven`, `Eye`), `scout::seen_score` / `halves` give the stars, `coach_text` the
assistant's text (`report_text` draws it, `*..*` in gold), `report` draws it all over the whole
screen last in `flip_hooked`, `input` takes all input until Back. Not a game screen: the game stays on the squad screen, a new
screen closes it (`forget`). Client: `remote_report` ('T'). Check: Assistant > Team report on
squad, single player, host and client; the tactics menu keeps four items; the selection is the
same after Back, also with a banned player in it.

**Scout screen** (switch `scout_screen`): a "Full scout report" chip on "<club> scouting completed"
stories (`news::story`, `SCOUT_CHIP`, `input`, `open_scout`). `scout_for` makes it in the Team
report's format and `report` draws it (`REPORT_SCOUT`): one pitch and the scout's text in the
second panel. Shape: the club's manager's +0x43 (else its assistant's), the same one the game's
report names (0x7be670). Players left out: injured, banned for its league (club +0x57).

## Player profile

| Item | Functions |
|---|---|
| Attribute bars, labels one font | `text_box_hooked`, `bar`, `cell`, `level_colour`, `is_attribute_name`, `attribute_font`, `note_grid_label`, table 0xddf570 |
| Info strip (flag, born, age, wage, value, caps) | `info_strip`, `money_texts`, `main_contract`, `day_month` |
| CA / PA stars, Hide | `ca_pa`, `ability`, `stars`, `star_mask`, `HIDE_CA_PA` |
| Three panels: reputation / status / positions | `reputation`, `player_status`, `positions`, `fit_pitch`, `dot`, `ring`; More / Less `DETAILS_BUTTON` |
| Stats table pills, +/- buttons | `stats_colour`, `comp.rs` `value_cell` |
| Injury cross, form tiles | `form_colour`, `player_status` |
| Last injuries and bans, moves (under the rows of Injuries & Bans, Transfer) | `history.rs`: `rows` (game lists 0x62ec20, 0x55cbc0, 0x86de70 mask 4), `note`, `flip` |

Staff +0x61 = player record; `profiled_staff` / `profiled_player` read screen param 0. Check:
`suite.ps1 ... profile` (shots `_profile`, `_profile_more` at 800x600).

## Compare

`compare_players` (find both by name, staff table 0xae23bc), `compare_text`, `compare_overlay`
(P1/P2 boxes, `RADAR`, `polygon`, pitches), `compare_ability`, `attributes` (0x543540). Picks live
on the host (0x7f7780 / 0x7f7920). Check: `suite.ps1 ... compare` (Action > Compare Players > Set as
1st / 2nd > Compare two chosen). Under the pitches: a line each from the assistant and the two
best judges among the coaches (`assistant::staff_views`, switch `compare_report`).

## Search and scout stars

| Item | Functions |
|---|---|
| View "Ability" | `other_item` (call 0x80a774), `search_view_bits`, `search_input` (0x80eb00), `ability_text`, `entry_ability`, `linked_person`, `row_names`, `staff_by_row_name` |
| Sort by stars | `get_results` (call 0x808e47, swaps `compare_by`) |
| Scout Rec stars | `scout.rs`: `intrinsic`, `stars`, `club_cuts`, `cuts`, `judging`, `error`; test `cargo test ... scout` |

Check: `suite.ps1 ... search` (`_search_ability`, `_search_potential`). Specs: `..\helper_search.md`,
`..\scout_stars.md`.

## News

| Item | Functions |
|---|---|
| Story layout, header, crest tile, category pill | `news::story`, `news::category`, `news::story_text`, `story_header`, `story_crest`, `story_link_crest`, `news::kind` |
| List rows (icon, short date) | `news::row`, `news::short_date`, `news::icon` |
| Links / chips / clicks | `news::link_list`, `word_links`, `story_links`, `pointer` (REMAP), `clicked`, `news_input` (0x7892b0), `PENDING_CLUB` |
| Mark all read, Next Unread | `news::mark_button`, `mark_all_read`, `PENDING_MARK` |
| Client links | see [network.md](network.md) |

Check: `suite.ps1 ... news`; for clicks use `click <name>` on a story word. Fixes: FIXES "News".

## Competition screens

`comp.rs`: `text` / `panel` (table crests, GD, `form_pills`, title `TITLES_SEEN`), `numbers` /
`value_cell` (stats bars, `"0 (1)"`), `titles` (history winner counts from table 0xae23d4, 26
bytes/row), `add_history` / `history_winners` (duplicate-row guards, hooks 0x4afce0 / 0x4991f0),
`flip`, `honours` (club Honours list: cup and count over the game's rows, switch `honours`). Check: open a competition (Competitions menu, club > league > History, Stats > History);
no suite part yet (manual; checklist item 8 in testing.md).

League table, under `stats_table`: `one_view` puts the title on the View row and gives the rows the
freed height, the whole competition on one view while a row stays readable (`table_area`: font 3's
height, at 800x600 `NARROW_ROW`, so 24 clubs scroll there). At 800x600 `narrow`: rows font 2
(`cell_font`, only cells as high as the head), pills 13 wide (`NARROW_FORM`), GD against Pld
(`gd_right`). A nation column (`nation_column`, a group) or a live table has no Form.

## Match screen

| Item | Functions |
|---|---|
| Crests by scores, header band, labels, events, minutes | `match_text`, `LAST_MINUTES`, `event_icon`, `ball`, `club_named` |
| Key stats strip, last-5-minutes bar | `match_overlays`, `key_stats`, `match_record` (live `*(param0+0x5f)`, afterwards param 0x20) |
| HALF / FULL TIME | `match_text` (period M+0x4f) |
| Man of the match on Player Ratings | `match_text` (his name by the slot `MOM_SLOT`), `light_man_of_match`, `half_of_row`: his half of the row, both sides stand in one list |
| Band of the League Table tab | `match_overlays`: no date there, so two lines |

Check: `suite.ps1 ... squad` (`_last_match`, `_last_match_stats`) and part `match` (live; moves the
game on, run last). Spec: `..\helper_match.md`.

## Save / Restore dialogs

No drawing of its own: typing into the file box needs a quiet screen. The fix is `comp.rs`
`TITLES_SEEN` (only the first screen title per full frame counts) plus the post-click full frame in
`poll_input` (`CLICK_CHECK`). Check: Save Game, type a name; `cmui.trace` should show few
"cmui redraw (poll)" lines. FIXES "Typing / redraws".
