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
        "Max 1 purchase" => (|r| r.bought, Way::Most(1), "purchases"),
        "Max 3 purchases" => (|r| r.bought, Way::Most(3), "purchases"),
        "No purchases > \u{80}2M" => (|r| r.biggest, Way::Most(2000), "k"),
        "Biggest purchase < \u{80}5M" => (|r| r.biggest, Way::Most(4999), "k"),
        "Only Bosman/free transfers" => (|r| r.paid, Way::Most(0), "paid purchases"),
        "Only buy players <23" => (|r| r.over_23, Way::Most(0), "purchases of 23+"),
        "No winter transfers" => (|r| r.winter_deals, Way::Most(0), "winter transfers"),
        "No sales in winter" => (|r| r.winter_sales, Way::Most(0), "winter sales"),
        "Release five players from your squad (no fee)" => (|r| r.released, Way::Least(5), "released"),
        "Sell your best player (avg rating 1 January)" => (|r| r.best_sold, Way::Least(1), "sold"),
        "Team gets no more than 3 red cards" => (|r| r.reds, Way::Most(3), "red cards"),
        "Team gets no more than 5 red cards" => (|r| r.reds, Way::Most(5), "red cards"),
        "At least 3 players with 10+ goals" => (|r| r.scorers_10, Way::Least(3), "players"),
        "At least 3 players with 5+ assists" => (|r| r.assists_5, Way::Least(3), "players"),
        "Player scores 20+ goals" => (|r| r.top_goals, Way::Least(20), "goals"),
        "Player becomes top scorer OR top assists OR highest average rating" => (|r| r.leads, Way::EndLeast(1), "of 3"),
        "Player gets two of three (top scorer/assists/average rating)" => (|r| r.leads, Way::EndLeast(2), "of 3"),
        "No staff" => (|r| r.staff, Way::EndMost(0), "staff members"),
        "Max 2 staff members in total" => (|r| r.staff, Way::EndMost(2), "staff members"),
        "No physio and no scout" => (|r| r.scouts_physios, Way::EndMost(0), "physios and scouts"),
        "Dutch staff only" => (|r| r.foreign_staff, Way::EndMost(0), "foreign staff"),
        "All coaches with coaching attributes below 5" => (|r| r.coach_best, Way::EndMost(4), "highest attribute"),
        "Assistant manager <10" => (|r| r.assistant_best, Way::EndMost(9), "highest attribute"),
        _ => return None,
    };
    Some(r)
}

/// The reading in words: "1 / 3 purchases", "€1.5M / €2M".
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
pub(super) unsafe fn stat(staff: usize, row: i32, field: i32) -> f32 {
    let stats = rd::<usize>(PLAYER_STATS);
    let player = ((staff + 0x61) as *const usize).read_unaligned();
    if !readable(stats, 0x34) || !readable(player, 70) {
        return 0.0;
    }
    let mut buf = [0u8; 0x40];
    game::<extern "thiscall" fn(usize, usize, i32, i32, *mut u8, i32, i32, i32, i32) -> i32>(FORMAT_STAT)(stats, player, row, field, buf.as_mut_ptr(), 0x40, 0, 0, 1);
    buffer_text(&buf).parse().unwrap_or(0.0)
}

pub(super) unsafe fn squad(club: usize) -> Vec<usize> {
    (0..50).map(|k| rd::<usize>(club + 0xd7 + k * 4)).filter(|&s| valid_staff(s)).collect()
}

/// Of top scorer, top assists and best average (10 matches or more) in the club's league, how
/// many one player of the club holds.
unsafe fn league_leads(club: usize) -> i32 {
    let best = league_leaders(club);
    squad(club).iter().map(|s| best.iter().filter(|b| *b == s).count() as i32).max().unwrap_or(0)
}

/// The club's league's top scorer, top assists and best average (10 matches or more), by the
/// league row (2) of the stats: a cup's goals or an earlier club's do not count. 0: nobody yet.
pub(super) unsafe fn league_leaders(club: usize) -> [usize; 3] {
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
    best.map(|b| b.1)
}

const LEAGUES: usize = 0xadadfc; // the League objects, by the competition's id (its first dword)
const STAFF_COMPS: usize = 0xae23e0; // the awards: 101 bytes a row, +4 the name
const STAFF_COMP_COUNT: usize = 0xae2388;
const KIND_TEAM_OF_YEAR: u8 = 0x32; // a staff history line: +0xd the award's row, +0x11 the year
const KIND_YEAR_AWARD: u8 = 0x34; // the same, +0x19 the place

/// The club's league table as it stands: the club's matches, defeats and points, the leader's
/// points, a season's matches (everyone twice), and the clubs played in the league this season with
/// how often beaten (only a league whose rules keep head-to-head records has them).
#[derive(Debug)]
pub(super) struct Standing {
    pub played: i32,
    pub lost: i32,
    pub points: i32,
    pub leader: i32,
    pub full: i32,
    pub beaten: Vec<(usize, i32)>,
}

/// The table of the club's league: League object `[LEAGUES][id]` (its +4 the competition), +0x3e
/// the teams, +0xb1 the rows of 0x3b bytes: +0 club, +5 head-to-head (14 bytes an opponent: +0
/// club, +5 won), +0x11 played, +0x14 lost, +0x19 points.
pub(super) unsafe fn standing(club: usize) -> Option<Standing> {
    let comp = rd::<usize>(club + 0x57);
    let leagues = rd::<usize>(LEAGUES);
    if !readable(comp, 4) || !readable(leagues, 4) {
        return None;
    }
    let id = rd::<i32>(comp);
    let at = leagues + id.max(0) as usize * 4;
    let league = if id >= 0 && readable(at, 4) { rd::<usize>(at) } else { 0 };
    if !readable(league, 0xb5) || rd::<usize>(league + 4) != comp {
        return None;
    }
    let (teams, rows) = (rd::<i16>(league + 0x3e).max(0) as usize, rd::<usize>(league + 0xb1));
    if !(2..=64).contains(&teams) || !readable(rows, teams * 0x3b) {
        return None;
    }
    let row = |k: usize| rows + k * 0x3b;
    let own = (0..teams).map(row).find(|&r| rd::<usize>(r) == club)?;
    let leader = (0..teams).map(|k| rd::<i16>(row(k) + 0x19) as i32).max().unwrap_or(0);
    let h2h = rd::<usize>(own + 5);
    let beaten = if h2h != 0 && readable(h2h, (teams - 1) * 14) { (0..teams - 1).map(|k| (rd::<usize>(h2h + k * 14), rd::<u8>(h2h + k * 14 + 5) as i32)).collect() } else { Vec::new() };
    if trace() {
        log(&format!("challenge table {}: teams {teams} rules {:?} h2h {h2h:#x}", latin1(&game_text(comp + 4, 51)), (0..4).map(|k| rd::<u8>(league + 0xc5 + k)).collect::<Vec<_>>()));
    }
    Some(Standing { played: rd::<u8>(own + 0x11) as i32, lost: rd::<u8>(own + 0x14) as i32, points: rd::<i16>(own + 0x19) as i32, leader, full: 2 * (teams as i32 - 1), beaten })
}

/// The awards of the squad's players: (award name in lower case, a team of the year, the place (1
/// the winner, 2 "runner up", 3 "third placed"), the club's index then (+8, seen for the Ballon
/// d'Or), the season of its date, its age in days).
pub(super) unsafe fn new_awards(club: usize) -> Vec<(String, bool, i32, i32, i32, i32)> {
    let records = rd::<usize>(history::STAFF_RECORDS);
    let (comps, count) = (rd::<usize>(STAFF_COMPS), rd::<i32>(STAFF_COMP_COUNT));
    if !mapped(records, 4) || count <= 0 || !readable(comps, count as usize * 0x65) {
        return Vec::new();
    }
    let today = rd::<i16>(TODAY + 2) as i32 * 366 + rd::<i16>(TODAY) as i32;
    let mut out = Vec::new();
    for s in squad(club) {
        let list = game::<extern "thiscall" fn(usize, usize, u32) -> usize>(history::STAFF_LIST)(records, s, 0);
        for item in history::newest(list) {
            let kind = rd::<u8>(item + 0xc);
            let id = rd::<i32>(item + 0xd);
            if ![KIND_TEAM_OF_YEAR, KIND_YEAR_AWARD].contains(&kind) || !(0..count).contains(&id) {
                continue;
            }
            let name = latin1(&game_text(comps + id as usize * 0x65 + 4, 51)).to_lowercase();
            let place = if kind == KIND_YEAR_AWARD { rd::<i8>(item + 0x19) as i32 } else { 1 };
            let age = today - (rd::<i16>(item + 2) as i32 * 366 + rd::<i16>(item) as i32);
            if trace() {
                log(&format!("challenge award {}: {name} kind {kind:#x} place {place} {} days ago: {}", latin1(&staff_name(s)), age, latin1(&history::sentence(records, item))));
            }
            let season = season_from(rd::<i16>(item), rd::<i16>(item + 2) as i32, SEASON_FROM);
            out.push((name, kind == KIND_TEAM_OF_YEAR, place, rd::<i32>(item + 8), season, age));
        }
        history::free(list);
    }
    out
}

/// A club's names in lower case, the long and the short, as the derbies go by.
unsafe fn club_names(club: usize) -> String {
    if !readable(club, 0x55) {
        return String::new();
    }
    format!("{} {}", latin1(&game_text(club + 4, 51)), latin1(&game_text(club + 0x38, 26))).to_lowercase()
}

/// The career goals' readings of a manager's season, for the goals he picked: his league table
/// (matches and defeats while they grow, the gap to the leader once half the matches are played,
/// the wins over a derby's rival) and his squad's new awards.
unsafe fn read_career(l: &mut League, name: &str, club: usize, now: i32) -> bool {
    let picked = |goal: &str| l.marks.iter().any(|m| same_manager(&m.name, name) && m.goal == goal);
    let table = picked(goals::UNBEATEN) || picked(goals::COMEBACK) || goals::DERBIES.iter().any(|d| picked(d.0));
    let awards = picked(goals::BALLON) || picked(goals::TEAM_OF_YEAR);
    let mut changed = false;
    if let Some(s) = standing(club).filter(|_| table) {
        if trace() {
            log(&format!("challenge standing {name}: {s:?}"));
        }
        if s.played >= l.stat(name, now, "played").unwrap_or(0) {
            changed |= l.set_stat(name, now, "played", s.played) | l.set_stat(name, now, "lost", s.lost) | l.set_stat(name, now, "full", s.full);
        }
        if l.stat(name, now, "gap").is_none() && s.played > 0 && s.played * 2 >= s.full {
            changed |= l.set_stat(name, now, "gap", s.leader - s.points);
        }
        let own = club_names(club);
        for (d, derby) in goals::DERBIES.iter().enumerate() {
            let rival = if goals::is_side(&own, derby.1) { derby.2 } else if goals::is_side(&own, derby.2) { derby.1 } else { continue };
            let wins = s.beaten.iter().filter(|b| goals::is_side(&club_names(b.0), rival)).map(|b| b.1).max().unwrap_or(0);
            if trace() {
                let found: Vec<String> = s.beaten.iter().filter(|b| goals::is_side(&club_names(b.0), rival)).map(|b| club_names(b.0)).collect();
                log(&format!("challenge derby {name}: {} rival {found:?} won {wins}", derby.0));
            }
            let key = format!("derby{d}");
            if wins > l.stat(name, now, &key).unwrap_or(0) {
                changed |= l.set_stat(name, now, &key, wins);
            }
        }
    }
    if awards {
        let new = new_awards(club);
        // (the Ballon d'Or of this season, won at this club: "2028/29 Ballon d'Or", place 1)
        let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
        if new.iter().any(|a| a.0 == "ballon d'or" && !a.1 && a.2 == 1 && a.3 == index && a.4 == now) {
            changed |= l.set_stat(name, now, "ballon", 1);
        }
        let toty = new.iter().filter(|a| a.1 && a.0.contains("team of the year") && a.5 <= 30).count() as i32;
        if toty > l.stat(name, now, "toty").unwrap_or(0) {
            changed |= l.set_stat(name, now, "toty", toty);
        }
    }
    changed
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
        // career goals: the first division's leaders in his squad, the last reading of a season stands
        if on(CAREER_GOALS) && readable(club, 0xd7 + 200) && l.marks.iter().any(|m| same_manager(&m.name, &name) && goals::LEADS.contains(&m.goal.as_str())) {
            let own = squad(club);
            let top = first_division(rd::<usize>(club + 0x57));
            let leaders = league_leaders(club);
            let held = leaders.map(|s| top && s != 0 && own.contains(&s));
            if trace() {
                let who: Vec<String> = leaders.iter().map(|&s| if s == 0 { String::new() } else { format!("{} ({}, {}, {})", latin1(&staff_name(s)), stat(s, 2, 2), stat(s, 2, 5), stat(s, 2, 0x11)) }).collect();
                log(&format!("challenge leaders {name}: first division {top}, {who:?}, in the squad {held:?}"));
            }
            match l.leaders.iter_mut().find(|e| e.1 == now && same_manager(&e.0, &name)) {
                Some(e) if e.2 == held => {}
                Some(e) => (e.2, changed) = (held, true),
                None => {
                    l.leaders.push((name.clone(), now, held));
                    changed = true;
                }
            }
        }
        if on(CAREER_GOALS) && readable(club, 0xd7 + 200) && l.marks.iter().any(|m| same_manager(&m.name, &name)) {
            changed |= read_career(l, &name, club, now);
        }
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
        let leads = l.rads.iter().filter(open).any(|r| r.text.starts_with("Player becomes") || r.text.starts_with("Player gets"));
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
    if today == DAY || today.1 < 1900 || rd::<i32>(STAFF_COUNT) <= 0 || !game_on() {
        return;
    }
    DAY = today;
    use_game();
    // (in a trace, once: which first divisions keep head-to-head records, the derbies' source)
    static mut LISTED: bool = false;
    if trace() && !LISTED {
        LISTED = true;
        let (comps, count, leagues) = (rd::<usize>(COMPS), rd::<i32>(COMP_COUNT), rd::<usize>(LEAGUES));
        for k in 0..count.max(0) as usize {
            let comp = comps + k * COMP_SIZE;
            let at = leagues + rd::<i32>(comp).max(0) as usize * 4;
            let league = if readable(at, 4) { rd::<usize>(at) } else { 0 };
            if first_division(comp) && readable(league, 0xc9) && rd::<usize>(league + 4) == comp {
                let rules: Vec<u8> = (0..4).map(|r| rd::<u8>(league + 0xc5 + r)).collect();
                log(&format!("challenge league {}: rules {rules:?} head-to-head {}", latin1(&game_text(comp + 4, 51)), rules.contains(&4)));
            }
        }
    }
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
        assert_eq!(note(1, Way::Most(3), "purchases"), "1 / 3 purchases");
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
