//! The national pool (switch `national_pool`, the user's wish of 2026-11-07): for a national team
//! manager, every player whose first nationality is the team's country (the game's search leaves
//! out the low reputations, and its nation filter has no entry for some countries, Cameroon), as
//! the assistant rates them; a + per row puts a player on the game's own Short List
//! (SCOUT_MANAGER::add_to_shortlist 0x7f75f0), so he is kept after Back. From the squad screen's
//! Assistant menu, "National pool", when the team is a national team.

use crate::*;

const SCOUT_MANAGER: usize = 0xb74448; // SCOUT_MANAGER*
const ADD_TO_SHORTLIST: usize = 0x7f75f0; // thiscall(this, Staff*, HumanManager* 0 = current) ret 8
const IS_SHORTLISTED: usize = 0x7f7490; // thiscall(this, Staff*, HumanManager* 0 = current) ret 8
/// At most this many a position group for a big country (the best by the assistant's eye).
const PER_GROUP: usize = 100;

#[derive(Clone)]
struct Man {
    staff: usize,
    name: Vec<u8>,
    positions: Vec<u8>,
    group: usize,
    age: i32,
    club: Vec<u8>,
    caps: i32,
    goals: i32,
    score: i32,
    in_squad: bool,
}

static mut OPEN: bool = false;
static mut TEAM: usize = 0;
static mut MEN: Vec<Man> = Vec::new();
static mut FILTER: usize = 0; // 0 all, 1..=4 a group
static mut NOT_IN_SQUAD: bool = false;
static mut PAGE: usize = 0;
/// This frame's click spots (rect, what).
static mut SPOTS: Vec<((i32, i32, i32, i32), Spot)> = Vec::new();

#[derive(Clone, Copy, PartialEq)]
enum Spot {
    Back,
    Filter(usize),
    NotInSquad,
    Prev,
    Next,
    AddTop,
    Add(usize), // the staff
}

// A network client: the list is gathered on the host (it judges through the game), key 'n' (team,
// a request number, players to put on the Short List first); the ticks come with it.
/// Client: the key whose answer is awaited (empty: none), the request count, its Short List.
static mut KEY: Vec<u8> = Vec::new();
static mut ASKS: u32 = 0;
static mut LISTED: Vec<usize> = Vec::new();

/// Opens the pool of the national team `team`.
#[allow(static_mut_refs)]
pub unsafe fn open(team: usize) {
    (OPEN, TEAM, FILTER, NOT_IN_SQUAD, PAGE) = (true, team, 0, false, 0);
    if remote::CLIENT {
        MEN.clear();
        ask(&[]);
    } else {
        MEN = gather(team);
    }
}

/// Client: the list asked again, with players to add first.
#[allow(static_mut_refs)]
unsafe fn ask(adds: &[usize]) {
    ASKS += 1;
    let mut args = vec![TEAM as u32, ASKS];
    args.extend(adds.iter().map(|&s| s as u32));
    KEY = key(b'n', &args);
}

/// Host: a client's key 'n' (team, number, players to add), while its channel is served (the
/// Short List calls take the served channel's manager): the pool as bytes, each man: staff u32,
/// group, age, caps, goals, score, in squad, on the Short List (u8 each), then name, positions
/// and club (u8 length, Latin-1).
pub unsafe fn remote(args: &[usize]) -> Vec<u8> {
    let team = args.first().copied().unwrap_or(0);
    if !readable(team, 0x5b) {
        return Vec::new();
    }
    for &s in args.iter().skip(2).take(20) {
        if valid_staff(s) {
            add(s);
        }
    }
    let mut out = Vec::new();
    for m in gather(team) {
        out.extend((m.staff as u32).to_le_bytes());
        out.extend([m.group as u8, m.age.clamp(0, 255) as u8, m.caps.clamp(0, 255) as u8, m.goals.clamp(0, 255) as u8, m.score.clamp(0, 100) as u8, m.in_squad as u8, shortlisted(m.staff) as u8]);
        for t in [&m.name, &m.positions, &m.club] {
            out.push(t.len().min(255) as u8);
            out.extend(&t[..t.len().min(255)]);
        }
    }
    out
}

/// Client: the host's list read back (the men, those on the Short List).
fn parse(v: &[u8]) -> (Vec<Man>, Vec<usize>) {
    let (mut men, mut listed, mut at) = (Vec::new(), Vec::new(), 0);
    while at + 11 <= v.len() {
        let staff = u32::from_le_bytes(v[at..at + 4].try_into().unwrap()) as usize;
        let b = &v[at + 4..at + 11];
        at += 11;
        let mut texts = Vec::new();
        for _ in 0..3 {
            let Some(&n) = v.get(at) else { return (men, listed) };
            let Some(t) = v.get(at + 1..at + 1 + n as usize) else { return (men, listed) };
            texts.push(t.to_vec());
            at += 1 + n as usize;
        }
        if b[6] != 0 {
            listed.push(staff);
        }
        let club = texts.pop().unwrap();
        let positions = texts.pop().unwrap();
        let name = texts.pop().unwrap();
        men.push(Man { staff, name, positions, group: b[0] as usize, age: b[1] as i32, club, caps: b[2] as i32, goals: b[3] as i32, score: b[4] as i32, in_squad: b[5] != 0 });
    }
    (men, listed)
}

#[cfg(test)]
#[test]
fn pool_bytes() {
    let mut v = 0x1234u32.to_le_bytes().to_vec();
    v.extend([2, 27, 40, 3, 81, 1, 1, 3]);
    v.extend(b"Bos");
    v.extend([2]);
    v.extend(b"DC");
    v.extend([4]);
    v.extend(b"Ajax");
    let (men, listed) = parse(&v);
    assert_eq!((men.len(), men[0].staff, men[0].age, men[0].score, men[0].in_squad, &men[0].club[..], listed), (1, 0x1234, 27, 81, true, &b"Ajax"[..], vec![0x1234]));
}

/// Client: the host's answer, when it came, taken in. True: something new to draw.
#[allow(static_mut_refs)]
unsafe fn take_answer() -> bool {
    if !remote::CLIENT || KEY.is_empty() {
        return false;
    }
    let Some(v) = ask_host(&KEY) else { return false };
    KEY.clear();
    (MEN, LISTED) = parse(&v);
    true
}

/// The country's players, best first in each group.
unsafe fn gather(team: usize) -> Vec<Man> {
    let nation = rd::<usize>(team + 0x53);
    if !readable(nation, 55) {
        return Vec::new();
    }
    // (by the nation's name: a team's nation record and a player's need not be one object)
    let target = game_text(nation + 4, 51);
    let judge = assistant::assistant_of(team).unwrap_or(0);
    // (the stand-in judge of a national team reads perfectly, as its pick and report)
    let _perfect = scout::perfect_if(true);
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    let (day, year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let mut known: Vec<(usize, bool)> = Vec::new();
    let mut groups: [Vec<Man>; 4] = Default::default();
    for k in 0..count.max(0) as usize {
        let s = base + k * 0x6e;
        let n = rd::<usize>(s + 0x1a);
        let ours = match known.iter().find(|x| x.0 == n) {
            Some(x) => x.1,
            None => {
                let ours = n != 0 && readable(n, 55) && game_text(n + 4, 51) == target;
                known.push((n, ours));
                ours
            }
        };
        let player = ((s + 0x61) as *const usize).read_unaligned();
        if !ours || !readable(player, 70) {
            continue;
        }
        let positions = positions_text(s);
        let group = match assistant::line_rank_of(&positions) {
            0 => 0,
            1 | 2 => 1,
            3..=5 => 2,
            _ => 3,
        };
        let score = if judge != 0 { scout::seen_score(s, judge, None).unwrap_or(0.0) } else { 0.0 };
        let club = rd::<usize>(s + 0x39);
        let born = rd::<i16>(s + 0x12) as i32;
        let age = if (1900..2200).contains(&born) { year - born - (day < rd::<i16>(s + 0x10) as i32) as i32 } else { 0 };
        groups[group].push(Man {
            staff: s,
            name: staff_name(s),
            positions,
            group,
            age,
            club: if club != 0 && readable(club, 0x40) { game_text(club + 4, 51) } else { b"Free agent".to_vec() },
            caps: rd::<u8>(s + 0x22) as i32,
            goals: rd::<u8>(s + 0x23) as i32,
            score: score.clamp(0.0, 100.0).round() as i32,
            in_squad: rd::<usize>(s + 0x24) == team,
        });
    }
    let mut out = Vec::new();
    for mut g in groups {
        g.sort_by_key(|m| -m.score);
        g.truncate(PER_GROUP);
        out.extend(g);
    }
    out
}

#[allow(static_mut_refs)]
unsafe fn shortlisted(staff: usize) -> bool {
    if remote::CLIENT {
        return LISTED.contains(&staff);
    }
    let sm = rd::<usize>(SCOUT_MANAGER);
    readable(sm, 4) && game::<extern "thiscall" fn(usize, usize, usize) -> i32>(IS_SHORTLISTED)(sm, staff, 0) & 0xff != 0
}

unsafe fn add(staff: usize) {
    if remote::CLIENT {
        LISTED.push(staff); // (ticked at once; the host's list follows)
        return ask(&[staff]);
    }
    let sm = rd::<usize>(SCOUT_MANAGER);
    if readable(sm, 4) && !shortlisted(staff) {
        game::<extern "thiscall" fn(usize, usize, usize) -> i32>(ADD_TO_SHORTLIST)(sm, staff, 0);
        log(&format!("national pool: {} to the short list", latin1(&staff_name(staff))));
    }
}

/// The rows the filters leave.
#[allow(static_mut_refs)]
unsafe fn shown() -> Vec<Man> {
    MEN.iter().filter(|m| (FILTER == 0 || m.group + 1 == FILTER) && !(NOT_IN_SQUAD && m.in_squad)).cloned().collect()
}

/// The pool over the whole screen (the flip pass).
#[allow(static_mut_refs)]
pub unsafe fn draw() {
    if !OPEN {
        return;
    }
    take_answer();
    SPOTS.clear();
    let (sw, sh) = screen();
    // (the game's content area: the menu bar at the left stays the game's and works, a click on it
    // closes the pool; user 2026-11-07, "more part of the game")
    let left = bar_end(sw);
    fill(left, 0, sw - 1, sh - 1, BACKGROUND);
    let my = (sh / 60).max(8); // the margin above and below
    let m = left + my; // the left edge of everything
    let (f1, f2, f3, f5) = (0xffff0001u32, 0xffff0002u32, 0xffff0003u32, 0xffff0005u32);
    let (white, muted) = ([0xf4, 0xf6, 0xf8], [0x8a, 0x94, 0xa4]);
    let line = (sh / 36).max(16);
    // a title band as the game's screens have one: the crest at the left, the title centred, a gold
    // line under it
    let band = (sh / 10).max(56);
    fill(m, my, sw - my, my + band, AREA);
    fill(m, my + band - 3, sw - my, my + band, GOLD);
    let crest = band - 14;
    if readable(TEAM, 0x40) {
        draw_crest(&game_text(TEAM + 4, 51), m + 12, my + 6, crest);
        text_at((m + sw - my) / 2, my + 4, my + band * 55 / 100, f5, white, &[game_text(TEAM + 4, 51).as_slice(), b" - National Pool"].concat(), 1);
    }
    // (a game call: not on a network client, whose screen says "the assistant")
    let judge = if remote::CLIENT { b"the assistant".to_vec() } else { assistant::assistant_of(TEAM).map(|a| staff_name(a)).unwrap_or_default() };
    let sub = format!("Every player of the country, rated by {} (a perfect eye) - + puts him on your Short List", latin1(&judge));
    text_at((m + sw - my) / 2, my + band * 55 / 100, my + band - 6, f2, muted, &to_latin1(&sub), 1);
    // the filters, the pages, add top 10
    let mut x = m;
    let top = my + band + 12;
    let tab = |x: i32, w: i32, label: &[u8], on: bool, spot: Spot| {
        let r = (x, top, x + w, top + line + 6);
        fill(r.0, r.1, r.2, r.3, if on { [0x3a, 0x5a, 0x86] } else { [0x24, 0x28, 0x30] });
        text_at((r.0 + r.2) / 2, r.1, r.3, f2, if on { GOLD } else { white }, label, 1);
        SPOTS.push((r, spot));
    };
    for (k, label) in ["All", "GK", "DEF", "MID", "ATT"].iter().enumerate() {
        let w = if k == 0 { 70 } else { 64 };
        tab(x, w, label.as_bytes(), FILTER == k, Spot::Filter(k));
        x += w + 6;
    }
    tab(x + 12, 150, b"Not in squad", NOT_IN_SQUAD, Spot::NotInSquad);
    let rows = shown();
    let per = ((sh - top - line * 4 - 2 * my - sh / 20) / (line + 4)).max(5) as usize;
    let pages = rows.len().div_ceil(per).max(1);
    PAGE = PAGE.min(pages - 1);
    let rx = sw - my;
    tab(rx - 180, 180, b"Add top 10 here", false, Spot::AddTop);
    tab(rx - 180 - 12 - 60, 60, b">", false, Spot::Next);
    text_at(rx - 180 - 12 - 60 - 10, top, top + line + 6, f2, muted, format!("{} / {}", PAGE + 1, pages).as_bytes(), 2);
    tab(rx - 180 - 12 - 60 - 90 - 60, 60, b"<", false, Spot::Prev);
    // the list
    let ly = top + line + 18;
    let w = sw - my - m;
    let cols = [m + 40, m + w * 30 / 100, m + w * 42 / 100, m + w * 48 / 100, m + w * 76 / 100, m + w * 88 / 100];
    for (label, x) in [("Name", cols[0]), ("Position", cols[1]), ("Age", cols[2]), ("Club", cols[3]), ("Caps / goals", cols[4]), ("Rating", cols[5])] {
        text_at(x, ly, ly + line, f1, muted, label.as_bytes(), 0);
    }
    let mut y = ly + line + 2;
    for man in rows.iter().skip(PAGE * per).take(per) {
        fill(m, y, sw - my, y + line + 2, [0x1e, 0x21, 0x26]);
        // the + box (a tick once on the Short List)
        let b = (m + 6, y + 2, m + 6 + line - 2, y + line);
        let on_list = shortlisted(man.staff);
        fill(b.0, b.1, b.2, b.3, if on_list { [0x3e, 0xa8, 0x6a] } else { [0x3a, 0x5a, 0x86] });
        if on_list {
            draw_icon(0xE73E, (b.0 + b.2) / 2, b.1 + 2, b.3 - b.1 - 4, white);
        } else {
            text_at((b.0 + b.2) / 2, b.1, b.3, f3, white, b"+", 1);
            SPOTS.push((b, Spot::Add(man.staff)));
        }
        text_at(cols[0], y, y + line, f3, white, &clip_text(f3, &man.name, cols[1] - cols[0] - 80), 0);
        if man.in_squad {
            text_at(cols[1] - 70, y, y + line, f1, GOLD, b"SQUAD", 0);
        }
        text_at(cols[1], y, y + line, f2, GOLD, &man.positions, 0);
        text_at(cols[2], y, y + line, f2, white, man.age.to_string().as_bytes(), 0);
        text_at(cols[3], y, y + line, f2, white, &clip_text(f2, &man.club, cols[4] - cols[3] - 10), 0);
        text_at(cols[4], y, y + line, f2, white, format!("{} / {}", man.caps, man.goals).as_bytes(), 0);
        let c = if man.score >= 75 { [0x4c, 0xc0, 0x7a] } else if man.score >= 60 { GOLD } else { [0xe0, 0x88, 0x58] };
        text_at(cols[5], y, y + line, f3, c, format!("{}%", man.score).as_bytes(), 0);
        y += line + 4;
    }
    if rows.is_empty() {
        let waiting = remote::CLIENT && !KEY.is_empty();
        text_at((m + sw) / 2, ly + line * 3, ly + line * 4, f3, muted, if waiting { b"Asking the host...".as_slice() } else { b"No player of the country found" }, 1);
    }
    // Back: the full width at the bottom, as the game's screens have it
    // (user 2026-11-13 polish: at 1680 rounded, a 20 px margin; the click rect is this one)
    let edge = if small_screen() { my } else { 20 };
    let back = (if small_screen() { m } else { m.max(20) }, sh - edge - sh / 18, sw - edge, sh - edge);
    if small_screen() {
        fill(back.0, back.1, back.2, back.3, [0x2c, 0x46, 0x6a]);
    } else {
        rounded(back.0, back.1, back.2, back.3, [0x2c, 0x46, 0x6a], 6);
    }
    text_at((back.0 + back.2) / 2, back.1, back.3, f3, white, b"Back", 1);
    SPOTS.push((back, Spot::Back));
}

/// The mouse while the pool is open: it takes every click. Some(redraw).
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if !OPEN {
        return None;
    }
    // (the menu bar stays the game's: a click there closes the pool and goes on to the game)
    if mx < bar_end(screen().0) {
        if released {
            OPEN = false;
            MEN.clear();
        }
        return None;
    }
    *(LEFT_CLICK as *mut u32) = 0;
    *(assistant::RIGHT_CLICK as *mut u32) = 0;
    // (a client's list on its way: drawn again when something came)
    let came = remote::CLIENT && !KEY.is_empty() && (std::mem::take(&mut remote::ARRIVED) | remote::hold_over());
    if !released {
        return Some(came);
    }
    let Some(&(_, spot)) = SPOTS.iter().find(|s| (s.0 .0..=s.0 .2).contains(&mx) && (s.0 .1..=s.0 .3).contains(&my)) else { return Some(false) };
    match spot {
        Spot::Back => {
            OPEN = false;
            MEN.clear();
        }
        Spot::Filter(k) => (FILTER, PAGE) = (k, 0),
        Spot::NotInSquad => (NOT_IN_SQUAD, PAGE) = (!NOT_IN_SQUAD, 0),
        Spot::Prev => PAGE = PAGE.saturating_sub(1),
        Spot::Next => PAGE += 1,
        Spot::AddTop => {
            let top: Vec<usize> = shown().iter().filter(|m| !m.in_squad && !shortlisted(m.staff)).take(10).map(|m| m.staff).collect();
            if remote::CLIENT {
                // (one question for all ten: each add asks the whole list again)
                LISTED.extend(&top);
                ask(&top);
            } else {
                for staff in top {
                    add(staff);
                }
            }
        }
        Spot::Add(staff) => add(staff),
    }
    Some(true)
}
