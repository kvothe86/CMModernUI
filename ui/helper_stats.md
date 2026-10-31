# Season stats of a player (Av R, Tck, Drb, Apps, Gls) - CM0102 3.9.68

Read from decompiles `re\decomp\0x7c1e30.c 0x7c2150.c 0x7c3920.c 0x7c25b0.c 0x7c2750.c 0x7c0d70.c 0x870700.c`
and asm of the call sites. Nothing run. `Player*` = `Staff*+0x61` (its first dword is the player index).
`PS` = `*(PlayerStats**)0xb74350` (`g_player_stats`).

## 1. Functions

| addr | name | convention | use |
|---|---|---|---|
| 0x7c3920 | PlayerStats::get_category_record | thiscall(ecx=PS; Player*, char cat) ret 8 -> StatsRecord* or 0 | record of one row |
| 0x7c2150 | player_stats_get_record_value | stdcall(StatsRecord*, char field, float dflt) ret 0xc -> float in ST0 | number from a record |
| 0x7c1e30 | PlayerStats::get_stat | thiscall(ecx=PS; Player*, char cat, char field, float dflt) ret 0x10 -> ST0 | = get_category_record + record_value |
| 0x7c25b0 | PlayerStats::format_player_stat | thiscall(ecx=PS; Player*, char cat, char field, char *buf, int buflen, int show_subs, int show_apps, int show_zero) ret 0x20 | the exact text the screens show |
| 0x7c2750 | PlayerStats::format_record_field | thiscall(ecx=PS; StatsRecord*, field, buf, buflen, show_subs, show_apps, show_zero) ret 0x1c | same, from a record |

Category (row) index: 1 Non Competitive, 2 League, 3 Cup, 4 Continental, 5 International, 6 Senior Club
(names from 0x7c14b0). The profile screen (0x870700, loop `i = 1..6`) draws all six.
- **League (2)**: league matches only. Cup = 3, Continental = 4.
- **Senior Club (6)**: all competitive club matches of the season, i.e. 2+3+4 added up (record_match 0x7c0d70 writes a
  match into its own category and again into 6, except when that category is 1 or 5). So 6 is what you want for "this season".
- Tactics screen 0x899ea0 itself already does: `cat = database_is_national_team(club 0x53b900) ? 5 : 6`, field 0x11.

Record lookup (0x7c3920): `idx = *(int*)(*(int*)(PS+0x30) + (*(int*)Player)*0x27 + (cat-1)*4)`;
if `idx < 0` there is no record (player has not played that kind of match) else `rec = *(int*)(PS+4) + idx*0x20`.
Also guard: `*(int*)Player < DAT_00ae236c` (player count) else the formatter returns without touching the row.

## Record layout (32 bytes, 0x20; seasonal, reset/archived at season end by 0x7c5fe0)
Offsets from 0x7c0d70 (writer) / 0x7c2150 / 0x7c2750 (readers):

| off | size | field | stat id (0x7c1b20 label) |
|---|---|---|---|
| +0x00 | u8 | appearances (starts+subs) | 1 Apps |
| +0x01 | u8 | of which as sub | |
| +0x02 | u8 | goals | 2 Gls |
| +0x03 | u8 | conceded (label "Con") | 3 |
| +0x04/05 | u8 | pens (scored / second count) | 4 |
| +0x06 | u8 | assists | 5 Asts |
| +0x07 | u8 | team goals | 6 Tgls |
| +0x08 | u8 | team conceded | 7 Tcon |
| +0x09 / +0x0a | u8 | team won / lost | 8 Won, 9 Lost |
| +0x0b / +0x0c | u8 | yellow / red | 10 Yel, 11 Red |
| +0x0d | u8 | man of the match | 12 MoM |
| +0x0e | u16 | **rating sum** (adds match-player byte +0x35 per game) | 0x11 Av R |
| +0x10 / +0x12 | u16 | passes attempted / completed | 13 (0xd) "Pass %" = +0x12/+0x10*100 |
| +0x14 | u16 | **tackles** total | 14 (0xe) Tck |
| +0x16 | u16 | **dribbles** total | 15 (0xf) Drb |
| +0x18 / +0x1a | u16 | shots / shots on target | 16 (0x10) Sh Tar = +0x1a/+0x18*100 |
| +0x1c | i32 | extra counter (not shown) | 0x16 |

## 2. How the screen turns them into text (0x7c2750; buffer pre-filled with "-", or "----" for field 0x11)
Profile call (asm 0x875981): `format_player_stat(PS, staff->p61, row, field, 0xde1f64, 2000, show_subs=1, show_apps=0, show_zero=1)`.
- **Apps** (1): `"%d"` of +0 (shown if +0 != 0 or show_zero); with show_subs=1 and subs>0 and show_apps!=0 it would be `"%d (%d)"`.
- **Gls** (2): `"%d"` of +2 (blank "-" if apps 0 and show_zero 0).
- **Tck** (0xe): apps = +0; if apps == 0 -> stays "-". else `"%3.1f"` of `(float)u16[+0x14] / apps` (per game, one decimal).
  With show_apps != 0: `"%3.1f / %d"` (value, apps).
- **Drb** (0xf): same with +0x16: `"%3.1f"` of `u16[+0x16] / apps`; apps 0 -> "-".
- **Av R** (0x11): apps = +0; if apps == 0 -> text stays `"----"`; else `"%4.2f"` of `(float)i16[+0xe] / apps`
  (show_apps != 0: `"%4.2f / %d"`). No offset/scale: the rating byte per match is added as is, so the unit is whatever 0x35 holds
  (game shows e.g. 6.80; unit not independently checked, only that the divide is plain).
- Pass % (0xd) `"%d%%"`, Sh Tar (0x10) `"%d%%"` (shown only if denominator > 0).
- Numeric twin in 0x7c2150: field 0x11 = +0xe/apps, 0xe = +0x14/apps, 0xf = +0x16/apps, 1 = apps, 2 = goals (u8 at +2);
  if apps == 0 these return the constant at 0x968970 = 0.0 (for 0xe/0xf/0x11). Field 0x12 is a "weighted" average (pads to 20 apps with a default rating), not what the screen shows.

## 3. Minimal recipe for the DLL
Text (safest, identical to the game; buf >= 0x33 bytes, the game asserts `buflen >= 0x33`, give 64+):
```
PS = *(void**)0xb74350;  Player* p = *(Player**)(staff + 0x61);   // check p != 0
// thiscall: ecx = PS; stdcall-style pushes, callee pops 0x20
call 0x7c25b0(PS; p, cat=6, field=0x11 /*Av R*/, buf, 64, 0, 0, 1)   // -> "6.83" or "----"
call 0x7c25b0(PS; p, cat=6, field=0x0e /*Tck */, buf, 64, 0, 0, 1)   // -> "2.1" or "-"
call 0x7c25b0(PS; p, cat=6, field=0x0f /*Drb */, buf, 64, 0, 0, 1)   // -> "0.4" or "-"
```
(use cat 5 if `0x53b900(club)` says national team, as the tactics screen does; cat 2 for the League row.)
In C: `typedef int (__thiscall *Fmt)(void*, void*, char, char, char*, int, int, int, int); ((Fmt)0x7c25b0)(PS, p, 6, 0x11, buf, 64, 0, 0, 1);`
Return 1 normally (0 with an error box if a pointer is null / buflen < 0x33).

Numbers (no text): `float v = ((float(__thiscall*)(void*,void*,char,char,float))0x7c1e30)(PS, p, 6, 0x11, 0.0f);`
Or by hand: `rec = *(u8**)(PS+4) + idx*0x20` with idx from the table above, then
`apps=rec[0]; if(apps){ avr=(i16)*(u16*)(rec+0xe)/(float)apps; tck=*(u16*)(rec+0x14)/(float)apps; drb=*(u16*)(rec+0x16)/(float)apps; }`
(`PS+0x30` = per-player table of 0x27 bytes, 6 dwords for the categories; `PS+4` = record buffer.) Prefer the text call: it keeps the game's rounding and "-" / "----" rule.

## Not found / unverified
- Unit of the match rating byte (+0x35 of the match-player record): not looked up; shown value = sum / apps.
- Meaning of +0x04/+0x05 ("Pens") and +0x1c not needed here and not checked.
- Not run in the game; calling convention taken from asm at 0x875954/0x875981/0x89d18a (ecx loaded from 0xb74350 before the call).
- Staff -> Player: profile uses `*(int*)(staff+0x61)` (non-null test) as the Player* argument; confirmed in the same asm.
