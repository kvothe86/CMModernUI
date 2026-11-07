//! Scout stars (switch `scout_stars`; design and measurements in `scout_stars.md`): the Rec
//! stars of the player search follow Mad Scientist's CM Scout Intrinsic weights, ranked against
//! the scout's own club's squad (the whole game when it has none), as the search's scout sees
//! them (his Judging Ability for current, Judging Potential for future; a fixed error per scout
//! and player).

use crate::{game, latin1, log, rd, readable, staff_name, text_at, text_width, STAFF_COUNT, STAFF_TABLE};

// A SearchEditSession: the current index at +0, ten PLAYER_SEARCH of 0x2b0 bytes from +1
// (0x804a40). A search's results: +0x1ee the entries, +0x1f6 their count; an entry is 0x2d
// bytes, +0xc the staff index, +0x29 / +0x2a current / potential stars (0x7b62a0, 0x819620).
const SEARCHES: usize = 10;
const SEARCH_SIZE: usize = 0x2b0;
const ENTRY: usize = 0x2d;
const GET_SCOUT: usize = 0x7ae950; // PLAYER_SEARCH::get_scout(this) -> Staff* or 0

/// Score points of error for each point of judging below 20 (at most ±0.7·(20 − J)).
const EYE: f64 = 0.7;
/// The current-score percentiles where 2, 3, 4 and 5 stars begin.
const BANDS: [f64; 4] = [0.30, 0.70, 0.90, 0.98];

/// Where a value comes from: a player attribute shown as the game does (intrinsic, converted
/// always / out of role for a pure goalkeeper / in role only for a goalkeeper), one stored as
/// 1..20, one where lower is better, or a staff record personality value.
#[derive(Clone, Copy)]
enum V {
    Always(usize),
    Outfield(usize),
    Keeping(usize),
    Raw(usize),
    Worse(usize),
    Staff(usize),
}
use V::*;

/// Mad Scientist's weight set (13 Mar 2021), columns GK D DM M AM A WB, player record offsets.
#[rustfmt::skip]
const WEIGHTS: [(V, [u16; 7]); 48] = [
    (Always(0x1e),   [1000, 11000, 7000, 250, 20, 1150, 1000]),        // Anticipation
    (Outfield(0x43), [0, 0, 2000, 40000, 45000, 10, 2000]),            // Creativity
    (Outfield(0x23), [0, 0, 0, 10, 2500, 0, 55000]),                   // Crossing
    (Always(0x24),   [1200, 600, 400, 5000, 100, 90, 500]),            // Decisions
    (Outfield(0x26), [0, 0, 2000, 40000, 52000, 14000, 22000]),        // Dribbling
    (Outfield(0x27), [0, 0, 5, 5000, 30000, 60000, 15]),               // Finishing
    (Always(0x2b),   [0, 15000, 12500, 5, 5, 25000, 500]),             // Heading
    (Always(0x31),   [0, 0, 2000, 10000, 7500, 1000, 1250]),           // Long Shots
    (Outfield(0x32), [0, 15000, 1250, 100, 0, 0, 1250]),               // Marking
    (Outfield(0x33), [0, 0, 100, 18300, 35000, 65000, 550]),           // Off The Ball
    (Always(0x37),   [7500, 3500, 15000, 60000, 25000, 1000, 10000]),  // Passing
    (Always(0x38),   [0, 0, 0, 1, 1, 10, 0]),                          // Penalties
    (Always(0x39),   [12000, 65000, 55000, 3000, 0, 0, 55000]),        // Positioning
    (Always(0x3e),   [2500, 25000, 20000, 2000, 100, 85, 25000]),      // Tackling
    (Outfield(0x41), [0, 0, 0, 0, 0, 0, 12500]),                       // Throw Ins
    (Keeping(0x2a),  [60000, 0, 0, 0, 0, 0, 0]),                       // Handling
    (Keeping(0x35),  [30000, 0, 0, 10, 850, 5000, 0]),                 // One On Ones
    (Keeping(0x3a),  [30000, 0, 0, 0, 0, 0, 0]),                       // Reflexes
    (Raw(0x1b),      [6000, 4000, 2000, 18300, 5000, 12500, 7000]),    // Acceleration
    (Raw(0x1d),      [25000, 0, 0, 0, 0, 0, 0]),                       // Agility
    (Raw(0x1f),      [1, 525, 750, 1500, 225, 850, 285]),              // Balance
    (Raw(0x22),      [0, 0, 0, 1, 1, 0, 0]),                           // Corners
    (Raw(0x28),      [0, 0, 0, 50, 5000, 12500, 20]),                  // Flair
    (Worse(0x2d),    [3000, 300, 300, 300, 200, 300, 300]),            // Injury Proneness
    (Raw(0x2e),      [25000, 20000, 10000, 200, 80, 18000, 2000]),     // Jumping
    (Raw(0x34),      [50, 200, 2100, 3000, 4500, 800, 8000]),          // Natural Fitness
    (Raw(0x36),      [250, 15000, 7500, 1000, 25000, 15000, 15000]),   // Pace
    (Raw(0x29),      [0, 0, 0, 1, 1, 0, 0]),                           // Set Pieces
    (Raw(0x3c),      [5, 350, 3200, 3250, 5000, 350, 12000]),          // Stamina
    (Raw(0x3d),      [1500, 15000, 15000, 1200, 50, 3000, 2900]),      // Strength
    (Raw(0x40),      [4, 8, 100, 25000, 5000, 200, 1000]),             // Technique
    (Raw(0x44),      [3, 2500, 10000, 20000, 22000, 2500, 18000]),     // Work Rate
    (Staff(0x56),    [200, 500, 750, 1000, 1000, 1000, 350]),          // Adaptability
    (Raw(0x1c),      [0, 8000, 30000, 2000, 2, 1, 15000]),             // Aggression
    (Staff(0x57),    [0, 0, 0, 0, 0, 0, 0]),                           // Ambition
    (Raw(0x20),      [16000, 16000, 16000, 5000, 30, 12000, 1000]),    // Bravery
    (Raw(0x21),      [30000, 25000, 5000, 25000, 3000, 4000, 5000]),   // Consistency
    (Staff(0x58),    [6000, 1500, 1500, 18300, 22000, 12000, 2000]),   // Determination
    (Worse(0x25),    [300, 100, 50, 3, 2, 2, 20]),                     // Dirtiness
    (Raw(0x2c),      [30000, 25000, 5000, 20000, 3000, 5000, 5000]),   // Important Matches
    (Raw(0x2f),      [500, 160, 155, 150, 3, 50, 10]),                 // Influence
    (Staff(0x59),    [250, 300, 300, 500, 750, 1000, 200]),            // Loyalty
    (Staff(0x5a),    [30000, 1500, 1500, 3000, 2000, 5500, 1500]),     // Pressure
    (Staff(0x5b),    [9000, 4500, 4000, 8000, 4000, 500, 2000]),       // Professionalism
    (Staff(0x5c),    [0, 0, 0, 0, 0, 0, 0]),                           // Sportsmanship
    (Raw(0x3f),      [30, 100, 2500, 25000, 850, 0, 10000]),           // Teamwork
    (Staff(0x5d),    [2000, 2500, 2500, 2000, 1250, 250, 1250]),       // Temperament
    (Raw(0x42),      [0, 1250, 3000, 1500, 4000, 0, 4000]),            // Versatility
];

/// The 1..20 value the game shows for an intrinsic attribute (0x544380 in role, 0x5442f0 out of
/// role, 0x544410 for non-players), in the exe's order of operations.
pub(crate) fn shown(intrinsic: i8, ability: i16, in_role: bool) -> f64 {
    let ability = if in_role { f64::from(ability) } else { f64::from(ability) * 0.1 };
    let t = (ability + 2.0 * f64::from(intrinsic)) * 0.05 + 10.0;
    ((t * t * 0.1 + t) * (1.0 / 3.0) + 0.5).clamp(1.0, 20.0).trunc()
}

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

/// The values of `WEIGHTS` for a player record (70 bytes) and his staff record (0x6e bytes) at
/// current ability `ability`.
fn values(p: &[u8], s: &[u8], ability: i16) -> Vec<f64> {
    let gk = p[0xf] as i8;
    let value = |v: V| match v {
        Always(o) => shown(p[o] as i8, ability, true),
        Outfield(o) => shown(p[o] as i8, ability, gk != 20),
        Keeping(o) => shown(p[o] as i8, ability, gk >= 15),
        Raw(o) => f64::from(p[o].clamp(1, 20)),
        Worse(o) => f64::from(21 - p[o].clamp(1, 20)),
        Staff(o) => f64::from(s[o].clamp(1, 20)),
    };
    WEIGHTS.iter().map(|w| value(w.0)).collect()
}

/// The intrinsic score 0..100: Σ(v/20·w)/Σw·100 in the best column of the positions he plays
/// at 15 or more (central defenders D, full backs WB, wide midfielders AM), all columns if none.
fn intrinsic(p: &[u8], s: &[u8], ability: i16) -> f64 {
    let values = values(p, s, ability);
    let ok = |o: usize| p[o] as i8 >= 15;
    let side = ok(0x17) || ok(0x18);
    let fits = [
        ok(0xf),                            // GK
        (ok(0x10) || ok(0x11)) && ok(0x19), // D: sweeper or defender, central
        ok(0x12),                           // DM
        ok(0x13) && ok(0x19),               // M: central
        ok(0x14) || (ok(0x13) && side),     // AM, or a wide midfielder
        ok(0x15),                           // A
        ok(0x16) || (ok(0x11) && side),     // WB, or a full back
    ];
    let any = fits.contains(&true);
    (0..7).filter(|&c| fits[c] || !any).map(|c| column_score(&values, c)).fold(0.0, f64::max)
}

/// The score in one column of `WEIGHTS`.
fn column_score(values: &[f64], c: usize) -> f64 {
    let (k, w) = WEIGHTS.iter().zip(values).fold((0.0, 0.0), |(k, w), ((_, ws), v)| {
        (k + v / 20.0 * f64::from(ws[c]), w + f64::from(ws[c]))
    });
    k / w * 100.0
}

/// A slot label's column of `WEIGHTS`.
fn column(label: &[u8]) -> usize {
    match label {
        b"GK" => 0,
        b"SW" | b"DC" => 1,
        b"DL" | b"DR" => 6,
        b"MC" => 3,
        l if l.starts_with(b"DM") => 2,
        l if l.starts_with(b"WB") => 6,
        l if l.starts_with(b"F") || l.starts_with(b"ST") => 5,
        _ => 4, // attacking and wide midfielders
    }
}

/// The team report's scale, the same for every club (cm25, 2026-10-29: the best eleven average
/// 85.9 at Real Madrid, 73.5 Feyenoord, 67.4 Twente, 59.2 MVV): where 1.5, 2 .. 5 stars begin.
const HALF_STARS: [f64; 8] = [51.0, 56.0, 61.0, 66.0, 70.0, 74.0, 79.0, 84.0];

/// Half stars 2..=10 for a score.
pub(crate) fn halves(score: f64) -> u8 {
    2 + HALF_STARS.iter().filter(|&&c| score >= c).count() as u8
}

/// A player's score as an assistant sees him: for a slot, or at his best position. A poor judge
/// is off by another fixed amount per position, so he can also misjudge what suits a player.
pub(crate) unsafe fn seen_score(staff: usize, assistant: usize, label: Option<&[u8]>) -> Option<f64> {
    let (s, p) = records(staff)?;
    let eye = judging(assistant, false)?;
    let (score, kind) = match label {
        Some(l) => (column_score(&values(p, s, word(p, 5)), column(l)), 2 + column(l) as u64),
        None => (intrinsic(p, s, word(p, 5)), 0),
    };
    Some(score + error(rd::<i32>(assistant), rd::<i32>(staff), kind, eye))
}

/// How far off a judge can be, in score points: ±0.7·(20 − Judging Ability); 0 for a perfect eye.
pub(crate) unsafe fn margin(judge: usize) -> Option<f64> {
    judging(judge, false).map(|j| (20.0 - j).max(0.0) * EYE)
}

/// A club's best eleven's intrinsic score on average, no one's eye in it (0 with no squad).
pub(crate) unsafe fn strength(club: usize) -> f64 {
    let mut squad: Vec<f64> = (0..50).filter_map(|k| records(rd::<usize>(club + 0xd7 + 4 * k))).map(|(s, p)| intrinsic(p, s, word(p, 5))).collect();
    squad.sort_by(|a, b| b.total_cmp(a));
    squad.truncate(11);
    squad.iter().sum::<f64>() / squad.len().max(1) as f64
}

/// A club's three best players by the intrinsic score itself, no one's eye in it.
pub(crate) unsafe fn best_three(club: usize) -> Vec<usize> {
    let mut squad: Vec<(f64, usize)> = (0..50)
        .map(|k| rd::<usize>(club + 0xd7 + 4 * k))
        .filter(|&s| readable(s, 0x6e) && crate::plays_for(s, club))
        .filter_map(|s| records(s).map(|(st, p)| (intrinsic(p, st, word(p, 5)), s)))
        .collect();
    squad.sort_by(|a, b| b.0.total_cmp(&a.0));
    if crate::trace() {
        let top: Vec<String> = squad.iter().take(6).map(|p| format!("{} {:.1} ca {}", crate::latin1(&crate::staff_name(p.1)), p.0, records(p.1).map_or(0, |(_, r)| word(&r, 5)))).collect();
        crate::log(&format!("best three of {club:#x}: {top:?}"));
    }
    squad.into_iter().take(3).map(|p| p.1).collect()
}

/// A player's future score as a judge sees him, by his Judging Potential.
pub(crate) unsafe fn seen_future(staff: usize, judge: usize) -> Option<f64> {
    let (s, p) = records(staff)?;
    Some(intrinsic(p, s, future_ability(p, s)) + error(rd::<i32>(judge), rd::<i32>(staff), 1, judging(judge, true)?))
}

/// The ability the future score uses: CA moved toward PA, fully at 20 and under, not from 28
/// (age at staff +0x18).
fn future_ability(p: &[u8], s: &[u8]) -> i16 {
    let (ca, pa) = (word(p, 5), word(p, 7));
    let g = ((28.0 - f64::from(word(s, 0x18))) / 8.0).clamp(0.0, 1.0);
    ca + (f64::from(pa.max(ca) - ca) * g) as i16
}

/// 1..5 stars on cut points (the scout's club's, else the whole game's).
fn stars(score: f64, cuts: &[f64; 4]) -> u8 {
    1 + cuts.iter().filter(|&&c| score >= c).count() as u8
}

/// A scout's fixed error on a player, in score points (`kind` 0 current, 1 future): the same on
/// every redraw and after reloading, different per scout.
fn error(scout: i32, player: i32, kind: u64, judging: f64) -> f64 {
    // splitmix64 of the two ids
    let mut h = ((scout as u32 as u64) << 32 | player as u32 as u64) ^ kind.wrapping_mul(0x9e3779b97f4a7c15);
    h = (h ^ h >> 30).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ h >> 27).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    let u = (h >> 11) as f64 / (1u64 << 53) as f64;
    (2.0 * u - 1.0) * (20.0 - judging) * EYE
}

/// Cut points from sorted current scores.
fn cut_points(sorted: &[f64]) -> [f64; 4] {
    BANDS.map(|q| sorted[((sorted.len() - 1) as f64 * q) as usize])
}

unsafe fn bytes(addr: usize, len: usize) -> &'static [u8] {
    std::slice::from_raw_parts(addr as *const u8, len)
}

/// A staff record and its player record, when it has one.
unsafe fn records(staff: usize) -> Option<(&'static [u8], &'static [u8])> {
    if !readable(staff, 0x6e) {
        return None;
    }
    let p = rd::<usize>(staff + 0x61);
    readable(p, 70).then(|| (bytes(staff, 0x6e), bytes(p, 70)))
}

/// The cut points over every player in the staff table.
unsafe fn cuts() -> Option<[f64; 4]> {
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    if count <= 0 || !readable(base, count as usize * 0x6e) {
        return None;
    }
    let mut all: Vec<f64> =
        (0..count as usize).filter_map(|k| records(base + k * 0x6e)).map(|(s, p)| intrinsic(p, s, word(p, 5))).collect();
    if all.is_empty() {
        return None;
    }
    all.sort_by(f64::total_cmp);
    let c = cut_points(&all);
    log(&format!("scout stars: {} players, cut points {c:.1?}", all.len()));
    Some(c)
}

/// Cut points against a club's own squad (club +0xd7: 50 staff), by its best eleven's current
/// scores, best first: 5 stars as good as its second best, 4 its fifth, 3 its eleventh (first
/// team level), 2 within 8 points of that. None for a squad of fewer than 11 players.
unsafe fn club_cuts(club: usize) -> Option<[f64; 4]> {
    if !readable(club + 0xd7, 200) {
        return None;
    }
    let mut squad: Vec<f64> = (0..50).filter_map(|k| records(rd::<usize>(club + 0xd7 + 4 * k))).map(|(s, p)| intrinsic(p, s, word(p, 5))).collect();
    if squad.len() < 11 {
        return None;
    }
    squad.sort_by(|a, b| b.total_cmp(a));
    Some([squad[10] - 8.0, squad[10], squad[4], squad[1]])
}

/// A scout's Judging Ability (or Judging Potential), 1..20 as shown: non-player record staff
/// +0x69, judgement +0x17, judging potential +0x18, its CA +4 (0x6e86d0).
/// While set, every judge reads perfectly (20): a national team's pick, report and pool, whose
/// stand-in judge the manager cannot choose (user, 2026-11-07). Set only through `perfect_if`.
static mut PERFECT: bool = false;

/// Perfect judging for as long as the returned guard lives, when `on`.
pub(crate) struct Perfect(bool);
impl Drop for Perfect {
    fn drop(&mut self) {
        unsafe { PERFECT = self.0 };
    }
}
pub(crate) unsafe fn perfect_if(on: bool) -> Perfect {
    let before = PERFECT;
    PERFECT = before || on;
    Perfect(before)
}
pub(crate) unsafe fn is_perfect() -> bool {
    PERFECT
}

unsafe fn judging(scout: usize, potential: bool) -> Option<f64> {
    let np = rd::<usize>(scout + 0x69);
    if PERFECT && readable(np, 68) {
        return Some(20.0);
    }
    readable(np, 68).then(|| shown(rd::<i8>(np + if potential { 0x18 } else { 0x17 }), rd::<i16>(np + 4), true))
}

// ponytail: the cut points are kept per session address; a new day's abilities count from the
// next session at another address (a few tenths of a point between days).
static mut CUTS: Option<(usize, [f64; 4])> = None;

/// Called by the search screen's results hook before the game builds the list: rewrites the
/// stars of every rated entry (either star byte above 0) in the session's ten searches, as each
/// search's scout sees the player. Unscouted entries and searches without a scout keep the game's.
pub(crate) unsafe fn restar(session: usize) {
    // ponytail: a network client's searches live in the host's game; it keeps the game's stars
    if crate::remote::CLIENT || !crate::on(crate::SCOUT_STARS) || !readable(session, 1 + SEARCHES * SEARCH_SIZE) {
        return;
    }
    // (the whole game's cut points only when a scout's club has no squad of eleven: they read
    // every player, 80 ms)
    let world = || match CUTS {
        Some((at, c)) if at == session => Some(c),
        _ => {
            let c = cuts()?;
            CUTS = Some((session, c));
            Some(c)
        }
    };
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    RATED.clear();
    for k in 0..SEARCHES {
        let search = session + 1 + k * SEARCH_SIZE;
        let scout = game::<extern "thiscall" fn(usize) -> usize>(GET_SCOUT)(search);
        if !readable(scout, 0x6e) {
            continue;
        }
        let (Some(eye_now), Some(eye_future)) = (judging(scout, false), judging(scout, true)) else { continue };
        // (against the scout's club, staff +0x39, as the game's own stars are; the whole game's
        // cut points when it has no squad of eleven)
        let Some(cuts) = club_cuts(rd::<usize>(scout + 0x39)).or_else(world) else { continue };
        let (list, n) = (rd::<usize>(search + 0x1ee), rd::<i32>(search + 0x1f6));
        if n <= 0 || !readable(list, n as usize * ENTRY) {
            continue;
        }
        let scout_id = rd::<i32>(scout);
        for e in (0..n as usize).map(|i| list + i * ENTRY) {
            let (now, future) = ((e + 0x29) as *mut u8, (e + 0x2a) as *mut u8);
            let who = rd::<i32>(e + 0xc);
            if (*now == 0 && *future == 0) || who < 0 || who >= count {
                continue;
            }
            let Some((s, p)) = records(base + who as usize * 0x6e) else { continue };
            let id = rd::<i32>(s.as_ptr() as usize);
            let seen = intrinsic(p, s, word(p, 5)) + error(scout_id, id, 0, eye_now);
            let seen_future = intrinsic(p, s, future_ability(p, s)) + error(scout_id, id, 1, eye_future);
            *now = stars(seen, &cuts);
            *future = stars(seen_future, &cuts).max(*now);
            if RATED.len() < 2048 {
                RATED.push((latin1(&staff_name(base + who as usize * 0x6e)).into_owned(), seen.clamp(0.0, 100.0).round() as i32, ((20.0 - eye_now).max(0.0) * EYE).round() as i32));
            }
        }
    }
}

/// The rated players of the session's searches as their scouts see them: full name, the CM
/// intrinsic score in %, the scout's margin (user, 2026-11-07: the rating after the name).
static mut RATED: Vec<(String, i32, i32)> = Vec::new();
/// This frame's names in a search list that have a rating: cell, font, name as drawn.
static mut RATED_NAMES: Vec<((i32, i32, i32, i32), u32, Vec<u8>)> = Vec::new();

/// A text drawn this frame: a search list's "Surname, First" of a rated player is noted for the
/// flip, which writes his rating after it.
#[allow(static_mut_refs)]
pub(crate) unsafe fn note_name(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, t: &[u8]) {
    // (the search screen's lists only: a squad list's "Bertasius, D" of a scouted player is no
    // place for it, user 2026-11-07)
    if RATED.is_empty() || RATED_NAMES.len() >= 64 || y2 - y1 > 40 || !t.contains(&b',') || !crate::comp::screen_ends_with(b"Search") {
        return;
    }
    if rating_of(t).is_some() {
        RATED_NAMES.push(((x1, y1, x2, y2), font, t.to_vec()));
    }
}

/// "Chistyakov, Dmitriy" -> the rating of Dmitriy Chistyakov.
#[allow(static_mut_refs)]
unsafe fn rating_of(t: &[u8]) -> Option<(i32, i32)> {
    let t = latin1(t);
    let (surname, first) = t.split_once(", ")?;
    // (a whole first name, not an initial: "D" is anyone's)
    if first.chars().count() < 2 {
        return None;
    }
    let full = format!("{first} {surname}");
    RATED.iter().find(|r| r.0 == full || (r.0.ends_with(surname) && r.0.contains(first))).map(|r| (r.1, r.2))
}

/// The flip pass: "78%" (and "±6" for a scout who can be off) after each rated name of this frame.
#[allow(static_mut_refs)]
pub(crate) unsafe fn flip() {
    for ((x1, y1, x2, y2), font, t) in std::mem::take(&mut RATED_NAMES) {
        // (the score as the scout sees him, his error in it but not shown: user, 2026-11-07, "that
        // makes it fun")
        let Some((pct, _)) = rating_of(&t) else { continue };
        let bytes = format!("{pct}%").into_bytes();
        let x = x1 + 2 + text_width(font, &t) + 10;
        if x + text_width(font, &bytes) > x2 - 4 {
            continue;
        }
        let c = if pct >= 75 { [0x4c, 0xc0, 0x7a] } else if pct >= 60 { crate::GOLD } else { [0xe0, 0x88, 0x58] };
        text_at(x, y1, y2, font, c, &bytes, 0);
    }
}

#[cfg(all(test, feature = "save_check"))] // needs a save and a crate that are not in the repository
mod tests {
    use super::*;
    use cm0102::{save::SaveFile, Database, Field, Player, Staff};

    /// The calibration's mapping by field name (`scout_stars.md`), in `WEIGHTS` order.
    fn by_name(p: &Player, s: &Staff) -> Vec<f64> {
        let a = p.shown_attributes(s.determination);
        let h = p.hidden_attributes();
        let v = [
            a[3], a[6], a[7], a[8], a[10], a[11], a[14], a[17], a[18], a[19], a[21], h[8], a[22], a[27], h[10], a[13],
            h[7], a[23], a[0], a[2], a[4], h[1], a[12], 21u8.saturating_sub(h[4].min(20)), a[16], h[6], a[20], a[24],
            a[25], a[26], a[29], a[30], s.adaptability, a[1], s.ambition, a[5], h[0], s.determination,
            21u8.saturating_sub(h[2].min(20)), h[3], a[15], s.loyalty, s.pressure, s.professionalism, s.sportsmanship,
            a[28], s.temperament, h[11],
        ];
        v.map(|x| f64::from(x.clamp(1, 20))).to_vec()
    }

    /// The calibration's score by field name.
    fn score_by_name(p: &Player, s: &Staff) -> f64 {
        let ok = |x: i8| x >= 15;
        let side = ok(p.left_side) || ok(p.right_side);
        let fits = [
            ok(p.goalkeeper),
            (ok(p.sweeper) || ok(p.defender)) && ok(p.central),
            ok(p.defensive_midfielder),
            ok(p.midfielder) && ok(p.central),
            ok(p.attacking_midfielder) || (ok(p.midfielder) && side),
            ok(p.attacker),
            ok(p.wing_back) || (ok(p.defender) && side),
        ];
        let v = by_name(p, s);
        let any = fits.contains(&true);
        (0..7)
            .filter(|&c| fits[c] || !any)
            .map(|c| {
                let k: f64 = WEIGHTS.iter().zip(&v).map(|((_, w), v)| v / 20.0 * f64::from(w[c])).sum();
                k / WEIGHTS.iter().map(|(_, w)| f64::from(w[c])).sum::<f64>() * 100.0
            })
            .fold(0.0, f64::max)
    }

    /// The team report's half stars and slot columns.
    #[test]
    fn report_stars() {
        assert_eq!([50.9, 51.0, 59.2, 66.0, 73.9, 84.0, 94.9].map(halves), [2, 3, 4, 6, 7, 10, 10]);
        let labels: [&[u8]; 9] = [b"GK", b"DC", b"DL", b"DMC", b"MC", b"MR", b"AML", b"FC", b"SW"];
        assert_eq!(labels.map(column), [0, 1, 6, 2, 3, 4, 4, 5, 1]);
    }

    /// Over every player of VVV.sav: the record offsets give the same values and scores as the
    /// named fields, the future score is never below the current one, and the cut points rise.
    /// Skipped without the save.
    #[test]
    fn offsets_match_the_named_fields() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../VVV.sav");
        let Ok(mut save) = SaveFile::open(std::path::Path::new(path)) else { return eprintln!("skipped: no {path}") };
        let db = Database::load_save(&mut save).unwrap();
        let mut scores = vec![];
        for s in &db.staff {
            let Some(p) = usize::try_from(s.player).ok().and_then(|i| db.players.get(i)) else { continue };
            let (mut pb, mut sb) = (vec![], vec![]);
            p.write(&mut pb);
            s.write(&mut sb);
            assert_eq!(values(&pb, &sb, p.current_ability), by_name(p, s), "staff {}", s.id);
            let now = intrinsic(&pb, &sb, p.current_ability);
            assert!((now - score_by_name(p, s)).abs() < 1e-9, "staff {}: {now} vs {}", s.id, score_by_name(p, s));
            assert!(intrinsic(&pb, &sb, future_ability(&pb, &sb)) >= now, "staff {}", s.id);
            scores.push(now);
        }
        scores.sort_by(f64::total_cmp);
        let c = cut_points(&scores);
        eprintln!("{} players, cut points {c:.2?}", scores.len());
        // VVV.sav is re-saved now and then (48.4 / 61.8 / 69.8 / 77.5 on 28 Oct, 48.6 / 62.1 / 70.2 / 78.0 on 29 Oct)
        assert!(c.windows(2).all(|w| w[0] < w[1]) && c[0] > 40.0 && c[3] < 85.0, "{c:?}");
        assert_eq!((stars(c[3], &c), stars(c[0] - 0.1, &c)), (5, 1));
        assert!(error(1, 2, 0, 20.0) == 0.0 && error(1, 2, 0, 1.0).abs() <= 19.0 * EYE);
    }
}
