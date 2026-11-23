//! What cmui writes down as the game goes on, on the host or in single player (user 2026-11-22):
//! each human club's bank balance at the end of every month, for the P&L report's chart and
//! forecast (the game itself keeps only this month, last month, this year and last year).
//!
//! One file per game, beside CM Challenge's league file: `cmui_diary_<game>.tsv`, a line
//! `B<TAB>club<TAB>year<TAB>month<TAB>balance` (club: its index in the club table; month 1..12;
//! the balance in the game's units). Written once a day; the month's last day's reading stands.
//! A club seen for the first time gets the two months before from the game's own figures (the
//! balance less this month's result, less last month's), so its chart has a line at once.
use crate::*;

const TODAY: usize = 0xae2c90; // Date: i16 day of the year, i16 year

static mut FILE: String = String::new();
static mut BALANCES: Vec<(usize, i32, i32, i64)> = Vec::new();
/// The human clubs' players' 31 attributes by month (the growth graph): staff id, year, month.
static mut ATTRS: Vec<(i32, i32, i32, [u8; 31])> = Vec::new();
/// The scouts' monthly assignments: club seed, year, month, the line looked for, the scout's id,
/// the three players' staff ids (`S` lines).
static mut SCOUTED: Vec<(usize, i32, i32, usize, i32, Vec<i32>)> = Vec::new();

/// The month's assignment of a club's chief scout (user 2026-11-22, "per maand"): the line where
/// the club's best player is weakest (`KEY_ATTRIBUTES`), and three players for it who look better
/// through the scout's eye (`scout::seen_score`), 17 to 31, worth no more than the transfer funds
/// and a quarter (or the squad's average value without funds), at no human club, not suggested in
/// the three months before. The scout: of the club's seven (+0x1b3), the best judge.
unsafe fn assign(club: usize, seed: usize, year: i32, month: i32) -> Option<(usize, i32, Vec<i32>)> {
    let scout = (0..7).map(|k| rd::<usize>(club + 0x1b3 + 4 * k)).filter(|&s| readable(s, 0x6e)).filter_map(|s| scout::margin(s).map(|m| (m, s))).min_by(|a, b| a.0.total_cmp(&b.0))?.1;
    let squad: Vec<usize> = challenge::squad_of(club).into_iter().filter(|&s| readable(rd::<usize>(s + 0x61), 70) && plays_for(s, club)).collect();
    // the club's best at each line, as the scout sees him
    let best = |line: usize| squad.iter().filter(|&&s| key_line(rd::<usize>(s + 0x61)) == line).filter_map(|&s| scout::seen_score(s, scout, None)).fold(0.0f64, f64::max);
    let line = (0..KEY_ATTRIBUTES.len()).min_by(|&a, &b| best(a).total_cmp(&best(b)))?;
    let bar = best(line);
    let funds = finreport::figures(club).map_or(0, |f| f.2);
    let values: Vec<i64> = squad.iter().map(|&s| ((s + 0x52) as *const i32).read_unaligned() as i64).collect();
    let limit = (funds * 5 / 4).max(values.iter().sum::<i64>() / values.len().max(1) as i64);
    #[allow(static_mut_refs)]
    let before: Vec<i32> = SCOUTED.iter().filter(|a| a.0 == seed && (a.1 * 12 + a.2) >= year * 12 + month - 3).flat_map(|a| a.5.clone()).collect();
    let humans: Vec<usize> = challenge::human_list().iter().map(|h| rd::<usize>(h.0 + 0x39)).collect();
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT).max(0) as usize);
    let (day, today_year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let mut found: Vec<(f64, i32)> = Vec::new();
    for k in 0..count {
        let s = base + k * 0x6e;
        let player = rd::<usize>(s + 0x61);
        if player == 0 || !readable(player, 70) || key_line(player) != line {
            continue;
        }
        let here = rd::<usize>(s + 0x39);
        if here == club || humans.contains(&here) || before.contains(&rd::<i32>(s)) {
            continue;
        }
        let age = dash::age_of(s, day, today_year);
        let value = ((s + 0x52) as *const i32).read_unaligned() as i64;
        if !(17..=31).contains(&age) || value > limit {
            continue;
        }
        if let Some(seen) = scout::seen_score(s, scout, None).filter(|&v| v > bar) {
            found.push((seen, rd::<i32>(s)));
        }
    }
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    Some((line, rd::<i32>(scout), found.into_iter().take(3).map(|f| f.1).collect()))
}

/// A club's assignments, newest first: (year, month, line, scout id, staff ids).
#[allow(static_mut_refs)]
pub unsafe fn scouted(seed: usize) -> Vec<(i32, i32, usize, i32, Vec<i32>)> {
    let mut out: Vec<(i32, i32, usize, i32, Vec<i32>)> = SCOUTED.iter().filter(|a| a.0 == seed).map(|a| (a.1, a.2, a.3, a.4, a.5.clone())).collect();
    out.sort_by_key(|a| std::cmp::Reverse((a.0, a.1)));
    out
}

fn scouted_line(a: &(usize, i32, i32, usize, i32, Vec<i32>)) -> String {
    format!("S\t{}\t{}\t{}\t{}\t{}\t{}\n", a.0, a.1, a.2, a.3, a.4, a.5.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))
}

fn parse_scouted(text: &str) -> Vec<(usize, i32, i32, usize, i32, Vec<i32>)> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 7 || f[0] != "S" {
                return None;
            }
            Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?, f[4].parse().ok()?, f[5].parse().ok()?, f[6].split(',').filter_map(|x| x.parse().ok()).collect()))
        })
        .collect()
}

/// The attributes that show a player's growth, by his best line (user 2026-11-22: "belangrijke
/// stats", not CA): the player record's rating for that line, and four attributes (indexes into
/// ATTRIBUTE_NAMES).
pub const KEY_ATTRIBUTES: [(&str, usize, [usize; 4]); 6] = [
    ("Goalkeeper", 15, [13, 23, 22, 3]),     // Handling, Reflexes, Positioning, Anticipation
    ("Defender", 17, [27, 18, 14, 22]),      // Tackling, Marking, Heading, Positioning
    ("Defensive midfielder", 18, [27, 21, 22, 30]), // Tackling, Passing, Positioning, Work Rate
    ("Midfielder", 19, [21, 6, 29, 25]),     // Passing, Creativity, Technique, Stamina
    ("Attacking midfielder", 20, [6, 10, 21, 29]), // Creativity, Dribbling, Passing, Technique
    ("Forward", 21, [11, 19, 20, 14]),       // Finishing, Off The Ball, Pace, Heading
];

/// A player's line for the growth graph: the best of the six ratings (player record +15..+21).
pub unsafe fn key_line(player: usize) -> usize {
    (0..KEY_ATTRIBUTES.len()).max_by_key(|&k| rd::<i8>(player + KEY_ATTRIBUTES[k].1)).unwrap_or(5)
}

/// Month 1..12 of a day of the year (0-based, 29 February counted in a leap year).
pub fn month_of(day: i32, year: i32) -> i32 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut left = day.max(0);
    for (k, days) in [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31].into_iter().enumerate() {
        let days = days + (leap && k == 1) as i32;
        if left < days {
            return k as i32 + 1;
        }
        left -= days;
    }
    12
}

fn month_before(year: i32, month: i32) -> (i32, i32) {
    if month == 1 { (year - 1, 12) } else { (year, month - 1) }
}

fn parse(text: &str) -> Vec<(usize, i32, i32, i64)> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() == 5 && f[0] == "B").then(|| Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?, f[4].parse().ok()?)))?
        })
        .collect()
}

/// The file's `A` lines: `A<TAB>staff id<TAB>year<TAB>month<TAB>31 values, comma separated`.
fn parse_attrs(text: &str) -> Vec<(i32, i32, i32, [u8; 31])> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 5 || f[0] != "A" {
                return None;
            }
            let v: Vec<u8> = f[4].split(',').filter_map(|x| x.parse().ok()).collect();
            Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?, v.try_into().ok()?))
        })
        .collect()
}

fn attrs_line(r: &(i32, i32, i32, [u8; 31])) -> String {
    format!("A\t{}\t{}\t{}\t{}\n", r.0, r.1, r.2, r.3.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))
}

/// Sets a month's balance; whether it changed.
fn set(rows: &mut Vec<(usize, i32, i32, i64)>, club: usize, year: i32, month: i32, balance: i64) -> bool {
    match rows.iter_mut().find(|r| r.0 == club && r.1 == year && r.2 == month) {
        Some(r) if r.3 == balance => false,
        Some(r) => {
            r.3 = balance;
            true
        }
        None => {
            rows.push((club, year, month, balance));
            true
        }
    }
}

/// Once a day (poll_input, host or single player): each human club's balance for this month.
#[allow(static_mut_refs)]
pub unsafe fn tick() {
    static mut DAY: (i16, i16) = (0, 0);
    let today = (rd::<i16>(TODAY), rd::<i16>(TODAY + 2));
    if today == DAY || today.1 < 1900 || rd::<i32>(STAFF_COUNT) <= 0 {
        return;
    }
    let Some(game) = challenge::game_number() else { return };
    DAY = today;
    let file = format!("cmui_diary_{game:08x}.tsv");
    if file != FILE {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        BALANCES = parse(&text);
        ATTRS = parse_attrs(&text);
        SCOUTED = parse_scouted(&text);
        WORKS = parse_works(&text);
        FILE = file;
    }
    let (year, month) = (today.1 as i32, month_of(today.0 as i32, today.1 as i32));
    if on(NEWS) {
        crate::cheesecorner::tick(year, month);
    }
    // the stadium works finished by today (`stadium.rs`)
    let mut changed = on(STADIUM) && stadium::complete();
    for (staff, _) in challenge::human_list() {
        let club = rd::<usize>(staff + 0x39);
        let (Some(seed), Some((balance, this_month, last_month))) = (finreport::seed_of(club), finreport::balance_now(club)) else { continue };
        if !BALANCES.iter().any(|r| r.0 == seed) {
            let before = month_before(year, month);
            let two = month_before(before.0, before.1);
            BALANCES.push((seed, before.0, before.1, balance - this_month));
            BALANCES.push((seed, two.0, two.1, balance - this_month - last_month));
        }
        changed |= set(&mut BALANCES, seed, year, month, balance);
        // the chief scout's assignment, once a month
        if on(SCOUT_SCREEN) && !SCOUTED.iter().any(|a| a.0 == seed && a.1 == year && a.2 == month) {
            if let Some((line, scout, ids)) = assign(club, seed, year, month) {
                SCOUTED.push((seed, year, month, line, scout, ids));
                changed = true;
            }
        }
        // the players' attributes this month (the growth graph)
        for s in challenge::squad_of(club) {
            if !readable(rd::<usize>(s + 0x61), 70) {
                continue;
            }
            let (id, now) = (rd::<i32>(s), attributes(s));
            match ATTRS.iter_mut().find(|r| r.0 == id && r.1 == year && r.2 == month) {
                Some(r) if r.3 == now => {}
                Some(r) => (r.3, changed) = (now, true),
                None => {
                    ATTRS.push((id, year, month, now));
                    changed = true;
                }
            }
        }
    }
    if changed {
        save();
    }
}

/// The stadium works under way (`stadium.rs`): club seed, kind, amount, the day they are done
/// (year x 366 + day of the year) (`G` lines).
pub static mut WORKS: Vec<(usize, u8, i32, i32)> = Vec::new();

fn parse_works(text: &str) -> Vec<(usize, u8, i32, i32)> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() == 5 && f[0] == "G").then(|| Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?, f[4].parse().ok()?)))?
        })
        .collect()
}

/// Written now (a stadium order).
pub unsafe fn save_now() {
    save();
}

#[allow(static_mut_refs)]
unsafe fn save() {
    if FILE.is_empty() {
        return;
    }
    let text: String = BALANCES
        .iter()
        .map(|r| format!("B\t{}\t{}\t{}\t{}\n", r.0, r.1, r.2, r.3))
        .chain(ATTRS.iter().map(attrs_line))
        .chain(SCOUTED.iter().map(scouted_line))
        .chain(WORKS.iter().map(|w| format!("G\t{}\t{}\t{}\t{}\n", w.0, w.1, w.2, w.3)))
        .collect();
    let temp = format!("{FILE}.tmp");
    if std::fs::write(&temp, text).is_ok() {
        let _ = std::fs::rename(&temp, &FILE);
    }
}


/// A club's month-end balances, oldest first: (year, month, balance).
#[allow(static_mut_refs)]
pub unsafe fn balances(seed: usize) -> Vec<(i32, i32, i64)> {
    let mut out: Vec<(i32, i32, i64)> = BALANCES.iter().filter(|r| r.0 == seed).map(|r| (r.1, r.2, r.3)).collect();
    out.sort();
    out
}

/// Host: a client's key `%` club: the balances as "year:month:balance" with spaces between.
pub unsafe fn remote(club: usize) -> Option<Vec<u8>> {
    let seed = finreport::seed_of(club)?;
    Some(balances(seed).iter().map(|b| format!("{}:{}:{}", b.0, b.1, b.2)).collect::<Vec<_>>().join(" ").into_bytes())
}

/// A player's attributes by month, oldest first: (year, month, the 31).
#[allow(static_mut_refs)]
pub unsafe fn growth(id: i32) -> Vec<(i32, i32, [u8; 31])> {
    let mut out: Vec<(i32, i32, [u8; 31])> = ATTRS.iter().filter(|r| r.0 == id).map(|r| (r.1, r.2, r.3)).collect();
    out.sort();
    out
}

/// The season's biggest risers and fallers of a club's squad (the season card, user 2026-11-22):
/// the change of each player's four key attributes added up, from his first reading of the season
/// (from July) to his last; two up and two down at most, names as the game writes them.
#[allow(static_mut_refs)]
pub unsafe fn movers(club: usize, season: i32) -> Vec<(Vec<u8>, i32)> {
    let in_season = |y: i32, m: i32| (y == season && m >= 7) || (y == season + 1 && m <= 6);
    let mut moves: Vec<(Vec<u8>, i32)> = Vec::new();
    for s in challenge::squad_of(club) {
        let player = rd::<usize>(s + 0x61);
        if !readable(player, 70) {
            continue;
        }
        let keys = KEY_ATTRIBUTES[key_line(player)].2;
        let months: Vec<(i32, i32, [u8; 31])> = growth(rd::<i32>(s)).into_iter().filter(|g| in_season(g.0, g.1)).collect();
        if let (Some(a), Some(b)) = (months.first(), months.last()) {
            let change: i32 = keys.iter().map(|&k| b.2[k] as i32 - a.2[k] as i32).sum();
            if change != 0 {
                moves.push((staff_name(s), change));
            }
        }
    }
    moves.sort_by_key(|m| -m.1);
    let up: Vec<(Vec<u8>, i32)> = moves.iter().filter(|m| m.1 > 0).take(2).cloned().collect();
    let down: Vec<(Vec<u8>, i32)> = moves.iter().rev().filter(|m| m.1 < 0).take(2).cloned().collect();
    up.into_iter().chain(down).collect()
}

/// Host: a client's key `(` staff: his months as `A` lines.
pub unsafe fn growth_remote(staff: usize) -> Option<Vec<u8>> {
    if !valid_staff(staff) {
        return None;
    }
    let id = rd::<i32>(staff);
    Some(growth(id).iter().map(|g| attrs_line(&(id, g.0, g.1, g.2))).collect::<String>().into_bytes())
}

/// Client: what `growth_remote` sent.
pub fn parse_growth(text: &str) -> Vec<(i32, i32, [u8; 31])> {
    parse_attrs(text).into_iter().map(|r| (r.1, r.2, r.3)).collect()
}

/// Client: what `remote` sent.
pub fn parse_remote(text: &str) -> Vec<(i32, i32, i64)> {
    text.split(' ')
        .filter_map(|p| {
            let mut f = p.split(':');
            Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?, f.next()?.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_and_the_file() {
        assert_eq!((month_of(0, 2026), month_of(31, 2026), month_of(59, 2026), month_of(59, 2028), month_of(364, 2026)), (1, 2, 3, 2, 12));
        assert_eq!(month_before(2027, 1), (2026, 12));
        let mut rows = parse("B\t17\t2026\t7\t-4835944\nB\t17\t2026\t6\t100\nnoise\n");
        assert_eq!(rows.len(), 2);
        assert!(!set(&mut rows, 17, 2026, 7, -4835944) && set(&mut rows, 17, 2026, 7, -1) && set(&mut rows, 17, 2026, 8, 5));
        assert_eq!(parse_remote("2026:6:100 2026:7:-1"), vec![(2026, 6, 100), (2026, 7, -1)]);
    }
}
