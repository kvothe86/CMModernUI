//! Competition screens, modern (part of the `stats_table` switch): the league table with crests,
//! goal difference, flat headers and thin zone lines; the player and team stats rankings with
//! medals for the top three and the value drawn by the view (Average Rating a coloured pill, Goals
//! and the like a bar against the leader, Yellow / Red Cards as cards, Form as W / D / L pills);
//! the history screens: plain season numbers, crests by the winners, and record lines in colour
//! (the number gold, names white, dates grey) where the game has them, so their links still fit.
//! All from traces of the screens: a rank button ("1st") or a season button ("2031/2") at the left
//! of each row, names and values after it.

use crate::*;

/// The screen's title ("German Bundesliga") and the list's ("League Table", "Average Rating",
/// "Saturday 18th October" ...): when either changes, what was learnt of the list is dropped.
static mut SCREEN: Vec<u8> = Vec::new();
static mut VIEW: Vec<u8> = Vec::new();
/// The rank cells of the rows (top, bottom, left, right), the table's header columns (label,
/// left, right, top, bottom), its zone lines (tops), each row's For / Ag, the rows drawn this
/// frame, a stats view's values this frame (box, text, font), the label column of a records list.
static mut ROWS: Vec<(i32, i32, i32, i32)> = Vec::new();
static mut HEADS: Vec<(Vec<u8>, i32, i32, i32, i32)> = Vec::new();
static mut LINES: Vec<i32> = Vec::new();
static mut SCORES: Vec<(i32, i32, i32)> = Vec::new();
static mut DRAWN: Vec<i32> = Vec::new();
static mut VALUES: Vec<((i32, i32, i32, i32), Vec<u8>, u32)> = Vec::new();
static mut BEST: f32 = 0.0;
static mut LABEL_RIGHT: i32 = i32::MIN;

const HEADERS: [&[u8]; 7] = [b"Pld", b"Won", b"Drn", b"Lst", b"For", b"Ag", b"Pts"];

// The clubs' form, as the Team Stats tab writes it (comp_screens_draw_team_stats_tab 0x4956e0):
// the competition record is screen parameter 0, its Comp_Stats at +8.
const TEAM_ENTRIES: usize = 0x4a4b20; // Comp_Stats(this, stat, *count, 0x4b) -> new array of 0x76-byte entries, +0 club index
const FORMAT_TEAM_STAT: usize = 0x4a39c0; // stdcall(entry, stat, buf, size, detailed) -> 1 / 0
const TEAM_STAT_LABEL: usize = 0x4a2810; // stdcall(stat, buf, size) -> 1 / 0 ("Form")
const FREE: usize = 0x9452ca; // operator delete
/// The competition the forms are of, and each club's ("WWDLX").
static mut FORMS: (usize, Vec<(usize, Vec<u8>)>) = (0, Vec::new());

/// A club's form in the competition on screen (single player and the host: the game's own
/// writer; a network client none).
#[allow(static_mut_refs)]
unsafe fn form_of(club: usize) -> Option<Vec<u8>> {
    if remote::CLIENT {
        return None;
    }
    let comp = screen_param(0);
    if !mapped(comp, 12) {
        return None;
    }
    if FORMS.0 != comp {
        FORMS = (comp, Vec::new());
        let stats = rd::<usize>(comp + 8);
        let mut buf = [0u8; 64];
        let label = game::<extern "stdcall" fn(i32, *mut u8, i32) -> i32>(TEAM_STAT_LABEL);
        // (stats 1..=17: another number asserts, comp_stats.cpp 2104)
        let stat = (1..=17).find(|&s| label(s, buf.as_mut_ptr(), 64) != 0 && buf.split(|&c| c == 0).next() == Some(b"Form"));
        if let (Some(stat), true) = (stat, mapped(stats, 4)) {
            let mut count = 0i32;
            let entries = game::<extern "thiscall" fn(usize, i32, *mut i32, i32) -> usize>(TEAM_ENTRIES)(stats, stat, &mut count, 0x4b);
            let format = game::<extern "stdcall" fn(usize, i32, *mut u8, i32, i32) -> i32>(FORMAT_TEAM_STAT);
            for i in 0..count.clamp(0, 64) as usize {
                let e = entries + i * 0x76;
                let index = rd::<i32>(e);
                if index < 0 || format(e, stat, buf.as_mut_ptr(), 64, 1) == 0 {
                    continue;
                }
                let club = rd::<usize>(CLUB_TABLE) + index as usize * CLUB_SIZE;
                FORMS.1.push((club, buf.split(|&c| c == 0).next().unwrap_or(&[]).to_vec()));
            }
            if entries != 0 {
                game::<extern "C" fn(usize)>(FREE)(entries);
            }
        }
    }
    FORMS.1.iter().find(|f| f.0 == club).map(|f| f.1.clone())
}

/// The form of the club of this competition called `name`: more clubs can have that name, the
/// club table's first is another's ("Excelsior").
#[allow(static_mut_refs)]
unsafe fn form_named(name: &[u8], club: usize) -> Option<Vec<u8>> {
    let form = form_of(club);
    let named = |c: usize, at: usize, max: usize| std::slice::from_raw_parts((c + at) as *const u8, max).split(|&b| b == 0).next() == Some(name);
    form.or_else(|| FORMS.1.iter().find(|f| named(f.0, 0x38, 26) || named(f.0, 4, 51)).map(|f| f.1.clone()))
}

/// Form pills (W green, D grey, L red, X dim), `n` wide, from x.
unsafe fn form_pills(x: i32, y1: i32, y2: i32, bw: i32, font: u32, t: &[u8]) {
    let mut x = x;
    for &c in t {
        let colour = match c {
            b'W' => [0x3e, 0xa8, 0x6a],
            b'D' | b'X' => [0x4c, 0x8c, 0xe0], // (X: a draw too)
            b'L' => [0xd0, 0x50, 0x48],
            _ => [0x2c, 0x30, 0x36],
        };
        pill(x, y1 + 3, x + bw - 3, y2 - 3, colour);
        {
            text_at(x + (bw - 3) / 2, y1, y2, font, WHITE, &[if c == b'X' { b'D' } else { c }], 1);
        }
        x += bw;
    }
}

/// How many rows, header columns, zone lines and scores were known when this full frame began.
static mut START: [usize; 4] = [0; 4];

/// A new screen or list: what was learnt before this frame is dropped. (The game draws a screen's
/// list before its titles, so what this frame learnt already belongs to the new list; a scroll
/// later redraws the rows without the header the table is known by.)
#[allow(static_mut_refs)]
unsafe fn forget() {
    if trace() {
        log(&format!("comp forget: screen {:?} view {:?} start {START:?} heads {}", latin1(&SCREEN), latin1(&VIEW), HEADS.len()));
    }
    ROWS.drain(..START[0].min(ROWS.len()));
    HEADS.drain(..START[1].min(HEADS.len()));
    LINES.drain(..START[2].min(LINES.len()));
    SCORES.drain(..START[3].min(SCORES.len()));
    START = [0; 4];
    BEST = 0.0;
    LABEL_RIGHT = i32::MIN;
    FORMS.0 = 0; // (the results go on: asked again when a list opens)
}

/// A full frame begins (the background hook).
#[allow(static_mut_refs)]
pub unsafe fn full_frame() {
    START = [ROWS.len(), HEADS.len(), LINES.len(), SCORES.len()];
    IN_FULL = true;
    TITLES_SEEN = (false, false);
    DATE_BUTTONS = false; // (drawn again before the rows when the screen has them)
}

/// A full frame is being drawn (its flip pass not yet come).
static mut IN_FULL: bool = false;
/// The screen title and the view title were taken since the last full frame began: only the first
/// of each counts (a list of wide rows, the saved games of Restore / Save Game, looks like view
/// titles; taking each one redrew the screen without end, and the file name box took no typing).
static mut TITLES_SEEN: (bool, bool) = (false, false);

/// A row of a list in a full frame with no header drawn before it in that frame: no table (Results,
/// a stats view switched to from the table): the old header goes at once.
#[allow(static_mut_refs)]
unsafe fn row_seen() {
    // (nothing: a scroll is a full frame without the header too; a new title's extra redraw
    // (REDRAW) puts right the first frame of a screen switched to)
}

fn ordinal(t: &[u8]) -> Option<i32> {
    let digits = t.iter().take_while(|c| c.is_ascii_digit()).count();
    let rest = &t[digits..];
    (digits > 0 && [&b"st"[..], b"nd", b"rd", b"th"].contains(&rest)).then(|| std::str::from_utf8(&t[..digits]).ok()?.parse().ok()).flatten()
}

/// "2031" or "2031/2": a season.
fn season(t: &[u8]) -> bool {
    t.len() >= 4 && t[..4].iter().all(|c| c.is_ascii_digit()) && (t.len() == 4 || (t[4] == b'/' && t[5..].iter().all(|c| c.is_ascii_digit())))
}

#[allow(static_mut_refs)]
fn table() -> bool {
    unsafe { HEADS.iter().any(|h| h.0 == b"Pts") }
}

/// A list titled by a date (Results, Fixtures): its positions are no ranking.
#[allow(static_mut_refs)]
fn dated() -> bool {
    let days = [&b"Monday"[..], b"Tuesday", b"Wednesday", b"Thursday", b"Friday", b"Saturday", b"Sunday"];
    // (also "Evening Fixtures", "Latest Results": the day's lists, whose positions are no medals)
    let named = |w: &[u8]| unsafe { VIEW.windows(w.len()).any(|x| x == w) };
    unsafe { DATE_BUTTONS || days.iter().any(|d| VIEW.starts_with(d)) || named(b"Fixture") || named(b"Result") }
}

/// The "<< Date" / "Date >>" buttons of Results and Fixtures were drawn (before the rows: the
/// list's date title comes after them).
static mut DATE_BUTTONS: bool = false;

/// A zone line between rows of the table (a dashed row): a thin line instead. True: drawn here.
#[allow(static_mut_refs)]
pub unsafe fn panel(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32) -> bool {
    if flags != 0x6400 || !table() {
        return false;
    }
    let had = LINES.len();
    LINES.retain(|&l| l != y1);
    START[2] = START[2].saturating_sub(had - LINES.len());
    if LINES.len() < 16 {
        LINES.push(y1);
    }
    fill(x1, y1, x2, y2, AREA);
    fill(x1 + 4, (y1 + y2) / 2, x2 - 4, (y1 + y2) / 2 + 1, [0x3a, 0x3f, 0x47]);
    true
}

/// A record line in colour, each word where the game draws it (its links stay over it): numbers
/// gold, dates and joining words grey, a club in brackets light, the rest white.
unsafe fn record_line(x1: i32, y1: i32, y2: i32, font: u32, t: &[u8]) {
    let mut at = 0;
    let words: Vec<&[u8]> = t.split(|&c| c == b' ').collect();
    for (k, &w) in words.iter().enumerate() {
        if !w.is_empty() {
            let digits = w.iter().filter(|c| c.is_ascii_digit()).count();
            let date = digits > 0 && (w.iter().filter(|&&c| c == b'.').count() == 2 || (season(w) && w.len() > 4));
            // (the record's own number: first in the line or after " - ", and no name after it:
            // "78 - 1860 München" has a club called 1860)
            let prev = words[..k].iter().rev().find(|p| !p.is_empty()).copied().unwrap_or(b"-");
            let next_name = words.get(k + 1).and_then(|n| n.first()).is_some_and(|c| c.is_ascii_uppercase() || *c >= 0xc0);
            let number = digits > 0 && w.iter().all(|c| c.is_ascii_digit() || b",.-%".contains(c)) && prev == b"-" && !next_name;
            let colour = if date {
                GREY
            } else if number {
                GOLD
            } else if [&b"-"[..], b"v", b"to", b"/", b"(H)", b"(A)"].contains(&w) {
                GREY
            } else if w.starts_with(b"(") || w.ends_with(b")") {
                LIGHT
            } else {
                WHITE
            };
            // (the game's measure leaves out trailing spaces: the words before, then each space)
            let before = &t[..at];
            let spaces = before.len() - before.trim_ascii_end().len();
            let space = text_width(font, b"a a") - text_width(font, b"aa");
            text_at(x1 + text_width(font, before.trim_ascii_end()) + spaces as i32 * space, y1, y2, font, colour, w, 0);
        }
        at += w.len() + 1;
    }
}

/// The list on screen has this title band (the first frame of a list: the one before; a new
/// title redraws).
#[allow(static_mut_refs)]
pub unsafe fn view_is(title: &[u8]) -> bool {
    VIEW == title
}

/// A profile left its History tab's Playing Career for a tab without a list title.
#[allow(static_mut_refs)]
pub unsafe fn leave_career() {
    if VIEW == b"Playing Career" {
        VIEW.clear();
    }
}

/// The cells of a club's Honours list this frame (left, top, bottom, font, text as drawn).
static mut HONOUR_CELLS: Vec<(i32, i32, i32, u32, Vec<u8>)> = Vec::new();

/// The years of an honour ("1984, 1997") and how many; a long list goes on in the rows under it.
fn years(t: &[u8]) -> Option<usize> {
    let t = t.trim_ascii();
    (t.first()?.is_ascii_digit() && t.iter().all(|c| c.is_ascii_digit() || b" ,/-".contains(c))).then(|| t.split(|&c| c == b',').filter(|y| !y.trim_ascii().is_empty()).count())
}

/// Over the game's Honours list: a cup in the place's colour before Winners, Runners Up and Third
/// Placed, and after them how many times.
#[allow(static_mut_refs)]
unsafe fn honours() {
    let mut cells = std::mem::take(&mut HONOUR_CELLS);
    cells.sort_by_key(|c| (c.1, c.0));
    let mut rows: Vec<(usize, u8, usize)> = Vec::new(); // the label's cell, the place, how many
    let mut open = false; // (the last label seen was a place)
    for (k, cell) in cells.iter().enumerate() {
        let Some(n) = years(&cell.4) else { continue };
        let label = cells[..k].iter().rposition(|c| c.1 == cell.1 && years(&c.4).is_none());
        let place = label.and_then(|l| [&b"Winners"[..], b"Runners Up", b"Third Placed"].iter().position(|p| *p == cells[l].4.trim_ascii()));
        match (label, place) {
            (Some(l), Some(p)) => {
                rows.push((l, p as u8, n));
                open = true;
            }
            // (no label: the row goes on with the years above, if those were a place's)
            (None, _) => {
                if let Some(r) = rows.last_mut().filter(|_| open) {
                    r.2 += n;
                }
            }
            // (a label that is no place: the rows under it belong to nothing)
            _ => open = false,
        }
    }
    for (l, place, n) in rows {
        let (x1, y1, y2, font, t) = &cells[l];
        let size = (y2 - y1) * 80 / 100;
        trophy(x1 - size - 8, y1 + (y2 - y1 - size) / 2, size, [GOLD, [0xb8, 0xbc, 0xc4], [0xc0, 0x80, 0x4c]][place as usize]);
        text_at(x1 + text_width(*font, t.trim_ascii_end()) + 12, *y1, *y2, *font, GOLD, format!("{n}x").as_bytes(), 0);
    }
}

/// A small cup.
unsafe fn trophy(x: i32, y: i32, s: i32, c: [u8; 3]) {
    let (cx, stem) = (x + s / 2, (s / 10).max(1));
    fill(x + s / 5, y, x + s * 4 / 5, y + s * 2 / 5, c); // the bowl
    dot(cx, y + s * 2 / 5, s * 3 / 10, c);
    for side in [x, x + s * 4 / 5] {
        fill(side, y + s / 10, side + s / 5, y + s / 10 + stem, c); // the handles
        fill(side, y + s * 3 / 10, side + s / 5, y + s * 3 / 10 + stem, c);
    }
    fill(x, y + s / 10, x + stem, y + s * 3 / 10 + stem, c);
    fill(x + s - stem, y + s / 10, x + s, y + s * 3 / 10 + stem, c);
    fill(cx - stem, y + s * 3 / 5, cx + stem, y + s * 4 / 5, c); // the stem and the foot
    fill(x + s / 4, y + s * 4 / 5, x + s * 3 / 4, y + s * 9 / 10, c);
}

/// An arrow `s` high, up or down, around cx.
unsafe fn arrow(cx: i32, top: i32, s: i32, up: bool, c: [u8; 3]) {
    for row in 0..s {
        let half = if row < s / 2 { row + 1 } else { (s / 8).max(1) };
        let y = if up { top + row } else { top + s - 1 - row };
        fill(cx - half, y, cx + half, y + 1, c);
    }
}

/// A text of a competition screen. True: drawn here.
#[allow(static_mut_refs)]
pub unsafe fn text(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, colour: u32, raw: &[u8]) -> bool {
    let t = raw.trim_ascii();
    let (w, h) = (x2 - x1, y2 - y1);
    let (_, sh) = screen();
    // the screen's title, the list's (never an empty box: a scroll draws wide empty ones)
    if t.is_empty() {
        return false;
    }
    if w > 1000 && y1 < sh / 8 && font & 0xffff >= 5 {
        if !std::mem::replace(&mut TITLES_SEEN.0, true) && SCREEN != t {
            SCREEN = t.to_vec();
            REDRAW = true; // (drawn again, now knowing the screen)
            forget();
        }
        return false;
    }
    // (in the upper half: the Back button at the bottom is as wide and tall)
    if flags == 0xc && w > 1000 && (40..=90).contains(&h) && y1 < sh / 2 {
        if !std::mem::replace(&mut TITLES_SEEN.1, true) && VIEW != t {
            VIEW = t.to_vec();
            REDRAW = true;
            forget();
        }
        // (the buttons count until a list without them: its title then comes without them)
        return false;
    }
    // (a dropdown over the list: its rows start with five spaces and go to `menu_row`)
    if h > 40 || t.is_empty() || raw.starts_with(b"     ") {
        return false;
    }
    // (only `flip` empties the cells, and it runs with the stats table)
    if on(HONOURS) && on(STATS_TABLE) && VIEW == b"Honours" && SCREEN.ends_with(b" History") && HONOUR_CELLS.len() < 256 {
        HONOUR_CELLS.push((x1, y1, y2, font, raw.to_vec()));
    }
    // Latest Scores (the screen, and a match's tab "<league> Latest"): a crest by each club, the
    // score gold, and under a club its scorers, injuries, red cards and missed penalties, each
    // with its icon and the minutes gold (a row: "  Allbäck 22", "  Venema injured 27")
    if SCREEN == b"Latest Scores" || VIEW.ends_with(b" Latest") {
        if flags == 0xc && w < 200 && (t == b":" || t.iter().all(|c| c.is_ascii_digit())) {
            let c = std::ffi::CString::new(raw).unwrap_or_default();
            game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1, x2, y2, flags, font, rgb(GOLD) as u32, c.as_ptr(), -1);
            return true;
        }
        if flags & 0x7 == 0x1 && w > 300 && raw.starts_with(b"  ") {
            let size = h - 4;
            if !t.last().is_some_and(|c| c.is_ascii_digit()) {
                // (a club without a crest in the pack: its name where the others' are)
                if !crest_name(x1, y1, x2, y2, flags, font, colour, raw) {
                    let c = std::ffi::CString::new(raw).unwrap_or_default();
                    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1 + size + 10, y1, x2, y2, flags, font, colour, c.as_ptr(), -1);
                }
                return true;
            }
            let words: Vec<&[u8]> = t.split(|&c| c == b' ').collect();
            let at = words.iter().position(|w| w.first().is_some_and(|c| c.is_ascii_digit()) || *w == b"pen" || *w == b"og").unwrap_or(words.len());
            // (the icon tells what happened: the words for it go)
            let said = [&b"injured"[..], b"sent", b"off", b"m/pen", b"missed"];
            let name = words[..at].iter().filter(|w| !said.contains(w)).copied().collect::<Vec<_>>().join(&b' ');
            let c = event_icon(x1 + 4 + size / 2, (y1 + y2) / 2, size * 36 / 100, t);
            // (under the club's name, which keeps the game's two leading spaces)
            let x = x1 + size + 10 + text_width(font, b"  a") - text_width(font, b"a");
            text_at(x, y1, y2, font, c, &name, 0);
            text_at(x + text_width(font, &name) + text_width(font, b"a a") - text_width(font, b"aa"), y1, y2, font, GOLD, &words[at..].join(&b' '), 0);
            return true;
        }
    }
    // a search's Rec column ("****", the scout's stars): star icons, the five a row has, gold
    if (1..=5).contains(&t.len()) && t.iter().all(|&c| c == b'*') && x1 < 300 && w < 160 {
        fill(x1, y1, x2 + 1, y2 + 1, AREA);
        let size = ((w - 12) / 5 - 4).min(h - 8).max(6);
        stars(x1 + (w - (5 * size + 16)) / 2, y1 + (h - size) / 2, size, t.len() as i32 * 40, GOLD);
        return true;
    }
    if t == b"<< Date" || t == b"Date >>" {
        DATE_BUTTONS = true;
        return false;
    }
    // the league table's boxes: champions a cup, promoted and relegated an arrow, play-offs both
    if on(TABLE_ICONS) && table() && (16..100).contains(&w) && [&b"C"[..], b"Pl", b"P", b"R"].contains(&t) {
        fill(x1 - 2, y1 - 2, x2 + 3, y2 + 3, AREA);
        let s = (h * 7 / 10).max(8);
        let (cx, top) = ((x1 + x2) / 2, y1 + (h - s) / 2);
        match t {
            b"C" => trophy(cx - s / 2, top, s, GOLD),
            b"P" => arrow(cx, top, s, true, [0x46, 0xc8, 0x6e]),
            b"R" => arrow(cx, top, s, false, [0xe8, 0x4c, 0x4c]),
            _ => {
                arrow(cx - s / 2, top, s, true, [0xe8, 0x9a, 0x2c]);
                arrow(cx + s / 2, top, s, false, [0xe8, 0x9a, 0x2c]);
            }
        }
        return true;
    }
    // the table's header and the history's: flat, grey
    let more = (t == b"Third Place" || t == b"Hosts") && w > 100 && SCREEN.ends_with(b"History");
    if (HEADERS.contains(&t) && w < 160) || ((t == b"Winners" || t == b"Runners-up") && w > 300) || more {
        let had = HEADS.len();
        HEADS.retain(|c| c.0 != t);
        START[1] = START[1].saturating_sub(had - HEADS.len());
        HEADS.push((t.to_vec(), x1, x2, y1, y2));
        fill(x1, y1, x2 + 1, y2 + 1, AREA);
        text_at((x1 + x2) / 2, y1, y2, font, GREY, t, 1);
        if t == b"Pld" {
            text_at(x1 - 12, y1, y2, font, GREY, b"GD", 2);
            text_at(x1 - 70 - 5 * 22 / 2 - 10, y1, y2, font, GREY, b"Form", 1);
        }
        return true;
    }
    // (the history screens only: "Bundesliga History", a club's "... History")
    let history = SCREEN.ends_with(b"History") || VIEW == b"Playing Career";
    // a season at the left of a history row: a plain number
    if history && season(t) && w < 200 && flags == 0xc {
        row_seen();
        let had = ROWS.len();
        ROWS.retain(|r| !(r.0 == y1 && r.2 == x1));
        START[0] = START[0].saturating_sub(had - ROWS.len());
        if ROWS.len() < 64 {
            ROWS.push((y1, y2, x1, x2));
        }
        // (the season a profile's Playing Career shows below is a white button with dark text:
        // gold on the selection colour)
        let c = colour as u16 as u32;
        let dark = (c >> 11 & 31) + (c >> 5 & 63) / 2 + (c & 31) < 30;
        fill(x1, y1, x2 + 1, y2 + 1, if dark { SELECTED } else { AREA });
        text_at(x1 + 12, y1, y2, font, if dark { GOLD } else { GREY }, t, 0);
        if let Ok(year) = std::str::from_utf8(&t[..4]).unwrap_or("").parse::<i32>() {
            SEASONS.retain(|s| s.0 != y1);
            if SEASONS.len() < 64 {
                SEASONS.push((y1, year));
            }
        }
        return true;
    }
    // a records list: the label grey, the value line in colour with the crest it names before it
    if history && flags == 0x1 && raw.starts_with(b"  ") && x1 < 200 && w > 300 && !stats_label(t) {
        LABEL_RIGHT = x2;
        text_at(x1 + 8, y1, y2, font, GREY, t.trim_ascii(), 0);
        return true;
    }
    let after_label = LABEL_RIGHT != i32::MIN && (x1 - LABEL_RIGHT - 2).abs() <= 2;
    let after_season = ROWS.iter().any(|r| r.0 == y1 && x1 > r.3 && x1 - r.3 < 20) && !HEADS.iter().any(|h| h.0 == b"Winners");
    if history && flags == 0x1 && w > 300 && (after_label || after_season) {
        // (a club in brackets: "Rory McKenna (Augsburg) - 10")
        let plain: Vec<u8> = t.iter().map(|&c| if c == b'(' || c == b')' { b' ' } else { c }).collect();
        if let Some(key) = crests_named(&plain).into_iter().next().or_else(|| crest_key(&plain)) {
            let size = h - 4;
            draw_crest(&key, x1 - size - 6, y1 + 2, size);
        }
        record_line(x1, y1, y2, font, raw);
        return true;
    }
    // Results / Fixtures: a club's league position as plain grey text ("12th", no ranking)
    if ordinal(t).is_some() && w < 160 && dated() {
        row_seen();
        let had = ROWS.len();
        ROWS.retain(|r| !(r.0 == y1 && r.2 == x1));
        START[0] = START[0].saturating_sub(had - ROWS.len());
        if ROWS.len() < 64 {
            ROWS.push((y1, y2, x1, x2));
        }
        fill(x1, y1, x2 + 1, y2 + 1, AREA);
        text_at((x1 + x2) / 2, y1, y2, font, GREY, t, 1);
        return true;
    }
    // ... and the score gold, "v" grey (the score comes in three texts in one box, "5    ", ":",
    // "    2": each in its colour, the box not filled)
    if dated() && ROWS.iter().any(|r| r.0 == y1) && w < 200 && (t == b"v" || t == b":" || (!t.is_empty() && t.iter().all(|c| c.is_ascii_digit()))) && flags == 0xc {
        let c = std::ffi::CString::new(raw).unwrap_or_default();
        let colour = rgb(if t == b"v" { GREY } else { GOLD }) as u32;
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1, x2, y2, flags, font, colour, c.as_ptr(), -1);
        return true;
    }
    // ... and the clubs' crests (before any table logic: a table's header may still be known)
    if dated() && flags & 0x7 == 0x1 && w > 200 && ROWS.iter().any(|r| r.0 == y1 && x1 > r.3) {
        return crest_name(x1, y1, x2, y2, flags, font, colour, raw);
    }
    // the rank: a plain number (a medal for a stats view's first three)
    if let Some(n) = ordinal(t).filter(|_| w < 160 && !dated()) {
        row_seen();
        let had = ROWS.len();
        ROWS.retain(|r| !(r.0 == y1 && r.2 == x1));
        START[0] = START[0].saturating_sub(had - ROWS.len());
        if ROWS.len() < 64 {
            ROWS.push((y1, y2, x1, x2));
        }
        DRAWN.push(y1);
        fill(x1, y1, x2 + 1, y2 + 1, AREA);
        let label = n.to_string();
        if !table() && n <= 3 {
            let c = [GOLD, [0xb8, 0xbc, 0xc4], [0xc0, 0x80, 0x4c]][n as usize - 1];
            dot((x1 + x2) / 2, (y1 + y2) / 2, h * 42 / 100, c);
            text_at((x1 + x2) / 2, y1, y2, font, [0x14, 0x16, 0x1a], label.as_bytes(), 1);
        } else {
            text_at((x1 + x2) / 2, y1, y2, font, LIGHT, label.as_bytes(), 1);
        }
        return true;
    }
    let Some(&(_, _, _, rx2)) = ROWS.iter().find(|r| r.0 == y1) else { return false };
    if x1 <= rx2 {
        return false;
    }
    // the history's winners and runners-up: the crest before the name; a winner's titles so far
    // after it, "(3)"
    if let Some(head) = HEADS.iter().find(|c| c.0 == b"Winners") {
        if flags & 0xf != 0x1 || w <= 200 || !crest_name(x1, y1, x2, y2, flags, font, colour, raw) {
            return false;
        }
        let year = SEASONS.iter().find(|s| s.0 == y1).map(|s| s.1);
        if trace() {
            log(&format!("winner {:?} y {y1} year {year:?} club {:?} comp {:#x} seasons {}", latin1(t), club_named(t), screen_param(0), SEASONS.len()));
        }
        // (the winner's cell: the left half's; the header's box is the label's own width only the
        // first frame after the screen opens, which the winners' names start left of)
        let _ = head;
        if let (true, Some(year), Some(club)) = (x1 < screen().0 * 45 / 100, year, club_named(t)) {
            if let Some(n) = titles(screen_param(0), club, year) {
                let x = x1 + (y2 - y1 - 4) + 10 + text_width(font, raw.trim_ascii_end()) + 8;
                text_at(x, y1, y2, font, GREY, format!("({n})").as_bytes(), 0);
            }
        }
        return true;
    }
    let value = number(t);
    if table() {
        let col = HEADS.iter().find(|c| x1 >= c.1 - 2 && x2 <= c.2 + 2).map(|c| c.0.clone());
        match col.as_deref() {
            Some(b"Pts") => {
                fill(x1, y1, x2 + 1, y2 + 1, [0x26, 0x2a, 0x31]);
                text_at((x1 + x2) / 2, y1, y2, font, GOLD, t, 1);
                return true;
            }
            Some(c @ (b"For" | b"Ag")) => {
                if let Some(v) = value {
                    let row = match SCORES.iter().position(|s| s.0 == y1) {
                        Some(i) => &mut SCORES[i],
                        None => {
                            SCORES.push((y1, i32::MIN, i32::MIN));
                            SCORES.last_mut().unwrap()
                        }
                    };
                    if c == b"For" {
                        row.1 = v as i32;
                    } else {
                        row.2 = v as i32;
                    }
                    // the goal difference at once (a table redrawn alone has no flip pass)
                    let (f, a) = (row.1, row.2);
                    if let (true, Some(pld)) = (f != i32::MIN && a != i32::MIN, HEADS.iter().find(|h| h.0 == b"Pld")) {
                        let x = pld.1 - 12;
                        let small = SMALL_FONT.unwrap_or(0xffff0001);
                        fill(x - 50, y1, x + 2, y2, AREA);
                        let d = f - a;
                        let c = if d > 0 { [0x4c, 0xc0, 0x7a] } else if d < 0 { [0xe0, 0x60, 0x58] } else { GREY };
                        let s = if d > 0 { format!("+{d}") } else { d.to_string() };
                        text_at(x, y1, y2, small, c, s.as_bytes(), 2);
                    }
                }
                return false;
            }
            Some(_) => return false,
            None => {}
        }
        // the club: its crest before the name, its form at the right of the cell (before GD)
        if flags & 0xf == 0x1 && w > 200 {
            let drawn = crest_name(x1, y1, x2, y2, flags, font, colour, raw);
            if let (Some(club), Some(pld)) = (club_named(t.trim_ascii()), HEADS.iter().find(|h| h.0 == b"Pld")) {
                if let Some(f) = form_named(t.trim_ascii(), club) {
                    let bw = (h + 2).min(22);
                    let small = SMALL_FONT.unwrap_or(0xffff0001);
                    form_pills(pld.1 - 70 - f.len() as i32 * bw, y1, y2, bw, small, &f);
                }
            }
            return drawn;
        }
        return false;
    }
    // a stats view: the club's crest; the value (or the form) kept for the flip pass, which knows
    // this frame's title (the rows of a view just switched to come before its title)
    // (a team stats list writes its clubs like player names, flags 0x9)
    if flags & 0x7 == 0x1 && w > 200 && crest_key(t).is_some() {
        return crest_name(x1, y1, x2, y2, flags, font, colour, raw);
    }
    let form = t.len() >= 3 && t.iter().all(|c| b"WDLX".contains(c));
    // (the wide last column only: a table's own cells, 113 px, drawn once more while the view
    // switches to a stats list, are no stats values)
    if flags == 0xc && (140..260).contains(&w) && (value.is_some() || form) {
        // drawn now by the title known so far (a list redrawn alone, a scroll, comes without a
        // flip pass), and again in the flip pass by this frame's
        BEST = BEST.max(value.unwrap_or(0.0));
        value_cell((x1, y1, x2, y2), t, font, BEST);
        if VALUES.len() < 64 {
            VALUES.push(((x1, y1, x2, y2), t.to_vec(), font));
        }
        return true;
    }
    false
}

/// The seasons of a history list's rows (top, first year), for the winners' title counts.
static mut SEASONS: Vec<(i32, i32)> = Vec::new();

const HISTORY: usize = 0xae23d4; // the competitions' season results, 26 bytes a row (0x4afd80):
const HISTORY_COUNT: usize = 0xae237c; // +4 ClubComp*, +8 i16 season, +0xa winner, +0xe runner-up

pub const ADD_HISTORY: usize = 0x4afce0; // comp_util_add_history_record(comp, winner, runner-up, third, extra), cdecl
pub const HISTORY_WINNERS: usize = 0x4991f0; // comp_screens_draw_history_winners_tab(comp, top, bottom), cdecl
pub static mut ADD_HISTORY_ORIG: usize = 0;
pub static mut HISTORY_WINNERS_ORIG: usize = 0;
const SEASON: usize = 0xae2c92; // the current season's year (i16), as add_history stores it

/// A competition's season result: not stored a second time. (The game wrote the Johan Cruijff
/// Schaal 2025 twice after PSV won it, user 2026-10-29: Cup::update_for_date records a finished
/// cup once by its +0x4c flag, but a cup paged back in from its CM3_TEMP file can come back
/// without it, `save_cup_to_disk` skipping unchanged cups.)
pub extern "C" fn add_history(comp: usize, winner: usize, runner_up: usize, third: usize, extra: usize) {
    unsafe {
        let (base, count) = (rd::<usize>(HISTORY), rd::<i32>(HISTORY_COUNT));
        let (cc, season) = (if mapped(comp, 8) { rd::<usize>(comp + 4) } else { 0 }, rd::<i16>(SEASON));
        if cc != 0 && count > 0 && readable(base, count as usize * 26) && (0..count as usize).map(|k| base + k * 26).any(|r| rd::<usize>(r + 4) == cc && rd::<i16>(r + 8) == season && rd::<usize>(r + 0xa) == winner) {
            log(&format!("history: {season} result of {cc:#x} already stored, not again"));
            return;
        }
        game::<extern "C" fn(usize, usize, usize, usize, usize)>(ADD_HISTORY_ORIG)(comp, winner, runner_up, third, extra)
    }
}

/// The history screen's winners tab: a competition's rows stored twice before (same season,
/// winner and runner-up) are let go first, the later copy (its slot freed, +4 = 0).
pub extern "C" fn history_winners(comp: usize, top: i32, bottom: i32) {
    unsafe {
        let (base, count) = (rd::<usize>(HISTORY), rd::<i32>(HISTORY_COUNT));
        let cc = if mapped(comp, 8) { rd::<usize>(comp + 4) } else { 0 };
        if !remote::CLIENT && cc != 0 && count > 0 && readable(base, count as usize * 26) {
            let mut seen = Vec::new();
            for r in (0..count as usize).map(|k| base + k * 26).filter(|&r| [cc, comp].contains(&rd::<usize>(r + 4))) {
                let key = (rd::<i16>(r + 8), rd::<usize>(r + 0xa), rd::<usize>(r + 0xe));
                if seen.contains(&key) {
                    *((r + 4) as *mut usize) = 0;
                    log(&format!("history: {} result of {cc:#x} stored twice, the copy let go", key.0));
                } else {
                    seen.push(key);
                }
            }
        }
        game::<extern "C" fn(usize, i32, i32)>(HISTORY_WINNERS_ORIG)(comp, top, bottom)
    }
}

/// Host: the history section of a client's block (`NETWORK_PLAN.md`): the winner rows of the
/// competition the screen being sent is about (screen parameter 0, read for the client's channel
/// while its screen is built), each season (i16), winner (u32) and his titles by then (u16); a
/// count first. Empty for a screen about no competition.
pub unsafe fn history_block(channel: usize) -> Vec<u8> {
    let mut out = vec![0u8, 0];
    // (only a channel that is served and has a screen: asked otherwise the screen manager stops
    // the game, as in host_answer's 'P')
    let node = SCREEN_MANAGER + 0x300 * channel + 0x7e2;
    if *((SCREEN_MANAGER + 0x382a) as *const i16) != channel as i16 || !mapped(node, 4) || *(node as *const usize) == 0 {
        return out;
    }
    let comp = screen_param(0);
    let (base, count) = (rd::<usize>(HISTORY), rd::<i32>(HISTORY_COUNT));
    if !mapped(comp, 8) || count <= 0 || !readable(base, count as usize * 26) {
        return out;
    }
    let cc = rd::<usize>(comp + 4);
    if cc == 0 {
        return out;
    }
    let rows: Vec<(i16, usize)> = (0..count as usize).map(|k| base + k * 26).filter(|&r| [cc, comp].contains(&rd::<usize>(r + 4)) && rd::<usize>(r + 0xa) != 0).map(|r| (rd::<i16>(r + 8), rd::<usize>(r + 0xa))).collect();
    let mut n = 0u16;
    for &(season, winner) in rows.iter().take(400) {
        let titles = rows.iter().filter(|r| r.1 == winner && r.0 <= season).count();
        out.extend(season.to_le_bytes());
        out.extend((winner as u32).to_le_bytes());
        out.extend((titles as u16).to_le_bytes());
        n += 1;
    }
    out[..2].copy_from_slice(&n.to_le_bytes());
    out
}

/// How many times a club had won a competition (`comp` a Comp, its ClubComp at +4) by the season
/// starting in `year` (stored as that year or the next): None when that title is not in the table.
unsafe fn titles(comp: usize, club: usize, year: i32) -> Option<usize> {
    // (a network client: from the block the host sent with this screen, `history_block`)
    if remote::CLIENT {
        let b = news::block_tail();
        let n = if b.len() >= 2 { u16::from_le_bytes([b[0], b[1]]) as usize } else { 0 };
        // (the row's own season first, as the host's lookup: a club that won the next season too
        // got that season's count, "PSV (26)" twice)
        let rows = b.get(2..2 + n * 8).unwrap_or(&[]);
        return [year, year + 1].into_iter().find_map(|s| {
            rows.chunks_exact(8).find(|e| i16::from_le_bytes([e[0], e[1]]) as i32 == s && u32::from_le_bytes(e[2..6].try_into().unwrap()) as usize == club)
        }).map(|e| u16::from_le_bytes([e[6], e[7]]) as usize);
    }
    let (base, count) = (rd::<usize>(HISTORY), rd::<i32>(HISTORY_COUNT));
    if !mapped(comp, 8) || count <= 0 || !readable(base, count as usize * 26) {
        return None;
    }
    // (the screen's parameter is the Comp, its ClubComp at +4; opened from a league table's History
    // button perhaps the ClubComp itself: rows of either, pointers both)
    let cc = rd::<usize>(comp + 4);
    if cc == 0 {
        return None; // (rows of a freed slot have +4 = 0)
    }
    let won: Vec<i32> = (0..count as usize).map(|k| base + k * 26).filter(|&r| [cc, comp].contains(&rd::<usize>(r + 4)) && rd::<usize>(r + 0xa) == club).map(|r| rd::<i16>(r + 8) as i32).collect();
    if trace() && won.is_empty() {
        let rows: Vec<(usize, i16)> = (0..count as usize).map(|k| base + k * 26).filter(|&r| rd::<usize>(r + 0xa) == club && [year, year + 1].contains(&(rd::<i16>(r + 8) as i32))).map(|r| (rd::<usize>(r + 4), rd::<i16>(r + 8))).collect();
        log(&format!("titles: comp {comp:#x} cc {cc:#x} club {club:#x} {year}: rows of the club then {rows:x?}"));
    }
    let season = [year, year + 1].into_iter().find(|s| won.contains(s))?;
    Some(won.iter().filter(|&&s| s <= season).count())
}

/// The crest key of a club named as shown ("Celta"): the name itself, else the full name of the
/// club the game knows by it.
unsafe fn crest_key(name: &[u8]) -> Option<Vec<u8>> {
    let name = name.trim_ascii();
    if name.is_empty() {
        return None;
    }
    if has_crest(name) {
        return Some(name.to_vec());
    }
    // (club_named reads every club: once a name)
    static mut KNOWN: Vec<(Vec<u8>, Option<Vec<u8>>)> = Vec::new();
    #[allow(static_mut_refs)]
    if let Some(k) = KNOWN.iter().find(|k| k.0 == name) {
        return k.1.clone();
    }
    let key = club_named(name).filter(|&c| mapped(c, 0x40)).map(|c| game_text(c + 4, 51)).filter(|k| has_crest(k));
    #[allow(static_mut_refs)]
    {
        if KNOWN.len() > 1000 {
            KNOWN.clear();
        }
        KNOWN.push((name.to_vec(), key.clone()));
    }
    key
}

/// A club's name with its crest before it.
unsafe fn crest_name(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, colour: u32, t: &[u8]) -> bool {
    let size = y2 - y1 - 4;
    if !crest_key(t).is_some_and(|k| draw_crest(&k, x1 + 4, y1 + 2, size)) {
        return false;
    }
    let c = std::ffi::CString::new(t).unwrap_or_default();
    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1 + size + 10, y1, x2, y2, flags, font, colour, c.as_ptr(), -1);
    true
}

/// A stats value: "12", "7.25", or appearances as "starts (subs)", "0 (1)", counted together.
fn number(t: &[u8]) -> Option<f32> {
    let s = std::str::from_utf8(t).ok()?.trim();
    // (digits only: "nan", "inf" are names, not numbers)
    if !s.as_bytes().first()?.is_ascii_digit() {
        return None;
    }
    match s.split_once(" (") {
        Some((a, b)) => Some(a.trim().parse::<f32>().ok()? + b.strip_suffix(')')?.trim().parse::<f32>().ok()?),
        None => s.parse().ok(),
    }
}

#[cfg(test)]
#[test]
fn numbers() {
    assert_eq!(number(b"12"), Some(12.0));
    assert_eq!(number(b" 7.25 "), Some(7.25));
    assert_eq!(number(b"0 (1)"), Some(1.0));
    assert_eq!(number(b"3 (2)"), Some(5.0));
    assert_eq!(number(b"Ham"), None);
}

/// One value of a stats view, by the view: a rating pill, cards, form pills or a bar against the
/// leader.
#[allow(static_mut_refs)]
unsafe fn value_cell((x1, y1, x2, y2): (i32, i32, i32, i32), t: &[u8], font: u32, best: f32) {
    let h = y2 - y1;
    fill(x1, y1, x2 + 1, y2 + 1, AREA);
    let view = VIEW.to_ascii_lowercase();
    let v = number(t).unwrap_or(0.0);
    if t.iter().all(|c| b"WDLX".contains(c)) {
        // form, oldest first: W green, D grey, L red, X (not played) dim
        let n = t.len() as i32;
        let (bw, gap) = (((x2 - x1 - 16) / n.max(1)).min(h + 4), 3);
        let mut x = (x1 + x2) / 2 - (n * bw) / 2;
        for &c in t {
            let colour = match c {
                b'W' => [0x3e, 0xa8, 0x6a],
                b'D' | b'X' => [0x4c, 0x8c, 0xe0], // (X: a draw too)
                b'L' => [0xd0, 0x50, 0x48],
                _ => [0x2c, 0x30, 0x36],
            };
            pill(x, y1 + 3, x + bw - gap, y2 - 3, colour);
            {
                text_at(x + (bw - gap) / 2, y1, y2, font, WHITE, &[if c == b'X' { b'D' } else { c }], 1);
            }
            x += bw;
        }
    } else if view.starts_with(b"average rating") {
        let c = if v >= 7.5 { [0x3e, 0xa8, 0x6a] } else if v >= 7.0 { [0xc8, 0xa8, 0x40] } else if v > 0.0 { [0xd0, 0x80, 0x40] } else { [0x4a, 0x50, 0x5a] };
        pill(x1 + 8, y1 + 3, x2 - 8, y2 - 3, c);
        text_at((x1 + x2) / 2, y1, y2, font, WHITE, t, 1);
    } else if view.starts_with(b"yellow cards") || view.starts_with(b"red cards") {
        let c = if view.starts_with(b"yellow") { [0xe8, 0xc8, 0x30] } else { [0xd8, 0x30, 0x30] };
        let (cw, ch) = ((h * 45 / 100).max(6), h * 72 / 100);
        let n = v as i32;
        let shown = n.min(5);
        let total = shown * (cw + 4) + if n > 5 { 40 } else { 0 };
        let mut x = (x1 + x2) / 2 - total / 2;
        for _ in 0..shown {
            fill(x, y1 + (h - ch) / 2, x + cw, y1 + (h + ch) / 2, c);
            x += cw + 4;
        }
        if n > 5 {
            text_at(x + 4, y1, y2, font, LIGHT, format!("x{n}").as_bytes(), 0);
        }
    } else {
        // a bar against the leader, the number on it
        let c = if view.starts_with(b"goals") { [0x3e, 0xa8, 0x6a] } else if view.starts_with(b"assists") { [0x4c, 0x9c, 0xe8] } else { [0x5a, 0x6a, 0x82] };
        let full = x2 - x1 - 16;
        let bw = if best > 0.0 { (full as f32 * v / best) as i32 } else { 0 };
        fill(x1 + 8, y1 + 4, x2 - 8, y2 - 4, [0x26, 0x2a, 0x31]);
        fill(x1 + 8, y1 + 4, x1 + 8 + bw.clamp(2, full), y2 - 4, c);
        text_at((x1 + x2) / 2, y1, y2, font, WHITE, t, 1);
    }
}

/// The flip pass: a stats view's values (against this frame's title and its biggest value), and
/// the table's goal difference at the right of the club's cell for the rows drawn this frame.
#[allow(static_mut_refs)]
pub unsafe fn flip(_full_frame: bool) {
    IN_FULL = false;
    honours();
    let drawn = std::mem::take(&mut DRAWN);
    let values = std::mem::take(&mut VALUES);
    let best = values.iter().filter_map(|v| number(&v.1)).fold(0.0f32, f32::max);
    // (only when the values came this frame: the view of the list is known now)
    if !drawn.is_empty() || !values.is_empty() {
        for (r, t, f) in &values {
            value_cell(*r, t, *f, best);
        }
    }
}
