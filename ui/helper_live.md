# Live in-match player data (exe 3.9.68, image base 0x400000)

Method: Ghidra decomp of the Tactics_Screen getters 0x8aa200..0x8ab480, tactics_screen_build 0x899ea0, ctor 0x8a7440,
0x899a50, MATCH_PLAYER::setup 0x6eaea0 / drain 0x6f56c0 / recover 0x6f55c0 / update_match_rating 0x6f4aa0,
rebuild_team_match_stats 0x6ce460, 0x723350 (Player Ratings panel), 0x71a240; capstone for 0x8aafd0. Static only, not run.
"unverified" = read from code but not seen at runtime.

## 1. Where the per-player data lives

**Engine E = *(match_obj + 0x57)** (CM3_MATCH_ENGINE*). match_obj = screen param 0 of the match screen (0x71a240 does
`iVar16 = *(iVar10+0x57)`; `*(iVar10+0x5f)` is M, the stats record of helper_match.md). Engine fields: +0x4792 = fixture
record (+0x1c home Club*, +0x20 away Club*), +0x8ed4 s16 tick/period marker (0x1ef half, 0x3de end), +0x8ebb flag.

**MATCH_PLAYER (0x1be bytes) for side s (0 home, 1 away), squad index i 0..19:**
`P = E + 0x4796 + (i + s*0x14)*0x1be` (0x6c2fe0 `get_match_player`). i 0..10 = starters, 11..19 = bench ("SB<n>" = i-10).
Squad index i is the "slot" every tactics-screen getter takes.

| P+ | type | meaning |
|---|---|---|
| 0x19 | s8 | pitch slot 0..10 (formation slot); 0xff = not on pitch / bench; 0xfb(-5) = empty entry (skip it); 0xfe(-2) = unavailable/injured at start |
| 0x1b | u8 | **displayed in-match rating 1..10**; recomputed by 0x6ce460 as `(s16[P+0x35]+500)/1000`, clamped 1..10 |
| 0x35 | s16 | live rating points (7000 = 7.0), changed every tick by 0x6f4aa0 and the adjust_*_ratings fns; bench start 0x1900 |
| 0x4d | float | **live condition (stamina), 0..10000** (10000 = 100 %). Drained by `drain_stamina` 0x6f56c0, small recovery in 0x6f55c0, reset at breaks by 0x6f5810. Set in setup from 0x62c590(staff,0) (raw 0..10000) |
| 0x51 | float | condition at kick-off (copy) |
| 0x69 | Staff* | the staff row ([0xae23bc] + id*0x6e); no search needed. Player* = *(Staff+0x61) (also stored at P+0x6d) |
| 0x1f,0x20,0x21,0x22 | s8 | minute "on", "sub", "off", "inj" (0xff = none; screen prints value+1). Tactics shows `inj <min>` if P+0x22>=0, else off/sub/on in that order (0x8aadb0) |
| 0x27 | u8 | side of this player (0 home) |
| 0xf9 | Club* | the player's club in this match |
| 0x19e | E* | back pointer to the engine |

Shirt number: not in MATCH_PLAYER (not found). It is in the match-record rows (see "M rows").

**Position in the match** (formation slot P+0x19 = f, 0..10): `code = u16 *( *(E + 0x8ebc + P[0x27]*4) + f*2 )`
(0x6c38c0 `get_player_position`, wrapper 0x717410 `MATCH_PLAYER::get_position(P)` returns 0 if P+0x19e==0 or f out of 0..10).
Code is a bit mask: low part 0x1 GK, 0x2 SW, 0x4 D, 0x8 DM, 0x10 M, 0x20 AM, 0x40 F; side part 0x80 L, 0x100/0x200/0x400 C
(three centre bands), 0x800 R. Text: `formation_get_position_abbreviation` 0x5b7810(code, char* buf, size>=10, 0), format "%s%s"
(0x99ec04), so the game prints "DC", "AMR", "GK" (no space; insert one if "D C" is wanted). Long form: 0x5b7580.
The second table used for the alternative position is `0x6c3920/0x717430`; not needed here. For a bench player (f = 0xff) the
tactics screen shows "SB<n>" instead of a position.

**M rows (post/stat record, M = *(match_obj+0x5f))**: 0x723350 (Player Ratings panel) reads, with
`R = M + 0x190 + side*0x625 + n*0x49` (the helper_match.md figure 0x198 is 8 too high; the team block starts at M+0x11f,
block stride 0x625, rows at block+0x71): R+0x00 int staff index (Staff = [0xae23bc]+idx*0x6e), R+0x1c s8 shirt number
(if <1 the screen shows n+1), R+0x1d status (0xfb = empty row), R+0x35 u8 rating shown as "%d", R+0x21 u8 second number
column (unverified meaning: goals/assists), R+0x2c/0x2d card icons, R+0x30 sub / R+0x32 on / R+0x33 inj / R+0x34 off minutes.
Whether R+0x35 is refreshed every tick (the panel redraws only changed cells, so it is polled live) is unverified; the engine
byte P+0x1b is the one the in-match tactics list uses.

## 2. How the in-match tactics screen gets each value

Tactics_Screen object T (0xc4e2 bytes, ctor 0x8a7440, built by 0x899ea0, handler 0x89f710): T+0 Club*, **T+4 = E** (0 when
opened outside a match), T+0xde9 s8 = index of own side in E (0/1), T+0xdea = the other side, T+0xde5 = opposing Club*,
T+0xde1 flag "opponent known". Own team: the ctor copies the 20 MATCH_PLAYERs of side T[0xde9] to **T+0x2735 + i*0x1be**
(snapshot at screen open, loop at ctor `0x6f` dwords each). Opponent (arg `param_6/opp = 1`): live
`E + 0x4796 + (i + T[0xdea]*0x14)*0x1be`. Getters (thiscall, ecx = T, i = slot, opp = 0/1; all return 0/empty when T+4 == 0
unless noted):
- 0x8aa200 `get_slot_number_text`: in-match shows M-row shirt number from T+0x137 block (`T + 0x1a8 + i*0x49 + side*0x625`,
  +0x1c; 0xfb at +0x1d = empty) else i+1.
- 0x8aa550 `get_slot_player_name_text`: name via Staff = P+0x69 (0x53dba0) or from the row (0x53e140).
- 0x8aa9d0 `get_slot_pitch_position(i,&code,opp)`: returns P+0x19 (f); when f == -2 uses P+0x1a. Then position text via 0x5b7810.
- 0x8aadb0 `get_slot_match_status_text`: "inj/off/sub/on <min>" from P+0x22/0x21/0x20/0x1f (see table).
- **0x8aafd0 `get_slot_condition(i,opp)`: `__ftol(float[P+0x4d] * 0.01)`** (asm 0x8ab059: fld [eax+0x4d]; fmul qword [0x967898]=0.01).
  Returns 0 for P+0x19 == -5. (Non-match branch, T+4 == 0: 0x62c590(staff,1) percent, 0xff if 0x62c400 == 2.)
  0x899ea0 prints it with "%d%%" (0x98e8d4); if the value is <= 0 it prints "Inj" (0x98e98c).
- **0x8ab150 `get_slot_match_rating(i,opp)`: u8 [P+0x1b]** (0 for -5 or -1). 0x899ea0 draws it with "%d" (0x98806c) when > 0
  (call at ~line 1835 of decomp/0x899ea0.c), together with goals from 0x8ab260.
- 0x8ab480 `get_slot_player(i,opp)`: in-match returns P+0x69 (Staff*) when P+0x19 != -5; outside a match from the T+0x137 rows
  (staff idx*0x6e + [0xae23bc]); own team non-match uses 0x8968f0.
- Column dispatch inside 0x899ea0: `local_929` (column type) 2/6 = status + condition (type 2) / rating + goals (type 6).

## 3. Telling the mod it is on the in-match tactics screen

- The tactics screen is its own screen node: draw/build 0x899ea0, handler 0x89f710 (`ScreenManager::open_screen(0x899ea0,
  0x89f710, ...)` in 0x899a50). Its params: **param 0 = T**, 1 = Club*, 2 = E (0 outside a match), 3 = flag, 4, 7, 8, 9 = -1,
  **param 10 = 6 in a match, 2 outside** (`(E!=0 ? 4:0) + 2`, 0x899a50). So: current screen handler == 0x89f710 (or a detour
  setting a flag in 0x899ea0 / 0x89f710) and `T = get_screen_param(0)`, `E = *(T+4) != 0` (or param 10 == 6).
- Opened from the match screen handler 0x71a240 (calls 0x899a50(club, flag, E, 0|1, -1, 0|1) then returns -4).
- Do not use "param 0 non-null" alone: the match screen also has param 0 (match_obj, whose +0x57 is E). A global
  "match in progress" flag: not found (engine byte E+0x8ebb is tested in setup/run_match_steps, meaning unverified).

## 4. Minimal recipe (mod, main thread)

```
mo = get_screen_param(0)                 // on the match screen: match_obj;  on the tactics screen: T (then E = *(T+4))
E  = *(mo + 0x57)                         // match screen;  tactics screen: E = *(T + 4)
for s in 0..1, i in 0..19:
  P = E + 0x4796 + (s*0x14 + i)*0x1be
  if (s8)P[0x19] == -5: continue         // empty entry
  staff     = *(u32*)(P + 0x69)           // Staff*; Player* = *(u32*)(staff + 0x61)
  cond_pct  = (int)(*(float*)(P + 0x4d) * 0.01f)    // what the tactics list prints, "%d%%"; raw 0..10000
  rating    = *(u8*)(P + 0x1b)           // 1..10 as printed; finer: *(s16*)(P+0x35) / 1000.0
  f         = (s8)P[0x19]                // 0..10 on pitch, 0xff bench / off
  code      = f in 0..10 ? *(u16*)( *(u32*)(E + 0x8ebc + P[0x27]*4) + f*2 ) : 0
  pos_text  = call 0x5b7810(code, buf, 10, 0)   // cdecl, "DC" style; or decode the bit mask above
  off/sub/on/inj minutes = P[0x21], P[0x20], P[0x1f], P[0x22]  (0xff = none)
```
To find the row of a given Staff S: scan the 40 players for `*(u32*)(P+0x69) == S` (same as `MATCH_MAN::find_match_player`
0x6e1cd0, which scans 20 players at stride 0x1be comparing +0x69); the side is P[0x27], club P+0xf9.
Own team on the tactics screen: the displayed values come from the snapshot at T+0x2735+i*0x1be (same field offsets); the
engine copy is the live one and equals it at the moment the screen opens.

## Open points

- Condition at P+0x4d is not clamped to 100 % display-wise (0x8aafd0 only truncates); a recovering/injured player at 0 shows "Inj".
- P+0x1b is refreshed by 0x6ce460 at tick 0, 1, 0x3de and period ends (also each step when E+0x8ebb != 0, run_match_steps
  line ~298); between those use P+0x35. 0x6ce460's tick 0/1 branches also clamp 1..10 the same way.
- M-row R+0x35 liveness, meaning of R+0x21, the "sub / on" semantics of P+0x1f/0x20 order (inferred from the text order in
  0x8aadb0), and the exact start of the MATCH_PLAYER-array copy for the away side in T are unverified.
