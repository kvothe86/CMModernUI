# Match stats sources and opening a club (exe 3.9.68, image base 0x400000)

Method: decomp `0x71e040.c`, `0x71c270.c`, `0x453d60.c`, new Ghidra run of 0x7643d0 and 0x71e750, plus capstone
disassembly (`0x71c270`, `0x71d9e0`, `0x7fc7e0`, `0x7fe600`). Ghidra mislabels the branch structure of
0x71c270 (see overview); asm below is authoritative. "Not found" = not located.

## A. Match statistics

### Where the match comes from
Screen parameters via `ScreenManager::get_screen_param` 0x7ff010 (ECX = ScreenManager 0xb74450, push index).
- **M** = the 0xd7a-byte match statistics record ("match data"). All stats below are M + offset.
- Live match: `open_match_screen` 0x718aa0(slot, match) sets **param 0 = match object**; **M = *(param0 + 0x5f)**.
- Post-match ("Last Match", `open_match_details_screen` 0x71b4a0 -> `build_match_details_screen` 0x71b630):
  param 0 = 0, param 0x1e = stored record index. 0x71b630 loads param 0x1f (0x4f-byte fixture, `Match_Stats::find_record_fixture`
  0x729e30) and param 0x20 (the 0xd7a record, `Match_Stats::read_match_record` 0x729600). Stats refresh then uses
  **M = param 0x20** (0x71e750: `param0 ? *(param0+0x5f) : param 0x20`).
- Both views: panels get M; param 0x1d = a private 0xd7a snapshot of M (alloc `operator new(0xd7a)`), used to redraw
  only changed cells.
- Team blocks have stride **0x625** (home block = away block - 0x625). Home/away offsets below differ by 0x625.

### Stats panel: `draw_match_stats_panel` 0x71e040 (layout), `refresh_match_stats_values` 0x71e750 (values)
0x71e040 builds 13 rows x 3 widgets (home value, label, away value). Param 9 = index of the first widget; row r:
home = base+3r, away = base+3r+2. 0x71e750 writes text via `set_widget_text` 0x801c70(widget, text).
Plain values use format 0x98806c (decimal), percentages use 0x98e8d4 (percent text). Types: byte unless noted.

| Row | Label (game text) | Home | Away | Arithmetic |
|---|---|---|---|---|
| 0 | "Shots On Goal" (= total shots) | M+0x127 | M+0x74c | none |
| 1 | On Target | M+0x128 | M+0x74d | none |
| 2 | Off Target | M+0x127 - M+0x128 | M+0x74c - M+0x74d | derived |
| 3 | Corners | M+0x129 | M+0x74e | |
| 4 | Free Kicks | M+0x12a | M+0x74f | |
| 5 | Throw Ins | M+0x12b | M+0x750 | |
| 6 | Fouls | M+0x12c | M+0x751 | |
| 7 | Offsides | M+0x12d | M+0x752 | |
| 8 | Passes Completed | s16 M+0x132 *100 / s16 M+0x130 | s16 M+0x757 *100 / s16 M+0x755 | integer division; denominator 0 -> text at 0x98ba0c (not decoded, probably "-" or 0) |
| 9 | Tackles Won | s16 M+0x136 *100 / s16 M+0x134 | s16 M+0x75b *100 / s16 M+0x759 | same |
| 10 | Headers Won | s16 M+0x13a *100 / s16 M+0x138 | **100 - home %** (cache check uses s16 M+0x75d; denominator is home M+0x138) | |
| 11 | Yellow Cards | M+0x12e | M+0x753 | |
| 12 | Red Cards | M+0x12f | M+0x754 | |

Notes: attempted/completed naming of the shorts is inferred from the formula (numerator = won/completed, denominator =
total). Pass% = (M+0x132)*100/(M+0x130) home, (M+0x757)*100/(M+0x755) away; Tackle% = (M+0x136)*100/(M+0x134) and
(M+0x75b)*100/(M+0x759). A mod can read these bytes directly; the percent is truncating integer division.
Header percent of the away side is not its own ratio (100 - home%). Writer of these counters: not traced
(engine `rebuild_team_match_stats` 0x6ce460 rebuilds side totals at tick 0, 1, 990).

### Match overview panel: `draw_match_overview_panel` 0x71c270(int *top_y, int *bottom_y)
- Called only from `build_match_screen` 0x718e90 (live) and `build_match_details_screen` 0x71b630 (stored).
- Record R: live = param 0 (nonzero branch, asm 0x71c2f2); stored = *(param 0x1f + 0x18) (asm 0x71c8cc, param 0 == 0).
  Both then converge at 0x71c594: **M = *(R + 0x5f)**, text period in M+0x4f (s16), shoot-out flag **int M+0x53**.
- Team ids M[0] (home, int) and M[1] (away); club table base DAT_00ae23b4, team colours via `0x53b640` from M+0x152 (home)
  and M+0x777 (away).
- Scorers/cards are drawn from per-team block rows M + 0x198 + side*0x625 + n*0x49 (not needed for the five stats).

### Possession bar "Last 5 Mins"
- Drawn in 0x71c270 (asm 0x71c5a4..0x71c7e0) **only if M+0x53 == 0** (no shoot-out). No other flag, no live-only test:
  the label "Last 5 Mins" (string 0xa16738), the bar frame (x 0x113..0x2cb) and the two colour halves
  (x 0x115..0x1ef home colour, 0x1ef..0x2c9 away colour) are created in both live and stored views, starting at 50/50
  (divider at 0x1ef). Param 0x21 = widget index of the bar label, param 0x22 = bar y (bottom_y at that time).
- Update: `refresh_possession_bar` 0x71d9e0 (called from 0x71c270 end, 0x71a240 live input, 0x71b630). It does:
  `if param0 == 0 return;` (stored view: **never updates, bar stays 50/50 with no percent text**; live view only).
  Reads M = *(param0+0x5f); skips the shoot-out case (M+0x53 != 0); skips when cache param 0x1d exists and
  `M+0x142 == cache+0x142`.
  Home % **p = byte M+0x142**; if p == 0 or byte M+0x767 == 0 then p = 50 (0x32). Away % = 100 - p
  (asm 0x71db64: `0x64 - esi`).
  Divider x = 0x115 + (p*436)/100 (436 = 0x2c9 - 0x115) via `set_widget_rect` 0x800890; texts set with
  `set_widget_text` on widgets (param 0x21)+3 (home "%d%%") and the following one for 100-p.
- M+0x767 is only tested for zero, so the pair (0x142, 0x767) are the two sides' bytes (0x767 - 0x142 = 0x625).
- Whether bytes 0x142/0x767 hold a rolling last-5-minutes share or the whole-match share: **not found** (no writer
  located in 0x6c0000..0x720000; engine candidates `compute_possession_percentages` 0x6ce9c0 (stores to engine
  +0xf5dd+side) and `record_possession_tick` 0x6cf750 with a `rolling_mode` argument). The game's own label says Last 5 Mins.
- Live vs post-match summary: stats rows identical in both (live M via param 0, stored M via param 0x20); stored view
  shows a static 50/50 possession bar.

## B. Opening a club like a click on a club name

### `open_club_squad_screen` 0x453d60 (cdecl, ret 0, no return value)
`(Club* club, int reserves, int restore_params, int alt_view, int locked)`. It builds nothing itself; it calls
`ScreenManager::open_screen` 0x7fe600(draw 0x454900 `club_screen_build`, handler 0x4671f0, single_instance 0,
exit fn 0x469da0 `club_screen_leave`, 0), returns early if that returns 0, then fills params with
`set_screen_param` 0x7ff270: p0 = club, p1 = reserves (forced 0 when `database_club_has_reserve_squad` 0x542740 is 0),
p4 = 1 (5 if alt_view), p0x1b = 1 (2 if alt_view), p0x20..0x22 = three best squad players. `open_screen` sets the
record's rebuild flag (+0x7ea = 1, asm 0x7fea0f) and makes the new node current.

### Callers (call-site scan, 8 sites in 5 functions)
0x7643d0 x3 (`menubar_handle_object_link`), 0x4671f0 x2 (club screen handler), 0x80eb00 x1, 0x838a80 x1, 0x89f710 x1.
Best model: **0x7643d0** `menubar_handle_object_link(short cell)`, "Handles a click on a linked object name". The cell's
type/value come from the screen's cell table: type = `*(short)(( &DAT_00b74c4c)[DAT_00b77c7a*0xc0] + 0xba966 + cell*0x18c)`
(0 -> default `DAT_00dd6bde`), value = same base `+ 0xba9a6`. For type **0x7d5** (club):
```
iVar2 = value;   // Club*
if (cond) { open_club_squad_screen(iVar2, 0, 0, 0, 0); return 0xfffffffc; }   // -4
```
`cond` (asm-checked from decomp): `(is_national(club)==0 || club+0x53==0 || *(club+0x53)+0x71 != 0) && (is_national(club)!=0 ||
club+0x53 != 0)`. For an ordinary club it reduces to **club+0x53 != 0** (0x53b900 = `database_is_national_team`).
Type 0x7d6 = reserves: `open_club_squad_screen(club, 1, 0, 0, 0)`, return -4. Type 0x7e6 -> `0x469e70(value,0,1)`; -4 too.
0x80eb00 (search screens) does `open_club_squad_screen(club, 0, 0, 1, 0); return -4;` as well.

### Return code handling
Handlers return an int to `ScreenManager::monitor` 0x7fc7e0 (loop: `poll_input` at 0x7fcc14 -> if handler slot present call
`[node+8](widget)` at 0x7fcd46 -> switch on code via jump table 0x7fdbcc, index code+13). **-4 (and -11) -> 0x7fd3a9:
sets record flag +0x7ea = 1 (rebuild/redraw the current node), then jumps to the common end 0x7fd525.** 0 = nothing.
So the convention is: handler calls the opener, then returns -4 so the loop rebuilds the (now current) new screen.

### Is calling it from inside `Display::poll_input` 0x55f620 safe?
Not recommended as is. Reasons (from 0x7fc7e0):
1. `poll_input` runs before the handler. Opening the club there makes the *new* node current, then the loop still calls
   `[node+8]` of the new node (it re-reads +0x7e2 at 0x7fcd3f) with the old widget index, before the new screen is built
   (rebuild flag handled at the start of the next iteration). A stale index or -1 goes into an unbuilt club handler 0x4671f0.
2. `open_screen` frees forward history nodes and may call exit fns (call eax at 0x7fe86b) of the screen being dropped,
   while `poll_input` (called through the Display object) is still on the stack.
3. No -4 can be returned from poll_input (it returns a widget index or -1).
Safer pattern: in the poll_input hook, on a click on the mod row only record `pending_club` and return -1 (consumes the
click). Detour the news screen handler `news_screens_news_input` 0x7892b0 (cdecl(short widget or -1), returns screen code):
`if (pending_club) { c = pending_club; pending_club = 0; open_club_squad_screen(c,0,0,0,0); return -4; } else original`.
The loop calls this handler right after poll_input in the same iteration, so the flow equals the game's own club click.
Needs to be on the game's main thread (it is the same thread as the loop). The club pointer must be a valid Club*
(table DAT_00ae23b4, stride 0x245; check club+0x53 != 0) and the call is cdecl with 5 stack args, caller cleans up.
Not run/tested; derived from static reading only.
