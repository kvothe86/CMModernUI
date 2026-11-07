//! A club's last results (switch `form_timeline`, the user's wish of 2026-11-07): read through
//! the game's FIXTURE_MANAGER::find_fixtures (0x5acbe0, as the club screen asks it, 0x454bdf),
//! drawn as a strip in the club header (opponent's crest over a W / D / L pill with the score) and
//! used by the match card (`card.rs`).
//!
//! A fixture (the list holds 0x4f-byte copies, freed with the list): +0xc the home club's index,
//! +0x10 the away club's, +0x28 the year, +0x2a the day of the year (the rebuild's
//! fixture_moves.rs `put_date`), +0x43 / +0x44 the home and away goals, 0xff before it is played
//! (the club screen's own test of +0x43, 0x454c0d; traced 2026-11-07 on the user's game: +0x2e /
//! +0x2f, first guessed, are part of another date).

use crate::*;

const FIXTURE_MANAGER: usize = 0xae2a58;
const FIND_FIXTURES: usize = 0x5acbe0; // thiscall(this, start day, start year, end day, end year, club index, List*, comp -1, type -2, side -1)
const LIST_NEW: usize = 0x68d020; // LLIST::LLIST, thiscall
const LIST_CLEAR: usize = 0x68d040; // LLIST teardown: frees its nodes and the copies
const GET_PREV: usize = 0x68d1d0;
const TODAY: usize = 0xae2c90;

/// One played match of a club: the opponent's club index, goals for and against, at home.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Result_ {
    pub opponent: i32,
    pub goals_for: i32,
    pub against: i32,
    pub home: bool,
    pub day: i32, // year * 366 + day of the year
}

impl Result_ {
    /// 'W', 'D' or 'L'.
    pub fn letter(&self) -> u8 {
        match self.goals_for.cmp(&self.against) {
            std::cmp::Ordering::Greater => b'W',
            std::cmp::Ordering::Equal => b'D',
            std::cmp::Ordering::Less => b'L',
        }
    }
}

/// The last `max` played matches of the club (index), newest first, from the last 200 days.
/// Kept per club and day: the game is asked once a day for a club.
#[allow(static_mut_refs)]
pub unsafe fn results(club: i32, max: usize) -> Vec<Result_> {
    static mut KEPT: Vec<(i32, i32, Vec<Result_>)> = Vec::new();
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let today = year * 366 + day;
    if let Some(k) = KEPT.iter().find(|k| k.0 == club && k.1 == today) {
        return k.2.iter().take(max).copied().collect();
    }
    let (start_day, start_year) = if day >= 200 { (day - 200, year) } else { (day + 366 - 200, year - 1) };
    let out = played(club, (start_day, start_year), (day, year), 10);
    KEPT.retain(|k| k.1 == today);
    if KEPT.len() < 64 {
        KEPT.push((club, today, out.clone()));
    }
    out.into_iter().take(max).collect()
}

/// The club's (index) played matches between two dates (day of the year, year), newest first, at
/// most `max` (asked of the game each time: `results` keeps a day's).
pub unsafe fn played(club: i32, (start_day, start_year): (i32, i32), (day, year): (i32, i32), max: usize) -> Vec<Result_> {
    let fm = rd::<usize>(FIXTURE_MANAGER);
    let mut out = Vec::new();
    if club >= 0 && readable(fm, 16) && (1900..2200).contains(&year) && (1900..2200).contains(&start_year) {
        let mut list = [0u8; 64];
        let l = list.as_mut_ptr() as usize;
        game::<extern "thiscall" fn(usize)>(LIST_NEW)(l);
        game::<extern "thiscall" fn(usize, i32, i32, u32, u32, i32, usize, i32, i32, i32) -> i32>(FIND_FIXTURES)(fm, start_day, start_year, day as u32, year as u32, club, l, -1, -2, -1);
        // newest first: the cursor at the tail, then back (as history::newest)
        ((l + 10) as *mut u32).write_unaligned(((l + 6) as *const u32).read_unaligned());
        let prev = game::<extern "thiscall" fn(usize) -> usize>(GET_PREV);
        let mut seen = 0;
        while let Some(f) = Some(prev(l)).filter(|&f| f != 0) {
            seen += 1;
            if seen > 1000 || !readable(f, 0x4f) {
                break;
            }
            let (h, a) = (rd::<i32>(f + 0xc), rd::<i32>(f + 0x10));
            // (the score at +0x43 / +0x44, 0xff before it is played: traced on the user's game,
            // Dorking 3-0 on 1 January 2029 = 03 00; +0x45 / +0x46 seem the half-time score)
            let (hg, ag) = (rd::<u8>(f + 0x43), rd::<u8>(f + 0x44));
            let played = hg != 0xff && ag != 0xff;
            if trace() && out.len() < 12 {
                let raw: Vec<u8> = (0x24..0x4f).map(|o| rd::<u8>(f + o)).collect();
                log(&format!("form {club}: home {h} away {a} day {} {} played {played} bytes 0x24.. {raw:02x?}", rd::<i16>(f + 0x2a), rd::<i16>(f + 0x28)));
            }
            if !played || hg == 0xff || ag == 0xff || hg > 40 || ag > 40 || (h != club && a != club) {
                continue;
            }
            let home = h == club;
            out.push(Result_ {
                opponent: if home { a } else { h },
                goals_for: if home { hg } else { ag } as i32,
                against: if home { ag } else { hg } as i32,
                home,
                day: rd::<i16>(f + 0x28) as i32 * 366 + rd::<i16>(f + 0x2a) as i32,
            });
            if out.len() >= max {
                break;
            }
        }
        game::<extern "thiscall" fn(usize)>(LIST_CLEAR)(l);
    }
    out.sort_by_key(|r| -r.day);
    out
}

/// The club's (index) fixture of today, played or not: the home and away club indices.
pub unsafe fn today_fixture(club: i32) -> Option<(i32, i32)> {
    let fm = rd::<usize>(FIXTURE_MANAGER);
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    if club < 0 || !readable(fm, 16) || !(1900..2200).contains(&year) {
        return None;
    }
    let mut list = [0u8; 64];
    let l = list.as_mut_ptr() as usize;
    game::<extern "thiscall" fn(usize)>(LIST_NEW)(l);
    game::<extern "thiscall" fn(usize, i32, i32, u32, u32, i32, usize, i32, i32, i32) -> i32>(FIND_FIXTURES)(fm, day, year, day as u32, year as u32, club, l, -1, -2, -1);
    ((l + 10) as *mut u32).write_unaligned(((l + 6) as *const u32).read_unaligned());
    let f = game::<extern "thiscall" fn(usize) -> usize>(GET_PREV)(l);
    let out = (f != 0 && readable(f, 0x4f)).then(|| (rd::<i32>(f + 0xc), rd::<i32>(f + 0x10))).filter(|m| m.0 == club || m.1 == club);
    game::<extern "thiscall" fn(usize)>(LIST_CLEAR)(l);
    out
}

/// A result's colours: W green, D blue-grey, L red.
pub fn colour(letter: u8) -> [u8; 3] {
    match letter {
        b'W' => [0x3e, 0xa8, 0x6a],
        b'D' => [0x6a, 0x7c, 0x96],
        _ => [0xd0, 0x50, 0x48],
    }
}

/// The club header's form: the last five as small W / D / L pills in a row centred on `cx`
/// (y1..y2), oldest at the left; no crests or scores (user, 2026-11-07: ten crests and scores
/// beside the name were too much; those are on the match card).
pub unsafe fn mini(club: usize, cx: i32, y1: i32, y2: i32) {
    let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
    let mut last = results(index, 5);
    if last.is_empty() || y2 - y1 < 8 {
        return;
    }
    last.reverse();
    let h = y2 - y1;
    let (w, gap) = (h * 14 / 10, 4);
    let font = SMALL_FONT.unwrap_or(0xffff0001);
    let x0 = cx - (last.len() as i32 * (w + gap) - gap) / 2;
    for (k, r) in last.iter().enumerate() {
        let x = x0 + k as i32 * (w + gap);
        pill(x, y1, x + w, y2, colour(r.letter()));
        text_at(x + w / 2, y1, y2, font, [0xf4, 0xf6, 0xf8], &[r.letter()], 1);
    }
}
