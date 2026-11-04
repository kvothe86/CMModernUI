//! The coach (switch `coach`): Claude as the club's coach, opened from a "Coach" button under the
//! menu bar: a page right of the bar (which stays the game's: using it leaves the page). Four tabs: a briefing for the next match, an analysis of the last match, an expanded
//! team report and questions typed in (a conversation). Only at 1680 wide and up, in single player
//! or on a network host, and only when cmui.ini has `claude_api_key=` (read once at start);
//! `claude_model=` picks another model than Haiku 5.5. Each tab's answer is one request, made on a
//! thread of its own when the tab is opened (again with Ask again); it is kept until the game day
//! changes. What goes with it: the squad as the assistant sees it (his team report), the next
//! opponent as the scout sees them, the texts of our last match screen and of the screen behind.

use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

type Rect = (i32, i32, i32, i32);

#[link(name = "winhttp")]
extern "system" {
    fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> usize;
    fn WinHttpConnect(session: usize, server: *const u16, port: u16, reserved: u32) -> usize;
    fn WinHttpOpenRequest(connect: usize, verb: *const u16, object: *const u16, version: *const u16, referrer: *const u16, accept: *const *const u16, flags: u32) -> usize;
    fn WinHttpSetTimeouts(handle: usize, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
    fn WinHttpSendRequest(request: usize, headers: *const u16, headers_len: u32, data: *const u8, data_len: u32, total: u32, context: usize) -> i32;
    fn WinHttpReceiveResponse(request: usize, reserved: usize) -> i32;
    fn WinHttpQueryHeaders(request: usize, level: u32, name: *const u16, buf: *mut u8, len: *mut u32, index: *mut u32) -> i32;
    fn WinHttpReadData(request: usize, buf: *mut u8, len: u32, read: *mut u32) -> i32;
    fn WinHttpCloseHandle(handle: usize) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn SetWindowLongA(hwnd: usize, index: i32, value: isize) -> isize;
    fn CallWindowProcA(proc_: isize, hwnd: usize, msg: u32, w: usize, l: isize) -> isize;
}

const TABS: [&str; 4] = ["Pre-match briefing", "Post-match analysis", "Team report", "Ask the coach"];
const ASK: usize = 3;
const TODAY: usize = 0xae2c90; // Date: i16 day of the year, i16 year

fn key() -> Option<&'static str> {
    static KEY: OnceLock<Option<String>> = OnceLock::new();
    KEY.get_or_init(|| setting("claude_api_key").filter(|k| !k.is_empty())).as_deref()
}

fn model() -> String {
    setting("claude_model").filter(|m| !m.is_empty()).unwrap_or_else(|| "claude-haiku-5-5".to_string())
}

/// Its button stands under the menu bar (counted where Game Options goes).
#[allow(static_mut_refs)]
pub fn button_shown() -> bool {
    unsafe { BUTTON.is_some() }
}

/// The coach is there: its switch, a key, a flagship-size screen, not a network client.
pub fn active() -> bool {
    on(COACH) && key().is_some() && !small_screen() && unsafe { !remote::CLIENT }
}

static mut ACTIVE: bool = false;
static mut OPEN: bool = false;
static mut TAB: usize = 0;
static mut HOVER: Option<usize> = None;
static mut BUTTON: Option<Rect> = None;
static mut BUTTON_HOVER: bool = false;
static mut FONT: u32 = 0;
static mut COLOUR: u32 = 0;
// the menu bar as drawn this frame (as `challenge` reads it)
static mut BAR_SEEN: Option<(i32, i32, i32, i32, u32, u32)> = None;
static mut BAR_OPTIONS: bool = false;
static mut BAR_STATUS: Option<i32> = None;
/// The answers shown per tab (game text, *..* in gold, " | " a new paragraph) and the day they
/// were asked; the conversation of the Ask tab (question, answer as Claude wrote it).
static mut ANSWERS: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
static mut DAY: i32 = -1;
static mut CHAT: Vec<(String, String)> = Vec::new();
static mut QUESTION: String = String::new();
static mut TYPED: Vec<u8> = Vec::new();
static mut SEND_TYPED: bool = false;
static mut DIRTY: bool = false;
/// The tab whose request is on its way.
static mut WAITING: Option<usize> = None;
static REPLY: Mutex<Option<(usize, Result<String, String>)>> = Mutex::new(None);
static ARRIVED: AtomicBool = AtomicBool::new(false);
/// The texts drawn since the last flip (top, left, text); the rows of the last full frame's;
/// our last match's screens (rows, the day, live).
static mut FRAME_TEXTS: Vec<(i32, i32, Vec<u8>)> = Vec::new();
static mut SCREEN: Vec<String> = Vec::new();
static mut LAST_MATCH: (Vec<String>, i32, bool) = (Vec::new(), -1, false);
/// Our next fixture as last seen on the squad or tactics screen.
static mut NEXT_FIXTURE: usize = 0;
static mut OLD_PROC: isize = 0;

unsafe fn today() -> i32 {
    rd::<i16>(TODAY + 2) as i32 * 366 + rd::<i16>(TODAY) as i32
}

/// A text the game draws (the text hook).
#[allow(static_mut_refs)]
pub unsafe fn note_text(x1: i32, y1: i32, t: &[u8]) {
    if ACTIVE && !OPEN && !t.is_empty() && FRAME_TEXTS.len() < 2000 {
        FRAME_TEXTS.push((y1, x1, t.to_vec()));
    }
}

/// A menu bar button drawn (`sidebar_item`).
pub unsafe fn bar_item(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, colour: u32, label: &str) {
    BAR_OPTIONS |= label.contains("options");
    if (label.contains("have news") || label.contains("waiting")) && y1 > screen().1 / 2 {
        BAR_STATUS = Some(BAR_STATUS.map_or(y1, |t| t.min(y1)));
        return;
    }
    if !label.contains("challenge") && label != "coach" && BAR_SEEN.is_none_or(|b| y2 > b.3) {
        BAR_SEEN = Some((x1, y1, x2, y2, font, colour));
    }
}

/// Texts by rows, top to bottom, the cells of a row left to right.
fn rows_of(mut texts: Vec<(i32, i32, Vec<u8>)>) -> Vec<String> {
    texts.sort_by_key(|t| (t.0, t.1));
    let mut rows: Vec<(i32, Vec<String>)> = Vec::new();
    for (y, _, t) in texts {
        let cell = latin1(&t).trim().to_string();
        match rows.last_mut() {
            Some(r) if y - r.0 <= 3 => r.1.push(cell),
            _ => rows.push((y, vec![cell])),
        }
    }
    let mut out: Vec<String> = rows.into_iter().map(|r| r.1.join("  |  ")).collect();
    out.dedup();
    out
}

/// The flip pass: the screen's texts kept, the button under the menu bar, the coach's screen.
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    ACTIVE = active();
    let seen = std::mem::take(&mut BAR_SEEN);
    let options = std::mem::take(&mut BAR_OPTIONS);
    let status = std::mem::take(&mut BAR_STATUS);
    let texts = std::mem::take(&mut FRAME_TEXTS);
    if !ACTIVE {
        (BUTTON, OPEN) = (None, false);
        return;
    }
    if full_frame && !OPEN {
        if !texts.is_empty() {
            SCREEN = rows_of(texts);
        }
        let own = human_club().map(|c| latin1(&c.0).into_owned()).filter(|n| !n.is_empty());
        // our match on screen: live, the last frame seen is the score as it stood; after it, the
        // tabs looked at (overview, stats, ratings) are put together
        if let (true, Some(own)) = (MATCH, own) {
            if SCREEN.iter().any(|r| r.contains(&own)) {
                let live = match_record().is_some_and(|r| r.1);
                if live || LAST_MATCH.2 || LAST_MATCH.1 != today() {
                    LAST_MATCH = (SCREEN.clone(), today(), live);
                } else {
                    for r in SCREEN.iter() {
                        if !LAST_MATCH.0.contains(r) && LAST_MATCH.0.len() < 400 {
                            LAST_MATCH.0.push(r.clone());
                        }
                    }
                }
            }
        }
        if let Some(club) = human_club_ptr() {
            let f = assistant::tactics_screen().map_or(0, |ts| assistant::next_fixture(Some(ts), club));
            let f = if f != 0 { f } else { assistant::next_fixture(None, club) };
            if f != 0 {
                NEXT_FIXTURE = f;
            }
        }
    }
    if full_frame {
        BUTTON = seen.filter(|_| options).and_then(|(x1, y1, x2, y2, font, colour)| {
            let (h, (_, sh)) = (y2 - y1, screen());
            // (in Game Options' place, which goes under cmui's buttons, under CM Challenge's
            // where it has one; in the bar's plain white)
            let (first, room) = bar_slot((x1, y1, x2, y2));
            let top = first + if on(CHALLENGE) && challenge::button_shown() { h + 4 } else { 0 };
            (top + h + room < status.unwrap_or(sh)).then(|| {
                (FONT, COLOUR) = (font, if BAR_WHITE != 0 { BAR_WHITE } else { colour });
                (x1, top, x2, top + h)
            })
        });
    }
    if OPEN {
        overlay();
        // (the menu bar stays the game's; the coach's button lit while its page is up, as the
        // game's own buttons are, the label white)
        if let Some(b) = BUTTON {
            bar_lit(b);
            sidebar_item(b.0, b.1, b.2, b.3, FONT, COLOUR, b"Coach");
        }
    } else if let Some(b) = BUTTON.filter(|_| !assistant::REPORT_OPEN) {
        if BUTTON_HOVER && BUTTON_DOWN_NOW() {
            bar_lit(b);
        }
        sidebar_item(b.0, b.1, b.2, b.3, FONT, COLOUR, b"Coach");
    }
}

fn margin(sh: i32) -> i32 {
    (sh / 88).max(6)
}

/// The tabs, then Back and Ask again.
unsafe fn spots() -> Vec<Rect> {
    let (sw, sh) = screen();
    let (m, bar) = (margin(sh), sh * 8 / 100);
    let (h, y, font) = (sh / 22, bar + m, FONT & !0xffff | 3);
    let mut x = bar_end(sw) + m * 2;
    let mut out = Vec::new();
    for t in TABS {
        let w = text_width(font, t.as_bytes()) + 56;
        out.push((x, y, x + w, y + h));
        x += w + m;
    }
    let back = (sw - m * 2 - sw / 8, sh - m - sh / 20, sw - m * 2, sh - m);
    out.push(back);
    out.push((back.0 - m - sw / 7, back.1, back.0 - m, back.3));
    out
}

/// The coach's screen, over everything the game drew.
#[allow(static_mut_refs)]
unsafe fn overlay() {
    let (sw, sh) = screen();
    let f = |n: u32| FONT & !0xffff | n;
    let (m, bar) = (margin(sh), sh * 8 / 100);
    // right of the game's menu bar, which stays usable (as the CM Challenge page)
    let left = bar_end(sw);
    fill(left, 0, sw - 1, sh - 1, BACKGROUND);
    fill(left, 0, sw - 1, bar, SIDEBAR);
    fill(left, bar - 3, sw - 1, bar, GOLD);
    let club = human_club().map_or(Vec::new(), |c| c.0);
    let size = bar * 70 / 100;
    let tx = left + if draw_crest(&club, left + m * 2, (bar - size) / 2, size) { m * 4 + size } else { m * 2 };
    text_at(tx, 4, bar * 58 / 100, f(6), WHITE, &[&club[..], b" - Coach"].concat(), 0);
    text_at(tx, bar * 54 / 100, bar - 6, f(3), LIGHT, &to_latin1(&format!("Claude ({}): sees your squad, the next opponent, your last match and the screen you came from", model())), 0);
    let spots = spots();
    let labels: Vec<&str> = TABS.iter().copied().chain(["Back", if TAB == ASK { "New conversation" } else { "Ask again" }]).collect();
    for (k, r) in spots.iter().enumerate() {
        let (active, hover) = (k == TAB, HOVER == Some(k));
        pill(r.0, r.1, r.2, r.3, if active || hover { SELECTED } else { [0x3a, 0x40, 0x4a] });
        text_at((r.0 + r.2) / 2, r.1, r.3, f(3), if active || hover { GOLD } else { WHITE }, labels[k].as_bytes(), 1);
    }
    let (x1, y1, x2, y2) = (left + m * 2, spots[0].3 + m, sw - m * 2, spots[4].1 - m);
    fill(x1, y1, x2, y2, AREA);
    let font = f(3);
    let line = game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((font & 0xffff) as i32) + 8;
    let mut bottom = y2 - 16;
    if TAB == ASK {
        // the question box at the foot
        let (bx1, by1, bx2, by2) = (x1 + 16, y2 - 16 - line * 2, x2 - 16, y2 - 16);
        fill(bx1, by1, bx2, by2, [0x2a, 0x2e, 0x36]);
        fill(bx1, by2 - 2, bx2, by2, GOLD);
        let (text, ink) = if TYPED.is_empty() { (b"Type a question and press Enter".to_vec(), GREY) } else { ([&TYPED[..], b"_"].concat(), WHITE) };
        let room = bx2 - bx1 - 32;
        // (the end of a long question stays in view)
        let mut shown = text.as_slice();
        while text_width(font, shown) > room && shown.len() > 1 {
            shown = &shown[1..];
        }
        text_at(bx1 + 16, by1, by2, font, ink, shown, 0);
        bottom = by1 - 16;
    }
    let body: Vec<u8> = if WAITING == Some(TAB) {
        b"The coach is thinking...".to_vec()
    } else if !ANSWERS[TAB].is_empty() {
        ANSWERS[TAB].clone()
    } else if TAB == ASK {
        b"Ask about your squad, the next match, tactics, training or transfers. Follow-up questions carry on the conversation; New conversation starts again.".to_vec()
    } else {
        b"No answer yet: Ask again.".to_vec()
    };
    assistant::report_text(x1 + 24, y1 + 20, x2 - 24, bottom, font, line, &body);
}

/// The mouse (poll_input): None when the coach has nothing to do with it.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if !ACTIVE {
        return None;
    }
    let arrived = ARRIVED.swap(false, Ordering::AcqRel) && take_reply();
    let inside = |r: Rect| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if !OPEN {
        let over = !assistant::REPORT_OPEN && BUTTON.is_some_and(inside);
        let changed = std::mem::replace(&mut BUTTON_HOVER, over) != over;
        if over && released {
            // (the button stands on Game Options' box: its click must not reach the game)
            *(LEFT_CLICK as *mut u32) = 0;
            open();
            return Some(true);
        }
        return if over { Some(changed) } else { changed.then_some(true) };
    }
    // the game's menu bar stays the game's: using it leaves the coach; its own button shuts it
    if mx < bar_end(screen().0) {
        HOVER = None;
        if released {
            (OPEN, REDRAW) = (false, true);
            if BUTTON.is_some_and(inside) {
                *(LEFT_CLICK as *mut u32) = 0;
                return Some(true);
            }
        }
        return None;
    }
    // (the game's poll does not run: a click would still be waiting for it after Back)
    *(LEFT_CLICK as *mut u32) = 0;
    *(assistant::RIGHT_CLICK as *mut u32) = 0;
    let spots = spots();
    let hover = spots.iter().position(|&s| inside(s));
    let mut redraw = std::mem::replace(&mut HOVER, hover) != hover || arrived || std::mem::take(&mut DIRTY);
    if std::mem::take(&mut SEND_TYPED) && WAITING.is_none() {
        let q = latin1(&std::mem::take(&mut TYPED)).trim().to_string();
        if !q.is_empty() {
            QUESTION = q.clone();
            ask(ASK, &q);
        }
        redraw = true;
    }
    if released {
        match hover {
            Some(k) if k < TABS.len() => {
                TAB = k;
                if k != ASK && ANSWERS[k].is_empty() && WAITING.is_none() {
                    ask(k, "");
                }
                redraw = true;
            }
            Some(4) => (OPEN, redraw) = (false, true),
            Some(5) if TAB == ASK => {
                CHAT.clear();
                ANSWERS[ASK].clear();
                redraw = true;
            }
            Some(5) if WAITING.is_none() => {
                ask(TAB, "");
                redraw = true;
            }
            _ => {}
        }
    }
    Some(redraw)
}

#[allow(static_mut_refs)]
unsafe fn open() {
    if FONT == 0 {
        return;
    }
    if DAY != today() {
        DAY = today();
        ANSWERS.iter_mut().for_each(Vec::clear);
        CHAT.clear();
    }
    OPEN = true;
    HOVER = None;
    hook_keys();
    if TAB != ASK && ANSWERS[TAB].is_empty() && WAITING.is_none() {
        ask(TAB, "");
    }
}

/// The game window's messages: keys go to the question box while the Ask tab is up.
extern "system" fn window_proc(hwnd: usize, msg: u32, w: usize, l: isize) -> isize {
    unsafe {
        if OPEN && TAB == ASK {
            #[allow(static_mut_refs)]
            match msg {
                0x100 => {
                    // WM_KEYDOWN: Backspace, Enter, Escape here (their WM_CHAR is left alone)
                    match w {
                        8 => _ = TYPED.pop(),
                        13 => SEND_TYPED = true,
                        27 => TYPED.clear(),
                        _ => {}
                    }
                    DIRTY = true;
                    return 0;
                }
                0x101 => return 0,
                0x102 => {
                    if (0x20..0x100).contains(&w) && TYPED.len() < 400 {
                        TYPED.push(w as u8);
                        DIRTY = true;
                    }
                    return 0;
                }
                _ => {}
            }
        }
        CallWindowProcA(OLD_PROC, hwnd, msg, w, l)
    }
}

unsafe fn hook_keys() {
    let hwnd = *(GAME_WINDOW as *const usize);
    if OLD_PROC == 0 && hwnd != 0 {
        OLD_PROC = SetWindowLongA(hwnd, -4, window_proc as extern "system" fn(usize, u32, usize, isize) -> isize as usize as isize); // GWL_WNDPROC
    }
}

/// What the coach is told to do, per tab.
const TASKS: [&str; 3] = [
    "Brief me for our next match: how the opponent will play and who to stop, their weak spot and how to hurt it, the XI and shape you would pick, the team instructions to set, and individual instructions for two or three players.",
    "Analyse our last match from its match screen texts in the game data: why the result went the way it did, who stood out for good or bad, and what to change for the next match. If there are no texts of our last match, tell me in one sentence to look at our last match's screen first.",
    "Write an expanded team report: the best XI and system for this squad, the depth at each position and where we are thin, who is in form and who is not, who to sell or loan out, which positions to strengthen and the profile of player to look for, and which young players to develop.",
];

/// A request for a tab, on its own thread; `question` for the Ask tab.
#[allow(static_mut_refs)]
unsafe fn ask(tab: usize, question: &str) {
    let (Some(key), Some(club)) = (key(), human_club_ptr()) else { return };
    let system = format!(
        "You are the first-team coach at {} in Championship Manager 01/02, talking to the manager, who plays the game. \
         Work only from the game data below; never make up players, results or numbers, and say so briefly when the data does not tell. \
         Talk like a football coach: direct and concrete, no filler. Plain text only: no markdown, headings, tables or emoji. \
         Put *asterisks* around player and club names. Separate paragraphs with a blank line; a list item starts with \"- \". \
         Give advice the manager can set in the game: formation and positions; team instructions (mentality, passing, tempo, width, closing down, tackling, marking, offside trap, counter attack, playmaker, target man); \
         player instructions (forward runs, run with ball, long shots, through balls, crossing, closing down, tight marking, free role). \
         Stars are the assistant manager's rating out of 5. Keep the answer under 350 words.\n\nGAME DATA\n{}",
        latin1(&game_text(club + 4, 51)),
        game_data(club)
    );
    let mut messages: Vec<(&str, String)> = Vec::new();
    if tab == ASK {
        let start = CHAT.len().saturating_sub(6);
        for (q, a) in CHAT[start..].iter() {
            messages.push(("user", q.clone()));
            messages.push(("assistant", a.clone()));
        }
        messages.push(("user", question.to_string()));
    } else {
        messages.push(("user", TASKS[tab].to_string()));
    }
    let body = request_json(&model(), &system, &messages);
    let key = key.to_string();
    WAITING = Some(tab);
    log(&format!("coach: asking for {} ({} bytes)", TABS[tab], body.len()));
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let reply = post(&key, &body).and_then(|r| answer_of(&r));
        log(&format!("coach: reply after {:.1} s", start.elapsed().as_secs_f64()));
        *REPLY.lock().unwrap_or_else(|e| e.into_inner()) = Some((tab, reply));
        ARRIVED.store(true, Ordering::Release);
    });
}

/// The answer that came, put where it is shown. True when there was one.
#[allow(static_mut_refs)]
unsafe fn take_reply() -> bool {
    let Some((tab, reply)) = REPLY.lock().unwrap_or_else(|e| e.into_inner()).take() else { return false };
    WAITING = None;
    match reply {
        Ok(text) => {
            log(&format!("coach: answer for {} ({} chars)", TABS[tab], text.len()));
            ANSWERS[tab] = if tab == ASK {
                CHAT.push((QUESTION.clone(), text.clone()));
                [b"*You:* ".as_slice(), &to_latin1(&QUESTION), b" | ", &game_words(&text)].concat()
            } else {
                game_words(&text)
            };
        }
        Err(e) => {
            log(&format!("coach: no answer for {}: {e}", TABS[tab]));
            ANSWERS[tab] = to_latin1(&format!("The coach could not answer: {e}"));
        }
    }
    true
}

/// Claude's text in the game's words: Latin-1, *..* for names, " | " between paragraphs.
fn game_words(text: &str) -> Vec<u8> {
    let lines: Vec<String> = text
        .replace("**", "*")
        .lines()
        .map(|l| l.trim().trim_start_matches('#').trim().replace("\u{2022} ", "- "))
        .filter(|l| !l.is_empty())
        .collect();
    let plain: String = lines
        .join(" | ")
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' => "'".to_string(),
            '\u{201c}' | '\u{201d}' => "\"".to_string(),
            '\u{2013}' | '\u{2014}' => "-".to_string(),
            '\u{2026}' => "...".to_string(),
            c if (c as u32) < 0x100 => c.to_string(),
            _ => String::new(),
        })
        .collect();
    to_latin1(&plain)
}

/// What the coach knows: the date, the squad as the assistant sees it, the next match and the
/// opponent as the scout sees them, our last match's screens, the screen behind.
#[allow(static_mut_refs)]
unsafe fn game_data(club: usize) -> String {
    let mut out = String::new();
    let (d, m, year) = day_month(TODAY);
    out += &format!("Today: {d} {} {year}\n", MONTHS[m]);
    let league = rd::<usize>(club + 0x57);
    if readable(league, 0x37) {
        out += &format!("Our league: {}\n", latin1(&game_text(league + 4, 51)));
    }
    // the squad: the assistant's report (his best and backup elevens), then every player
    let humans = rd::<usize>(assistant::HUMAN_MANAGER);
    let formation = if mapped(humans, 4) { game::<extern "thiscall" fn(usize, usize) -> usize>(assistant::ACTIVE_TACTIC)(humans, 0) } else { 0 };
    let slots = if mapped(formation, 0x200) { assistant::formation_slots(formation) } else { Vec::new() };
    if slots.len() >= 11 {
        out += &format!("Our formation now: {}\n", slots[..11].iter().map(|s| latin1(s).into_owned()).collect::<Vec<_>>().join(" "));
    }
    let report = assistant::report_for(club, slots, Vec::new());
    let mut lines = report.split(|&c| c == b'\n');
    let head: Vec<&[u8]> = lines.next().unwrap_or(b"").split(|&c| c == b'\t').collect();
    let rows: Vec<Vec<&[u8]>> = lines.map(|l| l.split(|&c| c == b'\t').collect::<Vec<_>>()).filter(|r| r.len() == 7).collect();
    if let [_, by, form, says, ..] = head[..] {
        out += &format!("Assistant manager {} recommends {}. He says: {}\n", latin1(by), latin1(form), latin1(says).replace('*', ""));
    }
    let a = assistant::assistant_of(club);
    let injuries = rd::<usize>(assistant::INJURIES);
    let day_now = rd::<i16>(TODAY) as i32;
    out += "\nOUR SQUAD (name, age, positions, stars, condition, average rating this season, value, wage, contract to, the assistant's choice)\n";
    for k in 0..50 {
        let staff = rd::<usize>(club + 0xd7 + 4 * k);
        if !readable(staff, 0x6e) || !plays_for(staff, club) || !readable(((staff + 0x61) as *const usize).read_unaligned(), 70) {
            continue;
        }
        let name = staff_name(staff);
        // (born: staff +0x10 a date; year 1900 when only the year at +0x18 is known)
        let (_, _, born) = day_month(staff + 0x10);
        let (born, born_day) = if born == 1900 { (rd::<u16>(staff + 0x18) as i32, 0) } else { (born, rd::<i16>(staff + 0x10) as i32) };
        let age = year - born - (day_now < born_day) as i32;
        let stars = a.and_then(|a| scout::seen_score(staff, a, None)).map_or("-".to_string(), |s| format!("{}", scout::halves(s) as f32 / 2.0));
        let fit = condition(staff).map_or("-".to_string(), |c| format!("{c}%"));
        let hurt = mapped(injuries, 4) && game::<extern "thiscall" fn(usize, usize) -> i32>(assistant::INJURY_STATUS)(injuries, staff) & 0xff == 2;
        let avr = latin1(&season_stat(staff, 0x11)).into_owned();
        let (wage, value) = money_texts(staff);
        let until = main_contract(staff).map_or("-".to_string(), |c| {
            let (_, m, y) = day_month(c + 0x2d);
            format!("{} {y}", MONTHS[m])
        });
        let role = match rows.iter().find(|r| r[0] == name.as_slice()) {
            Some(r) if r[3] == b"1" => format!("best XI at {}", latin1(r[5])),
            Some(r) if r[3] == b"2" => format!("backup XI at {}", latin1(r[5])),
            Some(_) => "not available".to_string(),
            None => "not in his two elevens".to_string(),
        };
        out += &format!(
            "- {}, {age}, {}, {stars} stars, {fit}{}, av. rating {}, {value}, {wage}, {until}, {role}\n",
            latin1(&name),
            latin1(&positions_text(staff)),
            if hurt { " INJURED" } else { "" },
            if avr.is_empty() { "-" } else { &avr }
        );
    }
    // the next match, the opponent as our scout sees them
    let fixture = NEXT_FIXTURE;
    if readable(fixture, 0x50) && (rd::<usize>(fixture + 0x1c) == club || rd::<usize>(fixture + 0x20) == club) {
        let home = rd::<usize>(fixture + 0x1c) == club;
        let opponent = rd::<usize>(fixture + if home { 0x20 } else { 0x1c });
        let comp = rd::<usize>(fixture + 0x14);
        let comp = if readable(comp, 0x55) { latin1(&game_text(comp + 4, 51)).into_owned() } else { String::new() };
        if readable(opponent, 0x1e0 + 80) {
            out += &format!("\nNEXT MATCH (as last seen on the squad screen): {} {}, {comp}\n", latin1(&game_text(opponent + 4, 51)), if home { "at home" } else { "away" });
            let scout = assistant::scout_for(opponent, club);
            let mut lines = scout.split(|&c| c == b'\n');
            let head: Vec<&[u8]> = lines.next().unwrap_or(b"").split(|&c| c == b'\t').collect();
            if let [_, by, form, says, ..] = head[..] {
                out += &format!("Our scout {} expects a {}. He says: {}\n", latin1(by), latin1(form), latin1(says).replace('*', "").replace(" | ", " "));
            }
            for r in lines.map(|l| l.split(|&c| c == b'\t').collect::<Vec<_>>()).filter(|r| r.len() == 7 && r[3] == b"1") {
                let stars = std::str::from_utf8(r[6]).ok().and_then(|h| h.parse::<f32>().ok()).map_or(0.0, |h| h / 2.0);
                out += &format!("- {} ({}), {stars} stars\n", latin1(r[0]), latin1(r[5]));
            }
        }
    }
    let cap = |rows: &[String], max: usize| -> String {
        let mut s = rows.join("\n");
        if s.len() > max {
            let end = s.char_indices().map(|c| c.0).take_while(|&at| at <= max).last().unwrap_or(0);
            s.truncate(end);
        }
        s
    };
    if LAST_MATCH.0.is_empty() {
        out += "\nOUR LAST MATCH: no match screen of ours seen since the game started\n";
    } else {
        out += &format!("\nOUR LAST MATCH (texts of its match screens, rows top to bottom){}\n{}\n", if LAST_MATCH.2 { ", seen while it was being played" } else { "" }, cap(&LAST_MATCH.0, 8000));
    }
    out += &format!("\nTHE SCREEN THE MANAGER CAME FROM (its texts)\n{}\n", cap(&SCREEN, 5000));
    out
}

/// A JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn request_json(model: &str, system: &str, messages: &[(&str, String)]) -> String {
    let messages: Vec<String> = messages.iter().map(|(role, text)| format!("{{\"role\":\"{role}\",\"content\":{}}}", json_string(text))).collect();
    // (low effort: little thinking before the answer, so it comes in seconds; the default is medium)
    format!(
        "{{\"model\":{},\"max_tokens\":8000,\"output_config\":{{\"effort\":\"low\"}},\"system\":{},\"messages\":[{}]}}",
        json_string(model),
        json_string(system),
        messages.join(",")
    )
}

/// A JSON string from its opening quote: the text and the index after its closing quote.
fn parse_string(s: &str, start: usize) -> (String, usize) {
    let mut out = String::new();
    let mut it = s[start + 1..].char_indices();
    while let Some((k, c)) = it.next() {
        match c {
            '"' => return (out, start + 2 + k),
            '\\' => match it.next().map(|x| x.1) {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('u') => {
                    let hex: String = (0..4).filter_map(|_| it.next().map(|x| x.1)).collect();
                    let mut code = u32::from_str_radix(&hex, 16).unwrap_or(0xfffd);
                    if (0xd800..0xdc00).contains(&code) {
                        // a surrogate pair: the low half follows as \uXXXX
                        let low: String = (0..6).filter_map(|_| it.next().map(|x| x.1)).collect();
                        let low = low.get(2..).and_then(|h| u32::from_str_radix(h, 16).ok()).unwrap_or(0xdc00);
                        code = 0x10000 + ((code - 0xd800) << 10) + (low.wrapping_sub(0xdc00) & 0x3ff);
                    }
                    out.push(char::from_u32(code).unwrap_or('?'));
                }
                Some(c) => out.push(c),
                None => {}
            },
            c => out.push(c),
        }
    }
    (out, s.len())
}

/// The string values of every `name` key in a JSON text, in order.
fn values_of(json: &str, name: &str) -> Vec<String> {
    let b = json.as_bytes();
    let (mut out, mut i, mut key) = (Vec::new(), 0, None::<String>);
    while i < b.len() {
        if b[i] == b'"' {
            let (s, end) = parse_string(json, i);
            i = end;
            let mut j = i;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b':' {
                (key, i) = (Some(s), j + 1);
                continue;
            }
            if key.as_deref() == Some(name) {
                out.push(s);
            }
            key = None;
        } else {
            if !b[i].is_ascii_whitespace() {
                key = None;
            }
            i += 1;
        }
    }
    out
}

/// The text blocks of a Messages API answer.
fn answer_of(json: &str) -> Result<String, String> {
    let stop = values_of(json, "stop_reason");
    if stop.first().map(String::as_str) == Some("refusal") {
        return Err("the request was declined".to_string());
    }
    let text = values_of(json, "text").join("\n\n");
    if text.trim().is_empty() {
        return Err("an empty answer".to_string());
    }
    Ok(if stop.first().map(String::as_str) == Some("max_tokens") { text + "\n\n(cut short)" } else { text })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// POST /v1/messages; the body of a 200 answer, else the API's message.
fn post(key: &str, body: &str) -> Result<String, String> {
    let null = std::ptr::null();
    unsafe {
        let agent = wide("cmui");
        // (automatic proxy, Windows 8.1 and up; else the default)
        let mut session = WinHttpOpen(agent.as_ptr(), 4, null, null, 0);
        if session == 0 {
            session = WinHttpOpen(agent.as_ptr(), 0, null, null, 0);
        }
        if session == 0 {
            return Err("no internet access (WinHTTP)".to_string());
        }
        WinHttpSetTimeouts(session, 15_000, 15_000, 30_000, 180_000);
        let connect = WinHttpConnect(session, wide("api.anthropic.com").as_ptr(), 443, 0);
        let request = if connect != 0 { WinHttpOpenRequest(connect, wide("POST").as_ptr(), wide("/v1/messages").as_ptr(), null, null, std::ptr::null(), 0x0080_0000) } else { 0 };
        let headers = wide(&format!("content-type: application/json\r\nx-api-key: {key}\r\nanthropic-version: 2026-10-21\r\n"));
        let mut result = Err("could not reach api.anthropic.com".to_string());
        if request != 0
            && WinHttpSendRequest(request, headers.as_ptr(), (headers.len() - 1) as u32, body.as_ptr(), body.len() as u32, body.len() as u32, 0) != 0
            && WinHttpReceiveResponse(request, 0) != 0
        {
            let (mut status, mut size) = (0u32, 4u32);
            // WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER
            WinHttpQueryHeaders(request, 19 | 0x2000_0000, null, &mut status as *mut u32 as *mut u8, &mut size, std::ptr::null_mut());
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let mut read = 0u32;
                if WinHttpReadData(request, buf.as_mut_ptr(), buf.len() as u32, &mut read) == 0 || read == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..read as usize]);
            }
            let text = String::from_utf8_lossy(&data).into_owned();
            result = if status == 200 {
                Ok(text)
            } else {
                Err(format!("error {status}: {}", values_of(&text, "message").into_iter().next().unwrap_or_else(|| text.chars().take(160).collect())))
            };
        }
        for h in [request, connect, session] {
            if h != 0 {
                WinHttpCloseHandle(h);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip() {
        let s = "a \"quote\" \\ back\nline\ttab \u{e9}\u{1}";
        let json = format!("{{\"text\":{}}}", json_string(s));
        assert_eq!(values_of(&json, "text"), vec![s.to_string()]);
    }

    #[test]
    fn answer_text_blocks() {
        let json = r#"{"content":[{"type":"thinking","thinking":"","signature":"x"},{"type":"text","text":"Play *Kok* 😀","cited_text":"no"}],"stop_reason":"end_turn"}"#;
        assert_eq!(answer_of(json), Ok("Play *Kok* \u{1f600}".to_string()));
        assert!(answer_of(r#"{"content":[],"stop_reason":"refusal"}"#).is_err());
        assert_eq!(values_of(r#"{"error":{"type":"x","message":"invalid x-api-key"}}"#, "message"), vec!["invalid x-api-key".to_string()]);
    }

    /// The real path to the API (WinHTTP, TLS, headers, the error read back), with a key that is
    /// not one: `cargo test --release --target i686-pc-windows-msvc coach -- --ignored`.
    #[test]
    #[ignore]
    fn api_answers_a_bad_key() {
        let body = request_json("claude-haiku-5-5", "Say hi.", &[("user", "Hi".to_string())]);
        let e = post("sk-ant-not-a-key", &body).unwrap_err();
        assert!(e.starts_with("error 401"), "{e}");
    }

    #[test]
    fn words_for_the_game() {
        assert_eq!(game_words("## Plan\n**Kok** \u{2014} start him\u{2019}s\n\n- press high \u{2026}"), b"Plan | *Kok* - start him's | - press high ...".to_vec());
    }
}
