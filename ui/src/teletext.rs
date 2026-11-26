//! Teletext in the style of BBC Ceefax (user 2026-11-22): the news screen's Teletext button opens
//! the football pages: P100 the index, P302 the manager's news headlines, P316.. the latest
//! results and P324.. the tables of the human managers' leagues (the viewer's own first).
//!
//! 40 x 25 cells on black: row 0 the header (page number, date, clock), rows 1..23 the page, row 24
//! Fastext (red Index, green News, yellow Results, cyan Tables). A page is made on the host
//! (`page_text`; a network client asks key `*`) as Latin-1 rows with teletext's own control codes:
//! 1..7 the colours (red, green, yellow, blue, magenta, cyan, white), 0x0d double height, 0x0c
//! normal height, 0x1d the background takes the colour, 0x1c black background. A control code
//! takes its cell as a space, as on a real set; the row under a double-height one is left out.
//! The first line of a page's text is its number of subpages; they turn every 8 seconds.
use crate::*;

pub static mut OPEN: bool = false;
static mut NUMBER: u16 = 100;
static mut SUB: usize = 0;
static mut SUBS: usize = 1;
static mut ROWS: Vec<Vec<u8>> = Vec::new();
static mut TURNED: Option<std::time::Instant> = None;
static mut OPENED_AT: Option<std::time::Instant> = None;
/// Where things are this frame: the page numbers on the rows (cells), Fastext, Back.
static mut LINKS: Vec<((i32, i32, i32, i32), u16)> = Vec::new();
static mut BACK: Option<(i32, i32, i32, i32)> = None;

const PALETTE: [[u8; 3]; 8] = [[0, 0, 0], [0xff, 0, 0], [0, 0xff, 0], [0xff, 0xff, 0], [0, 0, 0xff], [0xff, 0, 0xff], [0, 0xff, 0xff], [0xff, 0xff, 0xff]];
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

pub fn page_open() -> bool {
    unsafe { OPEN }
}

pub unsafe fn open() {
    (OPEN, NUMBER, SUB, OPENED_AT) = (true, 100, 0, Some(std::time::Instant::now()));
    fetch();
}

unsafe fn go(number: u16) {
    (NUMBER, SUB) = (number, 0);
    fetch();
}

/// The page from the host (or made here).
#[allow(static_mut_refs)]
unsafe fn fetch() {
    let text = if remote::CLIENT { ask_host(&key(b'*', &[NUMBER as u32, SUB as u32])).unwrap_or_default() } else { page_text(channel(), NUMBER, SUB) };
    let mut lines = text.split(|&c| c == b'\n');
    SUBS = lines.next().and_then(|l| std::str::from_utf8(l).ok()?.trim().parse().ok()).unwrap_or(1).max(1);
    ROWS = lines.map(|l| l.to_vec()).collect();
    TURNED = Some(std::time::Instant::now());
    REDRAW = true;
}

// ---- the pages (host) ----

/// A row of cells: control codes and text, cut at 40.
fn row(parts: &[(u8, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (code, text) in parts {
        if *code != 0 {
            out.push(*code);
        }
        out.extend(to_latin1(text));
    }
    out.truncate(40);
    out
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
            out.push((comp, latin1(&game_text(comp + 4, 51)).into_owned(), club));
        }
    }
    out.truncate(8);
    out
}

/// A club's short name (club +0x38), else its name.
unsafe fn short(club: usize) -> String {
    let s = latin1(&game_text(club + 0x38, 26)).into_owned();
    if s.trim().is_empty() { latin1(&game_text(club + 4, 51)).into_owned() } else { s }
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
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
    let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 60 { (day - 60, year) } else { (day + 305, year - 1) };
    form::played(index, start, (day, year), most + 4).into_iter().filter(|r| !r.friendly).take(most).collect()
}

/// The front page: the tabloid's headline on the manager's last match, then the other managers'.
unsafe fn front_page(channel: usize, rows: &mut Vec<Vec<u8>>) {
    let (_, _, club) = manager_of(channel);
    rows.push(title(RED, "THE DAILY CEEFAX"));
    rows.push(Vec::new());
    let base = rd::<usize>(CLUB_TABLE);
    let name = |i: i32| short(base + i.max(0) as usize * CLUB_SIZE).to_uppercase();
    let line_of = |c: usize, r: &form::Result_| {
        let (us, them) = (short(c), short(base + r.opponent.max(0) as usize * CLUB_SIZE));
        if r.home { format!("{us} {}-{} {them}", r.goals_for, r.against) } else { format!("{them} {}-{} {us}", r.against, r.goals_for) }
    };
    match last_results(club, 1).first() {
        Some(r) => {
            let (us, them) = (short(club).to_uppercase(), name(r.opponent));
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
            rows.push(row(&[(WHITE, &format!(" {}", cut(&line_of(club, r), 38)))]));
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
            elsewhere.push(row(&[(WHITE, &format!(" {}", cut(&line_of(other, r), 38)))]));
        }
    }
    if !elsewhere.is_empty() {
        rows.push(Vec::new());
        rows.push(row(&[(CYAN, " ELSEWHERE")]));
        rows.extend(elsewhere);
    }
}

/// Host: a page for a channel's manager: its subpages on the first line, then the rows.
pub unsafe fn page_text(channel: usize, number: u16, sub: usize) -> Vec<u8> {
    let leagues = leagues(channel);
    let mut rows: Vec<Vec<u8>> = Vec::new();
    let mut subs = 1;
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
        316..=323 if leagues.get((number - 316) as usize).is_some() => {
            let (_, name, club) = &leagues[(number - 316) as usize];
            rows.push(title(GREEN, &cut(&name.to_uppercase(), 34)));
            rows.push(Vec::new());
            rows.push(row(&[(YELLOW, " LATEST RESULTS")]));
            let results = latest_results(*club);
            if results.is_empty() {
                rows.push(row(&[(WHITE, " No matches played yet.")]));
            }
            for (home, hg, ag, away) in results.iter().take(18) {
                rows.push(row(&[(WHITE, &format!("{:>16}", cut(home, 16))), (YELLOW, &format!("{hg}-{ag}")), (WHITE, &cut(away, 17))]));
            }
        }
        // the league's top scorers and assists (user 2026-11-22: "the stats (top scorer)"): the
        // players' league row of their season stats, goals (2) and assists (5)
        340..=347 if leagues.get((number - 340) as usize).is_some() => {
            let (_, name, club) = &leagues[(number - 340) as usize];
            let (_, _, own) = manager_of(channel);
            let mut players: Vec<(i32, i32, String, String, bool)> = Vec::new();
            for (c, _) in challenge::league_table(*club).map_or(Vec::new(), |t| t.1) {
                for s in challenge::squad_of(c) {
                    let (goals, assists) = (challenge::season_stat(s, 2, 2) as i32, challenge::season_stat(s, 2, 5) as i32);
                    if goals > 0 || assists > 0 {
                        players.push((goals, assists, latin1(&staff_name(s)).into_owned(), short(c), c == own));
                    }
                }
            }
            rows.push(title(YELLOW, &cut(&name.to_uppercase(), 34)));
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, " TOP SCORERS                        Gls")]));
            players.sort_by_key(|p| (-p.0, -p.1));
            let scorers: Vec<_> = players.iter().filter(|p| p.0 > 0).take(10).collect();
            if scorers.is_empty() {
                rows.push(row(&[(WHITE, " No goals scored yet.")]));
            }
            for (k, p) in scorers.iter().enumerate() {
                rows.push(row(&[(if p.4 { YELLOW } else { WHITE }, &format!("{:>3} {:<19}{:<12}{:>3}", k + 1, cut(&p.2, 18), cut(&p.3, 11), p.0))]));
            }
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, " TOP ASSISTS                        Ast")]));
            players.sort_by_key(|p| (-p.1, -p.0));
            for (k, p) in players.iter().filter(|p| p.1 > 0).take(5).enumerate() {
                rows.push(row(&[(if p.4 { YELLOW } else { WHITE }, &format!("{:>3} {:<19}{:<12}{:>3}", k + 1, cut(&p.2, 18), cut(&p.3, 11), p.1))]));
            }
        }
        324..=331 if leagues.get((number - 324) as usize).is_some() => {
            let (_, name, club) = &leagues[(number - 324) as usize];
            let own_club = rd::<usize>(rd::<usize>(CHANNEL_MANAGER + 0x300 * channel) + 0x39);
            let table = challenge::league_table(*club).map_or(Vec::new(), |t| t.1);
            const PER: usize = 18;
            subs = table.len().div_ceil(PER).max(1);
            let sub = sub.min(subs - 1);
            rows.push(title(CYAN, &cut(&name.to_uppercase(), 34)));
            rows.push(Vec::new());
            rows.push(row(&[(CYAN, "    TEAM            P  W  D  L  GD Pts")]));
            for (k, (c, v)) in table.iter().enumerate().skip(sub * PER).take(PER) {
                let colour = if *c == own_club { YELLOW } else { WHITE };
                rows.push(row(&[(colour, &format!("{:>3} {:<14}{:>3}{:>3}{:>3}{:>3}{:>4}{:>4}", k + 1, cut(&short(*c), 14), v[0], v[1], v[2], v[3], v[4] - v[5], v[6]))]));
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
            rows.push(Vec::new());
            for (k, (_, name, _)) in leagues.iter().enumerate() {
                rows.push(row(&[(CYAN, &format!(" {}", cut(name, 36)))]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Results")), (WHITE, &(316 + k).to_string())]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Table")), (WHITE, &(324 + k).to_string())]));
                rows.push(row(&[(GREEN, &format!("{:<32}", "   Top scorers")), (WHITE, &(340 + k).to_string())]));
            }
            rows.push(Vec::new());
            rows.push(row(&[(MAGENTA, " Click a page number or a coloured key")]));
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
unsafe fn latest_results(club: usize) -> Vec<(String, i32, i32, String)> {
    let Some((_, table)) = challenge::league_table(club) else { return Vec::new() };
    let base = rd::<usize>(CLUB_TABLE);
    let index = |c: usize| ((c - base) / CLUB_SIZE) as i32;
    let ids: Vec<i32> = table.iter().map(|t| index(t.0)).collect();
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
    let name = |i: i32| short(base + i as usize * CLUB_SIZE);
    games.iter().map(|g| (name(g.1), g.3, g.4, name(g.2))).collect()
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

/// The flip pass: the set over the whole screen but the menu bar, while it is open.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    if !OPEN {
        return;
    }
    if SUBS > 1 && TURNED.is_some_and(|t| t.elapsed().as_secs() >= 8) {
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
    let m = (sh / 60).max(6);
    let left = bar_end(sw) + m;
    let (aw, ah) = (sw - m - left, sh - 2 * m);
    let ch = ah / 25;
    let cw = (aw / 40).min(ch * 5 / 6);
    let (x0, y0) = (left + (aw - cw * 40) / 2, m);
    fill(left, 0, sw, sh, [0x10, 0x10, 0x10]);
    fill(x0 - cw, y0, x0 + cw * 41, y0 + ch * 25, [0, 0, 0]);
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
    // (five keys of 8 cells; Back in the set itself: as a button under it, it stood on the news
    // screen's own Next and was hard to see, user 2026-11-22)
    let fastext = row(&[(RED, " Index "), (GREEN, "News   "), (YELLOW, "Results"), (CYAN, "Tables "), (WHITE, "Back")]);
    LINKS.clear();
    let mut all: Vec<Vec<u8>> = vec![header];
    all.extend(ROWS.iter().cloned());
    all.resize(24, Vec::new());
    all.push(fastext);
    let mut skip_next = false;
    for (r, cells) in all.iter().enumerate() {
        if std::mem::take(&mut skip_next) {
            continue;
        }
        let y = y0 + r as i32 * ch;
        let (mut fg, mut bg, mut double) = (WHITE, 0u8, false);
        let mut number = String::new();
        for (c, &b) in cells.iter().enumerate().take(40) {
            let x = x0 + c as i32 * cw;
            match b {
                1..=7 => fg = b,
                DOUBLE => {
                    double = true;
                    skip_next = true;
                }
                0x0c => double = false,
                NEW_BG => bg = fg,
                BLACK_BG => bg = 0,
                _ => {}
            }
            let h = if double { 2 * ch } else { ch };
            if bg != 0 {
                fill(x, y, x + cw, y + h, PALETTE[bg as usize]);
            }
            if b >= 0x20 {
                let glyph = [b];
                text_at(x + cw / 2, y, y + h, if double { tall_font } else { font }, PALETTE[fg as usize], &glyph, 1);
                // three digits in a row on the page: a link to that page
                if b.is_ascii_digit() && r > 0 && r < 24 {
                    number.push(b as char);
                    if number.len() == 3 {
                        if let Ok(n) = number.parse::<u16>() {
                            LINKS.push(((x0, y, x0 + cw * 40, y + h), n));
                        }
                    }
                } else {
                    number.clear();
                }
            }
        }
        // (the row's background runs to its end)
        if bg != 0 && cells.len() < 40 {
            fill(x0 + cells.len() as i32 * cw, y, x0 + 40 * cw, y + if double { 2 * ch } else { ch }, PALETTE[bg as usize]);
        }
    }
    // Fastext: four keys of 8 cells, then Back
    let fy = y0 + 24 * ch;
    for (k, n) in [100u16, 302, 316, 324].into_iter().enumerate() {
        let a = x0 + k as i32 * cw * 8;
        LINKS.push(((a, fy, a + cw * 8, fy + ch), n));
    }
    BACK = Some((x0 + 32 * cw, fy, x0 + 40 * cw, fy + ch));
}

/// The mouse: the news screen's button opens the set; while it is open it takes every click but
/// the menu bar's (which leaves it, as CM Challenge's page).
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    let inside = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if !OPEN {
        // (not under one of our pages: the news screen's button area outlives it there, user
        // 2026-11-23: the Rad's Spin on CM Challenge opened Teletext)
        if released && news::TELETEXT_BUTTON.is_some_and(|r| inside(r) && news::drawn(r, [0x5a, 0x5a, 0x10])) && !news::MENU_OPEN && !crate::finreport::covered() {
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
    // (a release counts only a moment after the opening click)
    if !released || OPENED_AT.is_some_and(|t| t.elapsed().as_millis() < 300) {
        return Some(tick);
    }
    if BACK.is_some_and(inside) {
        OPEN = false;
        return Some(true);
    }
    if let Some(&(_, n)) = LINKS.iter().rev().find(|l| inside(l.0)) {
        go(n);
        return Some(true);
    }
    Some(false)
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
}
