# Scout "Rec" stars: how the game does it, and a proposal (switch `scout_stars`)

Status: **built 2026-10-29** (`src/scout.rs`, switch `scout_stars`). **Changed the same day on
the user's choice (open point 2):** the cut points are the scout's own club's, not the whole
game's, as the game's own stars are. From the club's best eleven current scores, best first:
5★ as good as its 2nd best, 4★ its 5th, 3★ its 11th (first-team level), 2★ within 8 points of
that, else 1★. Future stars use the same cut points. The whole-game percentiles below stay as
the fallback for a club with fewer than eleven players. Reason: cm25 has 85,770 players, most
of them weak, so a £7M La Liga regular was top 2 % (5★). At Dortmund the same Scout #2 list now
reads 3★ at most. Rec stays the better of current and future (user's choice). Open point 1 (the Ability view) was not answered, so that view still shows
the true CA / PA. The VVV numbers below are from the 28 Oct save. The 29 Oct save gives cut
points 48.6 / 62.1 / 70.2 / 78.0.

## 1. How the game computes the stars now

A search result entry is 0x2d bytes: +0xc the staff index, **+0x29 current stars, +0x2a
potential stars** (0..5). The Rec column, the Ability view and the sorts read these two bytes
(0x818ab0 best of both, 0x819620 current, 0x8196e0 potential).

They are written in two places, both with the same ladder:

- `PLAYER_SEARCH::report_on_player` 0x7b62a0 (the scout reports on a player he watched),
  ladder at 0x7b687b..0x7b69cf, stored at 0x7b6a47 / 0x7b6bc4;
- `PLAYER_SEARCH::build_scout_targets` 0x7b0050 (filling a Scout #n list), ladder at
  0x7b07c6..0x7b0915.

Every other writer (load, copy, own squad, known people) only copies them or sets 0 (not scouted).

What goes in (report_on_player):

1. **score** = the `score` argument − rand(500) + `match_man_score_player_for_role` 0x6e2f20
   (player, flags 0x1e80, *manager = the scout*) + rand(500). For **potential**: the same with
   flags 0x1f00, and **only under age 25**. Older players get 0 potential.
2. **Where the scout's judging comes in:** 0x6e2f20 calls `match_man_judge_player_ability`
   0x6e86d0 (ported as `World::judged`, `rust\src\pass.rs:1667`). The scout's **Judging Player
   Ability** is used for current, **Judging Player Potential** for potential. The judged
   ability is `((truth + noise)·w + (20 − w)·known) / 20`, where noise and w come from the
   judging value times a share. That share is fixed per (player id + scout id + the save's
   random seed): no new roll on a redraw. A scout with good judging who knows the player well
   gets the true CA.
3. Then there are multipliers on the score, mostly from the player's physique: pace +
   acceleration under 15 gives ×0.75, under 20 ×0.95. Teamwork + work rate + strength +
   stamina + jumping under 25 gives ×0.85, under 40 ×0.95; a high total gives ×1.05..×1.2.
   Age over 34 gives ×0.85. Potential has its own variants of these bands.
4. **Stars are relative to the scout's own club**, not the world. T = the club's target rating
   `player_search_get_club_target_rating` 0x7bca90, clamped to 1750..35000. Club reputation
   R is club +0x80:
   - 5★: score > 1.2·T and score > 2·R + 12750
   - 4★: score > T and score > 2·R + 10750 (potential: + 10500)
   - 3★: score > 0.8·T
   - 2★: score > 0.7·T
   - 1★: score ≥ 0.6·T
   - else 0★
5. If both scores are ≤ 0.6·T, the player is not reported at all (unless forced). A 5★ entry
   also feeds the "scout recommends" news.

So today "3★" means "about your club's level". The proposal below changes that to "about
the top 30 % of all players in the game", as asked.

## 2. CM Scout Intrinsic (forum p19933, Mad Scientist's weights, 13 Mar 2021)

For each attribute with value v (1..20) and weight w: k = (v / 20)·w. The score is
Σk / Σw · 100, from 0 to 100. Dirtiness and Injury Proneness use 21 − v. There are seven
weight columns: GK, D (central defenders), DM, M, AM (wingers and attacking midfielders), A,
and WB (full backs and wing backs). The 48 weights are copied as they are into `scout.rs`.
Ambition and Sportsmanship have weight 0.

Values used: the profile's own 1..20 values. The 15 intrinsic attributes are converted with the
player's CA exactly as `get_player_attribute` 0x543540 does (`Player::shown_attributes`). One On
Ones, Penalties and Throw Ins are converted by kind (`hidden_attributes`). The others are taken
as stored, and the eight personality values come from the staff record. All values are clamped
to 1..20, since the patched exe allows more.

Positions: the score is the best column among the positions the player has at 15 or more:
- GK: goalkeeper
- D: sweeper or defender, central
- WB: wing back, or defender on a side
- DM: defensive midfielder
- M: midfielder, central
- AM: attacking midfielder, or midfielder on a side
- A: attacker

If he has none of them, the best of all seven columns is used.

Measured on `VVV.sav` (22,157 players; throwaway run): median 55.4, top 10 % ≥ 69.8, top 2 %
≥ 77.5, best 94.9. The score correlates 0.87 with CA. The top of the list is Vieira, Nesta,
Thuram, Kahn, Campbell, Keown, Southgate, Aimar, Adams, Cannavaro, Keane, Batistuta, Vieri,
Shevchenko, Verón.

## 3. Proposed formula

**Current score I** = the intrinsic score above, with the player's CA.

**Future score F** = the same score, with CA moved toward PA by age:
CA′ = CA + (PA − CA)·g, where g = clamp((28 − age) / 8, 0, 1). So age 20 and younger gets
the full PA, 24 gets half, and 28 and older gets none. This is the "mix of the intrinsic score
and PA": his own intrinsic values, at the ability he is expected to reach.
(Without the age factor, Keown at 35 with PA 169 came out 2nd for future.)
F is never below I.

**Stars (whole game).** The cut points are percentiles of I over every player in the staff
table, recomputed when the search screen opens (about 22k players, a few ms), so that any
database works:

| stars | current score at least | share of players (VVV) |
|---|---|---|
| 5 | 98th percentile (77.5) | 2 % |
| 4 | 90th (69.8) | 8 % |
| 3 | 70th (61.8) | 20 % |
| 2 | 30th (48.4) | 40 % |
| 1 | below | 30 % |

F is placed on the **same** cut points, so "Future 5★" means "will be as good as today's top
2 %". VVV gives future 5★ to 618 players and current 5★ to 445.

**The scout's eye.** seen = score + e, where e = (2u − 1)·(20 − J)·0.7 score points:
- J = the scout's Judging Ability for I and Judging Potential for F. Both are intrinsic in the
  save, so they are shown values converted the way the game does it (0x544410).
- u in [0, 1) is a hash of (scout staff id, player staff id, current / future), so every
  scout has a fixed opinion of every player, the same on every redraw and after reloading.
  Two scouts can disagree.

Measured on VVV, how often the seen stars differ from the true stars:

| judging | one star off | two off |
|---|---|---|
| 18 | 5 % | 0 |
| 15 | 13 % | 0 |
| 10 | 27 % | 0 |
| 5 | 39 % | 1 % |
| 1 | 47 % | 4 % |

The 0.7 can be tuned.

## 4. Where it hooks (lib.rs: one `mod` line and one call)

- In cmui's existing `get_results` hook (lib.rs, `RESULTS_CALL` 0x808e47, the search screen's
  call to `SearchEditSession::get_current_results`), before the game's call:
  `if on(SCOUT_STARS) { scout::restar(this) }`. `restar` takes the session's current search
  (its PLAYER_SEARCH: result buffer +0x1ee, count +0x1f6; its scout from `get_scout` 0x7ae950)
  and rewrites +0x29 / +0x2a in place for every entry the scout has rated (either byte > 0).
  Unscouted entries keep 0, and a search without a scout keeps the game's stars.
  - This one site covers the Player & Staff Search, every Scout #n list, the Rec column, the
    game's own sorts by stars, and players scouted before the switch was turned on.
  - Not touched: who gets reported (the 0.6·T gate) and the timing of the scout news.
- The switch: its own byte, like the 15 others (`make_exe.py`, `make_patches.py`,
  `install.py --list`), default on, name `scout_stars`. It needs no extra hook site, because it
  uses the search site that is already checked.
- The stars are written back into the search, so they stay in the save. Turning the switch off
  later shows the new stars until the scout reports on that player again.

## Open points for the user

1. **cmui's Ability view** (Current / Potential columns) shows the **true** CA / PA today. Should
   it show these scout stars too? That means a two-line change in lib.rs (call
   `scout::stars(...)` instead of reading CA / PA), and with no scout the judge would be the
   human manager.
2. Is whole-game OK for a lower-league save? Most targets there will show 1–2★. The other
   option is the same score with cut points from the scout's club's league instead of the world.
3. Half stars? The game's two bytes hold whole stars only. cmui's own star drawing can do halves,
   but only where cmui draws the stars itself (point 1).
