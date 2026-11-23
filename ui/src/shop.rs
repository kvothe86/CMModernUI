//! The club shop and merchandise (user 2026-11-23): what the game pays each club in merchandise
//! every Sunday (`Finance::merchandise` 0x59b7e0: attendance x reputation x a rate, the chairman's
//! business sense, 95..105%, into the finance record's merchandising line) gets a share more:
//! the club shop's level and the items on sale, and the week's results (kept small: a win +3%, a
//! draw +1%, a defeat -2%, at most 8% either way). A human club builds its shop and buys its items
//! on the stadium page (`stadium.rs`, kinds SHOP and ITEM; the diary's `M` lines, kept apart from
//! the stadium record so a new stadium keeps them); the computer's clubs have them by reputation.
//! The extra goes into the same merchandising line, so the game's own finance screens show it.
use crate::*;

/// The shop's levels: name, share more merchandise, price (game units), days to build.
pub const LEVELS: [(&str, i32, i64, i32); 4] = [
    ("No club shop", 0, 0, 0),
    ("Club shop", 30, 1_500_000, 120),
    ("Megastore", 45, 4_000_000, 180),
    ("Megastore and webshop", 60, 2_000_000, 90),
];
/// The items a club can put on sale: name, share more merchandise, price (game units).
pub const ITEMS: [(&str, i32, i64); 4] = [("Away shirt", 8, 50_000), ("Scarves and caps", 4, 25_000), ("Name and number", 8, 100_000), ("Kids' kits", 6, 40_000)];

const MERCHANDISE: usize = 0x59b7e0; // thiscall(finance table, club), ret 4
static mut ORIG: usize = 0;

pub unsafe fn install() {
    // (push ecx, push ebx, push esi, mov esi, [esp+0x10]: seven bytes moved)
    ORIG = hook_entry(MERCHANDISE, &[0x51, 0x53, 0x56, 0x8b, 0x74, 0x24, 0x10], merchandise as usize);
    log(&format!("shop: merchandise hook {}", ORIG != 0));
}

/// The share more merchandise a shop and its items give.
pub fn pct(level: u8, items: u8) -> i32 {
    LEVELS[level.min(3) as usize].1 + (0..4).filter(|k| items & 1 << k != 0).map(|k| ITEMS[k].1).sum::<i32>()
}

/// The human managers' clubs today (asked once a game day).
#[allow(static_mut_refs)]
unsafe fn humans() -> &'static [usize] {
    static mut DAY: i32 = -1;
    static mut CLUBS: Vec<usize> = Vec::new();
    let day = rd::<i16>(0xae2c92) as i32 * 366 + rd::<i16>(0xae2c90) as i32;
    if day != DAY {
        DAY = day;
        CLUBS = challenge::human_list().iter().map(|(staff, _)| rd::<usize>(staff + 0x39)).collect();
    }
    &CLUBS
}

/// A club's shop level and items: a human club's as it bought them, a computer club's by its
/// reputation (club +0x80): a club shop at 6,000 and up, an away shirt and scarves at 3,000.
#[allow(static_mut_refs)]
pub unsafe fn kit(club: usize) -> (u8, u8) {
    if humans().contains(&club) {
        let Some(seed) = finreport::seed_of(club) else { return (0, 0) };
        return diary::MERCH.iter().find(|m| m.0 == seed).map_or((0, 0), |m| (m.1, m.2));
    }
    match rd::<i16>(club + 0x80) as i32 {
        8500.. => (3, 0b1111),
        6000.. => (1, 0),
        3000.. => (0, 0b0011),
        _ => (0, 0),
    }
}

/// The week's results: a win +3, a draw +1, a defeat -2, at most 8 either way.
pub unsafe fn results_pct(club: usize) -> i32 {
    let base = rd::<usize>(CLUB_TABLE);
    if club < base {
        return 0;
    }
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 7 { (day - 7, year) } else { (day + 359, year - 1) };
    let index = ((club - base) / CLUB_SIZE) as i32;
    let sum: i32 = form::played(index, start, (day, year), 8)
        .iter()
        .filter(|r| !r.friendly)
        .map(|r| match r.letter() {
            b'W' => 3,
            b'D' => 1,
            _ => -2,
        })
        .sum();
    sum.clamp(-8, 8)
}

/// { 0x59b7e0 } A club's Sunday merchandise: the game's, then the share more on the same line.
unsafe extern "thiscall" fn merchandise(table: usize, club: usize) {
    let before = finreport::merch_month(club);
    game::<extern "thiscall" fn(usize, usize)>(ORIG)(table, club);
    if !on(STADIUM) {
        return;
    }
    let (Some(before), Some(after)) = (before, finreport::merch_month(club)) else { return };
    let week = after as i64 - before as i64;
    if week <= 0 {
        return;
    }
    let (level, items) = kit(club);
    let share = pct(level, items) + results_pct(club);
    let extra = week * share as i64 / 100;
    if extra != 0 {
        finreport::add_merch(club, extra);
    }
    if humans().contains(&club) {
        log(&format!("shop: week's merchandise {week}, {share:+}% = {extra} more (club {club:#x})"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shops_share() {
        assert_eq!(pct(0, 0), 0);
        assert_eq!(pct(1, 0), 30);
        assert_eq!(pct(3, 0b1111), 60 + 26);
        assert_eq!(pct(2, 0b0101), 45 + 8 + 8);
    }
}
