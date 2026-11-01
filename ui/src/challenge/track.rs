//! The RAD challenges the game can judge by itself: from the manager's own history (who he bought
//! and sold, for how much, when), the squad's season stats and the club's staff. On every human
//! turn the host reads them for this season's rows; when the season turns, `done` is set from the
//! last reading, unless the host ticked the row himself. The others stay the host's to tick.

use super::*;
use crate::*;

pub const OPEN: u8 = 0;
pub const GOOD: u8 = 1;
pub const BAD: u8 = 2;
const PLAYER_JOB: u8 = 11;
const WINTER_FROM: i16 = 335; // 1 December
const WINTER_TO: i16 = 60; // 1 March
/// From this day a deal is the coming season's (the transfer window opens in June).
const DEALS_FROM: i16 = 150;

/// How a value counts. Most / Least are settled as soon as the limit is passed; End ones are read
/// at the end of the season (staff can still be sent away, the top scorer can still change).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Way {
    Most(i32),
    Least(i32),
    EndMost(i32),
    EndLeast(i32),
}

/// What the game says of a season's reading, while it runs.
fn judge(way: Way, value: i32, old: u8) -> u8 {
    match way {
        Way::Most(n) if value > n => BAD,
        Way::Least(n) if value >= n => GOOD,
        // (a player sold takes his goals along: what was reached stays reached)
        Way::Most(_) if old == BAD => BAD,
        Way::Least(_) if old == GOOD => GOOD,
        _ => OPEN,
    }
}

/// Whether the challenge is met when the season is over.
fn passed(way: Way, value: i32, state: u8) -> bool {
    match way {
        Way::Most(n) => state != BAD && value <= n,
        Way::Least(_) => state == GOOD,
        Way::EndMost(n) => value <= n,
        Way::EndLeast(n) => value >= n,
    }
}

/// A season's readings of one manager.
#[derive(Default, Debug)]
pub struct Readings {
    bought: i32,
    biggest: i32, // the highest fee, in thousands
    paid: i32,    // purchases with a fee
    over_23: i32, // purchases of 23 or older
    winter_deals: i32,
    winter_sales: i32,
    released: i32,
    best_sold: i32,
    reds: i32,
    scorers_10: i32,
    assists_5: i32,
    top_goals: i32,
    leads: i32, // of top scorer, top assists, best average: how many one player of the club holds
    staff: i32,
    scouts_physios: i32,
    foreign_staff: i32,
    coach_best: i32,
    assistant_best: i32,
}

type Reading = fn(&Readings) -> i32;

/// How the game judges a challenge, by its text (`rules.tsv`, the euro sign as the game's byte
/// 0x80): the reading, the way, the unit. None: the host's to tick.
pub fn rule(text: &str) -> Option<(Reading, Way, &'static str)> {
    let r: (Reading, Way, &'static str) = match text {
        "Max 1 aankoop" => (|r| r.bought, Way::Most(1), "aankopen"),
        "Max 3 aankopen" => (|r| r.bought, Way::Most(3), "aankopen"),
        "Geen aankopen > \u{80}2M" => (|r| r.biggest, Way::Most(2000), "k"),
        "Grootste aankoop < \u{80}5M" => (|r| r.biggest, Way::Most(4999), "k"),
        "Alleen Bosman/vrije transfers" => (|r| r.paid, Way::Most(0), "betaalde aankopen"),
        "Alleen spelers <23 kopen" => (|r| r.over_23, Way::Most(0), "aankopen van 23+"),
        "Geen wintertransfers" => (|r| r.winter_deals, Way::Most(0), "wintertransfers"),
        "Geen verkopen in winter" => (|r| r.winter_sales, Way::Most(0), "winterverkopen"),
        "Ontsla vijf spelers uit je team (zonder vergoeding)" => (|r| r.released, Way::Least(5), "ontslagen"),
        "Verkoop je beste speler (avg rating 1 januari)" => (|r| r.best_sold, Way::Least(1), "verkocht"),
        "Haal als team niet meer dan 3 rode kaarten" => (|r| r.reds, Way::Most(3), "rode kaarten"),
        "Haal als team niet meer dan 5 rode kaarten" => (|r| r.reds, Way::Most(5), "rode kaarten"),
        "Minstens 3 spelers met 10+ goals" => (|r| r.scorers_10, Way::Least(3), "spelers"),
        "Minstens 3 spelers met 5+ assists" => (|r| r.assists_5, Way::Least(3), "spelers"),
        "Speler scoort 20+ goals" => (|r| r.top_goals, Way::Least(20), "goals"),
        "Speler wordt topscorer OF top assists OF hoogste gemiddelde" => (|r| r.leads, Way::EndLeast(1), "van 3"),
        "Speler haalt twee van de drie (topscorer/assists/HG)" => (|r| r.leads, Way::EndLeast(2), "van 3"),
        "Geen staf" => (|r| r.staff, Way::EndMost(0), "stafleden"),
        "Max 2 stafleden totaal" => (|r| r.staff, Way::EndMost(2), "stafleden"),
        "Geen fysio \u{e9}n geen scout" => (|r| r.scouts_physios, Way::EndMost(0), "fysio's en scouts"),
        "Alleen Nederlandse staf" => (|r| r.foreign_staff, Way::EndMost(0), "buitenlandse stafleden"),
        "Alle coaches met coaching attributes onder de 5" => (|r| r.coach_best, Way::EndMost(4), "hoogste attribuut"),
        "Assistent manager <10" => (|r| r.assistant_best, Way::EndMost(9), "hoogste attribuut"),
        _ => return None,
    };
    Some(r)
}

/// The reading in words: "1 / 3 aankopen", "€1.5M / €2M".
fn note(value: i32, way: Way, unit: &str) -> String {
    let limit = match way {
        Way::Most(n) | Way::Least(n) | Way::EndMost(n) | Way::EndLeast(n) => n,
    };
    if unit == "k" {
        return format!("\u{80}{:.1}M / \u{80}{}M", value as f32 / 1000.0, (limit + 1) / 1000);
    }
    format!("{value} / {limit} {unit}")
}

/// A fee as the game writes it in its sentence, in thousands: "...for £1.5M", "€450K".
fn fee_of(text: &str) -> Option<i32> {
    let at = text.rfind(['\u{80}', '\u{a3}', '$'])?;
    let tail: String = text[at..].chars().skip(1).take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == ',').collect();
    let tail = tail.replace(',', "");
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n: f32 = digits.parse().ok()?;
    Some(match tail[digits.len()..].chars().next() {
        Some('M' | 'm') => (n * 1000.0) as i32,
        Some('K' | 'k') => n as i32,
        _ => (n / 1000.0) as i32,
    })
}

/// What the manager's history says of a season's deals.
unsafe fn read_deals(staff: usize, year: i32, best: Option<i32>, r: &mut Readings) {
    let records = rd::<usize>(history::STAFF_RECORDS);
    if !mapped(records, 4) {
        return;
    }
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    let list = game::<extern "thiscall" fn(usize, usize, u32) -> usize>(history::STAFF_LIST)(records, staff, 0);
    for item in history::newest(list) {
        let (day, date_year, kind) = (rd::<i16>(item), rd::<i16>(item + 2) as i32, rd::<u8>(item + 0xc));
        let season = if day >= DEALS_FROM { date_year } else { date_year - 1 };
        if season != year || ![KIND_BOUGHT, KIND_SOLD].contains(&kind) || rd::<u8>(item + 0x29) != PLAYER_JOB {
            continue;
        }
        let (who, fee, other) = (rd::<i32>(item + 0xd), rd::<i32>(item + 0x1d), rd::<i32>(item + 0x25));
        let said = latin1(&history::sentence(records, item)).to_string();
        if trace() {
            log(&format!("challenge deal {staff:#x} day {day} {date_year} kind {kind} who {who} fee {fee} other {other}: {said}"));
        }
        let winter = day >= WINTER_FROM || day < WINTER_TO;
        r.winter_deals += winter as i32;
        if kind == KIND_BOUGHT {
            r.bought += 1;
            r.paid += (fee > 0) as i32;
            r.biggest = r.biggest.max(fee_of(&said).unwrap_or(fee / 1000));
            if (0..count).contains(&who) {
                let player = base + who as usize * 0x6e;
                let age = date_year - rd::<i16>(player + 0x12) as i32 - (day < rd::<i16>(player + 0x10)) as i32;
                r.over_23 += (age >= 23) as i32;
            }
        } else {
            r.winter_sales += winter as i32;
            r.released += (fee <= 0 && other < 0) as i32;
            r.best_sold += (best == Some(who)) as i32;
        }
    }
    history::free(list);
}

/// A season stat of a player in a stats row (2 League, 6 Senior Club) as a number.
unsafe fn stat(staff: usize, row: i32, field: i32) -> f32 {
    let stats = rd::<usize>(PLAYER_STATS);
    let player = ((staff + 0x61) as *const usize).read_unaligned();
    if !readable(stats, 0x34) || !readable(player, 70) {
        return 0.0;
    }
    let mut buf = [0u8; 0x40];
    game::<extern "thiscall" fn(usize, usize, i32, i32, *mut u8, i32, i32, i32, i32) -> i32>(FORMAT_STAT)(stats, player, row, field, buf.as_mut_ptr(), 0x40, 0, 0, 1);
    buffer_text(&buf).parse().unwrap_or(0.0)
}

unsafe fn squad(club: usize) -> Vec<usize> {
    (0..50).map(|k| rd::<usize>(club + 0xd7 + k * 4)).filter(|&s| valid_staff(s)).collect()
}

/// Of top scorer, top assists and best average (10 matches or more) in the club's league, how
/// many one player of the club holds.
unsafe fn league_leads(club: usize) -> i32 {
    let (table, count, league) = (rd::<usize>(CLUB_TABLE), rd::<i32>(CLUB_COUNT), rd::<usize>(club + 0x57));
    let mut best = [(0.0f32, 0usize); 3];
    for k in 0..count as usize {
        let other = table + k * CLUB_SIZE;
        if rd::<usize>(other + 0x57) != league {
            continue;
        }
        for s in squad(other) {
            let values = [stat(s, 2, 2), stat(s, 2, 5), if stat(s, 2, 1) >= 10.0 { stat(s, 2, 0x11) } else { 0.0 }];
            for (b, v) in best.iter_mut().zip(values) {
                if v > b.0 {
                    *b = (v, s);
                }
            }
        }
    }
    squad(club).iter().map(|s| best.iter().filter(|b| b.1 == *s).count() as i32).max().unwrap_or(0)
}

/// The club's staff: coaches, scouts, physios, the assistant, each once; a human manager not.
unsafe fn read_staff(club: usize, r: &mut Readings) {
    let row = |at: usize, n: usize| (0..n).map(move |k| rd::<usize>(club + at + k * 4)).filter(|&s| valid_staff(s));
    let assistant = game::<extern "C" fn(usize) -> usize>(assistant::CLUB_ASSISTANT)(club);
    let humans = human_staff();
    let coaches: Vec<usize> = row(0x19f, 5).filter(|s| !humans.contains(s)).collect();
    let others: Vec<usize> = row(0x1b3, 7).chain(row(0x1cf, 3)).filter(|s| !humans.contains(s)).collect();
    let mut all: Vec<usize> = coaches.iter().chain(others.iter()).copied().chain(Some(assistant).filter(|&a| valid_staff(a) && !humans.contains(&a))).collect();
    all.sort();
    all.dedup();
    r.staff = all.len() as i32;
    r.scouts_physios = others.len() as i32;
    let dutch = |s: usize| {
        let nation = rd::<usize>(s + 0x1a);
        let name = if readable(nation, 0x40) { latin1(&game_text(nation + 4, 51)).to_lowercase() } else { String::new() };
        name.contains("nether") || name.contains("holland")
    };
    r.foreign_staff = all.iter().filter(|&&s| !dutch(s)).count() as i32;
    let best = |s: usize| {
        let np = rd::<usize>(s + 0x69);
        if !readable(np, 68) {
            return 0;
        }
        assistant::SPECIALITIES.iter().map(|t| if t.1 { assistant::coaching(s, t.0) } else { rd::<i8>(np + t.0) as f64 } as i32).max().unwrap_or(0)
    };
    r.coach_best = coaches.iter().map(|&s| best(s)).max().unwrap_or(0);
    r.assistant_best = if valid_staff(assistant) { best(assistant) } else { 0 };
}

/// A manager's readings of this season.
unsafe fn read(staff: usize, club: usize, year: i32, leads: bool, best: Option<i32>) -> Readings {
    let mut r = Readings::default();
    read_deals(staff, year, best, &mut r);
    if !readable(club, 0x1cf + 12) {
        return r;
    }
    for s in squad(club) {
        let (goals, assists) = (stat(s, 6, 2) as i32, stat(s, 6, 5) as i32);
        r.reds += stat(s, 6, 11) as i32;
        r.scorers_10 += (goals >= 10) as i32;
        r.assists_5 += (assists >= 5) as i32;
        r.top_goals = r.top_goals.max(goals);
    }
    if leads {
        r.leads = league_leads(club);
    }
    read_staff(club, &mut r);
    r
}

/// The squad's best average at this moment: for "sell your best player", kept on 1 January.
unsafe fn best_player(club: usize) -> Option<i32> {
    let base = rd::<usize>(STAFF_TABLE);
    let rated = squad(club).into_iter().map(|s| (stat(s, 6, 0x11), s)).filter(|p| p.0 > 0.0);
    rated.max_by(|a, b| a.0.total_cmp(&b.0)).map(|p| ((p.1 - base) / 0x6e) as i32)
}

/// This season's rows read again, and rows of a season gone settled.
pub unsafe fn update() {
    let l = league();
    let (now, day) = (this_season(), rd::<i16>(TODAY));
    let mut changed = false;
    for (staff, name) in humans() {
        let open = |r: &&Rad| r.year == now && same_manager(&r.name, &name) && !r.manual;
        let club = rd::<usize>(staff + 0x39);
        // the best player of the first turn of the year
        if (0..31).contains(&day) && readable(club, 0xd7 + 200) && !l.best.iter().any(|b| b.1 == now && same_manager(&b.0, &name)) {
            if let Some(player) = best_player(club) {
                l.best.push((name.clone(), now, player));
                changed = true;
            }
        }
        let rows: Vec<u32> = l.rads.iter().filter(open).filter(|r| rule(&r.text).is_some()).map(|r| r.id).collect();
        if rows.is_empty() {
            continue;
        }
        let leads = l.rads.iter().filter(open).any(|r| r.text.starts_with("Speler wordt") || r.text.starts_with("Speler haalt"));
        let best = l.best.iter().find(|b| b.1 == now && same_manager(&b.0, &name)).map(|b| b.2);
        let readings = read(staff, club, now, leads, best);
        if trace() {
            log(&format!("challenge readings {name}: {readings:?}"));
        }
        for r in l.rads.iter_mut().filter(|r| rows.contains(&r.id)) {
            let Some((reading, way, unit)) = rule(&r.text) else { continue };
            let value = reading(&readings);
            let (state, said) = (judge(way, value, r.state), note(value, way, unit));
            if (state, Some(value), &said) != (r.state, r.value, &r.note) {
                (r.state, r.value, r.note) = (state, Some(value), said);
                changed = true;
            }
        }
    }
    // a season over: what the game read last settles it
    for r in l.rads.iter_mut().filter(|r| r.year < now && !r.closed && !r.manual) {
        let (Some(value), Some((_, way, _))) = (r.value, rule(&r.text)) else { continue };
        (r.done, r.closed) = (passed(way, value, r.state), true);
        changed = true;
    }
    if changed {
        save();
    }
}

/// A human turn on the host: once a game day.
pub unsafe fn tick() {
    static mut DAY: (i16, i16) = (0, 0);
    let today = (rd::<i16>(TODAY), rd::<i16>(TODAY + 2));
    if today == DAY || today.1 < 1900 || rd::<i32>(STAFF_COUNT) <= 0 || !network_game() {
        return;
    }
    DAY = today;
    use_game();
    let _ = seasons(); // (this season's top scorer goes in the file)
    update();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_settle_when_passed() {
        assert_eq!(judge(Way::Most(1), 1, OPEN), OPEN);
        assert_eq!(judge(Way::Most(1), 2, OPEN), BAD);
        assert_eq!(judge(Way::Most(3), 2, BAD), BAD); // a sold player's red card still counts
        assert_eq!(judge(Way::Least(3), 3, OPEN), GOOD);
        assert_eq!(judge(Way::Least(3), 2, GOOD), GOOD);
        assert_eq!(judge(Way::EndMost(0), 4, OPEN), OPEN);
    }

    #[test]
    fn the_seasons_end_decides() {
        assert!(passed(Way::Most(1), 1, OPEN));
        assert!(!passed(Way::Most(1), 1, BAD));
        assert!(!passed(Way::Least(5), 4, OPEN));
        assert!(passed(Way::Least(5), 4, GOOD));
        assert!(passed(Way::EndMost(0), 0, OPEN));
        assert!(!passed(Way::EndMost(0), 1, OPEN));
        assert!(passed(Way::EndLeast(2), 2, OPEN));
    }

    #[test]
    fn fees_as_the_game_writes_them() {
        assert_eq!(fee_of("  Bought A from B for \u{a3}1.5M"), Some(1500));
        assert_eq!(fee_of("  Bought A from B for \u{80}450K"), Some(450));
        assert_eq!(fee_of("  Bought A from B for \u{80}1,200,000"), Some(1200));
        assert_eq!(fee_of("  Bought A from B (part exchange)"), None);
        assert_eq!(note(1500, Way::Most(2000), "k"), "\u{80}1.5M / \u{80}2M");
        assert_eq!(note(1, Way::Most(3), "aankopen"), "1 / 3 aankopen");
    }

    /// Every challenge of a kind the game can read has its rule: a word changed in the rules file
    /// would quietly make it the host's.
    #[test]
    fn the_rules_file_and_the_table_agree() {
        let l = League::parse(&latin1(RULES), "");
        for t in l.tasks.iter().filter(|t| ["Transfers", "Staff"].contains(&t.0.as_str())) {
            assert!(rule(&t.1).is_some(), "{}", t.1);
        }
        assert_eq!(l.tasks.iter().filter(|t| rule(&t.1).is_some()).count(), 23);
    }
}
