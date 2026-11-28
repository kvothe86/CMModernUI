//! Teletext in the style of BBC Ceefax (user 2026-11-22): the news screen's Teletext button opens
//! the football pages: P100 the index, P301 the front page, P302 the manager's news headlines,
//! P316.. the latest results, P324.. the tables and P340.. the top scorers of the human managers'
//! leagues (the viewer's own first; any other league by its id as the page's parameter), P400 a
//! club's page and P450 a player's (user 2026-11-28: clubs and players to click through).
//!
//! 40 x 25 cells on black over everything right of the menu bar: row 0 the header (page number,
//! date, clock), rows 1..23 the page, row 24 Fastext (red Index, green News, yellow Results, cyan
//! Tables, Back, a red Close). A page is made on the host (`page_text`; a network client asks key
//! `*`) as Latin-1 rows with teletext's own control codes: 1..7 the colours (red, green, yellow,
//! blue, magenta, cyan, white), 0x0d double height, 0x0c normal height, 0x1d the background takes
//! the colour, 0x1c black background. A control code takes its cell as a space, as on a real set;
//! the row under a double-height one is left out. After the cells a row may have 0x1f and marks,
//! split by ';': `a-b=N` or `a-b=N.P` cells a..b link to page N (parameter P), `z<colour>` a
//! dashed line under the row (a table's zones: Europe, promotion, play-offs, relegation).
//! The first line of a page's text is its number of subpages; they turn every 8 seconds.
use crate::*;

pub static mut OPEN: bool = false;
static mut NUMBER: u16 = 100;
static mut PARAM: usize = 0;
static mut SUB: usize = 0;
static mut SUBS: usize = 1;
static mut ROWS: Vec<Vec<u8>> = Vec::new();
static mut TURNED: Option<std::time::Instant> = None;
static mut OPENED_AT: Option<std::time::Instant> = None;
/// Where things are this frame: (box, page, parameter); page 0 is Back, 1 Close.
static mut LINKS: Vec<((i32, i32, i32, i32), u16, usize)> = Vec::new();
/// The link under the mouse (lit), the pages followed here (Back).
static mut UNDER: Option<usize> = None;
static mut HISTORY: Vec<(u16, usize)> = Vec::new();
/// The subpage was turned by hand (the header's "1/2"): it no longer turns by itself on this page.
static mut HELD: bool = false;

const PALETTE: [[u8; 3]; 8] = [[0, 0, 0], [0xff, 0, 0], [0, 0xff, 0], [0xff, 0xff, 0], [0, 0, 0xff], [0xff, 0, 0xff], [0, 0xff, 0xff], [0xff, 0xff, 0xff]];
/// Black as the screen takes it: the lowest non-zero 16-bit colour.
const INK_BLACK: [u8; 3] = [0, 0, 8];
const RED: u8 = 1;
const GREEN: u8 = 2;
const YELLOW: u8 = 3;
const BLUE: u8 = 4;
const MAGENTA: u8 = 5;
const CYAN: u8 = 6;
const WHITE: u8 = 7;
const DOUBLE: u8 = 0x0d;
const NEW_BG: u8 = 0x1d;
const BLACK_BG: u8 = 0x1c;
const MARKS: u8 = 0x1f;
const BACK: u16 = 0;
const CLOSE: u16 = 1;
const NEXT_SUB: u16 = 2;
const CLUB_PAGE: u16 = 400;
const PLAYER_PAGE: u16 = 450;

pub fn page_open() -> bool {
    unsafe { OPEN }
}

#[allow(static_mut_refs)]
pub unsafe fn open() {
    (OPEN, NUMBER, PARAM, SUB, UNDER, OPENED_AT) = (true, 100, 0, 0, None, Some(std::time::Instant::now()));
    HISTORY.clear();
    HELD = false;
    fetch();
}

#[allow(static_mut_refs)]
unsafe fn go(number: u16, param: usize) {
    match number {
        NEXT_SUB => {
            (SUB, HELD) = ((SUB + 1) % SUBS.max(1), true);
            fetch();
            return;
        }
        CLOSE => {
            OPEN = false;
            return;
        }
        BACK => match HISTORY.pop() {
            Some((n, p)) => (NUMBER, PARAM) = (n, p),
            None => (NUMBER, PARAM) = (100, 0),
        },
        _ => {
            HISTORY.push((NUMBER, PARAM));
            (NUMBER, PARAM) = (number, param);
        }
    }
    (SUB, HELD) = (0, false);
    fetch();
}

/// The page from the host (or made here).
#[allow(static_mut_refs)]
unsafe fn fetch() {
    let text = if remote::CLIENT { ask_host(&key(b'*', &[NUMBER as u32, SUB as u32, PARAM as u32])).unwrap_or_default() } else { page_text(channel(), NUMBER, SUB, PARAM) };
    let mut lines = text.split(|&c| c == b'\n');
    SUBS = lines.next().and_then(|l| std::str::from_utf8(l).ok()?.trim().parse().ok()).unwrap_or(1).max(1);
    ROWS = lines.map(|l| l.to_vec()).collect();
    TURNED = Some(std::time::Instant::now());
    REDRAW = true;
}

// ---- the pages (host) ----

/// A row of cells: control codes and text, cut at 40.
fn row(parts: &[(u8, &str)]) -> Vec<u8> {
    linked(&parts.iter().map(|&(c, t)| (c, t, String::new())).collect::<Vec<_>>())
}

/// A row whose parts may link to a page ("N" or "N.P"; empty: none): the words of the part, its
/// spaces left out, are the link.
fn linked(parts: &[(u8, &str, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut marks = Vec::new();
    for (code, text, to) in parts {
        if *code != 0 {
            out.push(*code);
        }
        let t = to_latin1(text);
        let (lead, trail) = (t.iter().take_while(|&&c| c == b' ').count(), t.iter().rev().take_while(|&&c| c == b' ').count());
        let a = out.len() + lead;
        out.extend(&t);
        if !to.is_empty() && lead + trail < t.len() && a < 40 {
            marks.push(format!("{a}-{}={to}", (out.len() - trail - 1).min(39)));
        }
    }
    out.truncate(40);
    with_marks(out, &marks)
}

/// A row with marks after its 40 cells (none: the row as it is).
fn with_marks(mut r: Vec<u8>, marks: &[String]) -> Vec<u8> {
    if marks.is_empty() {
        return r;
    }
    let old = r.iter().position(|&c| c == MARKS).map(|k| r.split_off(k + 1));
    if old.is_none() {
        r.resize(40, b' ');
        r.push(MARKS);
    }
    let mut all: Vec<String> = old.map(|o| String::from_utf8_lossy(&o).split(';').filter(|m| !m.is_empty()).map(String::from).collect()).unwrap_or_default();
    all.extend(marks.iter().cloned());
    r.extend(all.join(";").as_bytes());
    r
}

/// A row's cells and its marks: links (first cell, last cell, page, parameter), the dashed line's
/// colour.
fn marks_of(r: &[u8]) -> (&[u8], Vec<(usize, usize, u16, usize)>, Option<u8>) {
    let Some(k) = r.iter().position(|&c| c == MARKS) else { return (r, Vec::new(), None) };
    let (mut links, mut zone) = (Vec::new(), None);
    for m in String::from_utf8_lossy(&r[k + 1..]).split(';') {
        if let Some(c) = m.strip_prefix('z') {
            zone = c.parse::<u8>().ok().filter(|c| (1..=7).contains(c));
        } else if let Some((span, to)) = m.split_once('=') {
            let (a, b) = span.split_once('-').unwrap_or((span, span));
            let (n, p) = to.split_once('.').unwrap_or((to, "0"));
            if let (Ok(a), Ok(b), Ok(n), Ok(p)) = (a.parse::<usize>(), b.parse::<usize>(), n.parse::<u16>(), p.parse::<usize>()) {
                links.push((a.min(39), b.min(39).max(a.min(39)), n, p));
            }
        }
    }
    (&r[..k], links, zone)
}

/// A page's title band: a coloured background, the words double height (blue on the light
/// backgrounds, white on the dark ones).
fn title(colour: u8, words: &str) -> Vec<u8> {
    let ink = if colour == YELLOW || colour == CYAN || colour == GREEN { BLUE } else { WHITE };
    let mut r = vec![colour, NEW_BG, ink, DOUBLE];
    r.extend(to_latin1(&format!(" {words}")));
    r.resize(40, b' ');
    r
}

/// The human managers' leagues, the channel's own first: (competition, its name, a club in it).
unsafe fn leagues(channel: usize) -> Vec<(usize, String, usize)> {
    let own = rd::<usize>(rd::<usize>(CHANNEL_MANAGER + 0x300 * channel) + 0x39);
    let mut clubs: Vec<usize> = vec![own];
    clubs.extend(challenge::human_list().iter().map(|h| rd::<usize>(h.0 + 0x39)));
    let mut out: Vec<(usize, String, usize)> = Vec::new();
    for club in clubs {
        if !readable(club, 0x5b) {
            continue;
        }
        let comp = rd::<usize>(club + 0x57);
        if readable(comp, 0x40) && challenge::league_table(club).is_some() && !out.iter().any(|l| l.0 == comp) {
            out.push((comp, comp_name(comp), club));
        }
    }
    out.truncate(8);
    out
}

unsafe fn comp_name(comp: usize) -> String {
    if readable(comp, 0x40) { latin1(&game_text(comp + 4, 51)).into_owned() } else { String::new() }
}

/// A club's short name (club +0x38), else its name.
unsafe fn short(club: usize) -> String {
    let s = latin1(&game_text(club + 0x38, 26)).into_owned();
    if s.trim().is_empty() { latin1(&game_text(club + 4, 51)).into_owned() } else { s }
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A club by its index (the club table), its index by the club; checked (a client's parameter).
unsafe fn club_at(index: usize) -> Option<usize> {
    let c = rd::<usize>(CLUB_TABLE) + index * CLUB_SIZE;
    (index < rd::<i32>(CLUB_COUNT).max(0) as usize && readable(c, 0x5b)).then_some(c)
}
unsafe fn club_index(club: usize) -> usize {
    (club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE
}
const STAFF_SIZE: usize = 0x6e;
unsafe fn staff_at(index: usize) -> Option<usize> {
    let s = rd::<usize>(STAFF_TABLE) + index * STAFF_SIZE;
    (index < rd::<i32>(STAFF_COUNT).max(0) as usize && readable(s, STAFF_SIZE)).then_some(s)
}
/// The links to a club's page, a player's, a league's page (a parameter is the index or id + 1).
unsafe fn to_club(club: usize) -> String {
    format!("{CLUB_PAGE}.{}", club_index(club) + 1)
}
unsafe fn to_player(staff: usize) -> String {
    format!("{PLAYER_PAGE}.{}", (staff - rd::<usize>(STAFF_TABLE)) / STAFF_SIZE + 1)
}
unsafe fn to_league(page: u16, comp: usize) -> String {
    format!("{page}.{}", rd::<i32>(comp) + 1)
}

// ---- the front page (user 2026-11-22, "in ceefax stijl"): nothing in the game changes ----
/// A channel's manager: his staff record, his name, his club.
unsafe fn manager_of(channel: usize) -> (usize, String, usize) {
    let staff = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel);
    let name = if readable(staff, 0x6e) { latin1(&staff_name(staff)).into_owned() } else { "The manager".into() };
    (staff, name, if readable(staff, 0x3d) { rd::<usize>(staff + 0x39) } else { 0 })
}

/// A club's last results (newest first), not friendlies.
unsafe fn last_results(club: usize, most: usize) -> Vec<form::Result_> {
    if !readable(club, 0x5b) {
        return Vec::new();
    }
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 60 { (day - 60, year) } else { (day + 305, year - 1) };
    form::played(club_index(club) as i32, start, (day, year), most + 4).into_iter().filter(|r| !r.friendly).take(most).collect()
}

/// A result as a row: the home club, the score, the away club, both clubs linked.
unsafe fn result_row(home: usize, hg: i32, ag: i32, away: usize) -> Vec<u8> {
    linked(&[(WHITE, &format!("{:>16}", cut(&short(home), 16)), to_club(home)), (YELLOW, &format!("{hg}-{ag}"), String::new()), (WHITE, &cut(&short(away), 17), to_club(away))])
}

/// The front page: the tabloid's headline on the manager's last match, then the other managers'.
unsafe fn front_page(channel: usize, rows: &mut Vec<Vec<u8>>) {
    let (_, _, club) = manager_of(channel);
    rows.push(title(RED, "THE DAILY CEEFAX"));
    rows.push(Vec::new());
    let base = rd::<usize>(CLUB_TABLE);
    let other = |i: i32| base + i.max(0) as usize * CLUB_SIZE;
    let line_of = |c: usize, r: &form::Result_| if r.home { result_row(c, r.goals_for, r.against, other(r.opponent)) } else { result_row(other(r.opponent), r.against, r.goals_for, c) };
    match last_results(club, 1).first() {
        Some(r) => {
            let (us, them) = (short(club).to_uppercase(), short(other(r.opponent)).to_uppercase());
            let margin = r.goals_for - r.against;
            let words = match margin {
                3.. => format!("{us} RUN RIOT"),
                2 => format!("{us} SINK {them}"),
                1 => format!("{us} EDGE {them}"),
                0 => format!("STALEMATE FOR {us}"),
                -1 => format!("MISERY FOR {us}"),
                -2 => format!("{them} STUN {us}"),
                _ => format!("{us} HUMILIATED"),
            };
            let mut big = vec![YELLOW, DOUBLE];
            big.extend(to_latin1(&format!(" {}", cut(&words, 37))));
            rows.push(big);
            rows.push(Vec::new());
            rows.push(line_of(club, r));
        }
        None => rows.push(row(&[(WHITE, " No match played yet this season.")])),
    }
    // (the other managers' results; no heading when there are none: single player, night 12)
    let mut elsewhere = Vec::new();
    for h in challenge::human_list() {
        let other = rd::<usize>(h.0 + 0x39);
        if other == club || !readable(other, 0x5b) {
            continue;
        }
        if let Some(r) = last_results(other, 1).first() {
            elsewhere.push(line_of(other, r));
        }
    }
    if !elsewhere.is_empty() {
        rows.push(Vec::new());
        rows.push(row(&[(CYAN, " ELSEWHERE")]));
        rows.extend(elsewhere);
    }
    if readable(club, 0x5b) {
        rows.push(Vec::new());
        rows.push(linked(&[(GREEN, &format!("{:<32}", format!(" {}", cut(&short(club), 30))), to_club(club)), (WHITE, "400", to_club(club))]));
    }
}

/// The places a seeding record of the UEFA seeding object ([0xde1f58]: 18-byte records from +0,
/// their count at +0x438; +4 the nation, +0xc..+0xe the places in the three European cups, by
/// the nation's coefficient rank, `rust\src\uefa.rs`) gives the club's nation.
unsafe fn europe_places(club: usize) -> Option<[u8; 3]> {
    let seeding = rd::<usize>(0xde1f58);
    if !readable(seeding, 0x440) || !readable(club, 0x57) {
        return None;
    }
    let nation = rd::<usize>(club + 0x53);
    let count = (rd::<u8>(seeding + 0x438) as usize).min(60);
    if !readable(nation, 4) {
        return None;
    }
    (0..count).map(|k| seeding + 18 * k).find(|&r| rd::<usize>(r + 4) == nation || rd::<i32>(r) == rd::<i32>(nation)).map(|r| [rd::<u8>(r + 0xc), rd::<u8>(r + 0xd), rd::<u8>(r + 0xe)])
}

/// A league table's dashed lines: under which place, its colour, what it is. A top flight (+0x1c
/// -1) its nation's European places (the first cup's, the second's but the place its cup winner
/// takes, the third's); every league its promotion, play-off and relegation places from the
/// League object.
unsafe fn zones(comp: usize, table: &[(usize, [i32; 7])]) -> Vec<(usize, u8, String)> {
    let league = challenge::league_object(comp);
    let teams = table.len();
    if league == 0 || !readable(league, 0xc2) || teams < 2 {
        return Vec::new();
    }
    let b = |o: usize| (rd::<i8>(league + o) as i32).clamp(0, teams as i32) as usize;
    let (up, play_off, down_play_off, down) = (b(0xbe), b(0xbf), b(0xc0), b(0xc1));
    let mut out: Vec<(usize, u8, String)> = Vec::new();
    let top = rd::<i32>(league + 0x1c) == -1;
    if top {
        if let Some(p) = europe_places(table[0].0) {
            // ponytail: the cup winners' places are taken as one of the second cup's (the game
            // gives the domestic cup winner the first); a cup won by a club already in Europe moves
            // the line one down in the game, not here
            let counts = [p[0] as usize, (p[1] as usize).saturating_sub(1), p[2] as usize];
            let mut at = 0;
            for (k, (n, colour)) in counts.into_iter().zip([GREEN, CYAN, WHITE]).enumerate() {
                at += n;
                let name = comp_name(challenge::comp_by_id([326, 328, 330][k]));
                if n > 0 && at < teams {
                    out.push((at, colour, if name.is_empty() { ["Champions League", "UEFA Cup", "Intertoto Cup"][k].into() } else { name }));
                }
            }
        }
    }
    // (a top flight's +0xbe is no promotion: the Club World Championship has 1, VVV.sav)
    let up = if top { 0 } else { up };
    if up > 0 {
        out.push((up, GREEN, "Promotion".into()));
    }
    if play_off > 0 {
        out.push((up + play_off, YELLOW, if top { "Play-offs" } else { "Promotion play-offs" }.into()));
    }
    out.sort_by_key(|z| z.0);
    if down_play_off > 0 && down + down_play_off < teams {
        out.push((teams - down - down_play_off, MAGENTA, "Relegation play-offs".into()));
    }
    if down > 0 && down < teams {
        out.push((teams - down, RED, "Relegation".into()));
    }
    out.retain(|z| (1..teams).contains(&z.0));
    out.dedup_by_key(|z| z.0);
    out
}

/// The competition of a league page: by its id (the parameter, id + 1), else the k-th of the
/// human managers' leagues.
unsafe fn league_page(leagues: &[(usize, String, usize)], k: u16, param: usize) -> Option<usize> {
    if param > 0 {
        let c = challenge::comp_by_id(param as i32 - 1);
        return (c != 0 && challenge::league_table_of(c).is_some()).then_some(c);
    }
    leagues.get(k as usize).map(|l| l.0)
}

/// Host: a page for a channel's manager: its subpages on the first line, then the rows.
pub unsafe fn page_text(channel: usize, number: u16, sub: usize, param: usize) -> Vec<u8> {
    let leagues = leagues(channel);
    let mut rows: Vec<Vec<u8>> = Vec::new();
    let mut subs = 1;
    let (_, _, own) = manager_of(channel);
    let league = |base: u16| (base..base + 8).contains(&number).then(|| league_page(&leagues, number - base, param)).flatten();
    match number {
        301 => front_page(channel, &mut rows),
        302 => {
            rows.push(title(RED, "FOOTBALL NEWS"));
            rows.push(Vec::new());
            for (k, h) in news::headlines(channel, 9).iter().enumerate() {
                let words = latin1(h).into_owned();
                let mut lines: Vec<String> = Vec::new();
                let mut line = String::new();
                for w in words.split_whitespace() {
                    if line.len() + w.len() + 1 > 37 {
                        lines.push(std::mem::take(&mut line));
                    }
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(w);
                }
                lines.push(line);
                for (i, l) in lines.iter().take(2).enumerate() {
                    rows.push(row(&[(if k % 2 == 0 { WHITE } else { CYAN }, &format!("{}{l}", if i == 0 { " " } else { "  " }))]));
                }
            }
            if rows.len() <= 2 {
                rows.push(row(&[(WHITE, " No news today.")]));
            }
        }
        316..=323 if league(316).is_some() => {
            let comp = league(316).unwrap();
            rows.push(title(GREEN, &cut(&comp_name(comp).to_uppercase(), 34)));
            rows.push(Vec::new());
            rows.push(linked(&[(YELLOW, " LATEST RESULTS          ", String::new()), (CYAN, "Table", to_league(324, comp)), (CYAN, "Scorers", to_league(340, comp))]));
            let results = latest_results(comp);
            if results.is_empty() {
                rows.push(row(&[(WHITE, " No matches played yet.")]));
            }
            for &(home, hg, ag, away) in results.iter().take(18) {
                rows.push(result_row(home, hg, ag, away));
            }
        }
        // the league's top scorers and assists (user 2026-11-22: "the stats (top scorer)"): the
        // players' league row of their season stats, goals (2) and assists (5)
        340..=347 if league(340).is_some() => {
            let comp = league(340).unwrap();
            let mut players: Vec<(i32, i32, usize, usize)> = Vec::new();
            for (c, _) in challenge::league_table_of(comp).map_or(Vec::new(), |t| t.1) {
                for s in challenge::squad_of(c) {
                    let (goals, assists) = (challenge::season_stat(s, 2, 2) as i32, challenge::season_stat(s, 2, 5) as i32);
                    if goals > 0 || assists > 0 {
                        players.push((goals, assists, s, c));
                    }
                }
            }
            rows.push(title(YELLOW, &cut(&comp_name(comp).to_uppercase(), 34)));
            rows.push(Vec::new());
            let line = |k: usize, p: &(i32, i32, usize, usize), v: i32| {
                let colour = if p.3 == own { YELLOW } else { WHITE };
                linked(&[(colour, &format!("{:>3} {:<19}", k + 1, cut(&latin1(&staff_name(p.2)), 18)), to_player(p.2)), (0, &format!("{:<12}", cut(&short(p.3), 11)), to_club(p.3)), (0, &format!("{v:>3}"), String::new())])
            };
            rows.push(row(&[(CYAN, " TOP SCORERS                        Gls")]));
            players.sort_by_key(|p| (-p.0, -p.1));
            let scorers: Vec<_> = players.iter().filter(|p| p.0 > 0).take(10).collect();
            if scorers.is_empty() {
                rows.push(row(&[(WHITE, " No goals scored yet.")]));
            }
            for (k, p) in scorers.iter().enumerate() {
                rows.push(line(k, p, p.0));
            }
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, " TOP ASSISTS                        Ast")]));
            players.sort_by_key(|p| (-p.1, -p.0));
            for (k, p) in players.iter().filter(|p| p.1 > 0).take(5).enumerate() {
                rows.push(line(k, p, p.1));
            }
        }
        // a table with its zones (user 2026-11-28: dashed lines for Europe, promotion and
        // relegation, play-offs too) and the divisions above and below
        324..=331 if league(324).is_some() => {
            let comp = league(324).unwrap();
            let table = challenge::league_table_of(comp).map_or(Vec::new(), |t| t.1);
            let zones = zones(comp, &table);
            const PER: usize = 16;
            subs = table.len().div_ceil(PER).max(1);
            let sub = sub.min(subs - 1);
            rows.push(title(CYAN, &cut(&comp_name(comp).to_uppercase(), 34)));
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, "    TEAM            P  W  D  L  GD Pts")]));
            for (k, (c, v)) in table.iter().enumerate().skip(sub * PER).take(PER) {
                let colour = if *c == own { YELLOW } else { WHITE };
                let r = linked(&[(colour, &format!("{:>3} ", k + 1), String::new()), (0, &format!("{:<14}", cut(&short(*c), 14)), to_club(*c)), (0, &format!("{:>3}{:>3}{:>3}{:>3}{:>4}{:>4}", v[0], v[1], v[2], v[3], v[4] - v[5], v[6]), String::new())]);
                rows.push(match zones.iter().find(|z| z.0 == k + 1) {
                    Some(z) => with_marks(r, &[format!("z{}", z.1)]),
                    None => r,
                });
            }
            // the key: two to a row
            for pair in zones.chunks(2) {
                let words: Vec<String> = pair.iter().map(|z| format!("- {:<16}", cut(&z.2, 16))).collect();
                let parts: Vec<(u8, &str, String)> = pair.iter().zip(&words).map(|(z, w)| (z.1, w.as_str(), String::new())).collect();
                rows.push(linked(&parts));
            }
            let league_obj = challenge::league_object(comp);
            let near = |o: usize| if league_obj != 0 { challenge::comp_by_id(rd::<i32>(league_obj + o)) } else { 0 };
            let (above, below) = (near(0x1c), near(0x20));
            let mut nav: Vec<(u8, String, String)> = vec![(GREEN, " Results".into(), to_league(316, comp)), (YELLOW, "Scorers".into(), to_league(340, comp))];
            for (c, arrow) in [(above, "Up"), (below, "Down")] {
                if c != 0 && challenge::league_table_of(c).is_some() {
                    nav.push((CYAN, format!("{arrow}: {}", cut(&comp_name(c), 9)), to_league(324, c)));
                }
            }
            rows.push(linked(&nav.iter().map(|(c, t, l)| (*c, t.as_str(), l.clone())).collect::<Vec<_>>()));
        }
        // a club's page: its place, last results, top scorers
        CLUB_PAGE if param > 0 && club_at(param - 1).is_some() => {
            let club = club_at(param - 1).unwrap();
            rows.push(title(MAGENTA, &cut(&short(club).to_uppercase(), 34)));
            rows.push(Vec::new());
            let comp = rd::<usize>(club + 0x57);
            if let Some((_, table)) = challenge::league_table_of(comp) {
                if let Some(k) = table.iter().position(|t| t.0 == club) {
                    let v = table[k].1;
                    rows.push(linked(&[(CYAN, &format!(" {}", cut(&comp_name(comp), 37)), to_league(324, comp))]));
                    rows.push(row(&[(WHITE, &format!(" Position {} of {}, {} pts from {} games", k + 1, table.len(), v[6], v[0]))]));
                }
            }
            rows.push(Vec::new());
            rows.push(row(&[(YELLOW, " LAST RESULTS")]));
            let results = last_results(club, 6);
            if results.is_empty() {
                rows.push(row(&[(WHITE, " No matches played yet.")]));
            }
            for r in &results {
                let Some(opp) = club_at(r.opponent.max(0) as usize) else { continue };
                let (mark, colour) = match r.goals_for.cmp(&r.against) {
                    std::cmp::Ordering::Greater => ("W", GREEN),
                    std::cmp::Ordering::Equal => ("D", YELLOW),
                    std::cmp::Ordering::Less => ("L", RED),
                };
                let date: String = finreport::date_text(r.day % 366, r.day / 366).split(' ').take(2).map(|w| cut(w, 3)).collect::<Vec<_>>().join(" ");
                rows.push(linked(&[(WHITE, &format!(" {date:<7}{} ", if r.home { "H" } else { "A" }), String::new()), (0, &format!("{:<20}", cut(&short(opp), 20)), to_club(opp)), (colour, &format!("{}-{} {mark}", r.goals_for, r.against), String::new())]));
            }
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, " TOP SCORERS              Apps Gls Ast")]));
            let mut squad: Vec<(i32, i32, i32, usize)> = challenge::squad_of(club).into_iter().map(|s| (challenge::season_stat(s, 2, 2) as i32, challenge::season_stat(s, 2, 5) as i32, challenge::season_stat(s, 2, 1) as i32, s)).filter(|p| p.2 > 0).collect();
            squad.sort_by_key(|p| (-p.0, -p.1, -p.2));
            for p in squad.iter().take(6) {
                rows.push(linked(&[(WHITE, &format!(" {:<25}", cut(&latin1(&staff_name(p.3)), 24)), to_player(p.3)), (0, &format!("{:>4}{:>4}{:>4}", p.2, p.0, p.1), String::new())]));
            }
            if squad.is_empty() {
                rows.push(row(&[(WHITE, " No games played yet.")]));
            }
        }
        // a player's page: his club, age, this season in the league
        PLAYER_PAGE if param > 0 && staff_at(param - 1).is_some() => {
            let s = staff_at(param - 1).unwrap();
            rows.push(title(GREEN, &cut(&latin1(&staff_name(s)).to_uppercase(), 34)));
            rows.push(Vec::new());
            let club = rd::<usize>(s + 0x39);
            if readable(club, 0x5b) && club >= rd::<usize>(CLUB_TABLE) && club_at(club_index(club)) == Some(club) {
                rows.push(linked(&[(CYAN, " Club  ", String::new()), (WHITE, &cut(&short(club), 30), to_club(club))]));
            }
            let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
            rows.push(row(&[(CYAN, " Age   "), (WHITE, &dashboard::age_of(s, day, year).to_string())]));
            rows.push(Vec::new());
            rows.push(row(&[(YELLOW, " THIS SEASON (LEAGUE)")]));
            for (label, field) in [("Appearances", 1), ("Goals", 2), ("Assists", 5)] {
                rows.push(row(&[(WHITE, &format!(" {label:<16}{:>4}", challenge::season_stat(s, 2, field) as i32))]));
            }
        }
        _ => {
            // the index (and any page not here)
            rows.push(title(BLUE, "CEEFAX FOOTBALL"));
            rows.push(Vec::new());
            if number != 100 {
                rows.push(row(&[(RED, &format!(" Page {number} not found"))]));
            }
            rows.push(row(&[(YELLOW, &format!("{:<32}", " Front page")), (WHITE, "301")]));
            rows.push(row(&[(YELLOW, &format!("{:<32}", " Latest news")), (WHITE, "302")]));
            if readable(own, 0x5b) {
                rows.push(linked(&[(YELLOW, &format!("{:<32}", format!(" {}", cut(&short(own), 30))), to_club(own)), (WHITE, "400", to_club(own))]));
            }
            rows.push(Vec::new());
            for (k, (_, name, _)) in leagues.iter().enumerate() {
                rows.push(row(&[(CYAN, &format!(" {}", cut(name, 36)))]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Results")), (WHITE, &(316 + k).to_string())]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Table")), (WHITE, &(324 + k).to_string())]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Top scorers")), (WHITE, &(340 + k).to_string())]));
            }
            rows.push(Vec::new());
            rows.push(row(&[(MAGENTA, " Click a page, a club or a player")]));
        }
    }
    let mut out = format!("{subs}").into_bytes();
    for r in rows.into_iter().take(23) {
        out.push(b'\n');
        out.extend(r);
    }
    out
}

/// The league's latest round: every league match of its clubs in the last days before the newest
/// one (home, goals, goals, away), from each club's own results (`form::played`).
unsafe fn latest_results(comp: usize) -> Vec<(usize, i32, i32, usize)> {
    let Some((_, table)) = challenge::league_table_of(comp) else { return Vec::new() };
    let base = rd::<usize>(CLUB_TABLE);
    let ids: Vec<i32> = table.iter().map(|t| club_index(t.0) as i32).collect();
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 40 { (day - 40, year) } else { (day + 325, year - 1) };
    let mut games: Vec<(i32, i32, i32, i32, i32)> = Vec::new(); // day, home, away, home goals, away goals
    for &id in &ids {
        for r in form::played(id, start, (day, year), 4) {
            if r.friendly || !ids.contains(&r.opponent) {
                continue;
            }
            let g = if r.home { (r.day, id, r.opponent, r.goals_for, r.against) } else { (r.day, r.opponent, id, r.against, r.goals_for) };
            if !games.contains(&g) {
                games.push(g);
            }
        }
    }
    let newest = games.iter().map(|g| g.0).max().unwrap_or(0);
    games.retain(|g| g.0 >= newest - 3);
    games.sort_by_key(|g| (g.0, g.1));
    let club = |i: i32| base + i as usize * CLUB_SIZE;
    games.iter().map(|g| (club(g.1), g.3, g.4, club(g.2))).collect()
}

// ---- the set (every machine) ----

#[repr(C)]
struct SystemTime {
    year: u16,
    month: u16,
    weekday: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    ms: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(t: *mut SystemTime);
}

/// The largest game font no taller than `h`.
unsafe fn font_for(h: i32) -> u32 {
    let tall = |n: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)(n as i32);
    (1..=7u32).filter(|&n| tall(n) <= h).max_by_key(|&n| tall(n)).map_or(0xffff0001, |n| 0xffff0000 | n)
}

/// Three digits on their own in a row (no more, no less): a page number, 100..899.
fn page_number(cells: &[u8]) -> Option<u16> {
    let mut run = String::new();
    for &b in cells.iter().chain(std::iter::once(&b' ')) {
        if b.is_ascii_digit() {
            run.push(b as char);
        } else if run.len() == 3 {
            return run.parse().ok().filter(|n| (100..900).contains(n));
        } else {
            run.clear();
        }
    }
    None
}

/// The flip pass: the set over everything right of the menu bar, while it is open.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    if !OPEN {
        return;
    }
    if SUBS > 1 && !HELD && TURNED.is_some_and(|t| t.elapsed().as_secs() >= 8) {
        SUB = (SUB + 1) % SUBS;
        fetch();
    }
    // (a client's page comes from the host after the first ask: asked again until it is here;
    // the set stayed black on a client, night 12)
    if remote::CLIENT && ROWS.iter().all(|r| r.is_empty()) && TURNED.is_some_and(|t| t.elapsed().as_millis() >= 300) {
        fetch();
    }
    // (the clock's seconds and the turning subpages: drawn again every second)
    REDRAW_AT = Some(std::time::Instant::now() + std::time::Duration::from_millis(1000));
    let (sw, sh) = screen();
    // (the whole page right of the menu bar is the set: the grey frame round it and the strip by
    // the bar showed the game's screen, user 2026-11-28 "only part of the screen")
    // (from the menu bar picture's own edge column, `menu_bars`: `bar_end` is where its buttons'
    // click area ends, 30 px further right at 2000 wide, and the game showed in between, user
    // 2026-11-28)
    let edge = |x: i32| pixel_at(x, 3).is_some_and(|p| (0..3).all(|i| (p[i] as i32 - [0x33, 0x38, 0x40][i]).abs() <= 6));
    let left = (40..bar_end(sw)).find(|&x| edge(x)).map_or(bar_end(sw), |x| x + 1);
    // (not [0, 0, 0]: the game's rectangle draws nothing in colour 0, the screen showed through,
    // user 2026-11-28 screenshot)
    // (inside the screen: to sw / sh, one past its edge, the game drew nothing at all, user
    // 2026-11-28; the dashboard's sw - 1 works)
    fill(left, 0, sw - 1, sh - 1, INK_BLACK);
    let m = (sh / 80).max(4);
    let (aw, ah) = (sw - left - 2 * m, sh - 2 * m);
    let ch = ah / 25;
    // (the set from the menu bar to the screen's edge, user 2026-11-28)
    let cw = aw / 40;
    let (x0, y0) = (left + m + (aw - cw * 40) / 2, m + (ah - ch * 25) / 2);
    let (font, tall_font) = (font_for(ch - 2), font_for(2 * ch - 4));
    // the header: page number, CEEFAX, the game's date, the clock
    let mut t = SystemTime { year: 0, month: 0, weekday: 0, day: 0, hour: 0, minute: 0, second: 0, ms: 0 };
    GetLocalTime(&mut t);
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    // (day and a three-letter month, as Ceefax did: "2 Septemb" was cut and pushed the clock off
    // the 40 cells on a page of two, night 12)
    let date: String = finreport::date_text(day, year).split(' ').take(2).map(|w| w.chars().take(3).collect::<String>()).collect::<Vec<_>>().join(" ");
    let more = if SUBS > 1 { format!(" {}/{}", SUB + 1, SUBS) } else { String::new() };
    // (one white run and the clock: each colour change takes a cell, and four of them pushed the
    // clock's last digit off the 40, night 12)
    let header = row(&[(WHITE, &format!("P{NUMBER} CEEFAX 1 {NUMBER}{more} {}", cut(&date, 9))), (YELLOW, &format!("{:02}:{:02}/{:02}", t.hour, t.minute, t.second))]);
    // ("1/2" turns to the next subpage by hand, user 2026-11-28; cell 0 is the colour code)
    let header = if more.is_empty() {
        header
    } else {
        let a = 1 + format!("P{NUMBER} CEEFAX 1 {NUMBER} ").len();
        with_marks(header, &[format!("{a}-{}={NEXT_SUB}", a + more.len() - 2)])
    };
    // Fastext: the four colours, Back, and Close on red (user 2026-11-28: "een duidelijke close")
    let mut fastext = row(&[(RED, "Index"), (GREEN, "News"), (YELLOW, "Results"), (CYAN, "Tables"), (WHITE, "Back"), (RED, "")]);
    fastext.extend([NEW_BG, WHITE]);
    fastext.extend(b" CLOSE");
    let fastext = with_marks(fastext, &["0-5=100".into(), "6-10=302".into(), "11-18=316".into(), "19-25=324".into(), "26-30=0".into(), "31-39=1".into()]);
    let mut all: Vec<Vec<u8>> = vec![header];
    all.extend(ROWS.iter().cloned());
    all.resize(24, Vec::new());
    all.push(fastext);
    let (mx, my) = (*(MOUSE_X as *const i32), *(MOUSE_Y as *const i32));
    let inside = |r: (i32, i32, i32, i32)| (r.0..r.2).contains(&mx) && (r.1..r.3).contains(&my);
    LINKS.clear();
    let mut skip_next = false;
    for (r, whole) in all.iter().enumerate() {
        if std::mem::take(&mut skip_next) {
            continue;
        }
        let (cells, links, zone) = marks_of(whole);
        let cells = &cells[..cells.len().min(40)];
        let y = y0 + r as i32 * ch;
        let double = cells.contains(&DOUBLE);
        let h = if double { 2 * ch } else { ch };
        // the row's links (a page number on its own links the whole row); the one under the mouse lit
        let mut boxes: Vec<((i32, i32, i32, i32), u16, usize)> = links.iter().map(|&(a, b, n, p)| ((x0 + a as i32 * cw, y, x0 + (b as i32 + 1) * cw, y + h), n, p)).collect();
        if links.is_empty() && (1..24).contains(&r) {
            if let Some(n) = page_number(cells) {
                boxes.push(((x0, y, x0 + cw * 40, y + h), n, 0));
            }
        }
        for b in &boxes {
            if inside(b.0) && b.1 > CLOSE {
                fill(b.0 .0, b.0 .1, b.0 .2, b.0 .3, [0, 0, 0xa0]);
            }
        }
        let (mut fg, mut bg, mut tall) = (WHITE, 0u8, false);
        for (c, &b) in cells.iter().enumerate() {
            let x = x0 + c as i32 * cw;
            match b {
                1..=7 => fg = b,
                DOUBLE => {
                    tall = true;
                    skip_next = true;
                }
                0x0c => tall = false,
                NEW_BG => bg = fg,
                BLACK_BG => bg = 0,
                _ => {}
            }
            let h = if tall { 2 * ch } else { ch };
            if bg != 0 {
                fill(x, y, x + cw, y + h, PALETTE[bg as usize]);
            }
            if b >= 0x20 {
                text_at(x + cw / 2, y, y + h, if tall { tall_font } else { font }, PALETTE[fg as usize], &[b], 1);
            }
        }
        // (the row's background runs to its end)
        if bg != 0 && cells.len() < 40 {
            fill(x0 + cells.len() as i32 * cw, y, x0 + 40 * cw, y + h, PALETTE[bg as usize]);
        }
        // Close lit: white with red letters
        for b in boxes.iter().filter(|b| b.1 == CLOSE && inside(b.0)) {
            fill(b.0 .0 + cw, b.0 .1, b.0 .2, b.0 .3, PALETTE[WHITE as usize]);
            text_at((b.0 .0 + cw + b.0 .2) / 2, b.0 .1, b.0 .3, font, PALETTE[RED as usize], b"CLOSE", 1);
        }
        // a zone's dashed line under the row
        if let Some(z) = zone {
            let ly = y + h - (ch / 12).max(1);
            let dash = (cw / 2).max(3);
            let mut x = x0;
            while x < x0 + 40 * cw {
                fill(x, ly, (x + dash).min(x0 + 40 * cw), ly + (ch / 14).max(2), PALETTE[z as usize]);
                x += dash * 2;
            }
        }
        LINKS.extend(boxes);
    }
}

/// The mouse: the news screen's button opens the set; while it is open it takes every click but
/// the menu bar's (which leaves it, as CM Challenge's page); moving over a link lights it.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    let inside = |r: (i32, i32, i32, i32)| (r.0..r.2).contains(&mx) && (r.1..r.3).contains(&my);
    if !OPEN {
        // (not under one of our pages: the news screen's button area outlives it there, user
        // 2026-11-23: the Rad's Spin on CM Challenge opened Teletext)
        if released && news::TELETEXT_BUTTON.is_some_and(|r| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my) && news::drawn(r, [0x5a, 0x5a, 0x10])) && !news::MENU_OPEN && !crate::finreport::covered() {
            *(LEFT_CLICK as *mut u32) = 0;
            open();
            return Some(true);
        }
        return None;
    }
    if mx < bar_end(screen().0) {
        if released {
            OPEN = false;
            REDRAW = true;
        }
        return None;
    }
    *(LEFT_CLICK as *mut u32) = 0;
    // (the poll returns here while the set is open, before its REDRAW_AT: the clock and a
    // client's page asked of the host came only with a click; once a second here, night 12)
    static mut TICK: Option<std::time::Instant> = None;
    let tick = TICK.is_none_or(|t| t.elapsed().as_millis() >= 1000);
    if tick {
        TICK = Some(std::time::Instant::now());
        if remote::CLIENT && ROWS.iter().all(|r| r.is_empty()) {
            fetch();
        }
    }
    let under = LINKS.iter().rposition(|l| inside(l.0));
    let moved = std::mem::replace(&mut UNDER, under) != under;
    // (a release counts only a moment after the opening click)
    if !released || OPENED_AT.is_some_and(|t| t.elapsed().as_millis() < 300) {
        return Some(tick || moved);
    }
    match under.map(|k| (LINKS[k].1, LINKS[k].2)) {
        Some((n, p)) => {
            go(n, p);
            Some(true)
        }
        None => Some(moved),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_the_set() {
        let r = row(&[(WHITE, &"x".repeat(60))]);
        assert_eq!(r.len(), 40);
        assert_eq!(r[0], WHITE);
        let t = title(CYAN, "EREDIVISIE");
        assert_eq!((t.len(), t[0], t[1], t[2], t[3]), (40, CYAN, NEW_BG, BLUE, DOUBLE));
        let t = title(RED, "NEWS");
        assert_eq!((t[0], t[1], t[2], t[3]), (RED, NEW_BG, WHITE, DOUBLE));
    }

    #[test]
    fn links_and_zones_survive_the_row() {
        let r = linked(&[(WHITE, "  1 ", String::new()), (0, "Ajax          ", "400.5".into()), (0, " 34", String::new())]);
        let r = with_marks(r, &["z2".into()]);
        let (cells, links, zone) = marks_of(&r);
        assert_eq!(cells.len(), 40);
        assert_eq!(links, vec![(5, 8, 400, 5)]);
        assert_eq!(zone, Some(2));
        assert_eq!(page_number(b" Table                           324"), Some(324));
        assert_eq!(page_number(b"  1 Ajax  34  1024"), None);
    }
}
