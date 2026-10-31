# Assistant pick: exploration and proposal (2026-10-29)

The user's idea: a button on the squad screen and on the pre-match tactics screen where the
assistant manager picks the side, as in FM: **best team**, **most fit team**, **squad
rotation** or **clear team**, to suit the tactic in use. A better assistant makes better
choices; a weak one can make poor ones.

Status: **built 2026-10-29** (`src/assistant.rs`, switch `assistant_pick` = byte 16, on in cm25,
main and the net pair via `set_switch.py`). The user's answers: the four modes as proposed;
captain and set pieces too; a line with his reasoning; its own switch, on by default.

What was built, and what differs from the proposal below:
- The button sits left of Clear Squad. A menu choice is carried out by the club screen's input
  (`club_input`), which returns -11 so the list is built again with the new selection.
- **Slots by widget:** the header boxes are not in slot order. Each box is a display widget of
  kind 0xa whose +0x48 is its slot (as the drop reads it, 0x468b90).
- **Availability:** injured (0x62c400 == 2) are left out, players short of fitness count less.
  "Sus", "Ban" and "Int" Inf cells on the squad list mark players as not available.
  `is_player_available_for_match` needs the next fixture, which this screen does not hand over.
- **Captain:** best Influence (player +0x2f) among the eleven, via 0x897760.
- **Set pieces:** Tactics roles 0 / 1 = free kicks L / R (by Set Pieces +0x29), 2 / 3 = corners
  L / R (by Corners +0x22), as Team Instructions shows them. Role 4 is not shown there and is
  left alone.
- **Penalties (added the same day):** the match engine takes an in-match penalty taker from the
  human's 50-place Penalty Takers list (`pick_restart_taker` 0x6ceb90 at 0x6cf0ff: places 0..49
  through `HUMAN_MANAGER::get_squad_number_player` 0x601070, score 10000 − place, so the highest
  place on the pitch takes it). The function map calls that list a "squad number" table; it is
  the one the squad screen's Penalty Takers view shows, numbers 1..50. The assistant clears the
  50 places (0x601560) and fills them with the available squad by Penalties (player +0x38,
  intrinsic), keepers last (0x601200, human 0 = the one at the screen). The Tactics shootout
  order (0x898040 / 0x898150, read by `choose_shootout_takers` 0x6be520) is left as it is.
- **The button** has the game's grey look: face and bevel are sampled from Clear Squad in the
  same frame, white text, a dark chevron. The note is two lines in the list's title band: what
  he did, then why, the captain and the penalty taker.
- Game calls by attribute id (0x5428a0 / 0x5431e0) assert ("Database.cpp 18237"), so
  attributes are read from the player record instead.
- **Rotation:** starters of the current selection count ×0.88, keepers excepted.
- **The note:** formation, changes, then up to two reasons: rested (under 90 %), not available,
  out of position, "X in for Y".
- Checked by script on Glory_v3 (Dortmund, assistant Ole Heck, judging and tactics 20). A poor
  assistant is not tested yet.
- **Tactics screen (before a match), added the same day:**
  - **Placement:** the button goes left of the top Last Match, in the gap that matches the squad
    screen (View · Sort By · gap · Clear Squad there, Tactics · View · gap · Last Match here).
  - **The pick** runs in the tactics screen's input (`tactics_screen_handle_event` 0x89f710,
    hooked; returns -11). Outside a match its rows read the club's Tactics store, so the new side
    shows.
  - **Slot positions:** from the screen's formation (+0xe44,
    `Formation::get_slot_position` 0x5b2ae0). The code's role bit is 0x1 GK, 0x2 SW, 0x4 D,
    0x8 DM, 0x10 M, 0x20 AM, 0x40 F; side 0x80 left, 0x800 right, else centre (0x100 / 0x400).
    Checked against the squad screen's boxes for the 4-4-2.
  - **Ok writes the screen's own copy back** (`apply_changes` 0x8a80d0). The assistant also
    writes that copy: captain at +0x4a20 (staff index), takers at +0x4a2c (`Staff*`, role × 2 +
    choice). Shootout order +0x4a54 and playmakers +0x4a24 are left alone. Checked: after Ok
    and reopening, the side and the captain stay.
  - **The note** in the formation band at its left; where the band is narrow, the second line
    shows only the captain and the penalty taker. Lines are cut at whole words.
  - Not during a match (the match tactics screen is left alone).

## Team report (added 2026-10-30, switch `team_report` = byte 17)

The user's request: "a team rapport; based on assistent manager rating, similar to how he picks
best squad, you get a report of your best 11 and best backup 11", on a new screen, a sub function
of the Assistant button, squad screen only, working in multiplayer.

- **Where:** the fifth item of the Assistant menu, on the squad screen only.
- **His formation (user's wish, same day):** the report shows the formation he recommends, not
  the tactic in use; only the report does, Best team and the other picks stay on the user's
  tactic. He rates that tactic and the eleven formations of `FORMATIONS`: `strength` = his best
  eleven for it, each man's stars score for the slot × how well he reads the man's fit, on
  average. To that `liking` adds his preferred shape, non-player record +0x43 (the staff profile's
  "Preferred Formation", 1 5-3-2 Sweeper, 2 5-3-2, 3 4-4-2, 4 4-3-3, 5 4-2-4, 6 3-5-2, 7
  4-1-2-1-2, 8 4-5-1, 9 3-4-3; cm25's assistants: 325 of 540 on 4-4-2, a handful on 0):
  6 − Tactics × 0.2 points, so a good tactician lets the squad decide more; and Attacking (+0xe),
  0.1 a point from 10 for each man up front (F, AM) above two. The highest wins; the tactic in
  use stays on a tie, and goes by the table's name when it has the same slots. The eight words
  at +0x23 are -1 for every assistant in cm25. The game's own choice for a computer manager is
  `choose_computer_tactic` 0x6e0230. Nothing is written. `formation_text` says why.
- **Why a table and not the game's presets:** Restructured 25-26 keeps the preset names on other
  layouts. Its "4-4-2" stands DC DC DMC MC MC MC AMC AMC AMC FC (also a new human's default), its
  "4-2-4" is a true 4-4-2. The first try took presets by name and so drew that layout under the
  title 4-4-2. The table holds the original game's presets, read from cm25's `tes.sav`. So the
  formation he names may not be under that name in the game's own tactics list on Restructured.
- **A poor assistant is wrong in three ways:** the stars (`scout::seen_score`, a fixed error per
  assistant, player and position, up to ±0.7 × (20 − Judging Ability)), the fit (`seen_fit`, up
  to ±(20 − Tactics) / 4) and the ability in the pick (the game's own judging).
- **The picks:** `report_for` runs the Best team rating (`Eye`, `candidates`, `eleven`) for the
  slots of his formation, then again without the first eleven, so no player is in both.
  Injured, banned and Inf-marked players are out as in the pick. Short of players: a slot stays
  empty.
- **Nothing is written.** One trap: `is_player_banned` 0x55d270 clears a banned player from the
  club's 20 selection slots at +0x1e0 as it answers, so `report_for` saves and restores them.
- **The screen:** an overlay, like compare, but over the whole window: title band with crest and
  assistant, the list of players with a mark for Best XI / Backup XI / Not available, the
  assistant's text under it, two pitches with shirts in the club's colours and the numbers of the
  list, Back. The reference picture's Next button has no meaning here and was left out.
- **The list (user's hints, same day):** only the two elevens, then those not available who score
  at least the weakest man of the two elevens; the rest of the squad is left out. Best XI first,
  then Backup XI, then not available, each by line of the pitch and rating.
- **Stars (user's hints):** half stars 1..5 from the CM Scout Intrinsic score in the column of the
  slot he plays (`scout::seen_score`; his best position when not available), with the assistant's
  fixed judging error. Not relative to the club as the scout stars are: one scale for every club
  (`HALF_STARS`), measured on `E:\cm25\tes.sav`: 5 from 84, 4 from 74, 3 from 66, 2 from 56.
  Best elevens average 85.9 Real Madrid, 73.5 Feyenoord, 67.4 Twente, 59.2 MVV.
- **The assistant's text (user's hints):** `coach_text`, made by the host, `*..*` around names (gold
  on screen): the shape, the two best men, the closest call between a starter and his backup
  (gap under 2 points), the biggest drop to the backup (5 or more), the weakest starter, and the
  best man not available if he would start.
- **While it is open** `input` takes every poll, so the game sees no mouse or keys; it clears the
  left and right click flags, or the click on Back would reach the game's widget under it. Only
  channel 0's poll runs through it, so a host reading the report does not hold up clients
  (`ScreenManager::monitor` feeds clients' packets directly).
- **Client:** key `T`, answered at once in the host's packet handler (the channel is the served
  one there, so screen parameter 3 is its fixture). The text is at most about 2.5 kB.
- **Numbers** are the list's, not squad numbers: the game has none to show (the table the map
  calls squad numbers is the Penalty Takers list).

## What the game already has

| What | Where | Notes |
|---|---|---|
| The Tactics store | `[0xde1d08]` | `this` for all calls below; `team_index` 0 = first team |
| Clear the 20 slots | `Tactics::clear_or_restore_selection` 0x896fe0 (club, team, use_undo, restore) | with `use_undo = 1` the human gets the game's own undo copy |
| Fill a slot | `Tactics::set_selected_player` 0x896a90 (club, slot 0..19, &staff_id, team) | slots 0..10 the eleven, 11..19 the bench (SB1..SB9) |
| Read a slot | `Tactics::get_selected_player` 0x8968f0 | |
| Captain | `Tactics::set_captain_from_slot` 0x897760; the game's own pick `Tactics_Screen::auto_select_captain` 0x8a84a0 | |
| The club's assistant | `database_get_club_assistant` 0x53cb80 (club) -> Staff* | falls back to the linked senior / B team's |
| His attributes | staff +0x69 -> non-player record (68 bytes): judgement +0x17, judging potential +0x18, tactics +0x21, man handling +0x19, motivating +0x1b | intrinsic values: shown 1..20 the way `scout::shown` converts them (0x544410) |
| A player's ability **as the assistant sees it** | `match_man_judge_player_ability` 0x6e86d0 (judge, player, club, kind 2 = current, rep -1) | the game's own noise, which shrinks with the judge's skill: the "better assistant, better choices" comes for free, and is the same model the AI managers use |
| Injured | `Injury::get_player_injury_status` 0x62c400 == 2 (`[0xb63d28]`) | 1 = still short of match fitness |
| Condition | `Injury::get_condition` 0x62c590 (percent) | cmui already uses it (`condition()`) |
| Banned | `Discipline::is_player_banned` 0x55d270 | needs the competition of the next match |
| Away with a national side | `NationalTeams::get_international_duty_club` 0x7757d0 | |
| Available for a fixture (all of the above) | `is_player_available_for_match` 0x45cbe0 (player, fixture, club, comp) | the squad screen calls it (0x48446f) with the next fixture; best single check |
| Position fit | player record +15..+26 (GK, SW, D, DM, M, AM, F, WB, then R, L, C) | cmui's `positions()` / `fit_pitch` already turn these into a 1..20 fit for a slot like "DL" |
| The slots' positions | the squad screen's header boxes (GK, DL, DR, DC, ... SB1..SB9) are drawn as texts in slot order | the tactic block (`Tactics::get_club_tactic` 0x896660) has them too, for the tactics screen |

Not usable as it is: `MATCH_MAN::pick_team` 0x6dbc50 (the AI's own pick, 7.8 kB). It works on
the match-day side object (0x18e3 bytes a side) that only exists while the day is played. The
match day already calls it for a human club whose manager did not pick a side.

## Proposal

**One button, "Assistant ▾"**, in the empty space of the squad screen's top row (between
Sort By and Clear Squad), drawn and handled by cmui like Mark all read on the news screen. Its
menu has four items:

1. **Best team.** For each of the eleven slots, the player with the highest
   `judged CA × fit factor × condition factor`, filled by scarcity (GK first, then the slots
   fewest players can play); the bench then gets the best remaining player for each line
   (a keeper, two defenders, two midfielders, two forwards, then the best of the rest).
2. **Most fit team.** The same, but players under 90 % condition only as a last resort, and
   condition weighs more.
3. **Squad rotation.** The same, but players who started the last two matches count less
   (e.g. ×0.85; the match records the game keeps for Av R), so the second string gets games
   where the gap is small.
4. **Clear team.** `clear_or_restore_selection` with undo (the game's own Clear Squad).

The four would be the user's own "best / most fit / rotation / clear"; each fills the
selection the way a drag does, so the tactics screen, the match and the team sheet stay the
game's.

**The assistant's quality** comes in three ways, all from his own attributes:

- **Judging ability (+0x17):** through 0x6e86d0, so he sees each player's CA with the game's
  noise. A poor judge picks a weaker player who "looks" better now and then.
- **Tactics (+0x21):** how well he reads position fit. Below about 12 he takes a player one
  step out of position for a natural one when the judged gap is big (a fixed error per
  assistant and player, hashed like the scout stars, so it does not change on every click).
- **No assistant:** the button says so ("No assistant manager") and stays grey.

Fixed per assistant and player means the same click gives the same team (no reroll exploit);
a new assistant gives other choices.

**The tactics screen (pre-match):** the same menu next to its own buttons. The slots come
from the tactic block there instead of the header texts.

## Open questions for the user

1. The four modes as above, or other weights (for example rotation by minutes played)?
2. Should the assistant also set the captain (the game's own 0x8a84a0) and the penalty
   takers, or only the eleven and the bench?
3. A line under the button saying what he did ("Your assistant picked a 4-4-2 with 3 changes")?
   It could also name a choice he made against your own last pick.
4. Its own switch (`assistant_pick`, default on), like the other features: yes?

## Effort

About a day's work. The menu button and its clicks follow the Mark all read pattern; the pick
is about 200 lines of Rust over the functions above. Testing is by script on Glory_v3:
compare the picked eleven with what a good and a poor assistant see (swap the assistant's
judgement byte in a throwaway copy).
