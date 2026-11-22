//! Career goals (switch `career_goals`): aims for a whole career (`Q` lines of `rules.tsv`). The
//! host picks ten for each manager at the start, a country with the three that ask one; the picks
//! can change only in the season of the first pick. A goal counts from the season it was picked,
//! once, for its points. The game follows all but one (the discipline table): from a manager's
//! history (titles, cups, promotions, his clubs and their countries) and the host's turns (league
//! table, leaders, awards, national team results). The host ticks the rest.

use super::*;

pub(super) const BIG_FIVE: [&str; 5] = ["England", "Spain", "Germany", "Italy", "France"];
pub(super) const CHOICE: &str = "a country of your choice";
/// How many goals a manager picks.
pub(super) const PICKS: usize = 10;

/// What the game knows of a manager's career.
#[derive(Default, Clone)]
pub(super) struct Career {
    pub facts: Vec<Fact>,
    pub seasons: Vec<(i32, usize, String)>, // every season up to now: the club he ended it with, its nation
    pub jobs: Vec<(i32, usize, String)>,    // every club he was hired at: the season, the club, its nation
    pub leads: Vec<(i32, [bool; 3])>,       // a season: the first division's top scorer, assists, average in his squad
    pub stats: Vec<(i32, String, i32)>,     // a season's readings by name (`track::read_career`)
    pub big_wins: Vec<(i32, usize, i32)>,   // each club's all-time biggest win: the season, the club, the winner's goals
    pub titles: Vec<(usize, usize, i32)>,   // the competitions' history rows of his clubs: the competition, the club, the season
    pub now: i32,
}

pub(super) const UNBEATEN: &str = "Go a full league season unbeaten";
pub(super) const COMEBACK: &str = "Win the league after trailing the leader by 10+ points at the halfway mark";
pub(super) const BALLON: &str = "Have the Ballon d'Or winner of the year in your team";
pub(super) const GOLDEN_BOOT: &str = "Have the European Golden Boot winner in your team";
pub(super) const CL_SCORER: &str = "Have the Champions League top scorer in your team";
pub(super) const TEAM_OF_YEAR: &str = "Have three players in the league's Team of the Year";

/// A side of a derby by the words of a club's names: any of the first, none of the second ("milan"
/// is in Internazionale Milano too).
type Side = (&'static [&'static str], &'static [&'static str]);
/// (a derby's index is its stat key "derby{d}": new ones go at the end)
pub(super) const DERBIES: [(&str, Side, Side); 19] = [
    ("Win the Revierderby (Dortmund v Schalke 04) ten times", (&["dortmund"], &[]), (&["schalke"], &[])),
    ("Win the Old Firm (Celtic v Rangers) ten times", (&["celtic"], &[]), (&["rangers"], &[])),
    ("Win Le Classique (PSG v Marseille) ten times", (&["paris saint", "paris sg", "psg"], &[]), (&["marseille"], &[])),
    ("Win the Derby della Madonnina (AC Milan v Inter) ten times", (&["milan"], &["inter"]), (&["inter"], &[])),
    ("Win De Klassieker (Ajax v Feyenoord) ten times", (&["ajax"], &[]), (&["feyenoord"], &[])),
    // ("R.C.D. Espanyol" is "Espanyol Barcelona" too)
    ("Win El Cl\u{e1}sico (Barcelona v Real Madrid) ten times", (&["barcelona"], &["espanyol"]), (&["real madrid"], &[])),
    ("Win the North London Derby (Arsenal v Tottenham) ten times", (&["arsenal"], &[]), (&["tottenham"], &[])),
    // (2026-11-22, the user's list; names as the 26/27 database has them)
    ("Win the Northwest Derby (Liverpool v Manchester United) ten times", (&["liverpool"], &[]), (&["manchester united", "man utd"], &[])),
    ("Win the Manchester Derby (Manchester City v Manchester United) ten times", (&["manchester city", "man city"], &[]), (&["manchester united", "man utd"], &[])),
    ("Win the Derby Parisien (PSG v Paris FC) ten times", (&["paris saint", "paris sg", "psg"], &[]), (&["paris fc"], &[])),
    ("Win the Choc des Olympiques (Lyon v Marseille) ten times", (&["olympique lyon", "olympique lyonnais"], &[]), (&["olympique marseille", "olympique de marseille"], &[])),
    ("Win Der Klassiker (Dortmund v Bayern M\u{fc}nchen) ten times", (&["dortmund"], &[]), (&["bayern m"], &[])),
    ("Win the Derby della Capitale (Roma v Lazio) ten times", (&["as roma"], &[]), (&["lazio"], &[])),
    ("Win the Derby dei Campioni (Juventus v AC Milan) ten times", (&["juventus"], &[]), (&["milan"], &["inter"])),
    ("Win the Derbi Sevillano (Sevilla v Betis) ten times", (&["sevilla"], &["betis"]), (&["betis"], &[])),
    ("Win the Derbi Barcelon\u{ed} (Espanyol v Barcelona) ten times", (&["espanyol"], &[]), (&["barcelona"], &["espanyol"])),
    ("Win El Derbi Madrile\u{f1}o (Atl\u{e9}tico v Real Madrid) ten times", (&["tico madrid", "tico de madrid"], &[]), (&["real madrid"], &[])),
    ("Win the Dundee Derby (Dundee v Dundee United) ten times", (&["dundee fc"], &[]), (&["dundee united", "dundee utd"], &[])),
    ("Win the Derby van het Noorden (Heerenveen v Groningen) ten times", (&["heerenveen"], &[]), (&["fc groningen"], &[])),
];

/// National teams' clashes: one competitive win (not a friendly) as the manager of either side.
/// (index = stat key "clash{d}")
pub(super) const CLASHES: [(&str, Side, Side); 5] = [
    ("Win the clash Netherlands v Germany as national team manager", (&["netherlands", "holland"], &[]), (&["germany"], &[])),
    ("Win the clash England v Scotland as national team manager", (&["england"], &[]), (&["scotland"], &[])),
    ("Win the clash Greece v Turkey as national team manager", (&["greece"], &[]), (&["turkey", "t\u{fc}rkiye", "turkiye"], &[])),
    ("Win the clash Argentina v Brazil as national team manager", (&["argentina"], &[]), (&["brazil", "brasil"], &[])),
    ("Win the clash Spain v Portugal as national team manager", (&["spain"], &[]), (&["portugal"], &[])),
];

pub(super) const HUNDRED: &str = "Score 100+ league goals in one season";
pub(super) const TIGHT: &str = "Concede fewer than 15 goals in one league season";
pub(super) const BLAST: &str = "Blast from the past: win the league with a former league winner after taking it up through at least one lower league";

/// Whether a club (its names in lower case) is that side of a derby.
pub(super) fn is_side(names: &str, side: Side) -> bool {
    side.0.iter().any(|w| names.contains(w)) && !side.1.iter().any(|w| names.contains(w))
}

/// The goals of the first division's leaders: read on the host's every turn (`track::update`).
pub(super) const LEADS: [&str; 5] = [
    "Have the top flight's top scorer in your team",
    "Have the top flight's top assist maker in your team",
    "Have the top flight's highest average rating in your team",
    "Have the top flight's top scorer three seasons in a row",
    "Have the top flight's top assist maker three seasons in a row",
];

/// Whether a goal is reached by what happened from season `from` to `upto`; None for a goal the
/// game does not follow (the host ticks it).
fn holds(goal: &str, c: &Career, from: i32, upto: i32, country: &str) -> Option<bool> {
    let facts: Vec<&Fact> = c.facts.iter().filter(|f| (from..=upto).contains(&f.season)).collect();
    let jobs: Vec<&(i32, usize, String)> = c.jobs.iter().filter(|j| j.0 <= upto).collect();
    let club = |season: i32| c.seasons.iter().find(|s| s.0 == season).map(|s| s.1);
    let nation = |season: i32| c.seasons.iter().find(|s| s.0 == season).map_or("", |s| s.2.as_str());
    let won = |f: &Fact, family: usize| f.kind == KIND_WON && f.family == Some(family);
    let title = |f: &Fact| f.kind == KIND_CHAMPIONS && f.top && f.domestic;
    let cup = |f: &Fact| won(f, 6);
    let cl = |f: &Fact| won(f, 0);
    let promoted = |f: &Fact| f.kind == KIND_PROMOTED || f.kind == KIND_CHAMPIONS && !f.top && f.domestic;
    // (a club's trophy: a national team's are left out)
    let trophy = |f: &Fact| f.kind == KIND_CHAMPIONS && f.domestic || f.kind == KIND_WON && (matches!(f.family, Some(0..=3 | 6 | 7)) || conference(f));
    let country_of = |f: &Fact| if f.domestic { f.nation.clone() } else { nation(f.season).to_string() };
    let count = |mut v: Vec<String>| {
        v.sort();
        v.dedup();
        v.len()
    };
    let nations_of = |p: &dyn Fn(&Fact) -> bool| count(facts.iter().filter(|f| p(f)).map(|f| country_of(f)).filter(|n| !n.is_empty()).collect());
    let clubs_of = |p: &dyn Fn(&Fact) -> bool| count(facts.iter().filter(|f| p(f)).filter_map(|f| club(f.season)).map(|c| c.to_string()).collect());
    let in_one = |ps: &[&dyn Fn(&Fact) -> bool]| (from..=upto).any(|s| ps.iter().all(|p| facts.iter().any(|f| f.season == s && p(f))));
    let same_club_later = |at: &Fact, p: &dyn Fn(&Fact) -> bool| facts.iter().any(|f| f.season > at.season && p(f) && club(f.season).is_some() && club(f.season) == club(at.season));
    let in_league = |f: &Fact| [KIND_FINISHED, KIND_CHAMPIONS, KIND_PROMOTED].contains(&f.kind) && f.domestic;
    let chosen = |p: &dyn Fn(&Fact) -> bool| !country.is_empty() && facts.iter().any(|f| p(f) && f.nation == country);
    // (the leaders of a season played out: its last reading)
    let led = |k: usize, s: i32| s < c.now && c.leads.iter().any(|e| e.0 == s && e.1[k]);
    let led_in = |k: usize| (from..=upto).any(|s| led(k, s));
    let stat = |s: i32, key: &str| c.stats.iter().find(|e| e.0 == s && e.1 == key).map(|e| e.2);
    // (a league season played out with every match of it in the table)
    let played_out = |s: i32| s < c.now && stat(s, "full").is_some_and(|n| n > 0 && stat(s, "played").unwrap_or(0) >= n);
    let date = |f: &Fact| f.year * 366 + f.day as i32;
    if let Some(d) = DERBIES.iter().position(|d| d.0 == goal) {
        let key = format!("derby{d}");
        return Some((from..=upto).filter_map(|s| stat(s, &key)).sum::<i32>() >= 10);
    }
    if let Some(d) = CLASHES.iter().position(|d| d.0 == goal) {
        let key = format!("clash{d}");
        return Some((from..=upto).any(|s| stat(s, &key) == Some(1)));
    }
    Some(match goal {
        "Win the league title in a country of your choice" => chosen(&title),
        "Win the national cup in a country of your choice" => chosen(&cup),
        "Win the national Super Cup in a country of your choice" => chosen(&|f| won(f, 7)),
        "Win the league title in three different countries" => nations_of(&title) >= 3,
        "Win the national cup in three different countries" => nations_of(&cup) >= 3,
        "Win the league and the cup in the same season" => in_one(&[&title, &cup]),
        "Win the league, the cup and the Super Cup in one season" => in_one(&[&title, &cup, &|f| won(f, 7)]),
        "Win five league titles with the same club" => facts.iter().filter(|f| title(f)).filter_map(|f| club(f.season)).any(|k| facts.iter().filter(|f| title(f) && club(f.season) == Some(k)).count() >= 5),
        "Win promotion with three different clubs" => clubs_of(&promoted) >= 3,
        "Win the league title straight after promotion" => facts.iter().any(|p| promoted(p) && facts.iter().any(|t| title(t) && t.season == p.season + 1 && club(t.season).is_some() && club(t.season) == club(p.season))),
        "Win the Champions League" => facts.iter().any(|f| cl(f)),
        "Win the Champions League with two different clubs" => clubs_of(&cl) >= 2,
        "Win the UEFA Cup / Europa League" => facts.iter().any(|f| won(f, 1)),
        "Win the Conference League" => facts.iter().any(|f| f.kind == KIND_WON && conference(f)),
        "Win the European Super Cup" => facts.iter().any(|f| won(f, 2)),
        "Win the Club World Cup" => facts.iter().any(|f| won(f, 3)),
        "Win the Champions League and the league title in the same season" => in_one(&[&cl, &title]),
        "Win the Champions League with a club outside the big five leagues" => facts.iter().any(|f| cl(f) && !nation(f.season).is_empty() && !BIG_FIVE.contains(&nation(f.season))),
        "Win the World Cup as a national team manager" => facts.iter().any(|f| won(f, 4)),
        "Win a continental tournament as a national team manager" => facts.iter().any(|f| won(f, 5)),
        "Have the top flight's top scorer in your team" => led_in(0),
        "Have the top flight's top assist maker in your team" => led_in(1),
        "Have the top flight's highest average rating in your team" => led_in(2),
        "Have the top flight's top scorer three seasons in a row" => (from..=upto).any(|s| (s..s + 3).all(|y| y <= upto && led(0, y))),
        BALLON => (from..=upto).any(|s| stat(s, "ballon") == Some(1)),
        // (a season played out: its last reading; `track::golden_boot`, the loaded European top flights)
        GOLDEN_BOOT => (from..=upto).any(|s| s < c.now && stat(s, "boot") == Some(1)),
        // (the same: `track::cl_top_scorer`, the competition's own stats)
        CL_SCORER => (from..=upto).any(|s| s < c.now && stat(s, "clscorer") == Some(1)),
        TEAM_OF_YEAR => (from..=upto).any(|s| stat(s, "toty").unwrap_or(0) >= 3),
        // (a season played out with every match of it in the table)
        UNBEATEN => (from..=upto).any(|s| played_out(s) && stat(s, "lost") == Some(0)),
        // (2026-11-22, the user's list)
        "Have the top flight's top assist maker three seasons in a row" => (from..=upto).any(|s| (s..s + 3).all(|y| y <= upto && led(1, y))),
        HUNDRED => (from..=upto).any(|s| stat(s, "gf").unwrap_or(0) >= 100),
        TIGHT => (from..=upto).any(|s| played_out(s) && stat(s, "ga").is_some_and(|n| n < 15)),
        "Win the league title three times in a row with one club" => facts.iter().any(|f| title(f) && club(f.season).is_some() && (1..3).all(|k| facts.iter().any(|g| title(g) && g.season == f.season + k && club(g.season) == club(f.season)))),
        "Win the national cup three times in five seasons in a country of your choice" => !country.is_empty() && (from..=upto).any(|s| facts.iter().filter(|f| cup(f) && f.nation == country && (s..s + 5).contains(&f.season)).count() >= 3),
        "Win the Champions League with three different clubs (Seedorf Challenge)" => clubs_of(&cl) >= 3,
        "Win all three European cups (Champions League, Europa League, Conference League) and the European Super Cup" => facts.iter().any(|f| cl(f)) && facts.iter().any(|f| won(f, 1)) && facts.iter().any(|f| f.kind == KIND_WON && conference(f)) && facts.iter().any(|f| won(f, 2)),
        // (the Super Cup and the Club World Cup within a year after the final, by their dates)
        "Win the Champions League, the European Super Cup and the Club World Cup in a row" => facts.iter().any(|f| cl(f) && [2, 3].iter().all(|&k| facts.iter().any(|g| won(g, k) && (1..400).contains(&(date(g) - date(f)))))),
        // (a club promoted by him that won its league before he came up with it: the competitions' history)
        BLAST => facts.iter().any(|p| promoted(p) && p.team != 0 && facts.iter().any(|t| title(t) && t.team == p.team && t.season > p.season && c.titles.iter().any(|h| h.1 == t.team && h.0 == t.table && h.2 < p.season))),
        COMEBACK => facts.iter().any(|f| title(f) && stat(f.season, "gap").unwrap_or(0) >= 10),
        "Win a competitive match with double figures (10+ goals)" => c.big_wins.iter().any(|w| (from..=upto).contains(&w.0) && w.2 >= 10 && club(w.0) == Some(w.1)),
        "Work in three different countries" => count(jobs.iter().map(|j| j.2.clone()).collect()) >= 3,
        "Work in five different countries" => count(jobs.iter().map(|j| j.2.clone()).collect()) >= 5,
        "Win a trophy in three different countries" => nations_of(&trophy) >= 3,
        "Win the league title in all of the big five leagues" => BIG_FIVE.iter().all(|n| facts.iter().any(|f| title(f) && f.nation == *n)),
        "Manage ten different clubs" => count(jobs.iter().map(|j| j.1.to_string()).collect()) >= 10,
        "Stay ten seasons at the same club" => {
            // (seasons played out: the one running is not one yet)
            let (mut run, mut best, mut last) = (0, 0, None);
            for s in from..=upto.min(c.now - 1) {
                let k = club(s);
                run = if k.is_none() { 0 } else if k == last { run + 1 } else { 1 };
                (last, best) = (k, best.max(run));
            }
            best >= 10
        }
        "Take a club from the lowest playable division to the top flight" => facts.iter().any(|l| l.lowest && same_club_later(l, &|f| in_league(f) && f.top)),
        "Take a club from the lowest playable division to the Champions League" => facts.iter().any(|l| l.lowest && same_club_later(l, &|f| f.family == Some(0))),
        "Win a trophy with five different clubs" => clubs_of(&trophy) >= 5,
        "Win the Champions League, the league and the cup in one season" => in_one(&[&cl, &title, &cup]),
        _ => return None,
    })
}

/// The Conference League: Restructured's tables count it for nothing, so by its name.
fn conference(f: &Fact) -> bool {
    !f.domestic && f.comp.contains("conference league")
}

/// The season a goal was reached in: None when it is not (yet), or the game does not follow it
/// (`followed` false). It counts what happened from season `from` on, the season it was picked
/// (a career started with its goals: all of it). `country`: for a goal that asks one.
pub(super) fn reached(goal: &str, c: &Career, from: i32, country: &str) -> (Option<i32>, bool) {
    match holds(goal, c, from, c.now, country) {
        None => (None, false),
        Some(false) => (None, true),
        Some(true) => ((from..=c.now).find(|&y| holds(goal, c, from, y, country) == Some(true)), true),
    }
}

/// Whether the game follows a goal (a career with nothing in it still answers).
pub(super) fn followed(goal: &str) -> bool {
    holds(goal, &Career::default(), 0, 0, "").is_some()
}

/// A small random number source (xorshift), seeded by the host's clock.
struct Dice(u64);

impl Dice {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

/// One random set of ten (indices into `goals`: category, text, points), the `keep` ones in it:
/// every category at least once, none more than twice (user 2026-11-22).
fn draw_ten(goals: &[(String, String, i32)], keep: &[usize], dice: &mut Dice) -> Vec<usize> {
    let mut cats: Vec<&str> = Vec::new();
    for g in goals {
        if !cats.contains(&g.0.as_str()) {
            cats.push(&g.0);
        }
    }
    let count = |set: &[usize], c: &str| set.iter().filter(|&&i| goals[i].0 == c).count();
    let free = |set: &[usize], c: &str| (0..goals.len()).filter(|&i| goals[i].0 == c && !set.contains(&i)).collect::<Vec<usize>>();
    let mut set: Vec<usize> = keep.to_vec();
    // the categories not in it yet first, in a random order (should there be more than places)
    let mut missing: Vec<&str> = cats.iter().copied().filter(|c| count(&set, c) == 0).collect();
    while set.len() < PICKS && !missing.is_empty() {
        let c = missing.remove(dice.below(missing.len()));
        let f = free(&set, c);
        if !f.is_empty() {
            set.push(f[dice.below(f.len())]);
        }
    }
    // then a second of three of them, each category at most twice
    while set.len() < PICKS {
        let open: Vec<&str> = cats.iter().copied().filter(|c| count(&set, c) < 2 && !free(&set, c).is_empty()).collect();
        if open.is_empty() {
            break;
        }
        let f = free(&set, open[dice.below(open.len())]);
        set.push(f[dice.below(f.len())]);
    }
    set
}

fn points_of(goals: &[(String, String, i32)], set: &[usize]) -> i32 {
    set.iter().map(|&i| goals[i].2).sum()
}

/// What a random set of ten comes to on average: the aim while no other manager has his ten.
pub(super) fn usual_points(goals: &[(String, String, i32)]) -> i32 {
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let n = 2000;
    ((0..n).map(|_| points_of(goals, &draw_ten(goals, &[], &mut dice)) as i64).sum::<i64>() / n) as i32
}

/// Ten goals for a manager at random (the host's button, user 2026-11-22): the rules of
/// `draw_ten`, and points close to `target` (the other managers' sets), so no one gets ten cheap
/// goals or ten dear ones: among many draws one within 5% of it at random, else the nearest.
pub(super) fn random_ten(goals: &[(String, String, i32)], keep: &[usize], target: i32, seed: u64) -> Vec<usize> {
    let mut dice = Dice(seed | 1);
    let room = (target * 5 / 100).max(10);
    let mut near: Vec<Vec<usize>> = Vec::new();
    let mut best: Option<(i32, Vec<usize>)> = None;
    for _ in 0..4000 {
        let set = draw_ten(goals, keep, &mut dice);
        let off = (points_of(goals, &set) - target).abs();
        if off <= room {
            near.push(set.clone());
        }
        if best.as_ref().is_none_or(|b| off < b.0) {
            best = Some((off, set));
        }
    }
    if near.is_empty() { best.map_or_else(Vec::new, |b| b.1) } else { near.swap_remove(dice.below(near.len())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(season: i32, kind: u8, comp: &str, nation: &str, top: bool, family: Option<usize>) -> Fact {
        Fact { season, kind, comp: comp.to_string(), domestic: !nation.is_empty(), top, nation: nation.to_string(), family, ..Default::default() }
    }

    /// A career at three clubs: Go Ahead from the lowest division up, PSV, then Porto.
    fn career() -> Career {
        let mut c = Career { now: 2040, ..Default::default() };
        for s in 2025..=2040 {
            let (club, nation) = match s {
                ..=2030 => (1, "Netherlands"),
                ..=2036 => (2, "Netherlands"),
                _ => (3, "Portugal"),
            };
            c.seasons.push((s, club, nation.to_string()));
        }
        c.jobs = vec![(2025, 1, "Netherlands".into()), (2031, 2, "Netherlands".into()), (2037, 3, "Portugal".into())];
        let mut lowest = fact(2025, KIND_PROMOTED, "dutch second division", "Netherlands", false, None);
        lowest.lowest = true;
        c.facts = vec![
            lowest,
            fact(2026, KIND_PROMOTED, "dutch first division", "Netherlands", false, None),
            fact(2027, KIND_CHAMPIONS, "dutch premier division", "Netherlands", true, None),
            fact(2031, KIND_CHAMPIONS, "dutch premier division", "Netherlands", true, None),
            fact(2031, KIND_WON, "dutch cup", "Netherlands", true, Some(6)),
            fact(2032, KIND_KNOCKED_OUT, "european champions cup", "", false, Some(0)),
            fact(2038, KIND_WON, "european champions cup", "", false, Some(0)),
            fact(2038, KIND_CHAMPIONS, "portuguese premier division", "Portugal", true, None),
            fact(2038, KIND_WON, "portuguese cup", "Portugal", true, Some(6)),
        ];
        c
    }

    #[test]
    fn goals_from_a_career() {
        let c = career();
        let at = |goal: &str| reached(goal, &c, 2025, "").0;
        assert_eq!(at("Win the league title straight after promotion"), Some(2027));
        assert_eq!(at("Take a club from the lowest playable division to the top flight"), Some(2027));
        assert_eq!(at("Take a club from the lowest playable division to the Champions League"), None); // (PSV went, not Go Ahead)
        assert_eq!(at("Win the league and the cup in the same season"), Some(2031));
        assert_eq!(at("Win the Champions League with a club outside the big five leagues"), Some(2038));
        assert_eq!(at("Win the Champions League, the league and the cup in one season"), Some(2038));
        assert_eq!(at("Win the league title in three different countries"), None);
        assert_eq!(at("Win promotion with three different clubs"), None);
        assert_eq!(at("Work in three different countries"), None);
        assert_eq!(at("Win a trophy in three different countries"), None);
        assert_eq!(at("Stay ten seasons at the same club"), None);
        // (followed since 2026-11-21: no "boot" reading in this career, so not reached)
        assert_eq!(reached(GOLDEN_BOOT, &c, 2025, ""), (None, true));
        let mut b = c.clone();
        b.stats.push((2030, "boot".into(), 1));
        b.now = 2031;
        assert_eq!(reached(GOLDEN_BOOT, &b, 2025, ""), (Some(2030), true));
        b.now = 2030; // (the season running: not played out yet)
        assert_eq!(reached(GOLDEN_BOOT, &b, 2025, ""), (None, true));
        // only what came after the pick, in the country chosen
        assert_eq!(reached("Win the league and the cup in the same season", &c, 2032, "").0, Some(2038));
        let title = "Win the league title in a country of your choice";
        assert_eq!(reached(title, &c, 2025, "Netherlands").0, Some(2027));
        assert_eq!(reached(title, &c, 2032, "Netherlands").0, None);
        assert_eq!(reached(title, &c, 2033, "Portugal").0, Some(2038));
        assert_eq!(reached(title, &c, 2025, ""), (None, true));
    }

    #[test]
    fn ten_seasons_and_ten_clubs() {
        let mut c = career();
        c.seasons.iter_mut().for_each(|s| s.1 = 9);
        // (sixteen seasons at one club, the running one not counted: reached in the tenth played out)
        assert_eq!(reached("Stay ten seasons at the same club", &c, 2025, "").0, Some(2034));
        c.jobs = (0..10).map(|k| (2025 + k, k as usize, "Netherlands".to_string())).collect();
        assert_eq!(reached("Manage ten different clubs", &c, 2025, "").0, Some(2034));
    }

    /// The first division's leaders: a season counts once played out, by its last reading.
    #[test]
    fn leaders_by_season() {
        let mut c = career();
        c.leads = vec![(2033, [true, false, false]), (2034, [true, true, false]), (2035, [true, false, false]), (2040, [true, true, true])];
        assert_eq!(reached(LEADS[0], &c, 2025, "").0, Some(2033));
        assert_eq!(reached(LEADS[1], &c, 2025, "").0, Some(2034));
        assert_eq!(reached(LEADS[2], &c, 2025, "").0, None); // (2040 still runs)
        assert_eq!(reached(LEADS[3], &c, 2025, "").0, Some(2035));
        assert_eq!(reached(LEADS[3], &c, 2034, "").0, None); // (picked after the first of the three)
    }

    /// What the host's turns read: derbies add up over seasons, unbeaten needs a full season.
    #[test]
    fn season_readings() {
        let mut c = career();
        let st = |s: i32, k: &str, v: i32| (s, k.to_string(), v);
        c.stats = vec![st(2031, "derby4", 6), st(2032, "derby4", 4), st(2033, "played", 34), st(2033, "full", 34), st(2033, "lost", 0), st(2034, "played", 20), st(2034, "full", 34), st(2034, "lost", 0), st(2038, "gap", 11)];
        assert_eq!(reached(DERBIES[4].0, &c, 2025, "").0, Some(2032));
        assert_eq!(reached(DERBIES[4].0, &c, 2032, "").0, None);
        assert_eq!(reached(UNBEATEN, &c, 2025, "").0, Some(2033));
        assert_eq!(reached(COMEBACK, &c, 2025, "").0, Some(2038));
        c.big_wins = vec![(2032, 2, 10), (2026, 2, 12)]; // (the second: before he came)
        assert_eq!(reached("Win a competitive match with double figures (10+ goals)", &c, 2025, "").0, Some(2032));
        assert!(is_side("ac milan milan", DERBIES[3].1) && !is_side("fc internazionale milano inter", DERBIES[3].1));
        assert!(is_side("fc internazionale milano inter", DERBIES[3].2));
    }

    /// Every goal of the rules: the game follows all but the discipline table, the host ticks that.
    #[test]
    fn the_rules_goals() {
        let l = League::parse(&latin1(RULES), "");
        assert_eq!(l.goals.len(), 75);
        let not: Vec<&String> = l.goals.iter().map(|g| &g.1).filter(|g| !followed(g)).collect();
        // (not the goal's whole text: make_challenge_editor.py marks a text found here as followed)
        assert!(not.len() == 1 && not[0].contains("Discipline"));
        assert!(DERBIES.iter().chain(CLASHES.iter()).all(|d| l.goals.iter().any(|q| q.1 == d.0)));
        assert!([UNBEATEN, COMEBACK, BALLON, GOLDEN_BOOT, CL_SCORER, TEAM_OF_YEAR, HUNDRED, TIGHT, BLAST].iter().all(|g| l.goals.iter().any(|q| q.1 == *g)));
        assert!(LEADS.iter().all(|g| l.goals.iter().any(|q| q.1 == *g)));
        assert_eq!(l.goals.iter().filter(|g| g.1.contains(CHOICE)).count(), 4);
        assert!(l.goals.iter().all(|g| g.2 > 0));
    }

    /// The goals of 2026-11-22: dominance, the former winner taken up, the European run.
    #[test]
    fn dominance_goals() {
        let mut c = career();
        let at = |c: &Career, goal: &str, country: &str| reached(goal, c, 2025, country).0;
        // (Go Ahead, club 1, promoted in 2025 and 2026, champions 2027: a former winner by the history)
        c.facts.iter_mut().filter(|f| f.season <= 2027).for_each(|f| f.team = 1);
        assert_eq!(at(&c, BLAST, ""), None);
        c.titles = vec![(0, 1, 1950)];
        assert_eq!(at(&c, BLAST, ""), Some(2027));
        c.titles = vec![(0, 1, 2027)]; // (his own title is no former one)
        assert_eq!(at(&c, BLAST, ""), None);
        // three titles in a row at PSV (club 2)
        for s in [2032, 2033] {
            c.facts.push(fact(s, KIND_CHAMPIONS, "dutch premier division", "Netherlands", true, None));
        }
        assert_eq!(at(&c, "Win the league title three times in a row with one club", ""), Some(2033));
        // the cup three times in five seasons
        let cup = "Win the national cup three times in five seasons in a country of your choice";
        for s in [2033, 2035] {
            c.facts.push(fact(s, KIND_WON, "dutch cup", "Netherlands", true, Some(6)));
        }
        assert_eq!(at(&c, cup, "Netherlands"), Some(2035));
        assert_eq!(at(&c, cup, "Portugal"), None);
        // CL (May 2039), Super Cup (August), Club World Cup (December): in a row by their dates
        let run = "Win the Champions League, the European Super Cup and the Club World Cup in a row";
        let dated = |season, year, day, family| Fact { year, day, ..fact(season, KIND_WON, "", "", false, Some(family)) };
        c.facts.push(dated(2038, 2039, 140, 0));
        c.facts.push(dated(2039, 2039, 230, 2));
        assert_eq!(at(&c, run, ""), None);
        c.facts.push(dated(2039, 2039, 350, 3));
        assert_eq!(at(&c, run, ""), Some(2039));
        // league goals and a tight defence from the host's readings
        let st = |s: i32, k: &str, v: i32| (s, k.to_string(), v);
        c.stats = vec![st(2033, "gf", 101), st(2034, "ga", 14), st(2034, "played", 20), st(2034, "full", 34), st(2035, "ga", 14), st(2035, "played", 34), st(2035, "full", 34), st(2036, "clash1", 1)];
        assert_eq!(at(&c, HUNDRED, ""), Some(2033));
        assert_eq!(at(&c, TIGHT, ""), Some(2035)); // (2034 not played out)
        assert_eq!(at(&c, CLASHES[1].0, ""), Some(2036));
        assert_eq!(at(&c, CLASHES[0].0, ""), None);
    }

    /// The `Q` lines of the built-in rules: category, text, points.
    fn rule_goals() -> Vec<(String, String, i32)> {
        String::from_utf8_lossy(super::super::RULES)
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() >= 4 && f[0] == "Q").then(|| (f[1].to_string(), f[2].to_string(), f[3].trim().parse().unwrap_or(0)))
            })
            .collect()
    }

    #[test]
    fn ten_at_random_by_the_rules() {
        let goals = rule_goals();
        let cats: std::collections::HashSet<&str> = goals.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(cats.len(), 7);
        let aim = usual_points(&goals);
        assert!((300..800).contains(&aim), "{aim}");
        let mut totals = Vec::new();
        for seed in 1..40u64 {
            let set = random_ten(&goals, &[], aim, seed * 7919);
            assert_eq!(set.len(), PICKS);
            let mut sorted = set.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), PICKS, "a goal twice");
            for c in &cats {
                let n = set.iter().filter(|&&i| goals[i].0 == *c).count();
                assert!((1..=2).contains(&n), "{c}: {n}");
            }
            let total = points_of(&goals, &set);
            assert!((total - aim).abs() <= (aim * 5 / 100).max(10), "{total} for {aim}");
            totals.push(set);
        }
        // not the same ten every time
        totals.sort();
        totals.dedup();
        assert!(totals.len() > 30);
        // a goal reached stays in the new ten
        let kept = goals.iter().position(|g| g.2 == 150).unwrap();
        assert!(random_ten(&goals, &[kept], aim, 5).contains(&kept));
    }
}
