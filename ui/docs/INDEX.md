# cmui docs: start here

cmui = a Rust DLL (`ui\src\*.rs`, built to `ui\target\i686-pc-windows-msvc\release\cmui.dll`) that
restyles CM 01/02 by hooking the original exe. Long history in dated rounds: `..\README.md`.
Source sizes: `lib.rs` ~5300 lines (almost everything), `news.rs`, `comp.rs`, `assistant.rs`,
`scout.rs`, `history.rs` (a player's injuries, bans, moves), `remote.rs` (network transport). Find a
function: Grep `fn <name>` in `ui\src`.

## Screen / feature -> doc -> main functions

| Screen / feature | Doc | Main functions (`lib.rs` unless named) |
|---|---|---|
| Palette, background, panels, menu bar, dropdowns | [screens.md](screens.md#menu-bar-and-theme) | `palette_init`/`apply_palette`, `background`, `panel`, `sidebar_item`, `menu_bars`, `menu_row`, `button_arrows` |
| Squad (club header, list, views, Ability view) | [screens.md](screens.md#squad) | `league_line`, `squad_member`, `fit_cells`, `selection_item`, `squad_ability_text`, `club_input` |
| Tactics pitch + list, live (in-match) tactics | [screens.md](screens.md#tactics-and-live-tactics) | `tactics_row`, `tactics_columns`, `tactics_labels`, `label_row_inside`, `live_*`, `tactics_input` |
| Assistant pick, team report | [screens.md](screens.md#assistant) , `..\assistant_pick.md` | `assistant.rs`: `pick_for`, `flip`, `input`, `run_pending`; `report_for`, `report` |
| Player profile | [screens.md](screens.md#player-profile) | `text_box_hooked` (bars), `info_strip`, `ca_pa`, `reputation`, `player_status`, `positions`, `ability` |
| Compare players | [screens.md](screens.md#compare) | `compare_players`, `compare_overlay`, `compare_text` |
| Player search, Ability view, scout stars | [screens.md](screens.md#search-and-scout-stars) , `..\scout_stars.md` | `other_item`, `get_results`, `search_input`, `ability_text`; `scout.rs` |
| News | [screens.md](screens.md#news) | `news.rs`: `story`, `row`, `word_links`, `mark_button`; `lib.rs`: `story_header`, `story_crest`, `news_input` |
| Competition table / stats / history | [screens.md](screens.md#competition-screens) | `comp.rs`: `text`, `panel`, `flip`, `titles`, `add_history`, `history_winners` |
| Match screen (overview, stats, live) | [screens.md](screens.md#match-screen) | `match_text`, `match_overlays`, `key_stats`, `event_icon`, `match_record` |
| CM Challenge (managers' league, network games; how to drop it) | [challenge.md](challenge.md) | `src\challenge\`: `prizes`, `moves`, `seasons`, `page`, `update`, `overlay` |
| Save / Restore dialogs, typing | [screens.md](screens.md#save--restore-dialogs) | `comp.rs` `TITLES_SEEN`, `poll_input` |
| Network (host builds client screens, packets, block) | [network.md](network.md) | `remote.rs`, `host_answer`, `screen_sent`, `news::screen_block`, `comp::history_block`, `client_loop` |
| Test scripts, saves, driving the game, logs | [testing.md](testing.md) | `poll_input`, `script`, `find_label` |
| Feature switches, exe patches, dsound stand-in | [switches.md](switches.md) | `FEATURES`, `load_features`, `exe_switches`; `make_patches.py`, `set_switch.py` |
| Crests, logos, flags, badge packs | `..\README.md` sections "Crests", "Flags...", "Badge packs" | `logos.py`; `crest_pixels`, `draw_crest`, `club_named` |

## Finding a fix

`..\FIXES.md` is the register: one row per bug, grouped by screen (Match screen, Squad / tactics /
Assistant, Typing / redraws, Competition screens, News, Network client). Columns: seen, cause,
where, check, status. Look there first when a fixed bug returns; compare cause and `Where` before
changing code. Add a row in the same step as every fix.

## Plans and background

| File | What |
|---|---|
| `..\NETWORK_PLAN.md` | the cmui block idea for network clients; steps 1-3 (news) done, 4-6 open |
| `..\NETWORK_NIGHT_PLAN.md`, `..\night2\REPORT.md` | the 2026-10-30 network night (prefetch, Assistant subs); roles and checklist in [testing.md](testing.md) |
| `..\night4\REPORT.md` | the 2026-10-31 night: all new features single player and network, the host's match day hang |
| `..\night6\PLAN.md` | the 2026-11-04 single player night: every menu and screen, coach off |
| `..\scout_stars.md`, `..\assistant_pick.md` | design and facts for those two features |
| `..\helper_*.md` | static exe findings (addresses, record offsets): `helper_search` (search View popup), `helper_squad` (squad View popup), `helper_match` (match stats, opening a club), `helper_live` (in-match player data), `helper_stats` (season stats Av R / Tck / Drb) |
| `..\proof\INDEX.md` | proof shots per resolution |
