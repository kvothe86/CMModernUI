# Testing cmui

Rule: **one game driver at a time** (never two agents or scripts driving games), and never touch
a folder another agent is testing in. The game runs with higher rights than the shell: it ignores
WM_CLOSE, cannot be read or ended from outside, and does not see the shell's environment. So all
control goes through files in the game folder that cmui polls.

## Files in the game folder

| File | Effect |
|---|---|
| `cmui.click` | script lines, read and deleted by `poll_input` (or the flip pass in a live match) |
| `cmui.log` | what cmui did; each script command is logged as `script: <line>` |
| `cmui.trace` | while present: trace lines (panel / text draws, clicks, block sizes), 200,000 at most |
| `cmui.timing` | per-frame timing log (`..\timing_summary.py` splits it per `mark`) |
| `cmui.quit` | cmui ends the game (checked every 500 ms by a thread) |
| `cmui.peek` | logs the first bytes of the hooked functions |
| `cmui.noremote` | a network client stops asking the host (test switch) |
| `cmui.silent` | in a client's folder at its start: the "Waiting for the host ..." box stays on |
| `cmui.ini` | switches, see [switches.md](switches.md) |

## Script commands (`script()` in `lib.rs`; one a line, `pause` ms after each)

| Command | Does |
|---|---|
| `x y` | click at game pixel (move, down, up on a later poll) |
| `click <label>` | click the centre of the newest drawn text equal to (else containing) label, case-insensitive; `\n` for two-line buttons (`Re-start\nMatch`) |
| `click Name\|-60` / `Name\|-11.3%` | offset in pixels / share of screen width from the label's centre |
| `click Name\|slot` | the squad selection cell on that name's row |
| `hover <label>` | move the mouse onto it (opens menus) |
| `move x y`, `down x y`, `up x y` | pointer and button, for drags |
| `type text` | one WM_CHAR per poll |
| `key vk` | WM_KEYDOWN (13 Enter, 9 Tab, 27 Esc) |
| `wait ms` | pause after this command; `pause ms` sets the default (1500) |
| `shot file.bmp` | back buffer to a 24-bit BMP (works with the window covered) |
| `peek 0xaddr` | logs a dword |
| `mark <id>` | unknown command: logs `script: mark <id>?` then `script: mark <id>`; used by `drive.ps1` as its end marker and by `timing_summary.py` |

Labels are recorded only once a script has come (the first script redraws the screen) and forgotten
at each full redraw. A label not found logs `script: no text "<label>" on screen`; this is how
scripts detect state (`shows`). Mouse points are scaled into a window larger than the game.

## Scripts (`ui\*.ps1`; PowerShell)

| Script | Args | Does |
|---|---|---|
| `drive.ps1` | commands as args | writes `cmui.click` in `$env:CMUI_GAME` (default `E:\CM0102`), waits for its marker, prints new log lines, turns `shot` BMPs into PNGs |
| `run.ps1` | `[shot.png] [seconds]` | quit running game, copy the fresh DLL, start `cm0102_ui.exe`, print log, screenshot (`shot.py`) |
| `retest.ps1` | drive commands | quit game, `install_all.ps1`, start, restore a save, run the steps. Env: `RETEST_GAME` (default `E:\cm25`), `RETEST_SAVE` (default `Glory_v3`), `RETEST_QUIT` |
| `install_all.ps1` | none (games closed) | copy `cmui.dll` into every game folder (cm25, main, `_net`, `_net2`, three F: folders), hash-checked ("= build" / "DIFFERENT") |
| `suite.ps1` | `game prefix manager club p1 p2 [part]` | every screen by label, shots `<prefix>_<name>.png`; parts menus, squad, profile, compare, news, search (default all) and match (only when named; moves the game on) |
| `check.ps1` | `[sp\|client] [res,..] [part]` | builds, runs suite per resolution (single player on `E:\CM0102_net`..., or a network pair via `net_pair.ps1`), writes `ui\check\<res>\` shots, sheets, `summary_<mode>.txt` |
| `net_pair.ps1` | `<host> <client>` | host restores the VVV save as network game "test", client joins as Test Client at Ajax |
| `net_join.ps1` | `<host> <client> [ip] [club]` | client half, for a host already at Manager Status |
| `net_tes.ps1` | `[client] [host]` | network pair from the user's `tes` save (host `E:\cm25`, client `F:\CM0102 - Restructured 2025-2026 - 1680x1050 - client`) |
| `net_rejoin.ps1` | `[client] [host]` | restart the client with the fresh DLL and rejoin the running host |
| `net_days.ps1` | `[n] [client] [host]` | Continue Game on host and client n times (marks news read first) |
| `net_res.ps1` | `W H` | builds `E:\CM0102_net_WxH` (+ `_c`) with the patcher's resolution change (32-bit PowerShell). Those folders were removed 2026-10-31 to save room: run this first before `check.ps1` at 800x600 or 1280x800 |
| `shot.py`, `peek.py`, `sheet.py`, `showcase.py` | | screenshot, memory peek (fails with error 5), contact sheets |

Pitfalls: game still running -> `cmui.quit` in its own folder first; the first manager-menu opening
after a load shows nothing (suite reopens it); client takes no input until the host finishes the
day ("poll_input is not running"); `net_tes.ps1` positions are 1680x1050; in `E:\CM0102` the file
`~net.out` is read-only (breaks hosting there).


## The user clicks, Claude watches (2026-11-01)

The fastest test: the user clicks through a list of screens while screenshots run. `shot` takes the
back buffer, which has **no text at 800x600 with cmfix `text4k=1`**: cmfix draws its text over the
window. A capture of the game window itself shows what the user sees; it must take only that window,
and only while it is the active window, **never the whole desktop**. The restore dialog always opens
on `E:\CM0102\` (the registry's Location), so `retest.ps1` types the save's name instead of clicking
it. Test folders from `net_res.ps1` need `text4k=1` and `shader=Lanczos` to look like the main game.
Fits made for small screens ask `small_screen()` (< 1680 wide): 1680x1050 and up stay as they are.
## Saves

| Save | Where used | What |
|---|---|---|
| `Glory_v3` | single player, `retest.ps1` default | Dortmund, German leagues |
| `VVV` (VVV.sav) | `check.ps1`, `suite` defaults, `net_pair.ps1` | VVV, network pair client at Ajax |
| `tes` | `net_tes.ps1`, night tests | network: Ajax host + PSV client (test test) |
| `tes2` | match tests | the `tes` game on 9.9.25, before Real Madrid v Ajax |
| `NETWORK.sav` | older network tests (memory) | two managers in it |

Load flow: `click Restore Saved Game`, `click <save>` or `type <name>`, `click Ok`, `wait 30000`+;
network: `click Network Play` > `Restore Network Game` (host) / `Join Network Game`, type the IP,
Next. Host: `127.0.0.1` on one PC (or the adapter the game binds, README "Testing hints").

## Installing a build

`cargo build --release --target i686-pc-windows-msvc` in `ui\`, then `install_all.ps1` (hard rule:
the same build everywhere, main `E:\CM0102` included; end with the hash lines). `dsfix\install.ps1`
does the same for the dsound stand-in ([switches.md](switches.md)).

## Reading results

`cmui.log` (panics are logged as `cmui panic: ...` before the game aborts), `script: no text` lines,
shots in `ui\check\` / `ui\proof\`, `FIXES.md` Check column for what to look at per bug. Regression
list per screen: below.

## Night runs (unattended; first used 2026-10-29/30, network night `..\NETWORK_NIGHT_PLAN.md`)

| Role | Who | What |
|---|---|---|
| Coordinator | main session (Opus) | the plan, builds, installs (`install_all.ps1`), hands out fixes, decides when a check passes |
| Tester | ONE Sonnet agent at a time, the only one that starts or drives games | runs the checklist, shots to `ui\<night>\<run>\`, writes `<run>\tester.md` (done, seen, labels not found) |
| Verifier | Sonnet agent, no games | reads the tester's shots against the checklist, writes `<run>\verify.md` (pass / fail / unclear per item, with the shot) |
| Fixer | Opus (cause unknown, cross-module, network) or Sonnet (clear cause in one place) | code only, no games; one fixer per file set; adds the `FIXES.md` line |
| Reviewer | Sonnet agent, no games | reads the night's changes, reports correctness risks |

Rules: never two game drivers; a game stuck for 5 minutes (no new log lines, a modal box that
won't close) is quit with `cmui.quit`, else closed by process; retried once from a fresh start,
then noted as blocked and the run goes on. No questions to the user. Loop: tester run A, verifier
on A while the tester does B, fixes for the fails, install, re-run only the failed items (C, D ...),
reviewer on all changes, stop when every item passes or is blocked with a reason; report in
`ui\<night>\REPORT.md`, statuses in `FIXES.md`.

### Regression checklist (each in single player, network host, network client)

1. Menu bar: icons, Competitions / Nations / Game Options dropdowns open and close, previous /
   next arrows.
2. Squad: Selection view, Ability view, fitness / form boxes, Assistant button (Best team, Most
   fit, Rotation, Clear) and its note.
3. Tactics: pitch shirts with box + %, touchline players too; list columns (Av R, Tck, Drb);
   Assistant on tactics; Load Formation file box takes typing.
4. Player profile: info strip, panels, attribute bars, CA/PA stars, injury cross.
5. Compare two players: boxes, radar, the right players for same-named ones.
6. Player search: Ability view, scout stars, sort by stars.
7. News: all stories readable, links blue and clicking (player, club, competition), chips,
   transfer strip (incl. "X get Y" and "M. Tel Aviv"), mood card only with a player, crest:
   board / own scout -> own club; Mark all read; Next Unread.
8. Competition: table crests, GD, form pills; Player Stats views incl. "0 (1)" bars; History:
   title counts "(n)" on first open, every route (Competitions menu, club > league > History,
   Stats > History), Schaal 2025 once.
9. Match: overview events (hat-trick minutes on one line, no light half bar), stats, live
   tactics: bench grey with positions, pending sub colours, red card "-".
10. Save Game / Restore: file box takes typing; network game name.
11. Network only: "Contacting Server" box below Game Options; no DLL error dialogs; client
    Assistant on squad and tactics; client screens complete in the first frame (prefetch).
