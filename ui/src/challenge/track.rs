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
pub(super) fn passed(way: Way, value: i32, state: u8) -> bool {
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
    // the squad (the RAD review, 2026-11-06): the starting XI is the 11 with most league starts this
    // season, the first team who played 10+ league games, as `ui\challenge_sim.py` judges them
    clean_sheets: i32, // a keeper's league clean sheets, the most
    xi_35: i32,
    xi_32: i32,
    xi_30: i32,
    xi_u23: i32,
    xi_u21: i32,
    xi_u18: i32,
    xi_age10: i32,    // the XI's average age, in tenths
    first_age10: i32, // the first team's
    xi_left: i32,
    back_young: i32, // of the XI's keeper and four defenders, how many are under 28 (5 when there are not five)
    capped: i32,     // first team players with caps this season
    africa: i32,
    south_america: i32,
    east: i32,
    scandinavia: i32,
    nbpt: i32,
    big5: i32,
    same_country: i32, // the most first team players of one country not the club's own
    xi_foreign: i32,
    xi_own: i32,
    xi_nations: i32,
    xi_not_nbpt: i32, // of the XI, how many are not of its one NED/BEL/POR/TUR nation (11: not one nation)
    xi_not_big5: i32,
    xi_not_other: i32,
    xi_not_africa: i32,
    xi_not_south_america: i32,
    xi_not_north_america: i32,
    xi_not_asia: i32,
    xi_not_oceania: i32,
    xi_dear: i32, // XI players worth more than 10M
    xi_homegrown: i32,
    xi_youth: i32,
    // the tactics (the match-day tracker, 2026-11-07): of the league matches seen this season, how
    // many broke the challenge; fewer than 10 seen count as broken until there are
    tac_sweeper: i32,
    tac_wing_backs: i32,
    tac_three_up: i32,
    tac_442: i32,
    tac_five_back: i32,
    tac_five_mid: i32,
    tac_wingers: i32,
    tac_no_striker: i32, // matches played without a striker (it wants 15)
    tac_one_back: i32,
}

/// What a match's formation (its eleven slot labels, "GK", "DC", "WBL", "AMR", "FC" ...) does for
/// the tactics challenges, in TACTICS order: with a sweeper, without wing backs, three or more
/// forwards, a 4-4-2, five or more defenders, five or more midfielders, without attacking wingers,
/// without a striker, one defender. Defenders: SW, D*, WB*; midfielders: DM*, M*, AMC; forwards
/// F*, ST.
pub(super) fn match_marks(labels: &[&[u8]]) -> [bool; 9] {
    let count = |f: &dyn Fn(&[u8]) -> bool| labels.iter().filter(|l| f(l)).count();
    let back = count(&|l| l.starts_with(b"SW") || (l.starts_with(b"D") && !l.starts_with(b"DM")) || l.starts_with(b"WB"));
    let mid = count(&|l| l.starts_with(b"DM") || l.starts_with(b"M") || l == b"AMC");
    let up = count(&|l| l.starts_with(b"F") || l.starts_with(b"ST"));
    let wingers = count(&|l| l == b"AML" || l == b"AMR");
    [
        count(&|l| l.starts_with(b"SW")) > 0,
        count(&|l| l.starts_with(b"WB")) == 0,
        up >= 3,
        back == 4 && mid == 4 && up == 2 && wingers == 0,
        back >= 5,
        mid >= 5,
        wingers == 0,
        up == 0,
        back == 1,
    ]
}

/// The tactics challenges' keys in the league file's season stats, in `match_marks` order.
const TACTICS: [&str; 9] = ["tac_sweeper", "tac_no_wing_backs", "tac_three_up", "tac_442", "tac_five_back", "tac_five_mid", "tac_no_wingers", "tac_no_striker", "tac_one_back"];

/// A league match played since the last look: the manager's formation now is the one he played
/// with (the active tactic, 0x5fbd00, as the Assistant reads it); counted in the season's stats:
/// `tac_matches` and a key of `TACTICS` for every challenge the match kept. The first look of a
/// season only marks where the league stands (what came before is not known).
unsafe fn watch_tactics(l: &mut League, name: &str, staff: usize, club: usize, now: i32) -> bool {
    let Some(s) = standing(club) else { return false };
    let seen = l.stat(name, now, "tac_seen");
    let Some(seen) = seen.filter(|&n| n <= s.played) else { return l.set_stat(name, now, "tac_seen", s.played) };
    if s.played == seen {
        return false;
    }
    let humans = rd::<usize>(assistant::HUMAN_MANAGER);
    let formation = if mapped(humans, 4) { game::<extern "thiscall" fn(usize, usize) -> usize>(assistant::ACTIVE_TACTIC)(humans, staff) } else { 0 };
    if !mapped(formation, 0x200) {
        return l.set_stat(name, now, "tac_seen", s.played);
    }
    let slots = assistant::formation_slots(formation);
    let labels: Vec<&[u8]> = slots.iter().take(11).map(|v| v.as_slice()).collect();
    let marks = match_marks(&labels);
    let n = s.played - seen;
    if trace() {
        log(&format!("challenge tactics {name}: {n} match(es), {:?} -> {marks:?}", labels.iter().map(|l| latin1(l).to_string()).collect::<Vec<_>>()));
    }
    let add = |l: &mut League, key: &str, by: i32| {
        let v = l.stat(name, now, key).unwrap_or(0) + by;
        l.set_stat(name, now, key, v)
    };
    add(l, "tac_matches", n);
    for (k, key) in TACTICS.iter().enumerate() {
        if marks[k] {
            add(l, key, n);
        }
    }
    l.set_stat(name, now, "tac_seen", s.played);
    true
}

/// The tactics readings from the season's stats (see `Readings`).
fn tactic_readings(l: &League, name: &str, now: i32, r: &mut Readings) {
    let matches = l.stat(name, now, "tac_matches").unwrap_or(0);
    let kept = |k: usize| l.stat(name, now, TACTICS[k]).unwrap_or(0);
    // (broken: the matches that did not keep it, and the ones still missing to 10)
    let broken = |k: usize| matches - kept(k) + (10 - matches).max(0);
    r.tac_sweeper = broken(0);
    r.tac_wing_backs = matches - kept(1);
    r.tac_three_up = broken(2);
    r.tac_442 = matches - kept(3);
    r.tac_five_back = broken(4);
    r.tac_five_mid = broken(5);
    r.tac_wingers = matches - kept(6);
    r.tac_no_striker = kept(7);
    r.tac_one_back = broken(8);
}

type Reading = fn(&Readings) -> i32;

const EAST: [&str; 28] = ["Albania", "Armenia", "Azerbaijan", "Belarus", "Bosnia", "Bosnia-Herzegovina", "Bulgaria", "Croatia", "Czech Republic", "Czechia", "Estonia", "Georgia", "Hungary", "Kosovo", "Latvia", "Lithuania", "Moldova", "Montenegro", "North Macedonia", "Macedonia", "FYR Macedonia", "Poland", "Romania", "Russia", "Serbia", "Slovakia", "Slovenia", "Ukraine"];
const SCANDINAVIA: [&str; 6] = ["Denmark", "Norway", "Sweden", "Finland", "Iceland", "Faroe Islands"];
const NBPT: [&str; 6] = ["Holland", "Netherlands", "Belgium", "Portugal", "Turkey", "Türkiye"];
const BIG5: [&str; 5] = ["England", "Spain", "Germany", "Italy", "France"];

/// The squad's readings (see `Readings`): from this season's league records (0x7c3920; +0 apps,
/// +1 as sub, +3 clean sheets), birth dates (+0x10), nations (+0x1a, their continent +0x71),
/// feet (player +0x30 / +0x3b), values (+0x52), the day each joined (+0x3e).
unsafe fn read_squad(club: usize, r: &mut Readings) {
    let stats = rd::<usize>(PLAYER_STATS);
    if !readable(stats, 0x34) {
        return;
    }
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    // (the club's country by name: the league's nation field is not the nation record a player's
    // +0x1a points at, so pointers never match; seen on Glory_v2, Dortmund's Germans were foreign)
    let own_name = {
        let league = rd::<usize>(club + 0x57);
        if readable(league, COMP_SIZE) { super::nation_name(rd::<usize>(league + COMP_NATION)) } else { String::new() }
    };
    struct Man {
        starts: i32,
        apps: i32,
        clean: i32,
        age: i32,
        joined_age: i32,
        nation: usize,
        nation_name: String,
        continent: String,
        left: bool,
        keeper: bool,
        defender: bool,
        capped: bool,
        value: i32,
    }
    let mut men: Vec<Man> = Vec::new();
    for s in squad(club) {
        let player = ((s + 0x61) as *const usize).read_unaligned();
        if !readable(player, 70) {
            continue;
        }
        let rec = |row: i32| {
            let p = game::<extern "thiscall" fn(usize, usize, i32) -> usize>(GET_RECORD)(stats, player, row);
            if p != 0 && readable(p, 32) { Some(p) } else { None }
        };
        let league = rec(2);
        let (apps, subs, clean) = league.map_or((0, 0, 0), |p| (rd::<u8>(p) as i32, rd::<u8>(p + 1) as i32, rd::<u8>(p + 3) as i32));
        let (_, _, y) = day_month(s + 0x10);
        let (born, born_day) = if y == 1900 { (rd::<u16>(s + 0x18) as i32, 0) } else { (y, rd::<i16>(s + 0x10) as i32) };
        let (_, _, joined) = day_month(s + 0x3e);
        let nation = rd::<usize>(s + 0x1a);
        let text = |p: usize| if readable(p, 0x40) { latin1(&game_text(p + 4, 51)).to_string() } else { String::new() };
        let continent = if readable(nation, 0x78) { text(rd::<usize>(nation + 0x71)) } else { String::new() };
        men.push(Man {
            starts: apps - subs,
            apps,
            clean,
            age: year - born - (day < born_day) as i32,
            joined_age: if joined > 1900 { joined - born } else { 99 },
            nation,
            nation_name: text(nation),
            continent,
            left: rd::<i8>(player + 0x30) > rd::<i8>(player + 0x3b),
            keeper: rd::<i8>(player + 0xf) >= 15,
            defender: rd::<i8>(player + 0x11) >= 15 && rd::<i8>(player + 0xf) < 15,
            capped: rec(5).is_some_and(|p| rd::<u8>(p) > 0),
            value: ((s + 0x52) as *const i32).read_unaligned(),
        });
    }
    men.sort_by_key(|m| -m.starts);
    let xi: Vec<&Man> = men.iter().filter(|m| m.apps > 0).take(11).collect();
    let first: Vec<&Man> = men.iter().filter(|m| m.apps >= 10).collect();
    let count = |ms: &[&Man], f: &dyn Fn(&Man) -> bool| ms.iter().filter(|m| f(m)).count() as i32;
    let avg10 = |ms: &[&Man]| if ms.is_empty() { 0 } else { ms.iter().map(|m| m.age * 10).sum::<i32>() / ms.len() as i32 };
    r.clean_sheets = men.iter().filter(|m| m.keeper).map(|m| m.clean).max().unwrap_or(0);
    r.xi_35 = count(&xi, &|m| m.age >= 35);
    r.xi_32 = count(&xi, &|m| m.age >= 32);
    r.xi_30 = count(&xi, &|m| m.age >= 30);
    r.xi_u23 = count(&xi, &|m| m.age < 23);
    r.xi_u21 = count(&xi, &|m| m.age < 21);
    r.xi_u18 = count(&xi, &|m| m.age < 18);
    r.xi_age10 = avg10(&xi);
    r.first_age10 = avg10(&first);
    r.xi_left = count(&xi, &|m| m.left);
    let back: Vec<&&Man> = xi.iter().filter(|m| m.keeper).take(1).chain(xi.iter().filter(|m| m.defender).take(4)).collect();
    r.back_young = if back.len() < 5 { 5 } else { back.iter().filter(|m| m.age < 28).count() as i32 };
    r.capped = count(&first, &|m| m.capped);
    let named = |m: &Man, set: &[&str]| set.contains(&m.nation_name.as_str());
    r.africa = count(&first, &|m| m.continent == "Africa");
    r.south_america = count(&first, &|m| m.continent == "South America");
    r.east = count(&first, &|m| named(m, &EAST));
    r.scandinavia = count(&first, &|m| named(m, &SCANDINAVIA));
    r.nbpt = count(&first, &|m| named(m, &NBPT));
    r.big5 = count(&first, &|m| named(m, &BIG5));
    let mut by_nation: Vec<(usize, i32)> = Vec::new();
    for m in first.iter().filter(|m| m.nation_name != own_name) {
        match by_nation.iter_mut().find(|n| n.0 == m.nation) {
            Some(n) => n.1 += 1,
            None => by_nation.push((m.nation, 1)),
        }
    }
    r.same_country = by_nation.iter().map(|n| n.1).max().unwrap_or(0);
    r.xi_foreign = count(&xi, &|m| m.nation_name != own_name);
    r.xi_own = count(&xi, &|m| m.nation_name == own_name);
    let mut nations: Vec<usize> = xi.iter().map(|m| m.nation).collect();
    nations.sort();
    nations.dedup();
    r.xi_nations = nations.len() as i32;
    let one = nations.len() == 1 && xi.len() == 11;
    let group = |set: &[&str]| if one && xi.iter().all(|m| named(m, set)) { 0 } else { 11 };
    r.xi_not_nbpt = group(&NBPT);
    r.xi_not_big5 = group(&BIG5);
    r.xi_not_other = if one && xi.iter().all(|m| !named(m, &NBPT) && !named(m, &BIG5)) { 0 } else { 11 };
    let outside = |continents: &[&str]| if xi.len() < 11 { 11 } else { count(&xi, &|m| !continents.contains(&m.continent.as_str())) };
    r.xi_not_africa = outside(&["Africa"]);
    r.xi_not_south_america = outside(&["South America"]);
    r.xi_not_north_america = outside(&["North America", "Central America"]);
    r.xi_not_asia = outside(&["Asia"]);
    r.xi_not_oceania = outside(&["Oceania"]);
    r.xi_dear = count(&xi, &|m| m.value > 10_000_000);
    r.xi_homegrown = count(&xi, &|m| m.joined_age <= 18);
    r.xi_youth = count(&xi, &|m| m.joined_age <= 18 && m.age <= 21);
}

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
        // the RAD review (2026-11-06): what the squad's records tell, judged at the season's end
        "Team gets no more than 1 red card" => (|r| r.reds, Way::Most(1), "red cards"),
        "Player becomes top scorer, top assists and highest average rating" => (|r| r.leads, Way::EndLeast(3), "of 3"),
        "Keeper 10+ clean sheets" => (|r| r.clean_sheets, Way::Least(10), "clean sheets"),
        "3 players 35+ in the starting XI" => (|r| r.xi_35, Way::EndLeast(3), "players 35+"),
        "5 players 30+ in the starting XI" => (|r| r.xi_30, Way::EndLeast(5), "players 30+"),
        "5 players 32+ in the starting XI" => (|r| r.xi_32, Way::EndLeast(5), "players 32+"),
        "5 players under 21 in the starting XI" => (|r| r.xi_u21, Way::EndLeast(5), "players under 21"),
        "5 players under 23 in the starting XI" => (|r| r.xi_u23, Way::EndLeast(5), "players under 23"),
        "Average age starting XI <=23" => (|r| r.xi_age10, Way::EndMost(230), AGE10),
        "Average age starting XI >=30" => (|r| r.xi_age10, Way::EndLeast(300), AGE10),
        "No player younger than 18 in the starting XI" => (|r| r.xi_u18, Way::EndMost(0), "players under 18"),
        "First team average age <=20" => (|r| r.first_age10, Way::EndMost(200), AGE10),
        "First team average age <=23" => (|r| r.first_age10, Way::EndMost(230), AGE10),
        "First team average age >=30" => (|r| r.first_age10, Way::EndLeast(300), AGE10),
        "First team average age >=32" => (|r| r.first_age10, Way::EndLeast(320), AGE10),
        "5 left-footed players in the starting XI" => (|r| r.xi_left, Way::EndLeast(5), "left-footed"),
        "Keeper + 4 defenders all 28+" => (|r| r.back_young, Way::EndMost(0), "under 28"),
        "At least 3 players with international caps this season" => (|r| r.capped, Way::EndLeast(3), "capped players"),
        "At least 3 homegrown players in the starting XI" => (|r| r.xi_homegrown, Way::EndLeast(3), "homegrown"),
        "At least 2 youth players promoted to the starting XI" => (|r| r.xi_youth, Way::EndLeast(2), "youth players"),
        "No player worth more than \u{80}10M in the starting XI" => (|r| r.xi_dear, Way::EndMost(0), "worth over 10M"),
        "5 players from Africa" => (|r| r.africa, Way::EndLeast(5), "players"),
        "5 players from South America" => (|r| r.south_america, Way::EndLeast(5), "players"),
        "5 players from Eastern Europe" => (|r| r.east, Way::EndLeast(5), "players"),
        "5 players from Scandinavia" => (|r| r.scandinavia, Way::EndLeast(5), "players"),
        "5 foreigners in the starting XI" => (|r| r.xi_foreign, Way::EndLeast(5), "foreigners"),
        "5 players from NED/BEL/POR/TUR" => (|r| r.nbpt, Way::EndLeast(5), "players"),
        "5 players from ENG/SPA/GER/ITA/FRA" => (|r| r.big5, Way::EndLeast(5), "players"),
        "3 players from the same country (not your own)" => (|r| r.same_country, Way::EndLeast(3), "of one country"),
        "No player from your own country in the starting XI (min. 15 times)" => (|r| r.xi_own, Way::EndMost(0), "own country"),
        "At least 5 different nationalities in the starting XI" => (|r| r.xi_nations, Way::EndLeast(5), "nationalities"),
        "Team of 1 nationality from NED/BEL/POR/TUR" => (|r| r.xi_not_nbpt, Way::EndMost(0), "not of it"),
        "Team of 1 nationality from ENG/SPA/GER/ITA/FRA" => (|r| r.xi_not_big5, Way::EndMost(0), "not of it"),
        "Team of 1 nationality from the other countries" => (|r| r.xi_not_other, Way::EndMost(0), "not of it"),
        "Fatbike kids challenge (African players only)" => (|r| r.xi_not_africa, Way::EndMost(0), "not African"),
        "Favela challenge (South American players only)" => (|r| r.xi_not_south_america, Way::EndMost(0), "not South American"),
        "Murica challenge (North American players only)" => (|r| r.xi_not_north_america, Way::EndMost(0), "not North American"),
        "Prawn cracker challenge (Asian players only)" => (|r| r.xi_not_asia, Way::EndMost(0), "not Asian"),
        "Down Under challenge (Oceanian players only)" => (|r| r.xi_not_oceania, Way::EndMost(0), "not Oceanian"),
        // the match-day tracker: league matches since it first looked, 10 at least
        "Play with a sweeper in the starting XI" => (|r| r.tac_sweeper, Way::EndMost(0), "matches without"),
        "Play without wing backs" => (|r| r.tac_wing_backs, Way::Most(0), "matches with"),
        "Play with at least three attackers" => (|r| r.tac_three_up, Way::EndMost(0), "matches without"),
        "Always 4-4-2 (no other formation)" => (|r| r.tac_442, Way::Most(0), "matches without"),
        "Play with at least five defenders" => (|r| r.tac_five_back, Way::EndMost(0), "matches without"),
        "Play with at least five midfielders" => (|r| r.tac_five_mid, Way::EndMost(0), "matches without"),
        "Play without attacking wingers" => (|r| r.tac_wingers, Way::Most(0), "matches with"),
        "Play without a striker (min. 15 competitive matches)" => (|r| r.tac_no_striker, Way::EndLeast(15), "matches"),
        "Play with 1 defender (Kamikaze)" => (|r| r.tac_one_back, Way::EndMost(0), "matches without"),
        _ => return None,
    };
    Some(r)
}

/// An average age kept in tenths.
const AGE10: &str = "average age";

/// The reading in words: "1 / 3 purchases", "€1.5M / €2M", "24.3 / 23 average age".
fn note(value: i32, way: Way, unit: &str) -> String {
    let limit = match way {
        Way::Most(n) | Way::Least(n) | Way::EndMost(n) | Way::EndLeast(n) => n,
    };
    if unit == AGE10 {
        return format!("{:.1} / {} {unit}", value as f32 / 10.0, limit / 10);
    }
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
            r.best_sold += (best == Some(who)) as i32;
        }
    }
    history::free(list);
}

/// The transfer manager's past transfers of this year in memory (TRANSFER_MANAGER::get_past_transfers
/// 0x8cc030 reads them, the club screen's Players In / Out shows them): 37-byte rows, +0 the staff
/// index, +0x10 the club he joined (-1: none, "Free Transfer"), +0x14 the club he left, +0x18 the
/// fee, +0x1c the kind (0 a transfer, 2 a loan, 3 let go to no club, 4 a free transfer at the end of
/// a contract), +0x1d the date (i16 day, i16 year). Traced on the user's Dorking game: Hebert's sale
/// to Rotherham is (Rotherham, Dorking, fee, 0); the five released on 24.10.28 (Dorking left, no club
/// joined, kind 3, day 297). Older years are moved out to transfers_<year>.tmp (not read: a season's
/// rows stay in memory).
const TRANSFER_MANAGER: usize = 0xde1f54;

unsafe fn past_transfers() -> Vec<(i32, i32, i32, i32, u8, i16, i32)> {
    let tm = rd::<usize>(TRANSFER_MANAGER);
    if !readable(tm, 0x86e) {
        return Vec::new();
    }
    let (rows, count) = (rd::<usize>(tm + 0x862), rd::<i32>(tm + 0x86a));
    if count <= 0 || !readable(rows, count as usize * 0x25) {
        return Vec::new();
    }
    (0..count as usize)
        .map(|k| rows + k * 0x25)
        .map(|t| (rd::<i32>(t), rd::<i32>(t + 0x10), rd::<i32>(t + 0x14), rd::<i32>(t + 0x18), rd::<u8>(t + 0x1c), rd::<i16>(t + 0x1d), rd::<i16>(t + 0x1f) as i32))
        .collect()
}

/// Players the club let go in the season (user, 2026-11-07): to no club, after the season began; a
/// contract running out at the start of the season is no release. Releases never reach the
/// manager's history (traced on the user's game), so they come from the club's Players Out.
unsafe fn releases(club: usize, year: i32) -> i32 {
    let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
    let all = past_transfers();
    let mut n = 0;
    for t in all.into_iter().filter(|t| t.2 == index && t.1 == -1) {
        let season = if t.5 >= DEALS_FROM { t.6 } else { t.6 - 1 };
        if trace() {
            log(&format!("challenge out {index}: staff {} fee {} kind {} day {} {} (season {season})", t.0, t.3, t.4, t.5, t.6));
        }
        n += (season == year && in_season(t.4, t.5, t.6)) as i32;
    }
    n
}

/// Whether a player let go (kind 3) was released in the season, not a contract running out: those
/// leave on the turn of a season, 1 July or 1 January (the user's Dorking game: Pinchard and Lakin,
/// kind 3 on 1 July 2028, day 182).
/// ponytail: a release done on 1 July or 1 January itself reads as a contract running out.
fn in_season(kind: u8, day: i16, year: i32) -> bool {
    let leap = (year % 4 == 0 && year % 100 != 0 || year % 400 == 0) as i16;
    kind == 3 && day != 0 && day != 181 + leap
}

#[cfg(test)]
#[test]
fn a_contract_running_out_is_no_release() {
    assert!(!in_season(3, 182, 2028) && !in_season(3, 181, 2027) && !in_season(3, 0, 2026));
    assert!(in_season(3, 297, 2028) && !in_season(0, 297, 2028) && !in_season(2, 297, 2028));
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

/// A competition's League object, 0 for none: only a league the game plays (loaded at the start)
/// has one, `[LEAGUES][id]` with its +4 the competition.
pub(super) unsafe fn league_object(comp: usize) -> usize {
    let leagues = rd::<usize>(LEAGUES);
    if !readable(comp, 4) || !readable(leagues, 4) || rd::<i32>(comp) < 0 {
        return 0;
    }
    let at = leagues + rd::<i32>(comp) as usize * 4;
    let league = if readable(at, 4) { rd::<usize>(at) } else { 0 };
    if readable(league, 0xc9) && rd::<usize>(league + 4) == comp { league } else { 0 }
}

/// The club's place in its league table and the number of teams (the match card, `card.rs`): one
/// more than the clubs with more points, or with as many points and a better goal difference
/// (+0x15 for, +0x17 against, as the table's For / Ag); None outside a league the game plays.
pub(crate) unsafe fn league_place(club: usize) -> Option<(i32, i32)> {
    let league = league_object(rd::<usize>(club + 0x57));
    if league == 0 {
        return None;
    }
    let (teams, rows) = (rd::<i16>(league + 0x3e).max(0) as usize, rd::<usize>(league + 0xb1));
    if !(2..=64).contains(&teams) || !readable(rows, teams * 0x3b) {
        return None;
    }
    let key = |r: usize| (rd::<i16>(r + 0x19) as i32, rd::<i16>(r + 0x15) as i32 - rd::<i16>(r + 0x17) as i32, rd::<i16>(r + 0x15) as i32);
    let all: Vec<usize> = (0..teams).map(|k| rows + k * 0x3b).collect();
    let own = *all.iter().find(|&&r| rd::<usize>(r) == club)?;
    let mine = key(own);
    Some((1 + all.iter().filter(|&&r| key(r) > mine).count() as i32, teams as i32))
}

/// The club's row of its league table: played, won, drawn, lost (bytes +0x11..+0x14), goals for
/// and against (+0x15, +0x17), points (+0x19); and the league's name (the season review).
pub(crate) unsafe fn league_row(club: usize) -> Option<([i32; 7], Vec<u8>)> {
    let comp = rd::<usize>(club + 0x57);
    let league = league_object(comp);
    if league == 0 {
        return None;
    }
    let (teams, rows) = (rd::<i16>(league + 0x3e).max(0) as usize, rd::<usize>(league + 0xb1));
    if !(2..=64).contains(&teams) || !readable(rows, teams * 0x3b) {
        return None;
    }
    let own = (0..teams).map(|k| rows + k * 0x3b).find(|&r| rd::<usize>(r) == club)?;
    let b = |o: usize| rd::<u8>(own + o) as i32;
    let s = |o: usize| rd::<i16>(own + o) as i32;
    if trace() {
        // (the goals at +0x15 / +0x17 are a guess: the raw row to check against the table)
        let raw: Vec<u8> = (0x11..0x1b).map(|o| rd::<u8>(own + o)).collect();
        log(&format!("league row {}: bytes 0x11.. {raw:02x?}", latin1(&game_text(club + 4, 51))));
    }
    Some(([b(0x11), b(0x12), b(0x13), b(0x14), s(0x15), s(0x17), s(0x19)], game_text(comp + 4, 51)))
}

/// The table of the club's league: League object `[LEAGUES][id]` (its +4 the competition), +0x3e
/// the teams, +0xb1 the rows of 0x3b bytes: +0 club, +5 head-to-head (14 bytes an opponent: +0
/// club, +5 won), +0x11 played, +0x14 lost, +0x19 points.
pub(super) unsafe fn standing(club: usize) -> Option<Standing> {
    let comp = rd::<usize>(club + 0x57);
    let league = league_object(comp);
    if league == 0 {
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
    r.released = releases(club, year);
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
    read_squad(club, &mut r);
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
        // the formation of every league match, whatever challenges he has (one drawn later in the
        // season finds the matches already counted)
        if readable(club, 0x60) {
            changed |= watch_tactics(l, &name, staff, club, now);
        }
        let rows: Vec<u32> = l.rads.iter().filter(open).filter(|r| rule(&r.text).is_some()).map(|r| r.id).collect();
        if rows.is_empty() {
            continue;
        }
        let leads = l.rads.iter().filter(open).any(|r| r.text.starts_with("Player becomes") || r.text.starts_with("Player gets"));
        let best = l.best.iter().find(|b| b.1 == now && same_manager(&b.0, &name)).map(|b| b.2);
        let mut readings = read(staff, club, now, leads, best);
        tactic_readings(l, &name, now, &mut readings);
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

/// The RAD review's census (2026-11-06): with `cmui.census` in the game folder, every squad of a
/// played first division into `challenge_census.tsv`, a player a line: the facts the challenges
/// could be judged by (nation, its continent and region, age, caps, feet, value, the age he joined
/// the club, positions) and fields 0..24 of three of this season's stat rows (2 league, 5
/// international, 6 all the club's games), as the game's own stats screen writes them. Read by
/// `ui\challenge_sim.py`. Nothing else uses it.
const GET_RECORD: usize = 0x7c3920; // PlayerStats::get_category_record, thiscall(stats, Player*, category) -> 32-byte record or 0

unsafe fn census() {
    // (asked of every poll: the file is looked for every 3 s)
    static mut LOOKED: Option<std::time::Instant> = None;
    if LOOKED.is_some_and(|t| t.elapsed().as_secs() < 3) {
        return;
    }
    LOOKED = Some(std::time::Instant::now());
    if std::fs::remove_file("cmui.census").is_err() {
        return;
    }
    let (comps, comp_count) = (rd::<usize>(COMPS), rd::<i32>(COMP_COUNT));
    let (table, count) = (rd::<usize>(CLUB_TABLE), rd::<i32>(CLUB_COUNT));
    log(&format!("challenge census asked: {count} clubs, {comp_count} competitions"));
    if count <= 0 || comp_count <= 0 {
        return;
    }
    let text = |p: usize, at: usize, n: usize| if readable(p, at + n) { latin1(&game_text(p + at, n)).to_string() } else { String::new() };
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let mut out = String::from("league\tclub\tplayer\tnation\tcontinent\tregion\tage\tcaps\tleft\tright\tvalue\tjoined_age\tpositions\tleague_record\tinternational_record\tclub_record\thistory_first\thistory_last\n");
    // (a category's 32-byte record as hex, "" none: 0x7c3920 as the stats screens read it; field
    // numbers past 0x11 stop the game, so the record and not the formatter)
    let stats = rd::<usize>(PLAYER_STATS);
    let record = |player: usize, row: i32| -> String {
        if !readable(stats, 0x34) {
            return String::new();
        }
        let r = game::<extern "thiscall" fn(usize, usize, i32) -> usize>(GET_RECORD)(stats, player, row);
        if r == 0 || !readable(r, 32) {
            return String::new();
        }
        (0..32).map(|i| format!("{:02x}", rd::<u8>(r + i))).collect()
    };
    // (every player of the database, not only today's first divisions: a past season's squad
    // holds men who have moved down or on since; `club` his club's index now, -1 none)
    let (staff_table, staff_count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    for k in 0..staff_count.max(0) as usize {
        let s = staff_table + k * 0x6e;
        let player = if readable(s, 0x6e) { ((s + 0x61) as *const usize).read_unaligned() } else { 0 };
        if !readable(player, 70) {
            continue;
        }
        let club = rd::<usize>(s + 0x39);
        let club_index = if club >= table && club < table + count as usize * CLUB_SIZE { ((club - table) / CLUB_SIZE) as i64 } else { -1 };
        let league = if club_index >= 0 { rd::<usize>(club + 0x57) } else { 0 };
        let in_table = league >= comps && league < comps + comp_count as usize * COMP_SIZE;
        let (league_name, club_name) = (if in_table { text(league, 4, 51) } else { String::new() }, format!("{club_index}"));
        {
            let nation = rd::<usize>(s + 0x1a);
            let continent = if readable(nation, 0x78) { rd::<usize>(nation + 0x71) } else { 0 };
            let continent_name = if readable(continent, 0x40) { text(continent, 4, 51) } else { format!("#{continent}") };
            let region = if readable(nation, 0x78) { rd::<i8>(nation + 0x75) } else { -1 };
            let born = |date: usize| {
                let (_, _, y) = day_month(date);
                if y == 1900 { (rd::<u16>(s + 0x18) as i32, 0) } else { (y, rd::<i16>(date) as i32) }
            };
            let (by, bd) = born(s + 0x10);
            let age = year - by - (day < bd) as i32;
            let (_, _, jy) = day_month(s + 0x3e);
            let joined_age = if jy > 1900 { jy - by } else { -1 };
            let positions: String = [(0xf, "GK"), (0x10, "SW"), (0x11, "D"), (0x12, "DM"), (0x13, "M"), (0x14, "AM"), (0x15, "F"), (0x16, "WB")].iter().filter(|p| rd::<i8>(player + p.0) >= 15).map(|p| p.1).collect::<Vec<_>>().join("/");
            out += &format!(
                "{league_name}\t{club_name}\t{}\t{}\t{continent_name}\t{region}\t{age}\t{}\t{}\t{}\t{}\t{joined_age}\t{positions}",
                latin1(&staff_name(s)),
                if readable(nation, 0x40) { text(nation, 4, 51) } else { String::new() },
                rd::<u8>(s + 0x22),
                rd::<i8>(player + 0x30),
                rd::<i8>(player + 0x3b),
                ((s + 0x52) as *const i32).read_unaligned(),
            );
            // (his past seasons: the first and last node of his chain in `player stats history.tmp`,
            // +0x18 and +0x1c of his 39-byte entry at stats +0x30, by the player's id, 0x7c3920)
            let entry = rd::<usize>(stats + 0x30) + rd::<u32>(player) as usize * 0x27;
            let (first, last) = if readable(entry, 0x20) { (rd::<i32>(entry + 0x18), rd::<i32>(entry + 0x1c)) } else { (-1, -1) };
            out += &format!("\t{}\t{}\t{}\t{first}\t{last}\n", record(player, 2), record(player, 5), record(player, 6));
        }
    }
    let _ = std::fs::write("challenge_census.tsv", to_latin1(&out));
    // (the history's lines name a club by its index: every club, its league and whether that is a
    // played first division; and a copy of the history file as it is now)
    let mut clubs = String::from("index\tclub\tleague\tfirst_division\tcountry\n");
    for k in 0..count as usize {
        let club = table + k * CLUB_SIZE;
        let league = rd::<usize>(club + 0x57);
        let in_table = league >= comps && league < comps + comp_count as usize * COMP_SIZE;
        let first = in_table && first_division(league) && league_object(league) != 0;
        let country = if in_table { super::nation_name(rd::<usize>(league + COMP_NATION)) } else { String::new() };
        clubs += &format!("{k}\t{}\t{}\t{}\t{country}\n", text(club, 4, 51), if in_table { text(league, 4, 51) } else { String::new() }, first as i32);
    }
    let _ = std::fs::write("challenge_clubs.tsv", to_latin1(&clubs));
    let _ = std::fs::copy("player stats history.tmp", "challenge_history.bin");
    log(&format!("challenge census: {} lines on {day} {year}", out.lines().count() - 1));
}

/// A human turn on the host: once a game day.
pub unsafe fn tick() {
    census();
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
        let (comps, count) = (rd::<usize>(COMPS), rd::<i32>(COMP_COUNT));
        for k in 0..count.max(0) as usize {
            let comp = comps + k * COMP_SIZE;
            let league = league_object(comp);
            if first_division(comp) && league != 0 {
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
    fn formations_against_the_tactics_challenges() {
        let f = |s: &str| s.split(' ').map(|l| l.as_bytes().to_vec()).collect::<Vec<_>>();
        let marks = |s: &str| {
            let v = f(s);
            match_marks(&v.iter().map(|l| l.as_slice()).collect::<Vec<_>>())
        };
        // a 4-4-2: no sweeper, no wing backs, two up, a 4-4-2, no wingers
        assert_eq!(marks("GK DL DC DC DR ML MC MC MR FC FC"), [false, true, false, true, false, false, true, false, false]);
        // 5-3-2 with a sweeper and wing backs: five back
        assert_eq!(marks("GK SW DC DC WBL WBR MC MC MC FC FC"), [true, false, false, false, true, false, true, false, false]);
        // 4-3-3 with wingers: three up only counts forwards, the wingers break "without"
        assert_eq!(marks("GK DL DC DC DR DMC MC MC AML AMR FC"), [false, true, false, false, false, false, false, false, false]);
        // no striker, five in midfield; and the Kamikaze: one defender, three up
        assert_eq!(marks("GK DL DC DC DR DMC ML MC MR AMC AMC"), [false, true, false, false, false, true, true, true, false]);
        assert_eq!(marks("GK DC DMC ML MC MR AMC AMC FC FC FC"), [false, true, true, false, false, true, true, false, true]);
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

    /// Since the RAD review (2026-11-06) every challenge the wheel can draw is one the game reads:
    /// a word changed in the rules file would quietly make it the host's. The archived ones (the
    /// tactics, the trivial ones) are kept for old rows only.
    #[test]
    fn the_rules_file_and_the_table_agree() {
        let l = League::parse(&latin1(RULES), "");
        for t in l.tasks.iter().filter(|t| !l.archived.contains(&t.1)) {
            assert!(rule(&t.1).is_some(), "drawn but not read: {}", t.1);
        }
        assert_eq!(l.archived.len(), 4);
        // (70 to draw; one archived is still read: "No player younger than 18", met 98% of the time)
        assert_eq!(l.tasks.iter().filter(|t| rule(&t.1).is_some()).count(), 71);
    }
}
