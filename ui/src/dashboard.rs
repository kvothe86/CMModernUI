//! The dashboard (switch `export_button`, user 2026-11-28: "remove the print feature and replace
//! export for dashboard ... like a normal website INSIDE the game"): the Print / Export dropdown of
//! the club and competition screens is a "Dashboard" button that opens a page of stats in cmui's
//! dark look over the screen (the menu bar stays, as on the stadium page). A club screen gives the
//! club's dashboard, a competition screen its league's; the game's own printout is gone.
//!
//! The host (or a single player) builds the page as lines of tab-separated fields (`build`); the
//! painter reads only those, so a network client asks the host for them (key 'd', the screen's
//! parameter 0) and draws the same page. A line's first field is its kind:
//! `H` title, subtitle, date, crest key; `T` a tile: label, value (`~WDL` form letters), note, tone
//! (g / r / n); `C` a card: kind (line, bars, table, cols, list), title, note; in a card `X` the x
//! labels (comma), `S` a series: name, colour, dashed, values shown, values (comma, empty a gap);
//! `B` a bar: label, value, fraction, against-average text, tone; `RH` / `R` a table's head and
//! rows (`^key|text` a crest and text, `~WDL` form letters, `W 2-1` a result); `L` a list row:
//! name, sub, value (`#` first: a heading); `N` a note instead of the card's content; `P` a
//! card's pills (`label=filter`, `*` the default: `x:N` last N places, `first:N` / `last:N`
//! series, `g:X` table rows of group `!X`); `TT` a chart's tooltip line per x place. Any line may
//! end with a link: `@c` club, `@l` league record (that dashboard, Back returns), `@p` a player
//! (his profile, from a club screen only: `take_person` in the club screen's input).
//! (user 2026-11-28, after the first look: "a bit more interactive", alignment)

use crate::*;

const TODAY: usize = 0xae2c90;
const FONT_HEIGHT: usize = 0x5e5cd0; // font_get_height(font), cdecl

const PAGE: [u8; 3] = [0x0d, 0x11, 0x17];
const CARD: [u8; 3] = [0x16, 0x1b, 0x24];
const CARD_LOW: [u8; 3] = [0x11, 0x15, 0x1c];
const EDGE: [u8; 3] = [0x2a, 0x32, 0x3e];
const GRID: [u8; 3] = [0x25, 0x2c, 0x36];
const DIM: [u8; 3] = [0x8c, 0x95, 0xa2];
const GREEN: [u8; 3] = [0x3c, 0xc8, 0x6e];
const RED: [u8; 3] = [0xe0, 0x58, 0x50];
const BLUE: [u8; 3] = [0x4a, 0x8e, 0xe8];
const SERIES: [[u8; 3]; 6] = [GREEN, BLUE, GOLD, [0xc0, 0x6c, 0xe0], RED, DIM];

type Model = Vec<Vec<String>>;

// ---- the data (host / single player) ----

fn txt(b: &[u8]) -> String {
    latin1(b).into_owned()
}

unsafe fn club_at(index: i32) -> usize {
    let c = rd::<usize>(CLUB_TABLE) + index.max(0) as usize * CLUB_SIZE;
    if index >= 0 && readable(c, 0x5b) { c } else { 0 }
}

unsafe fn index_of(club: usize) -> i32 {
    ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32
}

unsafe fn is_club(p: usize) -> bool {
    let base = rd::<usize>(CLUB_TABLE);
    p >= base && (p - base) % CLUB_SIZE == 0 && ((p - base) / CLUB_SIZE) < rd::<i32>(CLUB_COUNT).max(0) as usize && readable(p, 0x1b3 + 28)
}

unsafe fn long(club: usize) -> String {
    if club == 0 { "?".into() } else { txt(&game_text(club + 4, 51)) }
}

unsafe fn short(club: usize) -> String {
    let s = if club == 0 { String::new() } else { txt(&game_text(club + 0x38, 26)) };
    if s.is_empty() { long(club) } else { s }
}

/// A staff member's age on the day (staff +0x10 the birth date, +0x18 the year when only that is
/// known).
pub(crate) unsafe fn age_of(s: usize, day: i32, year: i32) -> i32 {
    let (bd, bm, by) = day_month(s + 0x10);
    let born = if by == 1900 { rd::<u16>(s + 0x18) as i32 } else { by };
    let born_day = if by == 1900 { 0 } else { [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][bm] + bd - 1 };
    year - born - (day < born_day) as i32
}

fn month_lengths(year: i32) -> [i32; 12] {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    [31, 28 + leap as i32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}

/// The month (0 January) of a day of the year (0-based).
fn month_of(day: i32, year: i32) -> usize {
    let mut left = day.max(0);
    for (m, len) in month_lengths(year).into_iter().enumerate() {
        if left < len {
            return m;
        }
        left -= len;
    }
    11
}

fn short_date(day: i32, year: i32) -> String {
    let m = month_of(day, year);
    let before: i32 = month_lengths(year)[..m].iter().sum();
    format!("{} {}", day - before + 1, MONTHS[m])
}

/// The season's slot of a month: July 0 .. June 11.
fn slot(month: usize) -> usize {
    (month + 6) % 12
}

fn ordinal(n: i32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn money(v: f64, sym: &str) -> String {
    let a = v.abs();
    let s = if a >= 1e9 {
        format!("{:.2}B", a / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{:.0}K", a / 1e3)
    } else {
        format!("{a:.0}")
    };
    format!("{}{sym}{s}", if v < 0.0 { "-" } else { "" })
}

fn line(f: &[&str]) -> String {
    f.iter().map(|s| s.replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t")
}

fn nums(v: &[Option<f64>]) -> String {
    v.iter().map(|x| x.map_or(String::new(), |x| format!("{:.2}", x).trim_end_matches('0').trim_end_matches('.').to_string())).collect::<Vec<_>>().join(",")
}

/// A league match of the season, once (both clubs' fixture lists have it).
#[derive(Clone, Copy, PartialEq)]
struct Match {
    key: i32,
    month: usize,
    home: i32,
    away: i32,
    score: Option<(i32, i32)>,
}

/// Every league match of the season of the clubs (indices), played or not, oldest first.
unsafe fn league_matches(comp: i32, clubs: &[i32], from: (i32, i32), to: (i32, i32)) -> Vec<Match> {
    let mut all: Vec<Match> = clubs
        .iter()
        .flat_map(|&c| form::fixtures(c, from, to))
        .filter(|f| f.comp == comp)
        .map(|f| Match { key: f.date.1 * 366 + f.date.0, month: month_of(f.date.0, f.date.1), home: f.home, away: f.away, score: f.score })
        .collect();
    all.sort_by_key(|m| (m.key, m.home, m.away));
    all.dedup_by_key(|m| (m.key, m.home, m.away));
    all
}

/// A club's league numbers from the played matches.
#[derive(Default)]
struct Tally {
    g: f64,
    f: f64,
    a: f64,
    p: f64,
    w: f64,
    cs: f64,
    hg: f64,
    hp: f64,
    ag: f64,
    ap: f64,
}

fn tally(all: &[Match], club: i32, upto: i32) -> Tally {
    let mut t = Tally::default();
    for m in all.iter().filter(|m| m.key <= upto && (m.home == club || m.away == club)) {
        let Some((h, a)) = m.score else { continue };
        let home = m.home == club;
        let (gf, ga) = if home { (h, a) } else { (a, h) };
        let p = if gf > ga { 3.0 } else if gf == ga { 1.0 } else { 0.0 };
        t.g += 1.0;
        t.f += gf as f64;
        t.a += ga as f64;
        t.p += p;
        t.w += (gf > ga) as i32 as f64;
        t.cs += (ga == 0) as i32 as f64;
        if home {
            t.hg += 1.0;
            t.hp += p;
        } else {
            t.ag += 1.0;
            t.ap += p;
        }
    }
    t
}

/// The club's place among the clubs after the matches up to `upto` (points, goal difference, goals).
fn place(all: &[Match], clubs: &[i32], club: i32, upto: i32) -> i32 {
    let key = |c: i32| {
        let t = tally(all, c, upto);
        (t.p as i32, (t.f - t.a) as i32, t.f as i32)
    };
    let mine = key(club);
    1 + clubs.iter().filter(|&&c| c != club && key(c) > mine).count() as i32
}

fn letter(gf: i32, ga: i32) -> char {
    if gf > ga { 'W' } else if gf == ga { 'D' } else { 'L' }
}

fn points(gf: i32, ga: i32) -> i32 {
    match letter(gf, ga) {
        'W' => 3,
        'D' => 1,
        _ => 0,
    }
}

/// The page of a screen's parameter: a club's, or a competition's (its object, +4 the record, or
/// the record itself). None on a network client (it asks the host) or without a date.
pub unsafe fn build(p: usize) -> Option<String> {
    if remote::CLIENT {
        return None;
    }
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    if !(1900..2200).contains(&year) {
        return None;
    }
    // (a national team has no league: its own page of its matches, user 2026-11-28; a league page
    // only for a division its clubs play in: a qualifying section with a table of its own had the
    // league page, and its Top Scorers were the players' league goals)
    let division = |r: usize| challenge::league_table_of(r).is_some_and(|(_, rows)| 2 * rows.iter().filter(|(c, _)| rd::<usize>(c + 0x57) == r).count() > rows.len());
    let out = if is_club(p) && national_team(p) {
        nation_page(p, day, year)
    } else if is_club(p) {
        club_page(p, day, year)
    } else {
        let inner = if readable(p, 8) { rd::<usize>(p + 4) } else { 0 };
        match [p, inner].into_iter().find(|&r| readable(r, 60) && division(r)) {
            Some(r) => league_page(r, day, year),
            // a cup, a continental or international competition: by its fixtures (a record's +0 is
            // its id, a small number; an object's is its vtable)
            None => match [inner, p].into_iter().filter(|&r| readable(r, 60) && (0..5000).contains(&rd::<i32>(r))).find_map(|r| cup_page(r, day, year)) {
                Some(page) => page,
                None => {
                    let name = if readable(inner, 60) { txt(&game_text(inner + 4, 51)) } else { String::new() };
                    vec![line(&["H", &name, "DASHBOARD", &finreport::date_text(day, year), &name]), line(&["N", "No matches of this competition to show yet."])]
                }
            },
        }
    };
    Some(out.join("\n"))
}

unsafe fn club_page(club: usize, day: i32, year: i32) -> Vec<String> {
    let mut out = Vec::new();
    let me = index_of(club);
    let season = if day >= 181 { year } else { year - 1 };
    let (from, now) = ((181, season), (day, year));
    let today = year * 366 + day;
    let record = rd::<usize>(club + 0x57);
    let league = if readable(record, 60) { txt(&game_text(record + 4, 51)) } else { String::new() };
    let sub = format!("CLUB DASHBOARD  -  {}{}/{:02}", if league.is_empty() { String::new() } else { format!("{}  -  ", league.to_uppercase()) }, season, (season + 1) % 100);
    out.push(line(&["H", &long(club), &sub, &finreport::date_text(day, year), &long(club)]));

    // the season's matches, all competitions (no friendlies, no reserve team), oldest first
    let played: Vec<form::Fixture> = form::fixtures(me, from, now)
        .into_iter()
        .filter(|f| f.score.is_some() && f.date.1 * 366 + f.date.0 <= today && !form::friendly(f.comp) && !form::comp_name(f.comp).starts_with(b"Reserve"))
        .collect();
    let mine = |f: &form::Fixture| {
        let (h, a) = f.score.unwrap_or((0, 0));
        if f.home == me { (h, a) } else { (a, h) }
    };
    let (comp, rows) = challenge::league_table(club).unwrap_or((-1, Vec::new()));
    let clubs: Vec<i32> = rows.iter().map(|r| index_of(r.0)).collect();
    let all = league_matches(comp, &clubs, from, now);
    let league_games: Vec<&form::Fixture> = played.iter().filter(|f| f.comp == comp).collect();
    let fkey = |f: &form::Fixture| f.date.1 * 366 + f.date.0;

    // ---- tiles ----
    if clubs.contains(&me) {
        let t = tally(&all, me, today);
        let now_place = place(&all, &clubs, me, today);
        let n = league_games.len();
        let before = (n > 5).then(|| place(&all, &clubs, me, fkey(league_games[n - 6])));
        let (note, tone) = match before {
            Some(b) if b > now_place => (format!("of {}  -  up {} in 5 games", clubs.len(), b - now_place), "g"),
            Some(b) if b < now_place => (format!("of {}  -  down {} in 5 games", clubs.len(), now_place - b), "r"),
            _ => (format!("of {}", clubs.len()), "n"),
        };
        out.push(line(&["T", "League position", &ordinal(now_place), &note, tone, &format!("@l{record}")]));
        out.push(line(&["T", "Points", &format!("{}", t.p as i32), &format!("{:.2} per game  -  {} played", t.p / t.g.max(1.0), t.g as i32), "n"]));
        let gd = (t.f - t.a) as i32;
        out.push(line(&["T", "Goal difference", &if gd == 0 { "0".to_string() } else { format!("{gd:+}") },&format!("{} for, {} against", t.f as i32, t.a as i32), if gd > 0 { "g" } else if gd < 0 { "r" } else { "n" }]));
    } else {
        for label in ["League position", "Points", "Goal difference"] {
            out.push(line(&["T", label, "-", "no league table", "n"]));
        }
    }
    let form: String = played.iter().rev().take(6).rev().map(|f| letter(mine(f).0, mine(f).1)).collect();
    let (wins, losses) = (form.matches('W').count(), form.matches('L').count());
    out.push(line(&["T", "Form", &format!("~{form}"), &format!("last {} matches, all competitions", form.len()), if wins > losses { "g" } else if losses > wins { "r" } else { "n" }]));
    match finreport::figures(club) {
        Some((_, balance, funds, m)) => {
            let shown = |v: i64| v as f64 * m.rate;
            out.push(line(&["T", "Bank balance", &money(shown(balance), &m.symbol), &format!("transfer funds {}", money(shown(funds), &m.symbol)), if balance < 0 { "r" } else { "n" }]));
        }
        None => out.push(line(&["T", "Bank balance", "-", "", "n"])),
    }

    // ---- form and points ----
    let pts: Vec<f64> = league_games.iter().map(|f| points(mine(f).0, mine(f).1) as f64).collect();
    let n = pts.len();
    let rolling: Vec<Option<f64>> = (0..n).map(|i| Some(pts[i.saturating_sub(4)..=i].iter().sum::<f64>() / (i - i.saturating_sub(4) + 1) as f64)).collect();
    out.push(line(&["C", "line", "Form & Points Trend", "league matches"]));
    if n == 0 {
        out.push(line(&["N", "No league matches played yet this season."]));
    } else {
        out.push(line(&["P", "Last 5=x:5", "Last 10=x:10", "*Last 15=x:15", "Season=x:0"]));
        out.push(line(&["X", &(0..n).map(|i| format!("M{}", i + 1)).collect::<Vec<_>>().join(",")]));
        out.push(line(&["S", "Points earned", "0", "0", "1", &nums(&pts.iter().map(|&v| Some(v)).collect::<Vec<_>>())]));
        out.push(line(&["S", "Rolling average (5)", "5", "1", "0", &nums(&rolling)]));
        // the tooltips: each match's date, opponent and score
        let tips: Vec<String> = league_games
            .iter()
            .map(|f| {
                let home = f.home == me;
                let (gf, ga) = mine(f);
                format!("{}  {} ({})  {gf}-{ga}", short_date(f.date.0, f.date.1), short(club_at(if home { f.away } else { f.home })), if home { "H" } else { "A" })
            })
            .collect();
        let mut tt = vec!["TT"];
        tt.extend(tips.iter().map(String::as_str));
        out.push(line(&tt));
    }

    // ---- against the league's average ----
    out.push(line(&["C", "bars", "Team Performance", "against the league average"]));
    let tallies: Vec<Tally> = clubs.iter().map(|&c| tally(&all, c, today)).filter(|t| t.g > 0.0).collect();
    let own = tally(&all, me, today);
    if own.g == 0.0 || tallies.is_empty() {
        out.push(line(&["N", "No league matches played yet this season."]));
    } else {
        let metrics: [(&str, fn(&Tally) -> f64, bool, bool); 7] = [
            ("Goals scored / game", |t| t.f / t.g, true, false),
            ("Goals conceded / game", |t| t.a / t.g, false, false),
            ("Points / game", |t| t.p / t.g, true, false),
            ("Win rate", |t| 100.0 * t.w / t.g, true, true),
            ("Clean sheets", |t| 100.0 * t.cs / t.g, true, true),
            ("Home points / game", |t| if t.hg > 0.0 { t.hp / t.hg } else { 0.0 }, true, false),
            ("Away points / game", |t| if t.ag > 0.0 { t.ap / t.ag } else { 0.0 }, true, false),
        ];
        for (label, f, higher, pct) in metrics {
            let v = f(&own);
            let avg = tallies.iter().map(f).sum::<f64>() / tallies.len() as f64;
            let top = tallies.iter().map(f).fold(0.0f64, f64::max).max(0.01);
            let d = v - avg;
            let shown = if pct { format!("{v:.0}%") } else { format!("{v:.2}") };
            let delta = if pct { format!("{d:+.0}% vs avg") } else { format!("{d:+.2} vs avg") };
            let (better, worse) = if higher { (d > 0.005, d < -0.005) } else { (d < -0.005, d > 0.005) };
            out.push(line(&["B", label, &shown, &format!("{:.3}", (v / top).clamp(0.0, 1.0)), &delta, if better { "g" } else if worse { "r" } else { "n" }]));
        }
    }

    // ---- recent results ----
    out.push(line(&["C", "table", "Recent Results", "all competitions"]));
    if played.is_empty() {
        out.push(line(&["N", "No matches played yet this season."]));
    } else {
        out.push(line(&["P", "*All=g:", "League=g:L", "Cups=g:C"]));
        out.push(line(&["RH", "Date", "Opponent", "Competition", "Result"]));
        for f in played.iter().rev().take(20) {
            let home = f.home == me;
            let other = club_at(if home { f.away } else { f.home });
            let (gf, ga) = mine(f);
            out.push(line(&[
                "R",
                &short_date(f.date.0, f.date.1),
                &format!("^{}|{} ({})", long(other), short(other), if home { "H" } else { "A" }),
                &txt(&form::comp_name(f.comp)),
                &format!("{} {gf}-{ga}", letter(gf, ga)),
                if f.comp == comp { "!L" } else { "!C" },
                &format!("@c{other}"),
            ]));
        }
    }

    // ---- goals by month ----
    out.push(line(&["C", "cols", "Goals Scored vs Conceded", "by month, all competitions"]));
    let mut by = [(0.0, 0.0, false); 12];
    for f in &played {
        let s = slot(month_of(f.date.0, f.date.1));
        let (gf, ga) = mine(f);
        by[s] = (by[s].0 + gf as f64, by[s].1 + ga as f64, true);
    }
    match by.iter().position(|b| b.2) {
        None => out.push(line(&["N", "No matches played yet this season."])),
        Some(a) => {
            let z = slot(month_of(day, year)).max(by.iter().rposition(|b| b.2).unwrap_or(a));
            out.push(line(&["X", &(a..=z).map(|s| MONTHS[(s + 6) % 12]).collect::<Vec<_>>().join(",")]));
            out.push(line(&["S", "Goals scored", "0", "0", "1", &nums(&by[a..=z].iter().map(|b| Some(b.0)).collect::<Vec<_>>())]));
            out.push(line(&["S", "Goals conceded", "1", "0", "1", &nums(&by[a..=z].iter().map(|b| Some(b.1)).collect::<Vec<_>>())]));
        }
    }

    // ---- top performers ----
    out.push(line(&["C", "list", "Top Performers", "this season, all competitions"]));
    let squad: Vec<(usize, f32, f32, f32, f32)> = challenge::squad_of(club)
        .into_iter()
        .filter(|&s| readable(rd::<usize>(s + 0x61), 70))
        .map(|s| (s, challenge::season_stat(s, 6, 2), challenge::season_stat(s, 6, 5), challenge::season_stat(s, 6, 1), challenge::season_stat(s, 6, 0x11)))
        .collect();
    top_rows(&mut out, "Goals", &squad, |p| p.1, |v| format!("{v:.0}"), 3);
    top_rows(&mut out, "Assists", &squad, |p| p.2, |v| format!("{v:.0}"), 3);
    let most = squad.iter().map(|p| p.3).fold(0.0f32, f32::max);
    let regular: Vec<_> = squad.iter().copied().filter(|p| p.3 >= (most / 3.0).max(3.0)).collect();
    top_rows(&mut out, "Average rating", &regular, |p| p.4, |v| format!("{v:.2}"), 3);
    out
}

type Man = (usize, f32, f32, f32, f32);

/// A heading and the best `n` of a squad by a stat (above zero).
unsafe fn top_rows(out: &mut Vec<String>, title: &str, squad: &[Man], by: fn(&Man) -> f32, shown: fn(f32) -> String, n: usize) {
    let mut best: Vec<&Man> = squad.iter().filter(|p| by(p) > 0.0).collect();
    best.sort_by(|a, b| by(b).total_cmp(&by(a)));
    out.push(line(&["L", &format!("#{title}"), "", ""]));
    if best.is_empty() {
        out.push(line(&["L", "-", "", ""]));
    }
    for p in best.into_iter().take(n) {
        out.push(line(&["L", &txt(&staff_name(p.0)), &txt(&positions_text(p.0)), &shown(by(p)), &format!("@p{}", p.0)]));
    }
}

unsafe fn league_page(record: usize, day: i32, year: i32) -> Vec<String> {
    let mut out = Vec::new();
    let (comp, rows) = challenge::league_table_of(record).unwrap_or((-1, Vec::new()));
    let season = if day >= 181 { year } else { year - 1 };
    let today = year * 366 + day;
    let name = txt(&game_text(record + 4, 51));
    out.push(line(&["H", &name, &format!("LEAGUE DASHBOARD  -  {}/{:02}", season, (season + 1) % 100), &finreport::date_text(day, year), &name]));
    let clubs: Vec<i32> = rows.iter().map(|r| index_of(r.0)).collect();
    let all = league_matches(comp, &clubs, (181, season), (180, season + 1));
    let played: Vec<(Match, i32, i32)> = all.iter().filter(|m| m.key <= today).filter_map(|m| m.score.map(|(h, a)| (*m, h, a))).collect();
    let pn = played.len().max(1) as f64;
    let pct = |c: usize| format!("{:.0}%", 100.0 * c as f64 / pn);

    // ---- tiles ----
    let goals: i32 = played.iter().map(|p| p.1 + p.2).sum();
    let (hw, dr, aw) = (played.iter().filter(|p| p.1 > p.2).count(), played.iter().filter(|p| p.1 == p.2).count(), played.iter().filter(|p| p.1 < p.2).count());
    out.push(line(&["T", "Matches played", &played.len().to_string(), &format!("of {} this season", all.len()), "n"]));
    out.push(line(&["T", "Goals per game", &format!("{:.2}", goals as f64 / pn), &format!("{goals} goals"), "n"]));
    out.push(line(&["T", "Home wins", &pct(hw), &format!("{hw} matches"), "n"]));
    out.push(line(&["T", "Draws", &pct(dr), &format!("{dr} matches"), "n"]));
    out.push(line(&["T", "Away wins", &pct(aw), &format!("{aw} matches"), "n"]));

    // a club's played league results, oldest first: (for, against)
    let results = |c: i32| -> Vec<(i32, i32)> { played.iter().filter(|p| p.0.home == c || p.0.away == c).map(|p| if p.0.home == c { (p.1, p.2) } else { (p.2, p.1) }).collect() };

    // ---- the title race: points after each matchday, the top five ----
    out.push(line(&["C", "line", "Title Race", "points after each matchday"]));
    if played.is_empty() {
        out.push(line(&["N", "No league matches played yet this season."]));
    } else {
        out.push(line(&["P", "Top 3=first:3", "*Top 5=first:5", "Bottom 5=last:5"]));
        let races: Vec<(usize, Vec<f64>)> = rows
            .iter()
            .map(|r| {
                let mut sum = 0.0;
                (r.0, results(index_of(r.0)).iter().map(|&(f, a)| {
                    sum += points(f, a) as f64;
                    sum
                }).collect())
            })
            .collect();
        let most = races.iter().map(|r| r.1.len()).max().unwrap_or(0);
        out.push(line(&["X", &(1..=most).map(|k| format!("M{k}")).collect::<Vec<_>>().join(",")]));
        // (colours by place within the five shown: the painter picks them)
        for (c, run) in &races {
            out.push(line(&["S", &short(*c), "-", "0", "0", &nums(&(0..most).map(|i| run.get(i).copied()).collect::<Vec<_>>())]));
        }
    }

    // ---- match trends ----
    out.push(line(&["C", "bars", "Match Trends", "league matches this season"]));
    trends(&mut out, &played.iter().map(|p| (p.1, p.2)).collect::<Vec<_>>());

    // ---- form guide ----
    out.push(line(&["C", "table", "Form Guide", "last five league matches"]));
    let mut guide: Vec<(usize, String, i32, i32)> = rows
        .iter()
        .map(|r| {
            let all = results(index_of(r.0));
            let last = &all[all.len().saturating_sub(5)..];
            (r.0, last.iter().map(|&(f, a)| letter(f, a)).collect(), last.iter().map(|&(f, a)| points(f, a)).sum(), last.iter().map(|&(f, a)| f - a).sum())
        })
        .filter(|g: &(usize, String, i32, i32)| !g.1.is_empty())
        .collect();
    guide.sort_by_key(|g| std::cmp::Reverse((g.2, g.3)));
    if guide.is_empty() {
        out.push(line(&["N", "No league matches played yet this season."]));
    } else {
        out.push(line(&["RH", "Club", "Form", "Pts"]));
        for (c, letters, pts, _) in &guide {
            out.push(line(&["R", &format!("^{}|{}", long(*c), short(*c)), &format!("~{letters}"), &pts.to_string(), &format!("@c{c}")]));
        }
    }

    // ---- goals by month ----
    out.push(line(&["C", "cols", "Goals by Month", "home and away goals"]));
    let mut by = [(0.0, 0.0, false); 12];
    for p in &played {
        let s = slot(p.0.month);
        by[s] = (by[s].0 + p.1 as f64, by[s].1 + p.2 as f64, true);
    }
    match (by.iter().position(|b| b.2), by.iter().rposition(|b| b.2)) {
        (Some(a), Some(z)) => {
            out.push(line(&["X", &(a..=z).map(|s| MONTHS[(s + 6) % 12]).collect::<Vec<_>>().join(",")]));
            out.push(line(&["S", "Home goals", "0", "0", "1", &nums(&by[a..=z].iter().map(|b| Some(b.0)).collect::<Vec<_>>())]));
            out.push(line(&["S", "Away goals", "1", "0", "1", &nums(&by[a..=z].iter().map(|b| Some(b.1)).collect::<Vec<_>>())]));
        }
        _ => out.push(line(&["N", "No league matches played yet this season."])),
    }

    // ---- top scorers: the league row (2) of the stats ----
    out.push(line(&["C", "list", "Top Scorers", "league matches only"]));
    let players: Vec<(usize, usize, f32, f32)> = rows
        .iter()
        .flat_map(|r| challenge::squad_of(r.0).into_iter().map(move |s| (s, r.0)))
        .filter(|&(s, _)| readable(rd::<usize>(s + 0x61), 70))
        .map(|(s, c)| (s, c, challenge::season_stat(s, 2, 2), challenge::season_stat(s, 2, 5)))
        .collect();
    for (title, goals, n) in [("Goals", true, 6usize), ("Assists", false, 3)] {
        let val = |p: &(usize, usize, f32, f32)| if goals { p.2 } else { p.3 };
        let mut best: Vec<_> = players.iter().filter(|p| val(p) > 0.0).collect();
        best.sort_by(|a, b| val(b).total_cmp(&val(a)));
        out.push(line(&["L", &format!("#{title}"), "", ""]));
        if best.is_empty() {
            out.push(line(&["L", "-", "", ""]));
        }
        for p in best.into_iter().take(n) {
            out.push(line(&["L", &txt(&staff_name(p.0)), &short(p.1), &format!("{:.0}", val(p)), &format!("@c{}", p.1)]));
        }
    }
    out
}

/// A competition's short label: the initials of its words ("World Cup Qualifying" WCQ), a
/// number kept ("Euro 2004" E2004).
fn initials(name: &str) -> String {
    let s: String = name.split_whitespace().map(|w| if w.chars().all(|c| c.is_ascii_digit()) { w.to_string() } else { w.chars().next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default() }).collect();
    s.chars().take(6).collect()
}

/// A national team's page (it plays no league): its matches of the last two years, friendlies
/// and competitions apart, its squad's caps and international goals (staff +0x22 / +0x23, the
/// squad: staff +0x24 the team).
unsafe fn nation_page(team: usize, day: i32, year: i32) -> Vec<String> {
    let mut out = Vec::new();
    let me = index_of(team);
    let today = year * 366 + day;
    out.push(line(&["H", &long(team), "NATIONAL TEAM DASHBOARD  -  THE LAST TWO YEARS", &finreport::date_text(day, year), &long(team)]));
    let all = form::fixtures(me, (day, year - 2), (day, year + 1));
    let played: Vec<&form::Fixture> = all.iter().filter(|f| f.score.is_some() && f.date.1 * 366 + f.date.0 <= today).collect();
    let mine = |f: &form::Fixture| {
        let (h, a) = f.score.unwrap_or((0, 0));
        if f.home == me { (h, a) } else { (a, h) }
    };
    let other = |f: &form::Fixture| club_at(if f.home == me { f.away } else { f.home });
    let comp_of = |f: &form::Fixture| txt(&form::comp_name(f.comp));

    // ---- tiles ----
    let n = played.len();
    let (w, d) = (played.iter().filter(|f| mine(f).0 > mine(f).1).count(), played.iter().filter(|f| mine(f).0 == mine(f).1).count());
    let (gf, ga) = played.iter().fold((0, 0), |s, f| (s.0 + mine(f).0, s.1 + mine(f).1));
    out.push(line(&["T", "Record", &format!("{w}-{d}-{}", n - w - d), &format!("won, drawn, lost  -  {n} matches"), "n"]));
    out.push(line(&["T", "Win rate", &format!("{:.0}%", 100.0 * w as f64 / n.max(1) as f64), &format!("{:.0}% in competitions", 100.0 * played.iter().filter(|f| !form::friendly(f.comp) && mine(f).0 > mine(f).1).count() as f64 / played.iter().filter(|f| !form::friendly(f.comp)).count().max(1) as f64), "n"]));
    out.push(line(&["T", "Goals", &format!("{gf}-{ga}"), &format!("{:.2} scored, {:.2} conceded a game", gf as f64 / n.max(1) as f64, ga as f64 / n.max(1) as f64), if gf > ga { "g" } else if gf < ga { "r" } else { "n" }]));
    let form: String = played.iter().rev().take(6).rev().map(|f| letter(mine(f).0, mine(f).1)).collect();
    let (fw, fl) = (form.matches('W').count(), form.matches('L').count());
    out.push(line(&["T", "Form", &format!("~{form}"), &format!("last {} matches", form.len()), if fw > fl { "g" } else if fl > fw { "r" } else { "n" }]));
    match all.iter().find(|f| f.score.is_none() && f.date.1 * 366 + f.date.0 >= today) {
        Some(f) => out.push(line(&["T", "Next match", &short_date(f.date.0, f.date.1), &format!("v {} ({})  -  {}", short(other(f)), if f.home == me { "H" } else { "A" }, comp_of(f)), "n"])),
        None => out.push(line(&["T", "Next match", "-", "none planned yet", "n"])),
    }

    // ---- form trend ----
    out.push(line(&["C", "line", "Form Trend", "points a match"]));
    if n == 0 {
        out.push(line(&["N", "No matches in the last two years."]));
    } else {
        let pts: Vec<f64> = played.iter().map(|f| points(mine(f).0, mine(f).1) as f64).collect();
        let rolling: Vec<Option<f64>> = (0..n).map(|i| Some(pts[i.saturating_sub(4)..=i].iter().sum::<f64>() / (i - i.saturating_sub(4) + 1) as f64)).collect();
        out.push(line(&["P", "Last 5=x:5", "Last 10=x:10", "*Last 15=x:15", "All=x:0"]));
        out.push(line(&["X", &(1..=n).map(|i| format!("{i}")).collect::<Vec<_>>().join(",")]));
        out.push(line(&["S", "Points", "0", "0", "1", &nums(&pts.iter().map(|&v| Some(v)).collect::<Vec<_>>())]));
        out.push(line(&["S", "Rolling average (5)", "5", "1", "0", &nums(&rolling)]));
        let tips: Vec<String> = played.iter().map(|f| format!("{} {}  v {} ({})  {}-{}  {}", short_date(f.date.0, f.date.1), f.date.1, short(other(f)), if f.home == me { "H" } else { "A" }, mine(f).0, mine(f).1, comp_of(f))).collect();
        let mut tt = vec!["TT"];
        tt.extend(tips.iter().map(String::as_str));
        out.push(line(&tt));
    }

    // ---- performance ----
    out.push(line(&["C", "bars", "Performance", "last two years"]));
    if n == 0 {
        out.push(line(&["N", "No matches in the last two years."]));
    } else {
        let rate = |pick: &dyn Fn(&form::Fixture) -> bool| {
            let set: Vec<&&form::Fixture> = played.iter().filter(|f| pick(f)).collect();
            (set.iter().filter(|f| mine(f).0 > mine(f).1).count() as f64 / set.len().max(1) as f64, set.len())
        };
        let (comp_w, comp_n) = rate(&|f| !form::friendly(f.comp));
        let (fr_w, fr_n) = rate(&|f| form::friendly(f.comp));
        let (home_w, home_n) = rate(&|f| f.home == me);
        let (away_w, away_n) = rate(&|f| f.home != me);
        let clean = played.iter().filter(|f| mine(f).1 == 0).count() as f64 / n as f64;
        let rows: [(&str, f64, String, String); 7] = [
            ("Win rate in competitions", comp_w, format!("{:.0}%", 100.0 * comp_w), format!("{comp_n} matches")),
            ("Win rate in friendlies", fr_w, format!("{:.0}%", 100.0 * fr_w), format!("{fr_n} matches")),
            ("Win rate at home", home_w, format!("{:.0}%", 100.0 * home_w), format!("{home_n} matches")),
            ("Win rate away", away_w, format!("{:.0}%", 100.0 * away_w), format!("{away_n} matches")),
            ("Goals scored / game", gf as f64 / n as f64 / 3.0, format!("{:.2}", gf as f64 / n as f64), String::new()),
            ("Goals conceded / game", ga as f64 / n as f64 / 3.0, format!("{:.2}", ga as f64 / n as f64), String::new()),
            ("Clean sheets", clean, format!("{:.0}%", 100.0 * clean), String::new()),
        ];
        for (label, frac, shown, note) in rows {
            out.push(line(&["B", label, &shown, &format!("{:.3}", frac.clamp(0.0, 1.0)), &note, "b"]));
        }
    }

    // ---- results, competitions and friendlies apart ----
    out.push(line(&["C", "table", "Results", "the latest first"]));
    if n == 0 {
        out.push(line(&["N", "No matches in the last two years."]));
    } else {
        out.push(line(&["P", "*All=g:", "Competitions=g:C", "Friendlies=g:F"]));
        out.push(line(&["RH", "Date", "Opponent", "Competition", "Result"]));
        for f in played.iter().rev().take(30) {
            let (a, b) = mine(f);
            let o = other(f);
            out.push(line(&[
                "R",
                &format!("{} {}", short_date(f.date.0, f.date.1), f.date.1 % 100),
                &format!("^{}|{} ({})", long(o), short(o), if f.home == me { "H" } else { "A" }),
                &comp_of(f),
                &format!("{} {a}-{b}", letter(a, b)),
                if form::friendly(f.comp) { "!F" } else { "!C" },
            ]));
        }
    }

    // ---- goals by competition ----
    out.push(line(&["C", "cols", "Goals by Competition", "scored and conceded"]));
    let mut comps: Vec<(i32, f64, f64, usize)> = Vec::new();
    for f in &played {
        let (a, b) = mine(f);
        match comps.iter_mut().find(|c| c.0 == f.comp) {
            Some(c) => *c = (c.0, c.1 + a as f64, c.2 + b as f64, c.3 + 1),
            None => comps.push((f.comp, a as f64, b as f64, 1)),
        }
    }
    if comps.is_empty() {
        out.push(line(&["N", "No matches in the last two years."]));
    } else {
        comps.sort_by_key(|c| std::cmp::Reverse(c.3));
        comps.truncate(8);
        let names: Vec<String> = comps.iter().map(|c| txt(&form::comp_name(c.0))).collect();
        out.push(line(&["X", &names.iter().map(|n| initials(n)).collect::<Vec<_>>().join(",")]));
        out.push(line(&["S", "Goals scored", "0", "0", "1", &nums(&comps.iter().map(|c| Some(c.1)).collect::<Vec<_>>())]));
        out.push(line(&["S", "Goals conceded", "1", "0", "1", &nums(&comps.iter().map(|c| Some(c.2)).collect::<Vec<_>>())]));
        let tips: Vec<String> = comps.iter().zip(&names).map(|(c, n)| format!("{n}  -  {} matches", c.3)).collect();
        let mut tt = vec!["TT"];
        tt.extend(tips.iter().map(String::as_str));
        out.push(line(&tt));
    }

    // ---- the squad: caps and international goals ----
    out.push(line(&["C", "list", "Squad", "the players called up"]));
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    let squad: Vec<(usize, u8, u8)> = if readable(base, count.max(0) as usize * 0x6e) {
        (0..count.max(0) as usize).map(|k| base + k * 0x6e).filter(|&s| rd::<usize>(s + 0x24) == team && readable(rd::<usize>(s + 0x61), 70)).map(|s| (s, rd::<u8>(s + 0x22), rd::<u8>(s + 0x23))).collect()
    } else {
        Vec::new()
    };
    if squad.is_empty() {
        out.push(line(&["N", "No players called up."]));
    } else {
        for (title, goals, n) in [("Most caps", false, 5usize), ("International goals", true, 4)] {
            let mut best: Vec<&(usize, u8, u8)> = squad.iter().filter(|s| if goals { s.2 > 0 } else { s.1 > 0 }).collect();
            best.sort_by_key(|s| std::cmp::Reverse(if goals { (s.2, s.1) } else { (s.1, s.2) }));
            out.push(line(&["L", &format!("#{title}"), "", ""]));
            if best.is_empty() {
                out.push(line(&["L", "-", "", ""]));
            }
            for s in best.into_iter().take(n) {
                let club = rd::<usize>(s.0 + 0x39);
                let link = if is_club(club) { format!("@c{club}") } else { String::new() };
                out.push(line(&["L", &txt(&staff_name(s.0)), &if is_club(club) { short(club) } else { String::new() }, &(if goals { s.2 } else { s.1 }).to_string(), &link]));
            }
        }
    }
    out
}

/// The Match Trends bars of played scores (home, away).
fn trends(out: &mut Vec<String>, played: &[(i32, i32)]) {
    if played.is_empty() {
        out.push(line(&["N", "No matches played yet."]));
        return;
    }
    let pn = played.len() as f64;
    let share = |c: usize| (c as f64 / pn, format!("{:.0}%", 100.0 * c as f64 / pn));
    let over = share(played.iter().filter(|p| p.0 + p.1 >= 3).count());
    let both = share(played.iter().filter(|p| p.0 > 0 && p.1 > 0).count());
    let nil = share(played.iter().filter(|p| p.0 == 0 && p.1 == 0).count());
    let clean = played.iter().map(|p| (p.0 == 0) as usize + (p.1 == 0) as usize).sum::<usize>() as f64 / (2.0 * pn);
    let (hg, ag) = (played.iter().map(|p| p.0).sum::<i32>() as f64 / pn, played.iter().map(|p| p.1).sum::<i32>() as f64 / pn);
    let bars: [(&str, f64, String); 6] = [
        ("Over 2.5 goals", over.0, over.1),
        ("Both teams scored", both.0, both.1),
        ("Clean sheets (a team)", clean, format!("{:.0}%", 100.0 * clean)),
        ("Goalless draws", nil.0, nil.1),
        ("Home goals / game", hg / 3.0, format!("{hg:.2}")),
        ("Away goals / game", ag / 3.0, format!("{ag:.2}")),
    ];
    for (label, frac, shown) in bars {
        out.push(line(&["B", label, &shown, &format!("{:.3}", frac.clamp(0.0, 1.0)), "", "b"]));
    }
}

/// A cup's match (a fixture read by `form::comp_fixtures`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cm {
    key: i32,
    day: i32,
    year: i32,
    ed: i32,
    stage: u8,
    round: u8,
    home: i32,
    away: i32,
    score: Option<(i32, i32)>,
}

fn cm_of(r: &[u8; 0x4f]) -> Cm {
    let i16_at = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]) as i32;
    let i32_at = |o: usize| i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
    let (hg, ag) = (r[0x43], r[0x44]);
    let (day, year) = (i16_at(0x2a), i16_at(0x28));
    Cm { key: year * 366 + day, day, year, ed: i16_at(0x2c), stage: r[0x42], round: r[0x34], home: i32_at(0xc), away: i32_at(0x10), score: (hg != 0xff && ag != 0xff && hg <= 40 && ag <= 40).then_some((hg as i32, ag as i32)) }
}

/// A round of a cup: its name, short label, first day, its matches (indices); a table phase's
/// matchday too.
#[derive(Debug, PartialEq)]
struct Rnd {
    name: String,
    short: String,
    key: i32,
    ix: Vec<usize>,
}

/// A cup's edition worked out from its fixtures: the groups (by phase: name, its groups as
/// (letter, teams)) and the rounds in order. A stage whose rounds all have the same teams (three
/// or more) is a table: its teams, split by who played whom, are its groups (the 26/27 league
/// phase one group of 36, the old Champions League a stage a group); the other stages' rounds are
/// knockout rounds, named by the teams left in them, qualifying rounds before the first table.
fn cup_shape(ms: &[Cm]) -> (Vec<(String, Vec<(String, Vec<i32>)>)>, Vec<Rnd>) {
    use std::collections::BTreeMap;
    let mut stages: BTreeMap<u8, BTreeMap<u8, Vec<usize>>> = BTreeMap::new();
    for (i, m) in ms.iter().enumerate() {
        stages.entry(m.stage).or_default().entry(m.round).or_default().push(i);
    }
    let teams_of = |ix: &[usize]| {
        let mut t: Vec<i32> = ix.iter().flat_map(|&i| [ms[i].home, ms[i].away]).collect();
        t.sort();
        t.dedup();
        t
    };
    let first_key = |ix: &[usize]| ix.iter().map(|&i| ms[i].key).min().unwrap_or(0);
    let is_table = |st: u8, rounds: &BTreeMap<u8, Vec<usize>>| {
        let n: Vec<usize> = rounds.values().map(|r| teams_of(r).len()).collect();
        st != 255 && n.len() >= 2 && n[0] >= 3 && n.iter().all(|&x| x == n[0])
    };
    // the tables: each stage's groups (teams joined by their matches), stages that start within
    // three weeks of each other one phase
    let mut tables: Vec<(i32, u8, Vec<i32>)> = Vec::new();
    for (st, rounds) in stages.iter().filter(|(s, r)| is_table(**s, r)) {
        let ix: Vec<usize> = rounds.values().flatten().copied().collect();
        let mut groups: Vec<Vec<i32>> = Vec::new();
        for &i in &ix {
            let (h, a) = (ms[i].home, ms[i].away);
            let found: Vec<usize> = (0..groups.len()).filter(|&g| groups[g].contains(&h) || groups[g].contains(&a)).collect();
            let mut merged = vec![h, a];
            for &g in found.iter().rev() {
                merged.extend(groups.remove(g));
            }
            merged.sort();
            merged.dedup();
            groups.push(merged);
        }
        groups.sort();
        for g in groups {
            tables.push((first_key(&ix), *st, g));
        }
    }
    tables.sort_by_key(|t| (t.0, t.1));
    let mut phases: Vec<(i32, Vec<(u8, Vec<i32>)>)> = Vec::new();
    for (key, st, g) in tables {
        match phases.last_mut() {
            Some(p) if key - p.0 <= 21 => p.1.push((st, g)),
            _ => phases.push((key, vec![(st, g)])),
        }
    }
    let many = phases.len() > 1;
    let phase_name = |k: usize, groups: &[(u8, Vec<i32>)]| {
        let base = if groups.len() == 1 && groups[0].1.len() > 8 { "League phase" } else { "Group stage" };
        if many { format!("{base} {}", k + 1) } else { base.to_string() }
    };
    let mut rounds: Vec<Rnd> = Vec::new();
    let mut shaped = Vec::new();
    for (k, (_, groups)) in phases.iter().enumerate() {
        let name = phase_name(k, groups);
        let letters: Vec<(String, Vec<i32>)> =
            groups.iter().enumerate().map(|(g, (_, t))| (if groups.len() == 1 { name.clone() } else { format!("Group {}", (b'A' + (g % 26) as u8) as char) }, t.clone())).collect();
        // its matchdays, all its groups together
        let mut by: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for (st, _) in groups.iter() {
            for (rd, ix) in &stages[st] {
                by.entry(*rd).or_default().extend(ix.iter().copied().filter(|&i| groups.iter().any(|g| g.0 == *st && g.1.contains(&ms[i].home))));
            }
        }
        by.retain(|_, ix| !ix.is_empty());
        for (rd, ix) in by {
            let ix: Vec<usize> = ix.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            rounds.push(Rnd { name: format!("{name}, matchday {}", rd + 1), short: if many { format!("G{}.{}", k + 1, rd + 1) } else { format!("MD{}", rd + 1) }, key: first_key(&ix), ix });
        }
        shaped.push((name, letters));
    }
    let table_start = phases.first().map_or(i32::MAX, |p| p.0);
    let mut ko: Vec<(i32, usize, Vec<usize>)> =
        stages.iter().filter(|(s, r)| !is_table(**s, r)).flat_map(|(_, r)| r.values().map(|ix| (first_key(ix), teams_of(ix).len(), ix.clone()))).collect();
    ko.sort_by_key(|k| k.0);
    let (mut q, mut plain) = (0, 0);
    for k in 0..ko.len() {
        let (key, t, ix) = &ko[k];
        let later_same = ko[k + 1..].iter().any(|o| o.1 == *t && o.0 > table_start.min(*key));
        // (qualifying only before a group stage: a cup of knockout rounds alone, the old UEFA
        // Cup, had every round "Qualifying round", user 2026-11-28)
        let (name, short) = if !phases.is_empty() && *key < table_start {
            q += 1;
            (format!("Qualifying round {q}"), format!("Q{q}"))
        } else if *t == 2 && later_same {
            ("Third place".into(), "3rd".into())
        } else if later_same && [4, 8, 16, 32].contains(t) {
            ("Play-off round".into(), "PO".into())
        } else {
            match t {
                2 => ("Final".into(), "F".into()),
                4 => ("Semi-finals".into(), "SF".into()),
                8 => ("Quarter-finals".into(), "QF".into()),
                16 => ("Round of 16".into(), "R16".into()),
                32 => ("Round of 32".into(), "R32".into()),
                _ => {
                    plain += 1;
                    (format!("Round {plain}"), format!("R{plain}"))
                }
            }
        };
        rounds.push(Rnd { name, short, key: *key, ix: ix.clone() });
    }
    rounds.sort_by_key(|r| r.key);
    (shaped, rounds)
}

/// A group's table from the played matches: (team, P, W, D, L, F, A, Pts), in order.
fn group_table(ms: &[Cm], teams: &[i32]) -> Vec<(i32, [i32; 7])> {
    let mut rows: Vec<(i32, [i32; 7])> = teams.iter().map(|&t| (t, [0; 7])).collect();
    for m in ms.iter().filter(|m| teams.contains(&m.home) && teams.contains(&m.away)) {
        let Some((h, a)) = m.score else { continue };
        for (team, f, ag) in [(m.home, h, a), (m.away, a, h)] {
            if let Some(r) = rows.iter_mut().find(|r| r.0 == team) {
                let p = points(f, ag);
                r.1 = [r.1[0] + 1, r.1[1] + (p == 3) as i32, r.1[2] + (p == 1) as i32, r.1[3] + (p == 0) as i32, r.1[4] + f, r.1[5] + ag, r.1[6] + p];
            }
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse((r.1[6], r.1[4] - r.1[5], r.1[4])));
    rows
}

unsafe fn cup_page(record: usize, day: i32, year: i32) -> Option<Vec<String>> {
    let comp = rd::<i32>(record);
    let today = year * 366 + day;
    let all: Vec<Cm> = form::comp_fixtures(comp, (0, year - 2), (365, year + 1)).iter().map(cm_of).filter(|m| m.home >= 0 && m.away >= 0).collect();
    if all.is_empty() {
        return None;
    }
    // the edition: the latest begun, else the first to come
    let mut eds: Vec<i32> = all.iter().map(|m| m.ed).collect();
    eds.sort();
    eds.dedup();
    let first = |e: i32| all.iter().filter(|m| m.ed == e).map(|m| m.key).min().unwrap_or(i32::MAX);
    let ed = eds.iter().copied().filter(|&e| first(e) <= today).max().unwrap_or(eds[0]);
    let mut ms: Vec<Cm> = all.into_iter().filter(|m| m.ed == ed).collect();
    ms.sort_by_key(|m| (m.key, m.home));
    let done = |m: &Cm| m.score.is_some() && m.key <= today;
    let played: Vec<&Cm> = ms.iter().filter(|m| done(m)).collect();
    let (groups, rounds) = cup_shape(&ms);
    let round_of = |i: usize| rounds.iter().find(|r| r.ix.contains(&i));
    let name = |t: i32| short(club_at(t));
    let link = |t: i32| {
        let c = club_at(t);
        if c == 0 || national_team(c) { String::new() } else { format!("@c{c}") }
    };

    let mut out = Vec::new();
    let title = txt(&game_text(record + 4, 51));
    let (y0, y1) = (ms.first().map_or(ed, |m| m.year), ms.last().map_or(ed, |m| m.year));
    let label = if y0 == y1 { y0.to_string() } else { format!("{y0}/{:02}", y1 % 100) };
    out.push(line(&["H", &title, &format!("CUP DASHBOARD  -  {label}"), &finreport::date_text(day, year), &title]));

    // ---- tiles ----
    let next = ms.iter().enumerate().find(|(_, m)| !done(m));
    let now_round = next.and_then(|(i, _)| round_of(i)).or(rounds.last());
    out.push(line(&["T", "Stage", now_round.map_or("-", |r| r.name.as_str()), &format!("{} of {} matches played", played.len(), ms.len()), "n"]));
    let mut teams: Vec<i32> = ms.iter().flat_map(|m| [m.home, m.away]).collect();
    teams.sort();
    teams.dedup();
    let left = now_round.filter(|r| !r.short.starts_with("MD") && !r.short.starts_with('G')).map(|r| r.ix.iter().flat_map(|&i| [ms[i].home, ms[i].away]).collect::<std::collections::BTreeSet<_>>().len());
    out.push(line(&["T", "Teams", &teams.len().to_string(), &left.map_or("in this edition".to_string(), |n| format!("{n} left")), "n"]));
    let goals: i32 = played.iter().filter_map(|m| m.score).map(|(h, a)| h + a).sum();
    out.push(line(&["T", "Goals per game", &format!("{:.2}", goals as f64 / played.len().max(1) as f64), &format!("{goals} goals"), "n"]));
    match played.iter().filter_map(|m| m.score.map(|s| (m, s))).max_by_key(|(_, (h, a))| ((h - a).abs(), h + a)) {
        Some((m, (h, a))) => out.push(line(&["T", "Biggest win", &format!("{h}-{a}"), &format!("{} v {}", name(m.home), name(m.away)), "n"])),
        None => out.push(line(&["T", "Biggest win", "-", "", "n"])),
    }
    let fin = rounds.iter().find(|r| r.short == "F" && r.ix.iter().all(|&i| done(&ms[i])));
    match (fin, next) {
        (Some(f), _) => {
            let m = ms[*f.ix.last().unwrap()];
            let (h, a) = m.score.unwrap_or((0, 0));
            let (winner, note) = if h == a { (String::from("-"), format!("{} {h}-{a} {}, decided after", name(m.home), name(m.away))) } else { (name(if h > a { m.home } else { m.away }), format!("{} {h}-{a} {} in the final", name(m.home), name(m.away))) };
            out.push(line(&["T", "Winner", &winner, &note, "g"]));
        }
        (None, Some((_, m))) => out.push(line(&["T", "Next match", &short_date(m.day, m.year), &format!("{} v {}", name(m.home), name(m.away)), "n"])),
        _ => out.push(line(&["T", "Next match", "-", "", "n"])),
    }

    // ---- the groups (pages of eight groups or twelve places; pills by phase and page), or the
    // round on now ----
    if groups.is_empty() {
        let r = now_round;
        out.push(line(&["C", "table", r.map_or("Matches", |r| r.name.as_str()), "this round"]));
        out.push(line(&["RH", "Date", "Home", "Away", "Score"]));
        for &i in r.map(|r| r.ix.as_slice()).unwrap_or(&[]) {
            let m = ms[i];
            let score = m.score.filter(|_| done(&m)).map_or("-".to_string(), |(h, a)| format!("{h}-{a}"));
            out.push(line(&["R", &short_date(m.day, m.year), &format!("^{}|{}", long(club_at(m.home)), name(m.home)), &format!("^{}|{}", long(club_at(m.away)), name(m.away)), &score]));
        }
    } else {
        let current = rounds.iter().filter(|r| r.ix.iter().any(|&i| done(&ms[i]))).last().and_then(|r| groups.iter().position(|(n, _)| r.name.starts_with(n.as_str()))).unwrap_or(0);
        out.push(line(&["C", "groups", &groups[current].0, "the tables so far"]));
        let mut pills: Vec<String> = Vec::new();
        let mut blocks: Vec<String> = Vec::new();
        for (k, (_, gs)) in groups.iter().enumerate() {
            let pages: Vec<Vec<(String, Vec<(i32, [i32; 7])>, usize)>> = if gs.len() == 1 {
                let table = group_table(&ms, &gs[0].1);
                table.chunks(12).enumerate().map(|(p, c)| vec![(gs[0].0.clone(), c.to_vec(), p * 12)]).collect()
            } else {
                gs.chunks(8).map(|c| c.iter().map(|g| (g.0.clone(), group_table(&ms, &g.1), 0)).collect()).collect()
            };
            for (p, page) in pages.iter().enumerate() {
                let tag = format!("{k}.{p}");
                let mut label = if groups.len() > 1 { format!("Stage {}", k + 1) } else { String::new() };
                if pages.len() > 1 {
                    let part = if gs.len() == 1 { format!("{}-{}", p * 12 + 1, p * 12 + page[0].1.len()) } else { format!("{}-{}", page[0].0.trim_start_matches("Group "), page[page.len() - 1].0.trim_start_matches("Group ")) };
                    label = if label.is_empty() { part } else { format!("{label} {part}") };
                }
                if !label.is_empty() {
                    pills.push(format!("{}{label}=g:{tag}", if k == current && p == 0 { "*" } else { "" }));
                }
                for (gname, rows, from) in page {
                    blocks.push(line(&["G", gname, &format!("!{tag}")]));
                    for (n, (t, v)) in rows.iter().enumerate() {
                        blocks.push(line(&[
                            "GR",
                            &(from + n + 1).to_string(),
                            &format!("^{}|{}", long(club_at(*t)), name(*t)),
                            &v[0].to_string(),
                            &v[1].to_string(),
                            &v[2].to_string(),
                            &v[3].to_string(),
                            &v[4].to_string(),
                            &v[5].to_string(),
                            &v[6].to_string(),
                            &link(*t),
                        ]));
                    }
                }
            }
        }
        if !pills.is_empty() {
            let mut p = vec!["P"];
            p.extend(pills.iter().map(String::as_str));
            out.push(line(&p));
        }
        out.extend(blocks);
    }

    // ---- trends ----
    out.push(line(&["C", "bars", "Match Trends", "this edition"]));
    trends(&mut out, &played.iter().filter_map(|m| m.score).collect::<Vec<_>>());

    // ---- results: the latest first, pills for the last three rounds ----
    out.push(line(&["C", "table", "Results", "the latest first"]));
    if played.is_empty() {
        out.push(line(&["N", "No matches played yet."]));
    } else {
        let mut recent: Vec<&str> = Vec::new();
        for (i, _) in ms.iter().enumerate().rev().filter(|(_, m)| done(m)) {
            if let Some(r) = round_of(i) {
                if !recent.contains(&r.short.as_str()) && recent.len() < 3 {
                    recent.push(r.short.as_str());
                }
            }
        }
        let mut p = vec!["P".to_string(), "*All=g:".to_string()];
        p.extend(recent.iter().rev().map(|s| format!("{s}=g:{s}")));
        out.push(line(&p.iter().map(String::as_str).collect::<Vec<_>>()));
        out.push(line(&["RH", "Round", "Home", "Away", "Score"]));
        for (i, m) in ms.iter().enumerate().rev().filter(|(_, m)| done(m)).take(40) {
            let (h, a) = m.score.unwrap_or((0, 0));
            let short_r = round_of(i).map_or("", |r| r.short.as_str());
            out.push(line(&["R", short_r, &format!("^{}|{}", long(club_at(m.home)), name(m.home)), &format!("^{}|{}", long(club_at(m.away)), name(m.away)), &format!("{h}-{a}"), &format!("!{short_r}")]));
        }
    }

    // ---- goals per round ----
    out.push(line(&["C", "cols", "Goals per Round", "home and away goals"]));
    let shown: Vec<&Rnd> = rounds.iter().filter(|r| r.ix.iter().any(|&i| done(&ms[i]))).collect();
    if shown.is_empty() {
        out.push(line(&["N", "No matches played yet."]));
    } else {
        let sum = |r: &Rnd, home: bool| r.ix.iter().filter(|&&i| done(&ms[i])).filter_map(|&i| ms[i].score).map(|(h, a)| if home { h } else { a }).sum::<i32>() as f64;
        out.push(line(&["X", &shown.iter().map(|r| r.short.as_str()).collect::<Vec<_>>().join(",")]));
        out.push(line(&["S", "Home goals", "0", "0", "1", &nums(&shown.iter().map(|r| Some(sum(r, true))).collect::<Vec<_>>())]));
        out.push(line(&["S", "Away goals", "1", "0", "1", &nums(&shown.iter().map(|r| Some(sum(r, false))).collect::<Vec<_>>())]));
        let tips: Vec<String> = shown
            .iter()
            .map(|r| {
                let n = r.ix.iter().filter(|&&i| done(&ms[i])).count();
                format!("{}  -  {n} of {} played, {:.2} goals a game", r.name, r.ix.len(), (sum(r, true) + sum(r, false)) / n.max(1) as f64)
            })
            .collect();
        let mut tt = vec!["TT"];
        tt.extend(tips.iter().map(String::as_str));
        out.push(line(&tt));
    }

    // ---- team leaders ----
    out.push(line(&["C", "list", "Team Leaders", "this edition"]));
    let stats: Vec<(i32, f64, f64, f64, f64)> = teams
        .iter()
        .map(|&t| {
            let mine: Vec<(i32, i32)> = played.iter().filter(|m| m.home == t || m.away == t).filter_map(|m| m.score.map(|(h, a)| if m.home == t { (h, a) } else { (a, h) })).collect();
            (t, mine.len() as f64, mine.iter().map(|x| x.0).sum::<i32>() as f64, mine.iter().map(|x| x.1).sum::<i32>() as f64, mine.iter().filter(|x| x.0 > x.1).count() as f64)
        })
        .filter(|s| s.1 > 0.0)
        .collect();
    let most_games = stats.iter().map(|s| s.1).fold(0.0f64, f64::max);
    type Lead = (i32, f64, f64, f64, f64);
    let leaders: [(&str, fn(&Lead) -> f64, bool, fn(&Lead) -> String); 3] = [
        ("Most goals", |s| s.2, true, |s| format!("{:.0}", s.2)),
        ("Best defence (conceded a game)", |s| s.3 / s.1, false, |s| format!("{:.2}", s.3 / s.1)),
        ("Most wins", |s| s.4, true, |s| format!("{:.0}", s.4)),
    ];
    for (title, by, high, shown) in leaders {
        let mut best: Vec<&Lead> = stats.iter().filter(|s| high || s.1 >= (most_games / 2.0).max(1.0)).collect();
        best.sort_by(|a, b| if high { by(b).total_cmp(&by(a)) } else { by(a).total_cmp(&by(b)) });
        out.push(line(&["L", &format!("#{title}"), "", ""]));
        for s in best.into_iter().take(3) {
            out.push(line(&["L", &name(s.0), &format!("{:.0} games", s.1), &shown(s), &link(s.0)]));
        }
    }
    Some(out)
}

// ---- the page ----

/// What a place on the page does: a link (`@c` club, `@l` league record: that dashboard; `@p` a
/// player's profile), a card's pill, a chart's x place (its tooltip).
#[derive(Clone, PartialEq)]
enum Hit {
    Link(String),
    Pill(usize, usize),
    Tip(usize, usize),
}

type Rect = (i32, i32, i32, i32);

static mut OPEN: bool = false;
static mut MODEL: Option<Model> = None;
static mut PARAM: usize = 0;
static mut BUTTON: Option<Rect> = None;
static mut SEEN: bool = false;
static mut BACK: Option<Rect> = None;
static mut BACK_HOVER: bool = false;
static mut OPENED_AT: Option<std::time::Instant> = None;
/// The pages followed here (Back goes to the one before), the pills picked per card.
static mut HISTORY: Vec<usize> = Vec::new();
static mut PILLS: [Option<usize>; 5] = [None; 5];
/// This frame's places that answer the mouse, and the one under it.
static mut HITS: Vec<(Rect, Hit)> = Vec::new();
static mut UNDER: Option<usize> = None;
/// Opened on a club screen (single player / host): a player's name opens his profile there, by
/// the club screen's input (`take_person`).
static mut FROM_CLUB: bool = false;
static mut PERSON: Option<usize> = None;
/// The tooltip of this frame, painted last: where, its lines.
static mut TIP: Option<(i32, i32, Vec<(Vec<u8>, [u8; 3])>)> = None;

pub fn page_open() -> bool {
    unsafe { OPEN }
}

/// The club screen's input: a player's name clicked on the page.
#[allow(static_mut_refs)]
pub unsafe fn take_person() -> Option<usize> {
    PERSON.take()
}

/// The text hook: the game drew the Dashboard button's label (its box is the button's).
pub unsafe fn note_button(x1: i32, y1: i32, x2: i32, y2: i32) {
    BUTTON = Some((x1, y1, x2, y2));
    SEEN = true;
}

unsafe fn fetch() -> Option<Model> {
    let text = ask_host(&key(b'd', &[PARAM as u32]))?;
    let text = latin1(&text).into_owned();
    Some(text.lines().map(|l| l.split('\t').map(String::from).collect()).collect())
}

/// What the host gives a client for key 'd'.
pub unsafe fn remote(p: usize) -> Option<Vec<u8>> {
    build(p).map(|s| to_latin1(&s))
}

/// Shows the page of `p` (a club, a competition or its record).
unsafe fn load(p: usize) {
    PARAM = p;
    PILLS = [None; 5];
    if remote::CLIENT {
        remote::forget(&key(b'd', &[p as u32]));
    }
    MODEL = fetch();
}

#[allow(static_mut_refs)]
unsafe fn open() {
    let p = screen_param(0);
    let p = if p == 0 { human_club_ptr().unwrap_or(0) } else { p };
    FROM_CLUB = !remote::CLIENT && is_club(p);
    HISTORY.clear();
    load(p);
    (OPEN, BACK_HOVER, UNDER, OPENED_AT) = (true, false, None, Some(std::time::Instant::now()));
}

/// A link works here: a profile only from a club screen.
unsafe fn live(link: &str) -> bool {
    link.len() > 2 && (FROM_CLUB || !link.starts_with("@p"))
}

#[allow(static_mut_refs)]
unsafe fn follow(link: &str) {
    let Ok(p) = link[2..].parse::<usize>() else { return };
    if link.starts_with("@p") {
        PERSON = Some(p);
        OPEN = false;
        REDRAW = true;
    } else {
        HISTORY.push(PARAM);
        load(p);
    }
}

/// The mouse: a click on the game's Dashboard button is taken before the game sees it (no
/// dropdown, no printout) and opens the page; while it is open the page takes every click but
/// the menu bar's: links, pills, Back; moving over a row or a chart redraws it lit.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    let inside = |r: Rect| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if !OPEN {
        let b = BUTTON?;
        if !inside(b) {
            return None;
        }
        let clicked = *(LEFT_CLICK as *const u32) != 0;
        let held = BUTTON_DOWN_NOW();
        if !(held || released || clicked) {
            return None;
        }
        *(LEFT_CLICK as *mut u32) = 0;
        if held {
            return Some(false);
        }
        open();
        return Some(true);
    }
    if MODEL.is_none() && remote::CLIENT {
        remote::ask();
        if let Some(m) = fetch() {
            MODEL = Some(m);
            return Some(true);
        }
    }
    if mx < bar_end(screen().0) {
        if released {
            OPEN = false;
            REDRAW = true;
        }
        return None;
    }
    *(LEFT_CLICK as *mut u32) = 0;
    *(assistant::RIGHT_CLICK as *mut u32) = 0;
    let over = BACK.is_some_and(inside);
    let under = HITS.iter().position(|h| inside(h.0));
    let redraw = (std::mem::replace(&mut BACK_HOVER, over) != over) | (std::mem::replace(&mut UNDER, under) != under);
    if !released || OPENED_AT.is_some_and(|t| t.elapsed().as_millis() < 300) {
        return Some(redraw);
    }
    if over {
        match HISTORY.pop() {
            Some(p) => load(p),
            None => OPEN = false,
        }
        return Some(true);
    }
    match under.map(|k| HITS[k].1.clone()) {
        Some(Hit::Pill(card, pill)) => PILLS[card] = Some(pill),
        Some(Hit::Link(l)) => follow(&l),
        _ => return Some(redraw),
    }
    Some(true)
}

/// The flip pass: the button forgotten on a whole frame without it; the page while it is open.
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    if full_frame && !std::mem::take(&mut SEEN) {
        BUTTON = None;
    }
    if OPEN {
        draw();
    }
}

unsafe fn tall(f: u32) -> i32 {
    game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32)
}

fn field(l: &[String], k: usize) -> &str {
    l.get(k).map(String::as_str).unwrap_or("")
}

/// A line's link field (`@...`), anywhere after its kind.
fn link_of(l: &[String]) -> Option<&str> {
    l.iter().skip(1).map(String::as_str).find(|f| f.starts_with('@'))
}

fn tone(t: &str) -> [u8; 3] {
    match t {
        "g" => GREEN,
        "r" => RED,
        "b" => BLUE,
        _ => DIM,
    }
}

fn mouse() -> (i32, i32) {
    unsafe { (*(MOUSE_X as *const i32), *(MOUSE_Y as *const i32)) }
}

fn hovered(r: Rect) -> bool {
    let (x, y) = mouse();
    (r.0..=r.2).contains(&x) && (r.1..=r.3).contains(&y)
}

/// A value as a chart shows it: whole numbers without decimals.
fn short_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 { format!("{v:.0}") } else { format!("{v:.2}") }
}

struct Fonts {
    small: u32,
    body: u32,
    head: u32,
    big: u32,
}

const F1: u32 = 0xffff0001;
const LIT: [u8; 3] = [0x1f, 0x27, 0x33];
const EDGE_LIT: [u8; 3] = [0x4a, 0x6e, 0xa8];

#[allow(static_mut_refs)]
unsafe fn draw() {
    HITS.clear();
    TIP = None;
    let (sw, sh) = screen();
    let left = bar_end(sw);
    fill(left, 0, sw - 1, sh - 1, PAGE);
    let m = (sh / 88).max(6);
    let (f2, f3, f5, f6) = (0xffff0002u32, 0xffff0003u32, 0xffff0005u32, 0xffff0006u32);
    let fonts = if sh < 750 {
        Fonts { small: F1, body: F1, head: f2, big: f3 }
    } else if sh < 950 {
        Fonts { small: F1, body: f2, head: f3, big: f5 }
    } else {
        Fonts { small: f2, body: f2, head: f3, big: f6 }
    };
    let model = MODEL.clone().unwrap_or_default();

    // the header: crest, name, what it is; the date and Back at the right
    let bar = sh * 8 / 100;
    fill(left, 0, sw - 1, bar, [0x11, 0x16, 0x1e]);
    fill(left, bar, sw - 1, bar + 1, EDGE);
    let b = (sw - m * 2 - sw / 10, m, sw - m * 2, bar - m);
    if let Some(h) = model.iter().find(|l| field(l, 0) == "H") {
        let mut tx = left + m * 2;
        let crest = to_latin1(field(h, 4));
        let cs = bar - m;
        if !crest.is_empty() && has_crest(&crest) {
            draw_crest(&crest, tx, (bar - cs) / 2, cs);
            tx += cs + m * 2;
        }
        let date = to_latin1(field(h, 3));
        let room = b.0 - m * 3 - text_width(fonts.body, &date) - tx;
        let title = to_latin1(&field(h, 1).to_uppercase());
        text_at(tx, 2, bar * 62 / 100, stadium::fit(&[f6, f5, f3, f2], &title, room), WHITE, &title, 0);
        let sub = to_latin1(field(h, 2));
        text_at(tx, bar * 56 / 100, bar - 2, stadium::fit(&[f2, F1], &sub, room), DIM, &sub, 0);
        text_at(b.0 - m * 2, 0, bar, fonts.body, LIGHT, &date, 2);
    }
    let (top_c, low_c) = if BACK_HOVER { ([0x3a, 0x62, 0xa0], [0x26, 0x44, 0x74]) } else { ([0x2c, 0x50, 0x8a], [0x1c, 0x36, 0x62]) };
    stadium::gradient(b, top_c, low_c, [0x3e, 0x66, 0xa8], 6);
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, WHITE, if HISTORY.is_empty() { b"Back" } else { b"< Back" }, 1);
    BACK = Some(b);

    let (x0, x1, y0, y1) = (left + m * 2, sw - m * 2, bar + m * 2, sh - m * 2);
    let no_cards = !model.iter().any(|c| field(c, 0) == "C");
    if MODEL.is_none() || no_cards {
        let say = model.iter().find(|l| field(l, 0) == "N").map(|n| field(n, 1).to_string()).unwrap_or_else(|| {
            if remote::CLIENT { "Fetching the dashboard from the host...".into() } else { "Nothing to show here yet.".into() }
        });
        text_at((x0 + x1) / 2, (y0 + y1) / 2 - 20, (y0 + y1) / 2 + 20, fonts.head, DIM, &to_latin1(&say), 1);
        return;
    }
    let g = m * 3 / 2;

    // the tiles, as tall as their three lines
    let tiles: Vec<&Vec<String>> = model.iter().filter(|l| field(l, 0) == "T").collect();
    let pad = m * 3 / 2;
    let th = pad + (tall(fonts.small) + 4) * 2 + tall(fonts.big) + 4 + pad / 2;
    let n = tiles.len().max(1) as i32;
    let tw = (x1 - x0 - g * (n - 1)) / n;
    for (k, t) in tiles.iter().enumerate() {
        let x = x0 + k as i32 * (tw + g);
        tile((x, y0, x + tw, y0 + th), t, &fonts, m);
    }

    // the cards: two in the middle row (the chart wide), three in the bottom row
    let mut cards: Vec<(&Vec<String>, Vec<&Vec<String>>)> = Vec::new();
    for l in &model {
        match field(l, 0) {
            "C" => cards.push((l, Vec::new())),
            "H" | "T" => {}
            _ => {
                if let Some(c) = cards.last_mut() {
                    c.1.push(l);
                }
            }
        }
    }
    let top = y0 + th + g;
    let hb = (y1 - top - g) / 2;
    let wide = x0 + (x1 - x0 - g) * 62 / 100;
    let w3 = (x1 - x0 - 2 * g) / 3;
    let low = top + hb + g;
    let rects = [(x0, top, wide, top + hb), (wide + g, top, x1, top + hb), (x0, low, x0 + w3, y1), (x0 + w3 + g, low, x0 + 2 * w3 + g, y1), (x0 + 2 * (w3 + g), low, x1, y1)];
    for (k, ((head, lines), r)) in cards.iter().zip(rects).enumerate() {
        card(k, r, head, lines, &fonts, m);
    }

    // the tooltip over everything, kept on the page
    if let Some((x, y, lines)) = TIP.take() {
        let lh = tall(fonts.small) + 6;
        let w = lines.iter().map(|l| text_width(fonts.small, &l.0)).max().unwrap_or(0) + m * 3;
        let h = lh * lines.len() as i32 + m;
        let tx = (x + 14).min(x1 - w).max(x0);
        let ty = y.max(top).min(y1 - h);
        stadium::gradient((tx, ty, tx + w, ty + h), [0x22, 0x2a, 0x36], [0x1a, 0x20, 0x2a], EDGE_LIT, 6);
        for (k, (t, c)) in lines.iter().enumerate() {
            let ly = ty + m / 2 + k as i32 * lh;
            text_at(tx + m * 3 / 2, ly, ly + lh, fonts.small, *c, t, 0);
        }
    }
}

unsafe fn frame(r: Rect, lit: bool) {
    stadium::gradient(r, CARD, CARD_LOW, if lit { EDGE_LIT } else { EDGE }, 8);
}

#[allow(static_mut_refs)]
unsafe fn tile(r: Rect, t: &[String], fonts: &Fonts, m: i32) {
    let link = link_of(t).filter(|l| live(l));
    let lit = link.is_some() && hovered(r);
    frame(r, lit);
    if let Some(l) = link {
        HITS.push((r, Hit::Link(l.to_string())));
    }
    let pad = m * 3 / 2;
    let (lh, vh) = (tall(fonts.small) + 4, tall(fonts.big) + 4);
    let label = to_latin1(&field(t, 1).to_uppercase());
    let ly = r.1 + pad * 2 / 3;
    text_at(r.0 + pad, ly, ly + lh, fonts.small, DIM, &label, 0);
    if link.is_some() {
        // (a tile that opens a page: an arrow at its top right)
        text_at(r.2 - pad, ly, ly + lh, fonts.small, if lit { WHITE } else { DIM }, b">", 2);
    }
    let (vy1, vy2) = (ly + lh, ly + lh + vh);
    let value = field(t, 2);
    if let Some(letters) = value.strip_prefix('~') {
        let rad = ((vy2 - vy1) * 38 / 100).min((r.2 - r.0 - 2 * pad) / 15).max(6);
        for (k, c) in letters.bytes().enumerate() {
            let cx = r.0 + pad + rad + k as i32 * (rad * 2 + rad / 2);
            dot(cx, (vy1 + vy2) / 2, rad, form::colour(c));
            text_at(cx, vy1, vy2, fonts.small, WHITE, &[c], 1);
        }
    } else {
        let v = to_latin1(value);
        text_at(r.0 + pad, vy1, vy2, stadium::fit(&[fonts.big, fonts.head, fonts.body], &v, r.2 - r.0 - 2 * pad), WHITE, &v, 0);
    }
    let note = to_latin1(field(t, 3));
    let nf = stadium::fit(&[fonts.small, F1], &note, r.2 - r.0 - 2 * pad);
    text_at(r.0 + pad, vy2, vy2 + lh, nf, tone(field(t, 4)), &note, 0);
}

/// A card: its title, its pills (or its note) at the right of the title, then its content as
/// its kind draws it, filtered by the pill picked.
#[allow(static_mut_refs)]
unsafe fn card(k: usize, r: Rect, head: &[String], lines: &[&Vec<String>], fonts: &Fonts, m: i32) {
    frame(r, false);
    let pad = m * 3 / 2;
    let hh = tall(fonts.head) + 6;
    let (ty1, ty2) = (r.1 + pad / 2, r.1 + pad / 2 + hh);
    let title = to_latin1(field(head, 2));
    text_at(r.0 + pad, ty1, ty2, fonts.head, WHITE, &title, 0);
    let pills: Vec<(&str, &str)> = lines.iter().find(|l| field(l, 0) == "P").map(|l| l[1..].iter().filter_map(|p| p.split_once('=')).collect()).unwrap_or_default();
    let mut filter = "";
    if pills.is_empty() {
        let note = to_latin1(field(head, 3));
        if text_width(fonts.small, &note) < r.2 - r.0 - 3 * pad - text_width(fonts.head, &title) {
            text_at(r.2 - pad, ty1, ty2, fonts.small, DIM, &note, 2);
        }
    } else {
        let chosen = PILLS[k].unwrap_or_else(|| pills.iter().position(|p| p.0.starts_with('*')).unwrap_or(0)).min(pills.len() - 1);
        filter = pills[chosen].1;
        let ph = tall(fonts.small) + 8;
        let py = (ty1 + ty2 - ph) / 2;
        let mut px = r.2 - pad;
        for (i, (label, _)) in pills.iter().enumerate().rev() {
            let label = to_latin1(label.trim_start_matches('*'));
            let w = text_width(fonts.small, &label) + m * 2;
            let pr = (px - w, py, px, py + ph);
            let on = i == chosen;
            let lit = hovered(pr);
            rounded(pr.0, pr.1, pr.2, pr.3, if on { [0x2c, 0x50, 0x8a] } else if lit { LIT } else { [0x1a, 0x20, 0x2a] }, ph / 2);
            text_at((pr.0 + pr.2) / 2, pr.1, pr.3, fonts.small, if on || lit { WHITE } else { DIM }, &label, 1);
            HITS.push((pr, Hit::Pill(k, i)));
            px = pr.0 - m / 2;
        }
    }
    let body = (r.0 + pad, ty2 + m / 2, r.2 - pad, r.3 - pad / 2);
    if let Some(n) = lines.iter().find(|l| field(l, 0) == "N") {
        text_at((body.0 + body.2) / 2, body.1, body.3, fonts.body, DIM, &to_latin1(field(n, 1)), 1);
        return;
    }
    match field(head, 1) {
        "line" => chart(k, body, lines, fonts, false, filter, m),
        "cols" => chart(k, body, lines, fonts, true, filter, m),
        "bars" => bars(body, lines, fonts, m),
        "table" => table(body, lines, fonts, m, filter),
        "groups" => groups(body, lines, fonts, m, filter),
        "list" => list(body, lines, fonts, m),
        _ => {}
    }
}

/// An antialiased line `w` pixels wide.
unsafe fn seg(x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: [u8; 3]) {
    let (bx, by) = ((x0.min(x1) - w - 1.0).floor() as i32, (y0.min(y1) - w - 1.0).floor() as i32);
    let (bw, bh) = ((x0.max(x1) + w + 2.0) as i32 - bx, (y0.max(y1) + w + 2.0) as i32 - by);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    let colour = rgb565(c);
    blend(bx, by, bw, bh, |col, row| {
        let (px, py) = (bx as f32 + col as f32 + 0.5, by as f32 + row as f32 + 0.5);
        let t = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
        let (qx, qy) = (x0 + t * dx - px, y0 + t * dy - py);
        (colour, ((w / 2.0 + 0.5 - (qx * qx + qy * qy).sqrt()).clamp(0.0, 1.0) * 255.0) as u32)
    });
}

/// A round step for an axis of `ticks` steps up to at least `max`.
fn nice_step(max: f64, ticks: f64) -> f64 {
    let raw = (max / ticks).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * mag).find(|&s| s >= raw).unwrap_or(10.0 * mag)
}

/// (name, colour, dashed, values shown, values)
type Series = (Vec<u8>, [u8; 3], bool, bool, Vec<Option<f64>>);

/// The x places and series a pill leaves: `x:N` the last N places (0 all), `first:N` / `last:N`
/// that many series (a series without a colour of its own takes one by its place among them).
fn pick(xs: Vec<Vec<u8>>, tips: Vec<String>, mut series: Vec<Series>, filter: &str) -> (Vec<Vec<u8>>, Vec<String>, Vec<Series>) {
    let (kind, n) = filter.split_once(':').map_or(("", 0), |(k, n)| (k, n.parse::<usize>().unwrap_or(0)));
    let (mut xs, mut tips) = (xs, tips);
    if kind == "x" && n > 0 && xs.len() > n {
        let from = xs.len() - n;
        xs.drain(..from);
        tips.drain(..from.min(tips.len()));
        for s in series.iter_mut() {
            s.4.drain(..from.min(s.4.len()));
        }
    }
    match kind {
        "first" => series.truncate(n),
        "last" => {
            let from = series.len().saturating_sub(n);
            series.drain(..from);
        }
        _ => {}
    }
    (xs, tips, series)
}

/// A line chart, or grouped columns (`cols`), with its axis and a legend under it; the x place
/// under the mouse gets a guide line and a tooltip.
#[allow(static_mut_refs, clippy::too_many_arguments)]
unsafe fn chart(card: usize, body: Rect, lines: &[&Vec<String>], fonts: &Fonts, cols: bool, filter: &str, m: i32) {
    let xs: Vec<Vec<u8>> = lines.iter().find(|l| field(l, 0) == "X").map(|l| field(l, 1).split(',').map(to_latin1).collect()).unwrap_or_default();
    let tips: Vec<String> = lines.iter().find(|l| field(l, 0) == "TT").map(|l| l[1..].to_vec()).unwrap_or_default();
    let series: Vec<Series> = lines
        .iter()
        .filter(|l| field(l, 0) == "S")
        .enumerate()
        .map(|(k, l)| (to_latin1(field(l, 1)), SERIES[field(l, 2).parse::<usize>().unwrap_or(k) % 6], field(l, 3) == "1", field(l, 4) == "1", field(l, 5).split(',').map(|v| v.parse().ok()).collect()))
        .collect();
    let (xs, tips, mut series) = pick(xs, tips, series, filter);
    // (the title race's clubs: colours by their place among those shown)
    let own_colours = lines.iter().filter(|l| field(l, 0) == "S").all(|l| field(l, 2) != "-");
    if !own_colours {
        for (k, s) in series.iter_mut().enumerate() {
            s.1 = SERIES[k % 5];
        }
    }
    let n = xs.len();
    if n == 0 || series.is_empty() {
        return;
    }
    let f = fonts.small;
    let lh = tall(f) + 4;
    let max = series.iter().flat_map(|s| s.4.iter().flatten()).fold(0.0f64, |a, &b| a.max(b)).max(1.0);
    let step = nice_step(max, 4.0);
    let ticks = (max / step).ceil().max(1.0) as i32;
    let ymax = step * ticks as f64;
    let label = |v: f64| to_latin1(&if step < 1.0 { format!("{v:.1}") } else { format!("{v:.0}") });
    let yw = (0..=ticks).map(|k| text_width(f, &label(step * k as f64))).max().unwrap_or(10) + 10;
    let shows_values = series.iter().any(|s| s.3);
    let (px0, px1) = (body.0 + yw, body.2 - 4);
    let (py0, py1) = (body.1 + lh / 2 + if shows_values { lh } else { 0 }, body.3 - 2 * lh - 6);
    if py1 - py0 < 20 || px1 - px0 < 40 {
        return;
    }
    let y_of = |v: f64| py1 as f32 - ((v / ymax) as f32) * (py1 - py0) as f32;
    for k in 0..=ticks {
        let y = y_of(step * k as f64) as i32;
        fill(px0, y, px1, y, if k == 0 { EDGE } else { GRID });
        text_at(px0 - 8, y - lh / 2, y + lh / 2, f, DIM, &label(step * k as f64), 2);
    }
    // the x places: points spread over the width (a little in from the ends); columns in groups
    // of at most a sixth of the width, centred
    let group = (px1 - px0) as f32 / n.max(6) as f32;
    let x_of = |i: usize| -> f32 {
        if cols {
            (px0 + px1) as f32 / 2.0 + (i as f32 - (n - 1) as f32 / 2.0) * group
        } else if n == 1 {
            (px0 + px1) as f32 / 2.0
        } else {
            let inset = 16.0;
            px0 as f32 + inset + ((px1 - px0) as f32 - 2.0 * inset) * i as f32 / (n - 1) as f32
        }
    };
    // the place under the mouse: the band half way to its neighbours
    let half = if cols { group / 2.0 } else if n > 1 { ((px1 - px0) as f32 - 32.0) / (n - 1) as f32 / 2.0 } else { 40.0 };
    let mut lit = None;
    for i in 0..n {
        let band = ((x_of(i) - half) as i32, py0, (x_of(i) + half) as i32, py1);
        HITS.push((band, Hit::Tip(card, i)));
        if hovered(band) {
            lit = Some(i);
        }
    }
    if let Some(i) = lit {
        if cols {
            rounded((x_of(i) - half) as i32 + 2, py0, (x_of(i) + half) as i32 - 2, py1, LIT, 4);
        } else {
            fill(x_of(i) as i32, py0, x_of(i) as i32, py1, [0x3a, 0x44, 0x52]);
        }
    }
    let widest = xs.iter().map(|x| text_width(f, x)).max().unwrap_or(10) + 10;
    let room = if cols { group } else { (px1 - px0) as f32 / n as f32 };
    let every = (widest as f32 / room).ceil().max(1.0) as usize;
    for (i, x) in xs.iter().enumerate() {
        if (n - 1 - i) % every == 0 || lit == Some(i) {
            text_at(x_of(i) as i32, py1 + 3, py1 + 3 + lh, f, if lit == Some(i) { WHITE } else { DIM }, x, 1);
        }
    }
    if cols {
        let bw = (group * 0.7 / series.len() as f32).max(2.0);
        for (k, s) in series.iter().enumerate() {
            for (i, v) in s.4.iter().enumerate().take(n) {
                let Some(v) = *v else { continue };
                let xa = x_of(i) - bw * series.len() as f32 / 2.0 + bw * k as f32;
                let yt = y_of(v);
                if v > 0.0 {
                    rounded(xa as i32 + 1, yt as i32, (xa + bw) as i32 - 1, py1, s.1, (bw as i32 / 4).min(4));
                }
                let shown = to_latin1(&short_num(v));
                if s.3 && text_width(f, &shown) <= bw as i32 + 6 {
                    text_at((xa + bw / 2.0) as i32, yt as i32 - lh, yt as i32, f, LIGHT, &shown, 1);
                }
            }
        }
    } else {
        for s in &series {
            let pts: Vec<Option<(f32, f32)>> = s.4.iter().enumerate().take(n).map(|(i, v)| v.map(|v| (x_of(i), y_of(v)))).collect();
            for w in pts.windows(2) {
                let (Some(a), Some(b)) = (w[0], w[1]) else { continue };
                if s.2 {
                    // dashed: 7 on, 5 off
                    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
                    let mut t = 0.0;
                    while t < len {
                        let e = (t + 7.0).min(len);
                        seg(a.0 + (b.0 - a.0) * t / len, a.1 + (b.1 - a.1) * t / len, a.0 + (b.0 - a.0) * e / len, a.1 + (b.1 - a.1) * e / len, 2.0, s.1);
                        t += 12.0;
                    }
                } else {
                    seg(a.0, a.1, b.0, b.1, 2.5, s.1);
                }
            }
            for (i, p) in pts.iter().enumerate() {
                let Some((x, y)) = *p else { continue };
                let big = lit == Some(i);
                if !s.2 && (n <= 24 || big) {
                    dot(x as i32, y as i32, if big { 6 } else { 4 }, s.1);
                    dot(x as i32, y as i32, if big { 3 } else { 2 }, CARD);
                }
                if s.3 && n <= 20 {
                    text_at(x as i32, y as i32 - lh - 5, y as i32 - 5, f, LIGHT, &to_latin1(&short_num(s.4[i].unwrap_or(0.0))), 1);
                }
            }
        }
    }
    // the tooltip of the place under the mouse: its label (and line), then each series' value
    if let Some(i) = lit {
        let mut out = vec![(xs[i].clone(), WHITE)];
        if let Some(t) = tips.get(i).filter(|t| !t.is_empty()) {
            out.push((to_latin1(t), LIGHT));
        }
        let mut vals: Vec<(f64, &Series)> = series.iter().filter_map(|s| s.4.get(i).copied().flatten().map(|v| (v, s))).collect();
        if !own_colours {
            vals.sort_by(|a, b| b.0.total_cmp(&a.0));
        }
        for (v, s) in vals {
            let mut t = s.0.clone();
            t.extend_from_slice(b":  ");
            t.extend(to_latin1(&short_num(v)));
            out.push((t, s.1));
        }
        TIP = Some((x_of(i) as i32 + m, py0, out));
    }
    // the legend, centred under the x labels
    let total: i32 = series.iter().map(|s| text_width(f, &s.0) + 34).sum();
    let mut lx = (body.0 + body.2 - total) / 2;
    let (ly1, ly2) = (body.3 - lh, body.3);
    let cy = (ly1 + ly2) / 2;
    for s in &series {
        if s.2 {
            seg(lx as f32, cy as f32, lx as f32 + 6.0, cy as f32, 2.0, s.1);
            seg(lx as f32 + 10.0, cy as f32, lx as f32 + 16.0, cy as f32, 2.0, s.1);
        } else {
            dot(lx + 7, cy, 5, s.1);
        }
        text_at(lx + 22, ly1, ly2, f, LIGHT, &s.0, 0);
        lx += text_width(f, &s.0) + 34;
    }
}

/// Bars: label, value, a bar of its share, and how it stands against the average; the rows
/// spread over the card.
unsafe fn bars(body: Rect, lines: &[&Vec<String>], fonts: &Fonts, m: i32) {
    let rows: Vec<&&Vec<String>> = lines.iter().filter(|l| field(l, 0) == "B").collect();
    if rows.is_empty() {
        return;
    }
    let rh = (body.3 - body.1) / rows.len() as i32;
    let w = body.2 - body.0;
    let deltas = rows.iter().any(|r| !field(r, 4).is_empty());
    let (vx, bx0) = (body.0 + w * 46 / 100, body.0 + w * 49 / 100);
    let bx1 = if deltas { body.0 + w * 76 / 100 } else { body.2 };
    for (k, r) in rows.iter().enumerate() {
        let (ya, yb) = (body.1 + k as i32 * rh, body.1 + (k as i32 + 1) * rh);
        if hovered((body.0, ya, body.2, yb)) {
            rounded(body.0 - m / 2, ya + 1, body.2 + m / 2, yb - 1, LIT, 4);
        }
        if k > 0 {
            fill(body.0, ya, body.2, ya, GRID);
        }
        let label = to_latin1(field(r, 1));
        text_at(body.0, ya, yb, stadium::fit(&[fonts.body, F1], &label, vx - body.0 - text_width(fonts.head, b"100%") - m), LIGHT, &label, 0);
        text_at(vx, ya, yb, fonts.head, WHITE, &to_latin1(field(r, 2)), 2);
        let bh = (rh / 5).clamp(4, 10);
        let cy = (ya + yb) / 2;
        rounded(bx0, cy - bh / 2, bx1, cy + bh / 2, [0x26, 0x2e, 0x3a], bh / 2);
        let frac: f32 = field(r, 3).parse().unwrap_or(0.0);
        let fx = bx0 + ((bx1 - bx0) as f32 * frac) as i32;
        let colour = if field(r, 5) == "n" { [0x7a, 0x84, 0x92] } else { tone(field(r, 5)) };
        if fx - bx0 > bh {
            rounded(bx0, cy - bh / 2, fx, cy + bh / 2, colour, bh / 2);
        }
        if deltas {
            let d = to_latin1(field(r, 4));
            text_at(body.2, ya, yb, stadium::fit(&[fonts.small, F1], &d, body.2 - bx1 - 8), if field(r, 5) == "n" { DIM } else { colour }, &d, 2);
        }
    }
}

/// A table: its head in small capitals, rows with lines between, the last column at the right;
/// `g:X` keeps the rows of group `!X` (`g:` all). As many rows as fit; a row with a link is lit
/// under the mouse and opens it.
#[allow(static_mut_refs)]
unsafe fn table(body: Rect, lines: &[&Vec<String>], fonts: &Fonts, m: i32, filter: &str) {
    let Some(head) = lines.iter().find(|l| field(l, 0) == "RH") else { return };
    let group = filter.strip_prefix("g:").unwrap_or("");
    let rows: Vec<&&Vec<String>> = lines.iter().filter(|l| field(l, 0) == "R" && (group.is_empty() || l.iter().any(|f| f.strip_prefix('!') == Some(group)))).collect();
    let cols = head.len() - 1;
    let hh = tall(fonts.small) + 8;
    let rh = tall(fonts.body) * 2 + 4;
    let fit = ((body.3 - body.1 - hh) / rh).max(0) as usize;
    let rows = &rows[..rows.len().min(fit)];
    if rows.is_empty() {
        text_at((body.0 + body.2) / 2, body.1, body.3, fonts.body, DIM, b"Nothing here yet.", 1);
        return;
    }
    let shown = |c: &str| -> Vec<u8> { to_latin1(c.split_once('|').map_or(c, |p| p.1)) };
    // the columns' widths: their widest cell, the room left to the widest column
    let crest = rh * 66 / 100;
    let mut widths: Vec<i32> = (1..=cols)
        .map(|k| {
            let cells = rows.iter().map(|r| {
                let c = field(r, k);
                match c.strip_prefix('~') {
                    Some(l) => l.len() as i32 * (rh * 62 / 100),
                    None => text_width(fonts.body, &shown(c)) + if c.starts_with('^') { crest + 8 } else { 0 },
                }
            });
            cells.chain([text_width(fonts.small, &to_latin1(&field(head, k).to_uppercase()))]).max().unwrap_or(0) + m * 2
        })
        .collect();
    let spare = body.2 - body.0 - widths.iter().sum::<i32>();
    let wide = (0..cols).max_by_key(|&k| widths[k]).unwrap_or(0);
    widths[wide] += spare;
    let x_at = |k: usize| body.0 + widths[..k].iter().sum::<i32>();
    for k in 0..cols {
        let t = to_latin1(&field(head, k + 1).to_uppercase());
        if k == cols - 1 {
            text_at(body.2, body.1, body.1 + hh, fonts.small, DIM, &t, 2);
        } else {
            text_at(x_at(k), body.1, body.1 + hh, fonts.small, DIM, &t, 0);
        }
    }
    fill(body.0, body.1 + hh, body.2, body.1 + hh, EDGE);
    for (i, r) in rows.iter().enumerate() {
        let (ya, yb) = (body.1 + hh + i as i32 * rh, body.1 + hh + (i as i32 + 1) * rh);
        let row = (body.0 - m / 2, ya + 1, body.2 + m / 2, yb - 1);
        let link = link_of(r).filter(|l| live(l));
        let lit = link.is_some() && hovered(row);
        if lit {
            rounded(row.0, row.1, row.2, row.3, LIT, 4);
        }
        if let Some(l) = link {
            HITS.push((row, Hit::Link(l.to_string())));
        }
        if i > 0 {
            fill(body.0, ya, body.2, ya, GRID);
        }
        for k in 0..cols {
            let c = field(r, k + 1);
            let x = x_at(k);
            let last = k == cols - 1;
            if let Some(letters) = c.strip_prefix('~') {
                let rad = (rh * 26 / 100).max(5);
                for (j, l) in letters.bytes().enumerate() {
                    let cx = x + rad + j as i32 * (rad * 2 + rad / 3);
                    dot(cx, (ya + yb) / 2, rad, form::colour(l));
                    text_at(cx, ya, yb, F1, WHITE, &[l], 1);
                }
            } else if let Some(rest) = c.strip_prefix('^') {
                let (key, text) = rest.split_once('|').unwrap_or((rest, rest));
                let key = to_latin1(key);
                if has_crest(&key) {
                    draw_crest(&key, x, (ya + yb - crest) / 2, crest);
                }
                let tx = x + crest + 8;
                let t = to_latin1(text);
                text_at(tx, ya, yb, stadium::fit(&[fonts.body, F1], &t, x + widths[k] - tx - m), if lit { WHITE } else { LIGHT }, &t, 0);
            } else {
                let b = c.as_bytes();
                let result = b.len() > 2 && matches!(b[0], b'W' | b'D' | b'L') && b[1] == b' ';
                let t = to_latin1(if result { &c[2..] } else { c });
                if result && last {
                    // the result as a pill in its colour, all as wide
                    let w = text_width(fonts.body, b"10-10") + m;
                    let ph = (rh * 60 / 100).max(tall(fonts.body) + 2);
                    rounded(body.2 - w, (ya + yb - ph) / 2, body.2, (ya + yb + ph) / 2, form::colour(b[0]), ph / 2);
                    text_at(body.2 - w / 2, ya, yb, fonts.body, WHITE, &t, 1);
                } else if last {
                    text_at(body.2, ya, yb, fonts.head, WHITE, &t, 2);
                } else {
                    text_at(x, ya, yb, stadium::fit(&[fonts.body, F1], &t, widths[k] - m), LIGHT, &t, 0);
                }
            }
        }
    }
}

/// Group tables (`G` a group's name and page tag, `GR` its rows: place, team, P W D L F A Pts,
/// link): the page the pill picked (`g:` all); one group as a whole table, more as a grid of
/// small ones (P, GD, Pts).
#[allow(static_mut_refs)]
unsafe fn groups(body: Rect, lines: &[&Vec<String>], fonts: &Fonts, m: i32, filter: &str) {
    let page = filter.strip_prefix("g:").unwrap_or("");
    let mut blocks: Vec<(&Vec<String>, Vec<&Vec<String>>)> = Vec::new();
    for l in lines {
        match field(l, 0) {
            "G" => blocks.push((l, Vec::new())),
            "GR" => {
                if let Some(b) = blocks.last_mut() {
                    b.1.push(l);
                }
            }
            _ => {}
        }
    }
    blocks.retain(|b| page.is_empty() || b.0.iter().any(|f| f.strip_prefix('!') == Some(page)));
    let n = blocks.len() as i32;
    if n == 0 {
        return;
    }
    let cols = match n {
        1 => 1,
        2 => 2,
        3..=6 => 3,
        _ => 4,
    };
    let rows_n = (n + cols - 1) / cols;
    let g = m * 2;
    let bw = (body.2 - body.0 - g * (cols - 1)) / cols;
    let bh = (body.3 - body.1 - m * (rows_n - 1)) / rows_n;
    let total = blocks.iter().map(|b| b.1.len()).max().unwrap_or(0);
    let big = n == 1 && lines.iter().filter(|l| field(l, 0) == "G").count() >= 1 && blocks[0].1.iter().any(|r| field(r, 1).parse::<usize>().unwrap_or(0) > 8);
    for (k, (head, rows)) in blocks.iter().enumerate() {
        let (c, r) = (k as i32 % cols, k as i32 / cols);
        let (x, y) = (body.0 + c * (bw + g), body.1 + r * (bh + m));
        group_box((x, y, x + bw, y + bh), field(head, 1), rows, fonts, m, n == 1, big || total > 8);
    }
}

/// One group's table; `full` with W D L, `league` the places of a league phase (1-8 through,
/// 9-24 the play-off) instead of a group's two.
#[allow(static_mut_refs)]
unsafe fn group_box(r: Rect, title: &str, rows: &[&Vec<String>], fonts: &Fonts, m: i32, full: bool, league: bool) {
    let nums: &[(&[u8], usize)] = if full { &[(b"P", 3), (b"W", 4), (b"D", 5), (b"L", 6), (b"GD", 0), (b"PTS", 9)] } else { &[(b"P", 3), (b"GD", 0), (b"PTS", 9)] };
    let hh = tall(fonts.small) + 6;
    let rh = ((r.3 - r.1 - hh) / rows.len().max(1) as i32).min(tall(fonts.body) * 2).max(tall(F1) + 2);
    let font = if rh >= tall(fonts.body) + 4 { fonts.body } else { F1 };
    let cw = text_width(fonts.small, b"PTS") + m * 3 / 2;
    let right = |j: usize| r.2 - (nums.len() - 1 - j) as i32 * cw;
    if !full {
        text_at(r.0, r.1, r.1 + hh, fonts.small, GOLD, &to_latin1(&title.to_uppercase()), 0);
    }
    for (j, (h, _)) in nums.iter().enumerate() {
        text_at(right(j), r.1, r.1 + hh, fonts.small, DIM, h, 2);
    }
    fill(r.0, r.1 + hh, r.2, r.1 + hh, EDGE);
    let crest = rh * 66 / 100;
    let pw = text_width(font, b"36") + 6;
    for (i, row) in rows.iter().enumerate() {
        let (ya, yb) = (r.1 + hh + i as i32 * rh, r.1 + hh + (i as i32 + 1) * rh);
        if yb > r.3 + 2 {
            break;
        }
        let rect = (r.0 - m / 2, ya + 1, r.2 + m / 2, yb - 1);
        let link = link_of(row).filter(|l| live(l));
        let lit = link.is_some() && hovered(rect);
        if lit {
            rounded(rect.0, rect.1, rect.2, rect.3, LIT, 4);
        }
        if let Some(l) = link {
            HITS.push((rect, Hit::Link(l.to_string())));
        }
        if i > 0 {
            fill(r.0, ya, r.2, ya, GRID);
        }
        let pos: usize = field(row, 1).parse().unwrap_or(0);
        let zone = if league { [(1..=8, GREEN), (9..=24, BLUE)].into_iter().find(|z| z.0.contains(&pos)).map(|z| z.1) } else { (pos <= 2).then_some(GREEN) };
        if let Some(c) = zone {
            fill(r.0, ya + 3, r.0 + 2, yb - 3, c);
        }
        text_at(r.0 + pw, ya, yb, font, DIM, field(row, 1).as_bytes(), 2);
        let cell = field(row, 2).trim_start_matches('^');
        let (key, team) = cell.split_once('|').unwrap_or((cell, cell));
        let key = to_latin1(key);
        let mut tx = r.0 + pw + m;
        if has_crest(&key) {
            draw_crest(&key, tx, (ya + yb - crest) / 2, crest);
        }
        tx += crest + 6;
        let t = to_latin1(team);
        text_at(tx, ya, yb, stadium::fit(&[font, F1], &t, right(0) - cw + m - tx), if lit { WHITE } else { LIGHT }, &t, 0);
        let num = |k: usize| field(row, k).parse::<i32>().unwrap_or(0);
        for (j, (_, f)) in nums.iter().enumerate() {
            let v = if *f == 0 {
                let gd = num(7) - num(8);
                if gd > 0 { format!("+{gd}") } else { gd.to_string() }
            } else {
                num(*f).to_string()
            };
            text_at(right(j), ya, yb, font, if *f == 9 { WHITE } else { LIGHT }, v.as_bytes(), 2);
        }
    }
}

/// A list of names: headings (`#`), each row name, what they are, a value at the right; a row
/// with a link is lit under the mouse and opens it.
#[allow(static_mut_refs)]
unsafe fn list(body: Rect, lines: &[&Vec<String>], fonts: &Fonts, m: i32) {
    let rows: Vec<&&Vec<String>> = lines.iter().filter(|l| field(l, 0) == "L").collect();
    if rows.is_empty() {
        return;
    }
    let rh = ((body.3 - body.1) / rows.len() as i32).min(tall(fonts.body) * 2 + 4);
    let w = body.2 - body.0;
    for (k, r) in rows.iter().enumerate() {
        let (ya, yb) = (body.1 + k as i32 * rh, body.1 + (k as i32 + 1) * rh);
        let name = field(r, 1);
        if let Some(h) = name.strip_prefix('#') {
            let h = to_latin1(&h.to_uppercase());
            text_at(body.0, ya, yb, fonts.small, GOLD, &h, 0);
            fill(body.0 + text_width(fonts.small, &h) + m, (ya + yb) / 2, body.2, (ya + yb) / 2, GRID);
            continue;
        }
        let row = (body.0 - m / 2, ya + 1, body.2 + m / 2, yb - 1);
        let link = link_of(r).filter(|l| live(l));
        let lit = link.is_some() && hovered(row);
        if lit {
            rounded(row.0, row.1, row.2, row.3, LIT, 4);
        }
        if let Some(l) = link {
            HITS.push((row, Hit::Link(l.to_string())));
        }
        let n = to_latin1(name);
        text_at(body.0 + m, ya, yb, stadium::fit(&[fonts.body, F1], &n, w * 55 / 100 - m), if lit { GOLD } else { WHITE }, &n, 0);
        let sub = to_latin1(field(r, 2));
        text_at(body.0 + w * 57 / 100, ya, yb, stadium::fit(&[fonts.small, F1], &sub, w * 30 / 100), DIM, &sub, 0);
        text_at(body.2, ya, yb, fonts.head, GOLD, &to_latin1(field(r, 3)), 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(23), "23rd");
        assert_eq!(money(1_250_000.0, "$"), "$1.2M");
        assert_eq!(money(-35_000.0, "$"), "-$35K");
        assert_eq!(nice_step(9.0, 4.0), 2.5);
        assert_eq!(nice_step(78.0, 4.0), 20.0);
        assert_eq!(slot(6), 0); // July
        assert_eq!(slot(5), 11); // June
        assert_eq!(month_of(59, 2024), 1); // 29 Feb
        assert_eq!(short_date(73, 2025), "15 Mar");
        assert_eq!(nums(&[Some(3.0), None, Some(1.5)]), "3,,1.5");
        assert_eq!(initials("World Cup Qualifying"), "WCQ");
        assert_eq!(initials("International Friendly"), "IF");
        assert_eq!(initials("European Championship 2004"), "EC2004");
        assert_eq!(short_num(3.0), "3");
        assert_eq!(short_num(1.25), "1.25");
        // 1 beat 2, 2 drew 3, 3 v 1 not played: 1 first, 3 second (goal difference 0 beats -2)
        let all = [
            Match { key: 1, month: 7, home: 1, away: 2, score: Some((2, 0)) },
            Match { key: 2, month: 7, home: 2, away: 3, score: Some((1, 1)) },
            Match { key: 3, month: 7, home: 3, away: 1, score: None },
        ];
        let t = tally(&all, 1, 9);
        assert_eq!((t.g, t.p, t.f, t.a, t.cs, t.hg), (1.0, 3.0, 2.0, 0.0, 1.0, 1.0));
        assert_eq!(place(&all, &[1, 2, 3], 1, 9), 1);
        assert_eq!(place(&all, &[1, 2, 3], 3, 9), 2);
        assert_eq!(place(&all, &[1, 2, 3], 2, 9), 3);
        assert_eq!(place(&all, &[1, 2, 3], 2, 1), 3); // after the first match only
    }

    /// A made-up World Cup: a qualifying tie, two groups of four (each its own stage, 3
    /// matchdays), semi-finals, the third place match and the final (stage 255).
    #[test]
    fn cup_shape_names() {
        let mut ms = Vec::new();
        let mut m = |key: i32, stage: u8, round: u8, home: i32, away: i32| ms.push(Cm { key, day: key, year: 2002, ed: 2002, stage, round, home, away, score: Some((1, 0)) });
        m(1, 255, 0, 90, 91);
        for (st, t) in [(0u8, [1, 2, 3, 4]), (1, [5, 6, 7, 8])] {
            for (rd, pairs) in [[(0, 1), (2, 3)], [(0, 2), (1, 3)], [(0, 3), (1, 2)]].iter().enumerate() {
                for &(a, b) in pairs {
                    m(100 + rd as i32 * 4 + st as i32, st, rd as u8, t[a], t[b]);
                }
            }
        }
        m(200, 255, 1, 1, 6);
        m(200, 255, 1, 5, 2);
        m(205, 255, 2, 6, 2);
        m(206, 255, 3, 1, 5);
        let (groups, rounds) = cup_shape(&ms);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "Group stage");
        assert_eq!(groups[0].1.iter().map(|g| (g.0.as_str(), g.1.len())).collect::<Vec<_>>(), [("Group A", 4), ("Group B", 4)]);
        let names: Vec<&str> = rounds.iter().map(|r| r.short.as_str()).collect();
        assert_eq!(names, ["Q1", "MD1", "MD2", "MD3", "SF", "3rd", "F"]);
        assert_eq!(rounds[1].ix.len(), 4); // a matchday has both groups' matches
        let table = group_table(&ms, &[1, 2, 3, 4]);
        assert_eq!(table[0].0, 1); // (1 won all three: 1-0 at home)
        assert_eq!(table[0].1, [3, 3, 0, 0, 3, 0, 9]);
        // a league phase: one stage of 36 teams that do not all meet
        let mut lp = Vec::new();
        for rd in 0..8u8 {
            for k in 0..18 {
                lp.push(Cm { key: 300 + rd as i32 * 10, day: 0, year: 2027, ed: 2027, stage: 1, round: rd, home: k, away: 18 + (k + rd as i32) % 18, score: None });
            }
        }
        // a cup of knockout rounds alone (the old UEFA Cup): 64, 32, 16, 8, 4, 2 teams
        let mut ko = Vec::new();
        for (rd, t) in [64, 32, 16, 8, 4, 2].into_iter().enumerate() {
            for k in 0..t / 2 {
                ko.push(Cm { key: 400 + rd as i32 * 20, day: 0, year: 2001, ed: 2001, stage: 255, round: rd as u8, home: k * 2, away: k * 2 + 1, score: None });
            }
        }
        let (g, r) = cup_shape(&ko);
        assert!(g.is_empty());
        assert_eq!(r.iter().map(|r| r.short.as_str()).collect::<Vec<_>>(), ["R1", "R32", "R16", "QF", "SF", "F"]);
        let (g, r) = cup_shape(&lp);
        assert_eq!((g[0].0.as_str(), g[0].1.len(), g[0].1[0].1.len()), ("League phase", 1, 36));
        assert_eq!(r[0].short, "MD1");
    }

    #[test]
    fn pills_pick() {
        let s = |name: &str, v: &[f64]| (name.as_bytes().to_vec(), GREEN, false, false, v.iter().map(|&x| Some(x)).collect::<Vec<_>>());
        let xs: Vec<Vec<u8>> = ["M1", "M2", "M3", "M4"].iter().map(|x| x.as_bytes().to_vec()).collect();
        let tips: Vec<String> = ["a", "b", "c", "d"].iter().map(|t| t.to_string()).collect();
        let (x, t, ser) = pick(xs.clone(), tips.clone(), vec![s("A", &[1.0, 2.0, 3.0, 4.0])], "x:2");
        assert_eq!((x.len(), t, ser[0].4.clone()), (2, vec!["c".to_string(), "d".to_string()], vec![Some(3.0), Some(4.0)]));
        let (x, _, _) = pick(xs.clone(), tips.clone(), vec![s("A", &[1.0, 2.0, 3.0, 4.0])], "x:0");
        assert_eq!(x.len(), 4);
        let five: Vec<Series> = ["A", "B", "C", "D", "E", "F"].iter().map(|n| s(n, &[1.0; 4])).collect();
        assert_eq!(pick(xs.clone(), tips.clone(), five.clone(), "first:3").2.len(), 3);
        let last = pick(xs, tips, five, "last:2").2;
        assert_eq!((last.len(), last[0].0.clone()), (2, b"E".to_vec()));
        assert_eq!(link_of(&["R".into(), "x".into(), "!L".into(), "@c12".into()]), Some("@c12"));
    }
}
