//! CM Challenge: the managers' league of a network game as a page of its own (switch `challenge`),
//! opened by a button under the menu bar. The host works it out, a client asks for the page (key
//! 'E') and only draws it. The prizes come from the game's own staff history (hired, sacked,
//! winners, rounds reached, league finishes), so they need no typing; the file `cmchallenge.tsv`
//! next to the host's exe keeps what the game does not: seasons entered on the old site
//! (`challenge_import.py`), the RAD rows and each season's top scorer. To drop the feature: switch
//! `challenge=0`, or remove this folder and the lines `docs\challenge.md` lists.

use crate::*;

mod draw;
mod track;
pub use draw::{bar_item, flip, input, overlay};

const RULES: &[u8] = include_bytes!("rules.tsv");
/// A league as `challenge_import.py` writes it: taken over by the first game with its managers,
/// then named after that game (`cmchallenge_<key>.tsv`), so a new game starts a league of its own.
const IMPORTED: &str = "cmchallenge.tsv";
const COMPS: usize = 0xae23d0; // the competition table, 107 bytes a row: +4 name, +0x5d nation, +0x69 reputation (i16)
const COMP_COUNT: usize = 0xae2378;
const COMP_SIZE: usize = 0x6b;
const COMP_NATION: usize = 0x5d;
const COMP_REPUTATION: usize = 0x69;
const FORMAT_NAME: usize = 0x53dba0; // database_format_staff_name(staff, out, size, style), cdecl
const TODAY: usize = 0xae2c90; // Date: i16 day of the year, i16 year
/// The first day of the year that counts for the next season: past the World Cup final, before
/// the supercups. A hiring goes by `HIRED_FROM`: after the leagues end it is the coming season's
/// (a manager hired days after his title won it at his old club); a move up to `LEFT_UNTIL` left last season's club.
const SEASON_FROM: i16 = 200;
const HIRED_FROM: i16 = 130;
const LEFT_UNTIL: i16 = 260;
const KIND_WON: u8 = 0;
const KIND_LOST_FINAL: u8 = 1;
const KIND_QUALIFIED: u8 = 3;
const KIND_KNOCKED_OUT: u8 = 4;
const KIND_CHAMPIONS: u8 = 5;
const KIND_PROMOTED: u8 = 8;
const KIND_FINISHED: u8 = 10;
const KIND_BOUGHT: u8 = 100; // +0xd the staff index, +0x1d the fee, +0x25 the other club, +0x29 job (11 a player)
const KIND_SOLD: u8 = 101;
const KIND_JOB: u8 = 0x96; // +8 the club's index, +0xd 1 hired, 0 resigned, else sacked
const HISTORY_ROWS: usize = 200; // a page's Historie rows at most

/// A line of a manager's history, as far as the prizes need it.
#[derive(Clone, Default)]
struct Fact {
    season: i32,
    kind: u8,
    comp: String, // lower case
    domestic: bool,
    top: bool, // the first division of its nation, or its main cup
    table: usize, // the competition's row
    year: i32, // the calendar year, as the competitions' result rows have it
    text: String, // the game's sentence, lower case
}

/// A cup's prizes by how far a club got: won, lost the final, semi final, quarter final, phase 2.
const FAMILIES: [[&str; 5]; 8] = [
    ["CL Winnaar", "CL Finalist", "CL Halve finale", "CL Kwartfinale", "CL Phase 2"],
    ["UEFA Cup Winnaar", "UEFA Cup Finalist", "UEFA Cup Halve finale", "UEFA Cup Kwartfinale", ""],
    ["Europese Supercup Winnaar", "", "", "", ""],
    ["WK Clubteams Winnaar", "WK Clubteams Finalist", "WK Clubteams Halve finalist", "", ""],
    ["WK Landenteams Winnaar", "WK Landenteams Finalist", "WK Landenteams Halve finale", "WK Landenteams Kwartfinale", ""],
    ["Continentaal landentoernooi Winnaar", "Continentaal landentoernooi Finalist", "Continentaal landentoernooi Halve finale", "", ""],
    ["Bekerwinnaar", "Finalist beker", "", "", ""],
    ["Nat. Supercup", "", "", "", ""],
];

/// The row of `FAMILIES` a competition is in, by its name (the database's words, or a data
/// update's): None for what the rules give nothing for. Of a nation's cups only the main one
/// counts: the old site has no Scottish League or Challenge Cup.
// ponytail: names, a database with other words needs its own here; the Comp's class would do too
fn family(comp: &str, domestic: bool, top: bool) -> Option<usize> {
    let has = |words: &[&str]| words.iter().any(|w| comp.contains(w));
    if domestic {
        return if has(&["super", "shield", "schaal", "trophy", "supertaca"]) { Some(7) } else { top.then_some(6) };
    }
    match () {
        _ if has(&["african", "asian", "concacaf", "cis ", "afro", "arab", "oceania", "american"]) && !has(&["nations", "asian cup", "gold cup", "copa america"]) => None,
        _ if has(&["european champions cup", "uefa champions", "champions league"]) => Some(0),
        _ if has(&["uefa cup", "europa league"]) => Some(1),
        _ if has(&["european super", "uefa super"]) => Some(2),
        _ if has(&["club world", "world club", "intercontinental"]) => Some(3),
        _ if has(&["world cup"]) => Some(4),
        _ if has(&["european championship", "copa america", "nations", "asian cup", "gold cup"]) => Some(5),
        _ => None,
    }
}

/// How far a line says a club got, an index of a `FAMILIES` row.
fn stage(f: &Fact) -> Option<usize> {
    match f.kind {
        KIND_WON => Some(0),
        KIND_LOST_FINAL => Some(1),
        KIND_QUALIFIED | KIND_KNOCKED_OUT if f.text.contains("semi final") => Some(2),
        KIND_QUALIFIED | KIND_KNOCKED_OUT if f.text.contains("quarter final") => Some(3),
        KIND_QUALIFIED | KIND_KNOCKED_OUT if f.text.contains("phase 2") => Some(4),
        _ => None,
    }
}

/// A season's prizes from a manager's history lines of that season: of a cup only the furthest
/// round, of a league the title or the place, promotion only without the title.
fn prizes(facts: &[Fact]) -> Vec<String> {
    let mut best = [None::<usize>; FAMILIES.len()];
    let mut out = Vec::new();
    for f in facts {
        if let (Some(fam), Some(s)) = (family(&f.comp, f.domestic, f.top), stage(f)) {
            best[fam] = Some(best[fam].map_or(s, |b: usize| b.min(s)));
        }
        if f.kind == KIND_CHAMPIONS {
            out.push(if f.top { "Kampioen" } else { "Kampioen LL" }.to_string());
        }
        // (a lower league won by a group or a play-off has no champions line)
        if f.kind == KIND_FINISHED && !f.top && f.text.contains("finished 1st") && !out.iter().any(|i| i == "Kampioen LL") {
            out.push("Kampioen LL".to_string());
        }
        if f.kind == KIND_FINISHED && f.top {
            for (place, item) in [("finished 2nd", "2e plaats"), ("finished 3rd", "3e plaats")] {
                if f.text.contains(place) {
                    out.push(item.to_string());
                }
            }
        }
    }
    if facts.iter().any(|f| f.kind == KIND_PROMOTED) && !out.iter().any(|i| i == "Kampioen LL") {
        out.push("Promotie LL".to_string());
    }
    for (fam, s) in best.iter().enumerate() {
        if let Some(item) = s.map(|s| FAMILIES[fam][s]).filter(|i| !i.is_empty()) {
            out.push(item.to_string());
        }
    }
    out
}

/// Two spellings of a club: the old site's "AC Milan" and the game's "Milan".
fn same_club(a: &str, b: &str) -> bool {
    let plain = |s: &str| s.chars().filter(|c| c.is_alphanumeric() && c.is_ascii()).collect::<String>().to_lowercase();
    let (a, b) = (plain(a), plain(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

/// The fifth season running at a club counts once (`clubs`: a manager's by season).
fn loyalty(clubs: &[String], k: usize) -> Option<&'static str> {
    (clubs[..k].iter().rev().take_while(|c| same_club(c, &clubs[k])).count() == 4).then_some("5+ seizoenen (loyaliteit)")
}

/// A line of a manager's jobs: a date, the club (a national team's left out).
#[derive(Clone, Copy)]
struct Job {
    year: i32,
    day: i16,
    club: usize,
    hired: bool,
}

/// What leaving a club costs, by season: by the time really there, so hired in April and gone the
/// next June is one season.
fn moves(jobs: &[Job]) -> Vec<(i32, &'static str)> {
    let mut out = Vec::new();
    let mut spell: Option<Job> = None;
    for j in jobs {
        if let Some(s) = spell.filter(|s| s.club != j.club || !j.hired) {
            let days = (j.year - s.year) * 365 + (j.day - s.day) as i32;
            let season = if j.day < LEFT_UNTIL { j.year - 1 } else { j.year };
            match (days + 100) / 365 {
                0 | 1 => out.push((season, "Vertrek na 1 seizoen")),
                2 => out.push((season, "Vertrek na 2 seizoenen")),
                _ => {}
            }
        }
        spell = if !j.hired { None } else { spell.filter(|s| s.club == j.club).or(Some(*j)) };
    }
    out
}

#[derive(Clone, Default)]
struct Season {
    name: String,
    year: i32,
    club: String,
    scorer: String,
    goals: String,
    rating: String,
    items: Vec<String>,
}

#[derive(Clone, Default)]
struct Rad {
    id: u32,
    name: String,
    year: i32,
    points: i32,
    done: bool,
    text: String,
    state: u8,          // what the game says so far: `track::OPEN`, `GOOD`, `BAD`
    value: Option<i32>, // the last reading; None: the game never judged this row
    note: String,
    manual: bool, // the host set `done` himself: the game leaves it alone
    closed: bool, // the season is over and `done` is settled
}

#[derive(Default)]
struct League {
    seasons: Vec<Season>,
    rads: Vec<Rad>,
    points: Vec<(String, i32, bool)>, // item, points, a trophy
    prizes: Vec<(String, String, i32, bool)>, // the same with their category, for the Regels page
    ranks: Vec<(String, i32)>,
    tasks: Vec<(String, String, i32)>, // category, text, points
    best: Vec<(String, i32, i32)>,     // a manager's best player on 1 January: name, season, staff index
}

impl League {
    fn parse(rules: &str, file: &str) -> League {
        let mut l = League::default();
        for line in rules.lines().chain(file.lines()) {
            let c: Vec<&str> = line.split('\t').collect();
            let number = |k: usize| c.get(k).and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
            let text = |k: usize| c.get(k).copied().unwrap_or("").to_string();
            match c[0] {
                "P" => {
                    l.points.push((text(2), number(3), number(4) != 0));
                    l.prizes.push((text(1), text(2), number(3), number(4) != 0));
                }
                "G" => l.ranks.push((text(1), number(2))),
                "C" => l.tasks.push((text(1), text(2), number(3))),
                "S" => l.seasons.push(Season {
                    name: text(1),
                    year: number(2),
                    club: text(3),
                    scorer: text(4),
                    goals: text(5),
                    rating: text(6),
                    items: text(7).split('|').filter(|i| !i.is_empty()).map(str::to_string).collect(),
                }),
                "R" => l.rads.push(Rad {
                    id: number(1) as u32,
                    name: text(2),
                    year: number(3),
                    points: number(4),
                    done: number(5) != 0,
                    text: text(6),
                    state: number(7) as u8,
                    value: c.get(8).and_then(|s| s.parse().ok()),
                    note: text(9),
                    manual: number(10) != 0,
                    closed: number(11) != 0,
                }),
                "B" => l.best.push((text(1), number(2), number(3))),
                _ => {}
            }
        }
        l.ranks.sort_by_key(|r| r.1);
        l
    }

    /// The file: the rules stay in the DLL.
    fn lines(&self) -> String {
        let mut out = String::new();
        for s in &self.seasons {
            out += &format!("S\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", s.name, s.year, s.club, s.scorer, s.goals, s.rating, s.items.join("|"));
        }
        for r in &self.rads {
            let value = r.value.map_or(String::new(), |v| v.to_string());
            out += &format!("R\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{value}\t{}\t{}\t{}\n", r.id, r.name, r.year, r.points, r.done as i32, r.text, r.state, r.note, r.manual as i32, r.closed as i32);
        }
        for b in &self.best {
            out += &format!("B\t{}\t{}\t{}\n", b.0, b.1, b.2);
        }
        out
    }

    fn item_points(&self, items: &[String]) -> i32 {
        items.iter().filter_map(|i| self.points.iter().find(|p| p.0 == *i)).map(|p| p.1).sum()
    }

    fn rad_points(&self, name: &str, year: Option<i32>) -> i32 {
        self.rads.iter().filter(|r| r.done && same_manager(&r.name, name) && year.is_none_or(|y| y == r.year)).map(|r| r.points).sum()
    }

    fn trophies(&self, items: &[String]) -> usize {
        items.iter().filter(|i| self.points.iter().any(|p| p.0 == **i && p.2)).count()
    }

    /// Rank, its next bound and the % of the way there, as the site's getRank.
    fn rank(&self, points: i32) -> (String, i32, i32) {
        let k = self.ranks.iter().rposition(|r| points >= r.1);
        let (name, low) = k.map_or(("Beginner".to_string(), 0), |k| self.ranks[k].clone());
        let high = self.ranks.get(k.map_or(0, |k| k + 1)).map_or(low, |r| r.1);
        (name, high, if high > low { ((points - low) * 100 / (high - low)).clamp(0, 100) } else { 100 })
    }

    /// A new RAD row, by the site's rules: no second challenge of a category in a season, none
    /// the manager had before while his category has others left, none another manager has this
    /// season. Which of those by `pick`, a random number.
    fn roll(&mut self, name: &str, year: i32, pick: usize) -> bool {
        let mine: Vec<&Rad> = self.rads.iter().filter(|r| r.name == name).collect();
        let category = |text: &str| self.tasks.iter().find(|t| t.1 == text).map(|t| t.0.clone());
        let used: Vec<String> = mine.iter().filter(|r| r.year == year).filter_map(|r| category(&r.text)).collect();
        let others: Vec<&str> = self.rads.iter().filter(|r| r.year == year).map(|r| r.text.as_str()).collect();
        let mut pool: Vec<&(String, String, i32)> = Vec::new();
        let mut cats: Vec<&String> = self.tasks.iter().map(|t| &t.0).collect();
        cats.dedup();
        for cat in cats.into_iter().filter(|c| !used.contains(c)) {
            let all: Vec<&(String, String, i32)> = self.tasks.iter().filter(|t| t.0 == *cat).collect();
            let fresh: Vec<&(String, String, i32)> = all.iter().copied().filter(|t| !mine.iter().any(|r| r.text == t.1)).collect();
            pool.extend((if fresh.is_empty() { all } else { fresh }).into_iter().filter(|t| !others.contains(&t.1.as_str())));
        }
        let Some(&(_, text, points)) = pool.get(pick % pool.len().max(1)).map(|t| (&t.0, &t.1, t.2)).as_ref() else { return false };
        let (text, id) = (text.clone(), self.rads.iter().map(|r| r.id).max().unwrap_or(0) + 1);
        self.rads.push(Rad { id, name: name.to_string(), year, points, text, ..Default::default() });
        true
    }
}

/// The name a line of the file goes by: "Ann" for "Ann Smith".
fn same_manager(file_name: &str, game_name: &str) -> bool {
    let first = |s: &str| s.split_whitespace().next().unwrap_or("").to_lowercase();
    first(file_name) == first(game_name)
}

static mut LEAGUE: Option<League> = None;
static mut FILE: String = String::new(); // this game's file, empty until `use_game`

#[allow(static_mut_refs)]
fn league() -> &'static mut League {
    unsafe { LEAGUE.get_or_insert_with(|| League::parse(&latin1(RULES), "")) }
}

/// A league's file belongs to a game with these managers: a line of it names one of them.
fn belongs(file: &str, names: &[String]) -> bool {
    file.lines().any(|l| {
        let name = l.split('\t').nth(if l.starts_with("R\t") { 2 } else { 1 });
        name.is_some_and(|f| names.iter().any(|n| same_manager(f, n)))
    })
}

/// A number for a game: the first manager and how he started, which no later manager changes.
// ponytail: the same friends at the same clubs from the same day share a league; rename the file
fn game_key(name: &str, year: i32, day: i16, club: usize) -> u32 {
    format!("{name} {year} {day} {club}").bytes().fold(0x811c9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x01000193))
}

/// The league of the game that is loaded: read when the game changes.
#[allow(static_mut_refs)]
unsafe fn use_game() {
    let humans = humans();
    let Some((first, name)) = humans.last() else { return };
    let start = facts_of(*first).1.first().map_or((0, 0, 0), |j| (j.year, j.day, j.club - rd::<usize>(CLUB_TABLE)));
    let file = format!("cmchallenge_{:08x}.tsv", game_key(name, start.0, start.1, start.2));
    if file == FILE {
        return;
    }
    let names: Vec<String> = humans.iter().map(|h| h.1.clone()).collect();
    if !std::path::Path::new(&file).exists() && std::fs::read(IMPORTED).is_ok_and(|t| belongs(&latin1(&t), &names)) {
        let _ = std::fs::rename(IMPORTED, &file);
    }
    LEAGUE = Some(League::parse(&latin1(RULES), &latin1(&std::fs::read(&file).unwrap_or_default())));
    log(&format!("challenge: league file {file}"));
    FILE = file;
}

#[allow(static_mut_refs)]
fn save() {
    unsafe {
        if FILE.is_empty() {
            return;
        }
        let temp = format!("{FILE}.tmp");
        if std::fs::write(&temp, to_latin1(&league().lines())).is_ok() {
            let _ = std::fs::rename(&temp, &FILE);
        }
    }
}

/// A network game: a client, or a host with a second manager in it.
// ponytail: two humans at one machine count too; the league needs two managers either way
unsafe fn network_game() -> bool {
    remote::CLIENT || human_staff().len() >= 2
}

/// The human managers' staff records: the end of the staff table, +0x5f set.
unsafe fn human_staff() -> Vec<usize> {
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    (1..=16.min(count)).map(|k| base + (count - k) as usize * 0x6e).filter(|&s| readable(s, 0x6e) && rd::<u8>(s + 0x5f) == 1).collect()
}

unsafe fn humans() -> Vec<(usize, String)> {
    let name = |s: usize| {
        let mut buf = [0u8; 200];
        game::<extern "C" fn(usize, *mut u8, i16, u8)>(FORMAT_NAME)(s, buf.as_mut_ptr(), 200, 1);
        buffer_text(&buf)
    };
    human_staff().into_iter().map(|s| (s, name(s))).collect()
}

/// A manager's history, oldest first, and his jobs at clubs.
unsafe fn facts_of(staff: usize) -> (Vec<Fact>, Vec<Job>) {
    let (mut facts, mut jobs) = (Vec::new(), Vec::new());
    let records = rd::<usize>(history::STAFF_RECORDS);
    if !mapped(records, 4) {
        return (facts, jobs);
    }
    let (comps, comp_count) = (rd::<usize>(COMPS), rd::<i32>(COMP_COUNT));
    // (the highest reputation of a nation's competitions is its first division's; of its cups,
    // by name, the main cup's)
    let cup = |c: usize| {
        let name = latin1(&game_text(c + 4, 51)).to_lowercase();
        [" cup", "kupa", "beker", "pokal", "coppa", "copa", "coupe", "taca"].iter().any(|w| name.contains(w))
    };
    let top_of = |comp: usize| (0..comp_count as usize).map(|k| comps + k * COMP_SIZE).filter(|&c| rd::<usize>(c + COMP_NATION) == rd::<usize>(comp + COMP_NATION) && (!cup(comp) || cup(c))).all(|c| rd::<i16>(c + COMP_REPUTATION) <= rd::<i16>(comp + COMP_REPUTATION));
    let list = game::<extern "thiscall" fn(usize, usize, u32) -> usize>(history::STAFF_LIST)(records, staff, 0);
    for item in history::newest(list).into_iter().rev() {
        let (day, year, kind) = (rd::<i16>(item), rd::<i16>(item + 2) as i32, rd::<u8>(item + 0xc));
        let season = if day >= SEASON_FROM { year } else { year - 1 };
        if kind == KIND_JOB {
            let (club, hired) = (rd::<i32>(item + 8), rd::<u8>(item + 0xd) == 1);
            if trace() {
                log(&format!("challenge job {staff:#x} day {day} {year} club {club} hired {hired}: {}", latin1(&history::sentence(records, item))));
            }
            if (0..rd::<i32>(CLUB_COUNT)).contains(&club) {
                let club = rd::<usize>(CLUB_TABLE) + club as usize * CLUB_SIZE;
                if !national_team(club) {
                    jobs.push(Job { year, day, club, hired });
                }
            }
            continue;
        }
        if ![KIND_WON, KIND_LOST_FINAL, KIND_QUALIFIED, KIND_KNOCKED_OUT, KIND_CHAMPIONS, KIND_PROMOTED, KIND_FINISHED].contains(&kind) {
            continue;
        }
        let index = rd::<i32>(item + 0xd);
        if !(0..comp_count).contains(&index) {
            continue;
        }
        let comp = comps + index as usize * COMP_SIZE;
        let nation = rd::<usize>(comp + COMP_NATION);
        if trace() {
            log(&format!("challenge fact {staff:#x} day {day} {year} kind {kind} comp {comp:#x} {} nation {nation:#x}: {}", latin1(&game_text(comp + 4, 51)), latin1(&history::sentence(records, item))));
        }
        facts.push(Fact {
            season,
            kind,
            comp: latin1(&game_text(comp + 4, 51)).to_lowercase(),
            domestic: nation != 0 && nation != usize::MAX - 1,
            top: top_of(comp),
            table: comp,
            year,
            text: latin1(&history::sentence(records, item)).to_lowercase(),
        });
    }
    history::free(list);
    (facts, jobs)
}

/// The club a manager ended a season with: the last hired by then, else the club he left first,
/// else the one he has now.
unsafe fn club_in(season: i32, jobs: &[Job], now: usize) -> usize {
    let starts = |j: &Job| if j.day >= HIRED_FROM { j.year } else { j.year - 1 };
    jobs.iter().rev().find(|j| j.hired && starts(j) <= season).or(jobs.iter().find(|j| !j.hired)).map_or(now, |j| j.club)
}

/// A title that ends a long wait: the same competition won before, 10, 20 or 30 years back, in the
/// competitions' result rows (`year` their calendar year; a club that never won it gets nothing,
/// as on the site), and no other title of the club within a year (a supercup in August ended
/// no wait after the league in May).
unsafe fn drought(club: usize, comp: usize, year: i32) -> Option<&'static str> {
    let (base, count) = (rd::<usize>(comp::HISTORY), rd::<i32>(comp::HISTORY_COUNT));
    if club == 0 || count <= 0 || !readable(base, count as usize * 26) {
        return None;
    }
    let won: Vec<(usize, i32)> = (0..count as usize).map(|k| base + k * 26).filter(|&r| rd::<usize>(r + 0xa) == club).map(|r| (rd::<usize>(r + 4), rd::<i16>(r + 8) as i32)).collect();
    if won.iter().any(|w| w.0 != comp && (year - 1..=year).contains(&w.1)) {
        return None;
    }
    let last = won.iter().filter(|w| w.0 == comp && w.1 < year).map(|w| w.1).max()?;
    [(30, "Eerste prijs 30+ jaar"), (20, "Eerste prijs 20+ jaar"), (10, "Eerste prijs 10+ jaar")].into_iter().find(|t| year - last >= t.0).map(|t| t.1)
}

/// The club's league top scorer this season: name, goals.
unsafe fn top_scorer(club: usize) -> Option<(String, i32)> {
    if !readable(club, 0xd7 + 50 * 4) {
        return None;
    }
    (0..50)
        .map(|k| rd::<usize>(club + 0xd7 + k * 4))
        .filter(|&s| valid_staff(s))
        .map(|s| (latin1(&staff_name(s)).to_string(), latin1(&season_stat(s, 2)).trim().parse::<i32>().unwrap_or(0)))
        .max_by_key(|p| p.1)
        .filter(|p| p.1 > 0)
}

unsafe fn this_season() -> i32 {
    let (day, year) = (rd::<i16>(TODAY), rd::<i16>(TODAY + 2) as i32);
    if day >= SEASON_FROM { year } else { year - 1 }
}

/// Every manager's seasons: as entered where the file has them, else from the game. This
/// season's top scorer goes in the file as it stands, so last season's is there at the turn.
unsafe fn seasons() -> Vec<Season> {
    let l = league();
    let now = this_season();
    let mut all = l.seasons.clone();
    let mut changed = false;
    for (staff, name) in humans() {
        let (facts, jobs) = facts_of(staff);
        let current = rd::<usize>(staff + 0x39);
        let first = facts.iter().map(|f| f.season).chain(jobs.iter().map(|j| j.year)).min().unwrap_or(now).min(now);
        let entered: Vec<Season> = all.iter().filter(|s| same_manager(&s.name, &name)).cloned().collect();
        let years: Vec<i32> = entered.iter().map(|s| s.year).chain(first..=now).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let clubs: Vec<usize> = years.iter().map(|&y| club_in(y, &jobs, current)).collect();
        let club_name = |c: usize| if readable(c, 0x40) { latin1(&game_text(c + 4, 51)).to_string() } else { String::new() };
        let names: Vec<String> = years.iter().enumerate().map(|(k, y)| entered.iter().find(|s| s.year == *y).map_or(club_name(clubs[k]), |s| s.club.clone())).collect();
        for (k, &year) in years.iter().enumerate() {
            if entered.iter().any(|s| s.year == year && !s.items.is_empty()) {
                continue;
            }
            let of_year: Vec<Fact> = facts.iter().filter(|f| f.season == year).cloned().collect();
            let mut items = prizes(&of_year);
            items.extend(loyalty(&names, k).map(str::to_string));
            items.extend(moves(&jobs).into_iter().filter(|m| m.0 == year).map(|m| m.1.to_string()));
            // (the first division's title or a main or European cup: a lower league's counts not)
            let waited = of_year.iter().filter(|f| [KIND_WON, KIND_CHAMPIONS].contains(&f.kind) && (f.top || !f.domestic));
            items.extend(waited.filter_map(|f| drought(clubs[k], f.table, f.year)).min().map(str::to_string));
            let kept = l.seasons.iter().position(|s| s.year == year && same_manager(&s.name, &name));
            if year == now {
                if let (Some((scorer, goals)), None) = (top_scorer(current), kept.filter(|&i| !l.seasons[i].items.is_empty())) {
                    let goals = goals.to_string();
                    match kept {
                        Some(i) if l.seasons[i].scorer == scorer && l.seasons[i].goals == goals => {}
                        Some(i) => (l.seasons[i].scorer, l.seasons[i].goals, changed) = (scorer, goals, true),
                        None => {
                            l.seasons.push(Season { name: name.clone(), year, club: names[k].clone(), scorer, goals, ..Default::default() });
                            changed = true;
                        }
                    }
                }
            }
            let (scorer, goals) = l.seasons.iter().find(|s| s.year == year && same_manager(&s.name, &name)).map_or((String::new(), String::new()), |s| (s.scorer.clone(), s.goals.clone()));
            all.retain(|s| !(s.year == year && same_manager(&s.name, &name)));
            all.push(Season { name: name.clone(), year, club: names[k].clone(), scorer, goals, rating: String::new(), items });
        }
    }
    if changed {
        save();
    }
    all.sort_by_key(|s| s.year);
    all
}

/// The page as text, a line a thing, for whoever draws it. `me`: the name of the manager asking,
/// `all`: he is the host, who spins and ticks for everyone.
unsafe fn page(me: &str, all: bool) -> String {
    use_game();
    let rows = seasons();
    let l = league();
    let now = this_season();
    let mut names: Vec<String> = Vec::new();
    for s in &rows {
        if !names.iter().any(|n| same_manager(n, &s.name)) {
            names.push(s.name.clone());
        }
    }
    let mine = |s: &Season, n: &str| same_manager(&s.name, n);
    let total_of = |n: &String| rows.iter().filter(|s| mine(s, n)).map(|s| l.item_points(&s.items)).sum::<i32>() + l.rad_points(n, None);
    names.sort_by_key(|n| -total_of(n));
    let mut out = format!("C\t{}/{}\t{}\t{}\t{now}\n", now, now + 1, me, all as i32);
    for n in &names {
        let total = total_of(n);
        let (rank, next, percent) = l.rank(total);
        let step = l.ranks.iter().filter(|r| total >= r.1).count();
        let club = rows.iter().rev().find(|s| mine(s, n)).map_or("", |s| &s.club);
        out += &format!("K\t{n}\t{club}\t{total}\t{rank}\t{next}\t{percent}\t{step}\n");
    }
    for r in &l.ranks {
        out += &format!("G\t{}\t{}\n", r.0, r.1);
    }
    let clubs_of = |n: &String| {
        let mut c: Vec<&str> = Vec::new();
        for s in rows.iter().filter(|s| mine(s, n)) {
            if !c.iter().any(|o| same_club(o, &s.club)) {
                c.push(&s.club);
            }
        }
        c.len()
    };
    let years = rows.iter().map(|s| s.year).collect::<std::collections::BTreeSet<_>>().len();
    let trophies: usize = rows.iter().map(|s| l.trophies(&s.items)).sum();
    out += &format!("F\tSeizoenen\t{years}\nF\tPrijzen\t{trophies}\n");
    if let Some(n) = names.iter().max_by_key(|n| clubs_of(n)) {
        out += &format!("F\tClubhopper\t{n} ({} clubs)\n", clubs_of(n));
    }
    if let Some(s) = rows.iter().filter(|s| !s.goals.is_empty()).max_by_key(|s| s.goals.parse::<i32>().unwrap_or(0)) {
        out += &format!("F\tBeste topscorer\t{} ({})\n", s.scorer, s.goals);
    }
    for n in &names {
        let (items, club) = rows.iter().find(|s| s.year == now && mine(s, n)).map_or((Vec::new(), ""), |s| (s.items.clone(), s.club.as_str()));
        let points = l.item_points(&items) + l.rad_points(n, Some(now));
        let shown: Vec<String> = items.iter().map(|i| format!("{i};{};{}", l.item_points(std::slice::from_ref(i)), l.trophies(std::slice::from_ref(i)))).collect();
        out += &format!("L\t{n}\t{club}\t{points}\t{}\n", shown.join("|"));
    }
    for r in &l.rads {
        out += &format!("R\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.id, r.name, r.points, r.done as i32, r.text, r.year, r.state, r.note, track::rule(&r.text).is_some() as i32, r.manual as i32);
    }
    for s in rows.iter().rev().take(HISTORY_ROWS) {
        let points = l.item_points(&s.items) + l.rad_points(&s.name, Some(s.year));
        out += &format!("H\t{}/{}\t{}\t{}\t{}\t{}\t{}\t{points}\t{}\n", s.year, s.year + 1, s.name, s.club, s.scorer, s.goals, s.rating, s.items.join(", "));
    }
    out += &stats_lines(&rows, &names);
    for (category, item, points, trophy) in &l.prizes {
        out += &format!("P\t{category}\t{item}\t{points}\t{}\n", *trophy as i32);
    }
    for t in &l.tasks {
        out += &format!("A\t{}\t{}\t{}\t{}\n", t.0, t.1, t.2, track::rule(&t.1).is_some() as i32);
    }
    out
}

/// The Stats page: the three records, then a manager's card.
fn stats_lines(rows: &[Season], names: &[String]) -> String {
    let l = league();
    let total = |s: &Season| l.item_points(&s.items) + l.rad_points(&s.name, Some(s.year));
    let mut out = String::new();
    if let Some(s) = rows.iter().max_by_key(|s| total(s)) {
        out += &format!("T\tRecord Seizoen\t{}\t{} ({}/{})\n", total(s), s.name, s.year, s.year + 1);
    }
    let trophies = |n: &String| rows.iter().filter(|s| same_manager(&s.name, n)).map(|s| l.trophies(&s.items)).sum::<usize>();
    let done = |n: &String| l.rads.iter().filter(|r| r.done && same_manager(&r.name, n)).count();
    if let Some(n) = names.iter().max_by_key(|n| trophies(n)) {
        out += &format!("T\tTrophy Collector\t{}\t{n}\n", trophies(n));
    }
    if let Some(n) = names.iter().max_by_key(|n| done(n)) {
        out += &format!("T\tRAD Meester\t{}\t{n}\n", done(n));
    }
    for n in names {
        let own: Vec<&Season> = rows.iter().filter(|s| same_manager(&s.name, n)).collect();
        let items: i32 = own.iter().map(|s| l.item_points(&s.items)).sum();
        let rads = l.rad_points(n, None);
        let (rank, _, _) = l.rank(items + rads);
        let season = |s: &Season| format!("{} ({}/{})", total(s), s.year, s.year + 1);
        let (best, worst) = (own.iter().copied().max_by_key(|s| total(s)), own.iter().copied().min_by_key(|s| total(s)));
        let avg = if own.is_empty() { 0 } else { (items + rads) / own.len() as i32 };
        let mut clubs: Vec<&str> = Vec::new();
        for s in &own {
            if !clubs.iter().any(|c| same_club(c, &s.club)) {
                clubs.push(&s.club);
            }
        }
        let bars: Vec<String> = own.iter().map(|s| format!("{}={}", s.year % 100, total(s))).collect();
        let mut palmares: Vec<String> = Vec::new();
        for c in &clubs {
            let won: Vec<&String> = own.iter().filter(|s| same_club(&s.club, c)).flat_map(|s| s.items.iter()).filter(|i| l.trophies(std::slice::from_ref(i)) > 0).collect();
            let mut seen: Vec<&String> = Vec::new();
            for i in &won {
                if !seen.contains(i) {
                    seen.push(i);
                    palmares.push(format!("{c};{}x {i}", won.iter().filter(|w| w == &i).count()));
                }
            }
        }
        let scorer = own.iter().copied().filter(|s| !s.goals.is_empty()).max_by_key(|s| s.goals.parse::<i32>().unwrap_or(0)).map_or(String::new(), |s| format!("{} ({})", s.scorer, s.goals));
        let all_rads = l.rads.iter().filter(|r| same_manager(&r.name, n)).count();
        out += &format!(
            "Y\t{n}\t{rank}\t{}\t{avg}\t{}\t{}\t{}\t{}\t{all_rads}\t{items}\t{rads}\t{scorer}\t{}\t{}\n",
            own.len(),
            best.map_or(String::new(), season),
            worst.map_or(String::new(), season),
            clubs.len(),
            done(n),
            bars.join(","),
            palmares.join("|"),
        );
    }
    out
}

const ACT_NONE: u32 = 0;
const ACT_ROLL: u32 = 1;
const ACT_TICK: u32 = 2;
const ACT_DELETE: u32 = 3;

/// The host does a click: a new RAD row for a manager, a row ticked (his word then stands over the
/// game's) or a row taken away.
unsafe fn act(action: u32, arg: u32, name: &[u8]) {
    use_game();
    let l = league();
    match action {
        ACT_ROLL => {
            let who = latin1(name).to_string();
            // (the file's own spelling when it has one, so a manager's rows stay together)
            let who = l.rads.iter().find(|r| same_manager(&r.name, &who)).map_or(who, |r| r.name.clone());
            let pick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |t| t.subsec_micros()) as usize;
            if l.roll(&who, this_season(), pick) {
                track::update();
                save();
            }
        }
        ACT_TICK => {
            if let Some(r) = l.rads.iter_mut().find(|r| r.id == arg) {
                (r.done, r.manual) = (!r.done, true);
                save();
            }
        }
        ACT_DELETE => {
            l.rads.retain(|r| r.id != arg);
            save();
        }
        _ => {}
    }
}

/// Host: a client's 'E': the page for its manager, read only.
pub unsafe fn remote(channel: i16) -> Vec<u8> {
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel.max(0) as usize);
    let me = humans().into_iter().find(|h| h.0 == manager).map_or(String::new(), |h| h.1);
    to_latin1(&page(&me, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(kind: u8, comp: &str, domestic: bool, top: bool, text: &str) -> Fact {
        Fact { season: 2032, kind, comp: comp.to_string(), domestic, top, table: 0, year: 2033, text: text.to_string() }
    }

    /// A season as on the old site, from what the game writes.
    #[test]
    fn a_season_as_entered_by_hand() {
        let got = prizes(&[
            fact(KIND_QUALIFIED, "european champions cup", false, false, "qualified for european champions cup phase 2"),
            fact(KIND_QUALIFIED, "european champions cup", false, false, "qualified for european champions cup semi final"),
            fact(KIND_LOST_FINAL, "european champions cup", false, false, "european champions cup runners up"),
            fact(KIND_WON, "dutch super cup", true, false, "dutch super cup winners"),
            fact(KIND_WON, "european super cup", false, false, "european super cup winners"),
            fact(KIND_WON, "fifa club world championship", false, false, "fifa club world championship winners"),
            fact(KIND_CHAMPIONS, "dutch premier division", true, true, "dutch premier division champions"),
            fact(KIND_FINISHED, "dutch premier division", true, true, "finished 1st in dutch premier division"),
        ]);
        assert_eq!(got, ["Kampioen", "CL Finalist", "Europese Supercup Winnaar", "WK Clubteams Winnaar", "Nat. Supercup"]);
    }

    #[test]
    fn lower_leagues() {
        assert_eq!(prizes(&[fact(KIND_CHAMPIONS, "turkish 2. division group 4", true, false, ""), fact(KIND_PROMOTED, "turkish 2. division group 4", true, false, "")]), ["Kampioen LL"]);
        assert_eq!(prizes(&[fact(KIND_PROMOTED, "german second division", true, false, "")]), ["Promotie LL"]);
        assert_eq!(prizes(&[fact(KIND_FINISHED, "turkish 2. division group 4", true, false, "finished 1st in turkish 2. division group 4"), fact(KIND_PROMOTED, "", true, false, "")]), ["Kampioen LL"]);
        assert_eq!(prizes(&[fact(KIND_FINISHED, "scottish first division", true, false, "finished 2nd in scottish first division")]), Vec::<String>::new());
        assert_eq!(prizes(&[fact(KIND_FINISHED, "italian serie a", true, true, "finished 2nd in italian serie a")]), ["2e plaats"]);
    }

    #[test]
    fn cups_go_by_the_furthest_round() {
        let semi = fact(KIND_KNOCKED_OUT, "uefa cup", false, false, "knocked out in uefa cup semi final leg 2");
        let quarter = fact(KIND_QUALIFIED, "uefa cup", false, false, "qualified for uefa cup quarter finals");
        assert_eq!(prizes(&[quarter, semi]), ["UEFA Cup Halve finale"]);
        assert_eq!(family("asian champions cup", false, false), None);
        assert_eq!(family("european cup winners cup", false, false), None);
        assert_eq!(family("european championship", false, false), Some(5));
        assert_eq!(family("scottish cup", true, true), Some(6));
        assert_eq!(family("scottish league cup", true, false), None);
    }

    #[test]
    fn club_moves() {
        let clubs: Vec<String> = ["Ajax", "Ajax", "Ajax", "Ajax", "Ajax", "Ajax", "AC Milan", "Milan"].iter().map(|s| s.to_string()).collect();
        assert_eq!(loyalty(&clubs, 4), Some("5+ seizoenen (loyaliteit)"));
        assert_eq!((loyalty(&clubs, 3), loyalty(&clubs, 5)), (None, None));
        assert!(same_club("AC Milan", "Milan") && !same_club("Ajax", "Milan"));
        // hired in April, gone the next June; then two seasons at a third club
        let job = |year, day, club| Job { year, day, club, hired: true };
        let jobs = [job(2025, 192, 1), job(2030, 116, 2), job(2031, 152, 3), job(2033, 180, 4)];
        assert_eq!(moves(&jobs), [(2030, "Vertrek na 1 seizoen"), (2032, "Vertrek na 2 seizoenen")]);
    }

    fn sample() -> League {
        let rules = "P\tNationaal\tKampioen\t10\t1\nP\tTeamwissel\tVertrek na 1 seizoen\t-10\t0\nG\tBronze\t25\nG\tSilver\t75\nC\tLeeftijd\tA\t6\nC\tLeeftijd\tB\t3\nC\tStaff\tC\t3\n";
        let file = "S\tAnn\t2031\tAjax\tDoe\t50\t10\tKampioen|Vertrek na 1 seizoen\nS\tAnn\t2032\tAjax\t\t\t\tKampioen\nR\t1\tAnn\t2032\t6\t1\tA\nR\t2\tAnn\t2032\t3\t0\tB\n";
        League::parse(rules, file)
    }

    #[test]
    fn a_new_game_starts_its_own_league() {
        assert!(belongs("S\tAnn\t2031\tAjax\nR\t1\tBob\t2031\t3\t0\tX\n", &["Bob Jones".to_string()]));
        assert!(!belongs("S\tAnn\t2031\tAjax\n", &["Carl Doe".to_string()]));
        assert_ne!(game_key("Ann Smith", 2025, 192, 16), game_key("Ann Smith", 2025, 192, 17));
    }

    #[test]
    fn the_file_goes_round() {
        let l = sample();
        assert_eq!(League::parse("", &l.lines()).lines(), l.lines());
        assert_eq!(l.seasons[0].items.len(), 2);
    }

    #[test]
    fn points_and_ranks_as_the_site() {
        let l = sample();
        assert_eq!(l.item_points(&l.seasons[0].items), 0);
        assert_eq!(l.rad_points("Ann", Some(2032)), 6);
        assert_eq!(l.rank(10), ("Beginner".to_string(), 25, 40));
        assert_eq!(l.rank(50), ("Bronze".to_string(), 75, 50));
        assert_eq!(l.rank(80), ("Silver".to_string(), 75, 100));
    }

    #[test]
    fn a_roll_keeps_the_rules() {
        let mut l = sample();
        // (Leeftijd is used this season, so only Staff's C is left)
        for pick in 0..3 {
            let mut l2 = sample();
            assert!(l2.roll("Ann", 2032, pick));
            assert_eq!(l2.rads.last().unwrap().text, "C");
        }
        assert!(l.roll("Ann", 2032, 0));
        assert!(!l.roll("Ann", 2032, 0)); // nothing left this season
    }
}
