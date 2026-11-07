//! The page, laid out as the CM Challenge site in cmui's own skin: a game screen with a title band,
//! tabs and a Back button, right of the game's menu bar, which stays in use. The host builds the
//! page as text (`page`); a client asks for it (key 'E') and only reads: the host spins and ticks.

use super::*;
use crate::*;

// cmui's colours
const BG: [u8; 3] = [0x22, 0x26, 0x2c];
const CARD: [u8; 3] = [0x2c, 0x30, 0x36];
const BORDER: [u8; 3] = [0x3a, 0x40, 0x4a];
const PRIMARY: [u8; 3] = GOLD;
const CYAN: [u8; 3] = [0x4c, 0x8c, 0xe0];
const ROSE: [u8; 3] = [0xd0, 0x50, 0x48];
const INK: [u8; 3] = WHITE;
const MUTED: [u8; 3] = GREY;
const DARK: [u8; 3] = [0x14, 0x18, 0x1c];
const SUCCESS: [u8; 3] = [0x3e, 0xa8, 0x6a];
const DANGER: [u8; 3] = ROSE;
const MEDAL: [u8; 3] = GOLD;
/// The ranks' colours, lowest first: none yet, bronze, silver, gold, diamond, platinum, champion, legend.
const RANKS: [[u8; 3]; 8] = [[0x47, 0x55, 0x69], [0xcd, 0x7f, 0x32], [0xc0, 0xc8, 0xd4], [0xf5, 0xb8, 0x2e], [0x67, 0xe8, 0xf9], [0xa5, 0xb4, 0xfc], [0xc0, 0x84, 0xfc], [0xf4, 0x3f, 0x5e]];

// Segoe Fluent / MDL2 icons
const ICON_STAR: u32 = 0xE735;
const ICON_TROPHY: u32 = 0x1F3C6;
const ICON_CHECK: u32 = 0xE73E;
const ICON_CROSS: u32 = 0xE711;
const ICON_CLOCK: u32 = 0xE823;

const TABS: [&[u8]; 6] = [b"Dashboard", b"Season", b"RAD", b"Stats", b"History", b"Rules"];
const TAB_RAD: usize = 2;
const TAB_GOALS: usize = 6; // shown after RAD when the host has career goals on
const SPIN_MS: u128 = 4000;

/// The game's menu bar seen this frame: its lowest button's box, font and colour, and whether it
/// is the bar of the plain game days (Game Options; a match has another); ours under it.
static mut BAR_SEEN: Option<(i32, i32, i32, i32, u32, u32)> = None;
static mut BAR_OPTIONS: bool = false;
static mut BAR_STATUS: Option<i32> = None; // the top of the status items at the bar's foot (news, player waiting)
static mut BUTTON:Option<(i32, i32, i32, i32)> = None;
static mut BUTTON_HOVER: bool = false;
static mut FONT: u32 = 0;
static mut COLOUR: u32 = 0;
pub static mut OPEN: bool = false;
static mut PAGE: Vec<u8> = Vec::new();
static mut KEY: Vec<u8> = Vec::new();
static mut ASKS: u32 = 0;
static mut WAITING: bool = false; // a client's page on its way
static mut LOOKED: Option<std::time::Instant> = None;
static mut TAB: usize = 0;
static mut PICK: usize = 0; // the manager on Stats and RAD
static mut FILTER: usize = 0; // History: 0 all, else that manager
static mut SCROLL: usize = 0; // History, Rules: the first row
static mut BACK_YEARS: i32 = 0; // RAD: how many seasons back
static mut SEASON_BACK: usize = 0; // Season tab: how many seasons back
static mut STATS_YEAR: i32 = 0; // Stats: the season counted, 0 all (the host counts it)
static mut STATS_YEARS: Vec<i32> = Vec::new(); // the seasons there are, newest first (from the page)
static mut HOVER: Option<usize> = None;
static mut HOVERED: Option<Spot> = None; // the spot of `HOVER`, while `SPOTS` is drawn anew
static mut SPOTS: Vec<(Rect, Spot)> = Vec::new();
static mut SPIN: Option<(std::time::Instant, Vec<u8>)> = None; // the host's wheel: since when, for whom
static mut SPIN_DRAWN: Option<std::time::Instant> = None;
static mut NEW_FROM: Option<i32> = None; // the highest RAD row when the wheel started
static mut NEW_ROW: Option<i32> = None; // the row the wheel gave, marked in the list
static mut NOTHING_LEFT: bool = false; // the wheel found no challenge left
static mut PICKING: Option<usize> = None; // Goals: the goal whose country the host is choosing

type Rect = (i32, i32, i32, i32);
type Lines<'a> = [&'a Vec<&'a [u8]>];

#[derive(Clone, PartialEq)]
enum Spot {
    Back,
    Tab(usize),
    Manager(usize),
    Filter(usize),
    Spin,
    Tick(u32),
    Delete(u32),
    Earlier,
    Later,
    Up,
    Down,
    Goal(usize),
    Country(usize),
    Nation(usize),
    Cancel,
    SeasonEarlier,
    SeasonLater,
    Rate(u32), // year << 4 | mark

    StatsEarlier,
    StatsLater,
}

/// A name's first word, as the page's lines match managers.
fn first_word(s: &[u8]) -> Vec<u8> {
    s.split(|&c| c == b' ').next().unwrap_or(b"").to_ascii_lowercase()
}

/// A menu bar button drawn (`sidebar_item`).
pub unsafe fn bar_item(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, colour: u32, label: &str) {
    BAR_OPTIONS |= label.contains("options");
    // (the status items stand at the bar's very foot: the button goes above them, not under)
    // ("Waiting For Players" can also stand at the top, in Latest Scores' place)
    if is_status(label) && y1 > screen().1 / 2 {
        BAR_STATUS = Some(BAR_STATUS.map_or(y1, |t| t.min(y1)));
        return;
    }
    // (not ours, drawn the same way: it would walk down the bar)
    if !label.contains("challenge") && BAR_SEEN.is_none_or(|b| y2 > b.3) {
        BAR_SEEN = Some((x1, y1, x2, y2, font, colour));
    }
}

/// "You Have News" and "Player Waiting": they come and go at the foot of the bar.
fn is_status(label: &str) -> bool {
    label.contains("have news") || label.contains("waiting")
}

/// The button stands under the bar (the coach's goes under it).
pub unsafe fn button_shown() -> bool {
    BUTTON.is_some()
}

/// The button under the bar (the flip pass).
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    let seen = std::mem::take(&mut BAR_SEEN);
    let options = std::mem::take(&mut BAR_OPTIONS);
    let status = std::mem::take(&mut BAR_STATUS);
    if full_frame {
        BUTTON = seen.filter(|_| options && game_on()).and_then(|(x1, y1, x2, y2, font, colour)| {
            let (h, (_, sh)) = (y2 - y1, screen());
            // (in Game Options' place, which goes under cmui's buttons; in the bar's plain white)
            let (top, room) = bar_slot((x1, y1, x2, y2));
            (top + h + room < status.unwrap_or(sh)).then(|| {
                FONT = font;
                COLOUR = if BAR_WHITE != 0 { BAR_WHITE } else { colour };
                (x1, top, x2, top + h)
            })
        });
    }
    // (the team and scout reports fill the whole screen, the menu bar too)
    if let Some(b) = BUTTON.filter(|_| !assistant::REPORT_OPEN) {
        // (as the game's own buttons: lit with a gold edge while open or pressed, the label white)
        if OPEN || (BUTTON_HOVER && BUTTON_DOWN_NOW()) {
            bar_lit(b);
        }
        sidebar_item(b.0, b.1, b.2, b.3, FONT, COLOUR, b"CM\nChallenge");
    }
}

#[allow(static_mut_refs)]
unsafe fn open() {
    if FONT == 0 {
        return;
    }
    OPEN = true;
    PAGE.clear();
    (TAB, PICK, FILTER, SCROLL, BACK_YEARS, HOVER, SPIN) = (0, 0, 0, 0, 0, None, None);
    (SEASON_BACK, STATS_YEAR) = (usize::MAX, 0);
    (NEW_FROM, NEW_ROW, NOTHING_LEFT, PICKING) = (None, None, false, None);
    request(ACT_NONE, 0, &[]);
}

/// The page again: at once on the host (after what he clicked), by a question on a client.
#[allow(static_mut_refs)]
unsafe fn request(action: u32, arg: u32, name: &[u8]) {
    if remote::CLIENT {
        ASKS += 1;
        // (a mark for his own season goes with the question: the host sets it)
        let (a, b) = if action == ACT_RATE { (action, arg) } else { (0, 0) };
        KEY = key(b'E', &[ASKS, STATS_YEAR as u32, a, b]);
        WAITING = true;
        LOOKED = None;
        return;
    }
    act(action, arg, name);
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel());
    let me = humans().into_iter().find(|h| h.0 == manager).map_or(String::new(), |h| h.1);
    let text = page(&me, true, STATS_YEAR);
    if trace() {
        log(&format!("challenge page:\n{text}"));
    }
    PAGE = page_bytes(&text);
}

/// The mouse (poll_input): None when the page and the button have nothing to do with it.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if !remote::CLIENT {
        track::tick();
    }
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
    // the game's menu bar stays the game's: using it leaves the page; the page's own button shuts it
    if mx < bar_end(screen().0) {
        HOVER = None;
        if released {
            (OPEN, SPIN, REDRAW) = (false, None, true);
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
    let hover = SPOTS.iter().position(|s| inside(s.0));
    let mut redraw = std::mem::replace(&mut HOVER, hover) != hover;
    if let Some((since, name)) = &SPIN {
        redraw |= SPIN_DRAWN.is_none_or(|t| t.elapsed().as_millis() > 80);
        if since.elapsed().as_millis() > SPIN_MS {
            let name = name.clone();
            SPIN = None;
            request(ACT_ROLL, 0, &name);
            redraw = true;
        }
    } else if let (true, Some(k)) = (released, hover) {
        match SPOTS[k].1.clone() {
            Spot::Back => OPEN = false,
            Spot::Tab(t) => {
                (TAB, SCROLL, BACK_YEARS, PICKING) = (t, 0, 0, None);
                request(ACT_NONE, 0, &[]);
            }
            Spot::Manager(m) => (PICK, PICKING, SCROLL) = (m, None, 0),
            // a career goal: picked or dropped while the manager's goals can change, else ticked off
            Spot::Goal(k) => {
                let (name, picking) = {
                    let lines: Vec<Vec<&[u8]>> = PAGE.split(|&c| c == b'\n').map(fields).collect();
                    let name = lines.iter().filter(|l| l[0] == b"K").nth(PICK).map_or(Vec::new(), |l| l[1].to_vec());
                    let picking = lines.iter().find(|l| l[0] == b"M" && first_word(l[1]) == first_word(&name)).is_none_or(|l| field(l, 2) == b"1");
                    (name, picking)
                };
                request(if picking { ACT_PICK } else { ACT_GOAL_TICK }, k as u32, &name);
            }
            Spot::Country(k) => (PICKING, SCROLL) = (Some(k), 0),
            Spot::Nation(i) => {
                let said = {
                    let lines: Vec<Vec<&[u8]>> = PAGE.split(|&c| c == b'\n').map(fields).collect();
                    let name = lines.iter().filter(|l| l[0] == b"K").nth(PICK).map_or(&b""[..], |l| l[1]);
                    let nation = lines.iter().filter(|l| l[0] == b"N").nth(i).map_or(&b""[..], |l| field(l, 1));
                    [name, b"\t", nation].concat()
                };
                if let Some(k) = PICKING.take() {
                    request(ACT_COUNTRY, k as u32, &said);
                }
                SCROLL = 0;
            }
            Spot::Cancel => (PICKING, SCROLL) = (None, 0),
            Spot::Filter(f) => (FILTER, SCROLL) = (f, 0),
            Spot::Spin => {
                let lines: Vec<Vec<&[u8]>> = PAGE.split(|&c| c == b'\n').map(fields).collect();
                let name = lines.iter().filter(|l| l[0] == b"K").nth(PICK).map_or(Vec::new(), |l| l[1].to_vec());
                NEW_FROM = Some(lines.iter().filter(|l| l[0] == b"R").map(|l| number(l[1])).max().unwrap_or(0));
                (NEW_ROW, NOTHING_LEFT, BACK_YEARS) = (None, false, 0);
                SPIN = Some((std::time::Instant::now(), name));
            }
            Spot::Tick(id) => request(ACT_TICK, id, &[]),
            Spot::Delete(id) => request(ACT_DELETE, id, &[]),
            Spot::Earlier => BACK_YEARS += 1,
            Spot::Later => BACK_YEARS = (BACK_YEARS - 1).max(0),
            Spot::Up => SCROLL = SCROLL.saturating_sub(10),
            Spot::Down => SCROLL += 10,
            Spot::Rate(arg) => {
                let me = {
                    let lines: Vec<Vec<&[u8]>> = PAGE.split(|&c| c == b'\n').map(fields).collect();
                    lines.iter().find(|l| l[0] == b"C").map_or(Vec::new(), |l| field(l, 2).to_vec())
                };
                request(ACT_RATE, arg, &me);
            }
            Spot::SeasonEarlier => SEASON_BACK = SEASON_BACK.saturating_add(1),
            Spot::SeasonLater => SEASON_BACK = SEASON_BACK.saturating_sub(1),
            // (0 all, then the seasons newest first: the host counts the one chosen)
            Spot::StatsEarlier | Spot::StatsLater => {
                let mut choices = vec![0];
                choices.extend(STATS_YEARS.iter().copied());
                let at = choices.iter().position(|&y| y == STATS_YEAR).unwrap_or(0);
                let at = if SPOTS[k].1 == Spot::StatsEarlier { (at + 1).min(choices.len() - 1) } else { at.saturating_sub(1) };
                STATS_YEAR = choices[at];
                request(ACT_NONE, 0, &[]);
            }
        }
        redraw = true;
    }
    if remote::CLIENT && WAITING {
        let arrived = std::mem::take(&mut remote::ARRIVED) | remote::hold_over();
        if arrived || LOOKED.is_none_or(|t| t.elapsed().as_millis() > 500) {
            LOOKED = Some(std::time::Instant::now());
            redraw = true;
        }
    }
    Some(redraw)
}

fn fields(line: &[u8]) -> Vec<&[u8]> {
    line.split(|&c| c == b'\t').collect()
}

fn number(t: &[u8]) -> i32 {
    std::str::from_utf8(t).ok().and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn field<'a>(line: &'a [&'a [u8]], k: usize) -> &'a [u8] {
    line.get(k).copied().unwrap_or(b"")
}

/// The game's font numbers go by size; a small screen two sizes down.
unsafe fn f(n: u32) -> u32 {
    FONT & !0xffff | n.saturating_sub(if screen().1 < 800 { 2 } else { 0 }).max(1)
}

/// A rounded box: the middle at once, the corners smooth.
unsafe fn round((x1, y1, x2, y2): Rect, r: i32, c: [u8; 3]) {
    let r = r.min((y2 - y1) / 2).min((x2 - x1) / 2).max(1);
    fill(x1 + r, y1, x2 - r, y2, c);
    fill(x1, y1 + r, x1 + r, y2 - r, c);
    fill(x2 - r, y1 + r, x2, y2 - r, c);
    let colour = rgb565(c);
    for (x, y, dx, dy) in [(x1, y1, 1, 1), (x2 - r, y1, 0, 1), (x1, y2 - r, 1, 0), (x2 - r, y2 - r, 0, 0)] {
        blend(x, y, r, r, |col, row| {
            let (cx, cy) = (if dx == 1 { r as f32 } else { 0.0 }, if dy == 1 { r as f32 } else { 0.0 });
            let (px, py) = (col as f32 + 0.5 - cx, row as f32 + 0.5 - cy);
            (colour, ((r as f32 + 0.5 - (px * px + py * py).sqrt()).clamp(0.0, 1.0) * 255.0) as u32)
        });
    }
}

/// A panel of the skin.
unsafe fn card(r: Rect) {
    fill(r.0, r.1, r.2, r.3, AREA);
}

unsafe fn lit(spot: &Spot) -> bool {
    #[allow(static_mut_refs)]
    let hovered = HOVERED.as_ref();
    hovered == Some(spot)
}

#[allow(static_mut_refs)]
unsafe fn spot(r: Rect, s: Spot) {
    SPOTS.push((r, s));
}

/// A card's heading: an icon, capitals and the line under them.
unsafe fn heading(x: i32, y: i32, right: i32, h: i32, icon: u32, title: &[u8]) {
    draw_icon(icon, x + h / 2, y + h / 6, h * 6 / 10, MEDAL);
    text_at(x + h + 4, y, y + h, f(4), INK, title, 0);
    fill(x, y + h + 2, right, y + h + 3, CARD);
}

/// A button: on `active` the site's indigo.
unsafe fn button(r: Rect, label: &[u8], s: Spot, active: bool) {
    let c = if active { SELECTED } else if lit(&s) { BORDER } else { CARD };
    round(r, 4, c);
    text_at((r.0 + r.2) / 2, r.1, r.3, f(3), if active { GOLD } else { INK }, label, 1);
    spot(r, s);
}

/// The managers as buttons, left to right from x; the end.
unsafe fn chips(x: i32, y: i32, h: i32, names: &[&[u8]], chosen: usize, to_spot: fn(usize) -> Spot) -> i32 {
    let mut x = x;
    for (k, n) in names.iter().enumerate() {
        let w = text_width(f(3), n) + 28;
        button((x, y, x + w, y + h), n, to_spot(k), k == chosen);
        x += w + 8;
    }
    x
}

/// A big number with a label under it.
unsafe fn tile(r: Rect, value: &[u8], label: &[u8], sub: &[u8]) {
    card(r);
    let h = r.3 - r.1;
    let value = clip_text(f(6), value, r.2 - r.0 - 12);
    text_at((r.0 + r.2) / 2, r.1 + h / 10, r.1 + h * 55 / 100, f(6), PRIMARY, &value, 1);
    text_at((r.0 + r.2) / 2, r.1 + h * 52 / 100, r.1 + h * 77 / 100, f(2), MUTED, label, 1);
    text_at((r.0 + r.2) / 2, r.1 + h * 72 / 100, r.3 - 2, f(2), INK, &clip_text(f(2), sub, r.2 - r.0 - 12), 1);
}

/// The page, right of the game's menu bar, over what the game drew this frame: its title band, its
/// tabs and its Back as on a game screen.
#[allow(static_mut_refs)]
pub unsafe fn overlay() {
    if !OPEN {
        return;
    }
    if remote::CLIENT && WAITING {
        if let Some(v) = ask_host(&KEY) {
            PAGE = v;
            WAITING = false;
        }
    }
    let (sw, sh) = screen();
    let left = bar_end(sw);
    let (x1, x2) = (left + sw / 120, sw - sw / 76);
    // (the game's rectangle takes the last pixel, not one past it)
    fill(left, 0, sw - 1, sh - 1, BACKGROUND);
    HOVERED = HOVER.and_then(|k| SPOTS.get(k)).map(|s| s.1.clone());
    SPOTS.clear();
    let page = std::mem::take(&mut PAGE);
    {
        let lines: Vec<Vec<&[u8]>> = page.split(|&c| c == b'\n').map(fields).collect();
        let of = |k: &[u8]| lines.iter().filter(|l| l[0] == k).collect::<Vec<_>>();
        let head = of(b"C");
        let empty = Vec::new();
        let head = head.first().copied().unwrap_or(&empty);
        let managers = of(b"K");
        // the title band: the viewer's crest, where he stands
        let band = (x1, sh * 16 / 1000, x2, sh * 116 / 1000);
        let h = band.3 - band.1;
        fill(band.0, band.1, band.2, band.3, BG);
        fill(band.0, band.3 - 3, band.2, band.3, GOLD);
        let first = |s: &[u8]| s.split(|&c| c == b' ').next().unwrap_or(b"").to_ascii_lowercase();
        let place = managers.iter().position(|m| first(m[1]) == first(field(head, 2)));
        if let Some(m) = place.map(|k| managers[k]) {
            let size = h * 76 / 100;
            draw_crest(m[2], x1 + h / 4, band.1 + (h - size) / 2, size);
        }
        draw_icon(ICON_TROPHY, x2 - h * 2 / 3, band.1 + h / 8, h * 6 / 10, GOLD);
        text_at((x1 + x2) / 2, band.1 + 4, band.1 + h * 58 / 100, f(6), INK, b"CM Challenge", 1);
        let line = match place {
            // (alone in the league: his rank, not a place)
            Some(k) if managers.len() == 1 => [managers[k][4], format!(" with {} points", latin1(managers[k][3])).as_bytes(), b"  -  Season ", field(head, 1)].concat(),
            Some(k) => [format!("{} with {} points", ordinal(k + 1), latin1(managers[k][3])).as_bytes(), b"  -  Season ", field(head, 1)].concat(),
            None => [b"Season ", field(head, 1)].concat(),
        };
        text_at((x1 + x2) / 2, band.1 + h * 54 / 100, band.3 - 6, f(4), INK, &line, 1);
        // the tabs
        let (tabs, tab_h) = (band.3 + sh * 17 / 1000, sh * 59 / 1000);
        let with_goals = !of(b"Q").is_empty();
        let order: &[usize] = if with_goals { &[0, 1, TAB_RAD, TAB_GOALS, 3, 4, 5] } else { &[0, 1, TAB_RAD, 3, 4, 5] };
        let tw = (x2 - x1) / order.len() as i32;
        for (slot, &k) in order.iter().enumerate() {
            let (label, slot) = (if k == TAB_GOALS { &b"Goals"[..] } else { TABS[k] }, slot as i32);
            let r = (x1 + slot * tw, tabs, x1 + (slot + 1) * tw - 2, tabs + tab_h);
            let s = Spot::Tab(k);
            fill(r.0, r.1, r.2, r.3, if lit(&s) && TAB != k { CARD } else { AREA });
            text_at((r.0 + r.2) / 2, r.1, r.3, f(4), if TAB == k { GOLD } else { INK }, label, 1);
            if TAB == k {
                fill(r.0, r.3 - 3, r.2, r.3, GOLD);
            }
            spot(r, s);
        }
        // Back
        let b = (x1, sh * 929 / 1000, x2, sh * 988 / 1000);
        fill(b.0, b.1, b.2, b.3, if lit(&Spot::Back) { SELECTED } else { BORDER });
        text_at((b.0 + b.2) / 2, b.1, b.3, f(4), if lit(&Spot::Back) { GOLD } else { INK }, b"Back", 1);
        spot(b, Spot::Back);
        let area = (x1, tabs + tab_h + sh * 15 / 1000, x2, b.1 - sh * 15 / 1000);
        if page.is_empty() {
            text_at((x1 + x2) / 2, sh / 2 - 30, sh / 2 + 30, f(4), MUTED, b"Fetching the standings...", 1);
        } else {
            let names: Vec<&[u8]> = managers.iter().map(|k| k[1]).collect();
            // (the seasons there are, newest first: the Season and Stats steppers)
            let mut years: Vec<i32> = of(b"L").iter().map(|l| number(field(l, 5))).filter(|&y| y > 0).collect();
            years.sort_unstable_by(|a, b| b.cmp(a));
            years.dedup();
            STATS_YEARS = years;
            match TAB {
                0 => dashboard(area, &managers, &of(b"F"), &of(b"G")),
                1 => this_year(area, &of(b"L"), &of(b"R"), &of(b"P"), &of(b"Q"), &of(b"W"), number(field(head, 4)), field(head, 2)),
                TAB_RAD => rads(area, &of(b"R"), &of(b"A"), head, &names),
                3 => stats(area, &of(b"T"), &of(b"Y"), &names),
                4 => past(area, &of(b"H"), &names),
                TAB_GOALS if with_goals => goals(area, &of(b"Q"), &of(b"W"), &of(b"M"), &of(b"N"), head, &names),
                _ => rules(area, &of(b"P"), &of(b"G"), &of(b"A")),
            }
        }
    }
    PAGE = page;
    if SPIN.is_some() {
        SPIN_DRAWN = Some(std::time::Instant::now());
    }
}

/// A rank's medal: a coloured disc with a star.
unsafe fn medal(cx: i32, cy: i32, size: i32, step: usize) {
    let c = RANKS[step.min(RANKS.len() - 1)];
    round((cx - size / 2, cy - size / 2, cx + size / 2, cy + size / 2), size / 2, c);
    if step > 0 {
        draw_icon(ICON_STAR, cx, cy - size * 3 / 10, size * 6 / 10, DARK);
    }
}

/// A bar on a dark track.
unsafe fn bar(x1: i32, x2: i32, cy: i32, h: i32, percent: i32, c: [u8; 3]) {
    round((x1, cy - h / 2, x2, cy + h / 2), h / 2, DARK);
    let w = (x2 - x1) * percent.clamp(0, 100) / 100;
    if w >= h {
        round((x1, cy - h / 2, x1 + w, cy + h / 2), h / 2, c);
    }
}

/// Dashboard: the standings with rank and the way to the next, the facts, the ranks; with one
/// manager his career.
unsafe fn dashboard((x1, y1, x2, y2): Rect, rows: &Lines, facts: &Lines, ranks: &Lines) {
    let (w, h) = (x2 - x1, y2 - y1);
    // one manager (a single player game): his career, the standing a short card and the facts in two rows
    let alone = rows.len() == 1;
    let (tiles, legend) = (h * if alone { 17 } else { 15 } / 100, h * 12 / 100);
    let list_end = if alone { y1 + h * 28 / 100 } else { y2 - tiles - legend - 28 };
    card((x1, y1, x2, list_end));
    let (x1i, x2i) = (x1 + 16, x2 - 16);
    let head = h * 5 / 100;
    heading(x1i, y1 + 8, x2i, head, ICON_TROPHY, if alone { b"CAREER" } else { b"STANDINGS" });
    let col = |p: i32| x1i + (x2i - x1i) * p / 100;
    let top = y1 + head + 14;
    for (label, p, align) in [(&b"#"[..], 3, 2), (b"MANAGER", 12, 0), (b"RANK", 45, 1), (b"PROGRESS", 52, 0), (b"POINTS", 100, 2)] {
        text_at(col(p), top, top + 24, f(2), MUTED, label, align);
    }
    // (by the rows there are: one manager's career row was a third of the room, his name and club
    // on top of each other, night 6 E-1 / C-36 / G-11)
    let rh = ((list_end - top - 30) / (rows.len() as i32).max(1)).min(if alone { h / 10 } else { h / 7 });
    for (k, r) in rows.iter().enumerate() {
        let y = top + 26 + k as i32 * rh;
        let (name, club, total, rank, next, percent, step) = (r[1], r[2], r[3], r[4], r[5], number(r[6]), number(field(r, 7)));
        if k % 2 == 0 {
            round((x1i - 6, y + 2, x2i + 6, y + rh - 2), 4, BG);
        }
        text_at(col(3), y, y + rh, f(4), MUTED, (k + 1).to_string().as_bytes(), 2);
        let size = rh * 66 / 100;
        draw_crest(club, col(4) + 6, y + (rh - size) / 2, size);
        text_at(col(12), y + rh / 8, y + rh / 2 + 3, f(4), INK, name, 0);
        text_at(col(12), y + rh / 2 + 1, y + rh * 7 / 8, f(2), MUTED, club, 0);
        medal(col(45), y + rh / 2, rh * 52 / 100, step as usize);
        let (bx1, bx2, by) = (col(52), col(90), y + rh * 62 / 100);
        text_at(bx1, y + rh / 8, by - 5, f(2), MUTED, rank, 0);
        text_at(bx2, y + rh / 8, by - 5, f(2), MUTED, &[total, b" / ", next].concat(), 2);
        bar(bx1, bx2, by + 3, 10, percent, PRIMARY);
        let wide = text_width(f(4), total) + 24;
        let badge = (col(100) - wide, y + rh / 4, col(100), y + rh * 3 / 4);
        round(badge, 4, PRIMARY);
        text_at((badge.0 + badge.2) / 2, badge.1, badge.3, f(4), DARK, total, 1);
    }
    // the facts: an icon, a big value, what it is
    let y = list_end + 12;
    let n = (facts.len() as i32).max(1).min(if alone { 3 } else { 99 });
    let tw = (w - (n - 1) * 12) / n;
    for (k, t) in facts.iter().enumerate() {
        let (x, y) = (x1 + (k as i32 % n) * (tw + 12), y + (k as i32 / n) * (tiles + 12));
        card((x, y, x + tw, y + tiles));
        let icon = [0xE787, ICON_TROPHY, 0xE821, 0xE7C1][k.min(3)];
        draw_icon(icon, x + tw / 7, y + tiles / 3, tiles / 3, MEDAL);
        let room = tw - tw / 4 - 8;
        let font = if text_width(f(6), t[2]) <= room { f(6) } else { f(4) };
        text_at(x + tw / 4, y + tiles / 8, y + tiles * 6 / 10, font, INK, &clip_text(font, t[2], room), 0);
        text_at(x + tw / 4, y + tiles * 6 / 10, y + tiles * 9 / 10, f(2), MUTED, t[1], 0);
    }
    // the ranks to reach
    let y = y + (if alone { facts.len().div_ceil(3).max(1) as i32 } else { 1 }) * (tiles + 12);
    card((x1, y, x2, y2));
    let n = (ranks.len() as i32).max(1);
    let cw = (w - 24) / n;
    for (k, r) in ranks.iter().enumerate() {
        let x = x1 + 12 + k as i32 * cw;
        let (cy, size) = (y + legend / 2, legend * 42 / 100);
        medal(x + size, cy, size, k + 1);
        text_at(x + size * 2, y + legend / 6, cy + 2, f(2), INK, &clip_text(f(2), r[1], cw - size * 2), 0);
        text_at(x + size * 2, cy, y + legend * 5 / 6, f(2), MUTED, &[r[2], b" pnt"].concat(), 0);
    }
}

/// Season: this season so far, a card a manager with the game's results as chips.
/// "<  label  >" at the right of a row ending at x2: the arrows only where there is more.
unsafe fn stepper(x2: i32, y: i32, h: i32, label: &[u8], earlier: Option<Spot>, later: Option<Spot>) {
    let w = text_width(f(3), label) + 32;
    let (right, mid) = (x2 - h - 6, x2 - h - 6 - w);
    if let Some(s) = later {
        button((x2 - h, y, x2, y + h), b">", s, false);
    }
    round((mid, y, right, y + h), 4, CARD);
    text_at((mid + right) / 2, y, y + h, f(3), GOLD, label, 1);
    if let Some(s) = earlier {
        button((mid - 6 - h, y, mid - 6, y + h), b"<", s, false);
    }
}

/// Season: a season of every manager (the stepper picks it, this season first). A card each: the
/// prizes at the left, each with its competition, team and day under it; at the right the RAD
/// challenges done (green) or failed (red) that season. The design the user chose on 2026-11-06.
#[allow(static_mut_refs)]
unsafe fn this_year((x1, y1, x2, y2): Rect, rows: &Lines, rads: &Lines, prizes: &Lines, goals: &Lines, marks: &Lines, now: i32, me: &[u8]) {
    let years = STATS_YEARS.clone();
    // (opened: the running season, not a later one a league file may already hold)
    if SEASON_BACK == usize::MAX {
        SEASON_BACK = years.iter().position(|&y| y == now).unwrap_or(0);
    }
    SEASON_BACK = SEASON_BACK.min(years.len().saturating_sub(1));
    let year = years.get(SEASON_BACK).copied().unwrap_or(now);
    let sh = (y2 - y1) * 5 / 100;
    let label = format!("Season {year}/{}", year + 1);
    stepper(x2, y1, sh, label.as_bytes(), (SEASON_BACK + 1 < years.len()).then_some(Spot::SeasonEarlier), (SEASON_BACK > 0).then_some(Spot::SeasonLater));
    let rows: Vec<&&Vec<&[u8]>> = rows.iter().filter(|r| number(field(r, 5)) == year).collect();
    let top = y1 + sh + 10;
    let n = (rows.len() as i32).max(1);
    let bh = (y2 - top - (n - 1) * 12) / n;
    let head_line = text_width(f(3), b"Mg") * 2 / 3 + 8;
    // (the lists' rows: small text, so more fits)
    let line = text_width(f(2), b"Mg") * 2 / 3 + 6;
    let small = text_width(f(1), b"Mg") * 2 / 3 + 4;
    for (k, r) in rows.iter().enumerate() {
        let y = top + k as i32 * (bh + 12);
        card((x1, y, x2, y + bh));
        let head = (head_line * 2).min(bh / 3);
        draw_crest(r[2], x1 + 14, y + 8, head - 10);
        text_at(x1 + head + 16, y + 6, y + head / 2 + 6, f(4), INK, r[1], 0);
        text_at(x1 + head + 16, y + head / 2 + 4, y + head + 2, f(2), MUTED, r[2], 0);
        let wide = text_width(f(5), r[3]) + 34;
        let badge = (x2 - 14 - wide, y + 10, x2 - 14, y + head - 2);
        round(badge, 4, PRIMARY);
        text_at((badge.0 + badge.2) / 2, badge.1, badge.3, f(5), DARK, r[3], 1);
        text_at(badge.0 - 10, badge.1, badge.3, f(2), MUTED, b"points this season", 2);
        // his own mark for the season, 1 to 10: boxes to click on his own card (a click on the mark
        // he has takes it away), the mark as text on the others' (user, 2026-11-06)
        let mark = number(field(r, 6));
        let (mx, my, side) = ((x1 + x2) / 2 - 140, y + 12, (head - 18).clamp(14, 26));
        if !me.is_empty() && first_word(r[1]) == first_word(me) {
            text_at(mx - 10, my, my + side, f(2), MUTED, b"Your mark", 2);
            for v in 1..=10u32 {
                let bx = mx + (v as i32 - 1) * (side + 3);
                let s = Spot::Rate((year as u32) << 4 | if mark == v as i32 { 0 } else { v });
                let fill_c = if mark >= v as i32 { PRIMARY } else if lit(&s) { BORDER } else { CARD };
                round((bx, my, bx + side, my + side), 3, fill_c);
                text_at(bx + side / 2, my, my + side, f(1), if mark >= v as i32 { DARK } else { MUTED }, v.to_string().as_bytes(), 1);
                spot((bx, my, bx + side, my + side), s);
            }
        } else if mark > 0 {
            text_at(mx, my, my + side, f(2), MUTED, format!("Mark {mark}/10").as_bytes(), 0);
        }
        let (left, split, right, bottom) = (x1 + 16, (x1 + x2) / 2 + 8, x2 - 16, y + bh - 4);
        let body = y + head + 6;
        text_at(left, body, body + small, f(1), MUTED, b"TROPHIES", 0);
        text_at(split, body, body + small, f(1), MUTED, b"CHALLENGES DONE", 0);
        let list_top = body + small + 4;
        // the results (trophies and places), then the bonuses (club changes, loyalty, a long wait
        // ended: the rules' categories) under their own heading (user, 2026-11-06)
        let bonus = |name: &[u8]| prizes.iter().any(|p| field(p, 2) == name && [&b"Club changes"[..], b"Trophy Drought"].contains(&field(p, 1)));
        let (mut won, mut bonuses): (Vec<Entry>, Vec<Entry>) = (Vec::new(), Vec::new());
        for item in r[4].split(|&c| c == b'|').filter(|i| !i.is_empty()) {
            let p: Vec<&[u8]> = item.split(|&c| c == b';').collect();
            let e = Entry { icon: (field(&p, 2) != b"0").then_some((ICON_TROPHY, MEDAL)), name: field(&p, 0).to_vec(), points: Some(number(field(&p, 1))), detail: field(&p, 3).to_vec(), muted: false };
            if bonus(field(&p, 0)) { bonuses.push(e) } else { won.push(e) }
        }
        if won.is_empty() {
            text_at(left, list_top, list_top + line, f(2), MUTED, if year == now { &b"No results yet this season"[..] } else { b"No results" }, 0);
        }
        let end = entries((left - 4, list_top, split - 16, bottom), &won, line, small);
        if !bonuses.is_empty() {
            let at = end.max(list_top + line + 4) + 4;
            text_at(left, at, at + small, f(1), MUTED, b"BONUSES", 0);
            entries((left - 4, at + small + 4, split - 16, bottom), &bonuses, line, small);
        }
        // the challenges of that season: done or reached (green, with what the game read and who
        // decided), failed (red)
        let shown: Vec<Entry> = rads.iter().filter(|c| first_word(c[2]) == first_word(r[1]) && number(field(c, 6)) == year).filter_map(|c| {
            let (text, points, note, host) = (field(c, 5), number(field(c, 3)), field(c, 8), field(c, 10) == b"1");
            match standing(c, now).2 {
                2 => Some(Entry { icon: Some((ICON_CROSS, DANGER)), name: [text, if host { &b"  -  failed (host)"[..] } else { b"  -  failed" }].concat(), points: None, detail: Vec::new(), muted: true }),
                1 => {
                    let who: &[u8] = if host { b"ticked by the host" } else { b"the game" };
                    let detail = if note.is_empty() { who.to_vec() } else { [note, b"  -  ", who].concat() };
                    Some(Entry { icon: Some((ICON_CHECK, SUCCESS)), name: text.to_vec(), points: Some(points), detail, muted: false })
                }
                _ => None,
            }
        }).collect();
        if shown.is_empty() {
            text_at(split, list_top, list_top + line, f(2), MUTED, b"None", 0);
        }
        let end = entries((split - 4, list_top, right, bottom), &shown, line, small);
        // the career goals reached that season, a group of their own (user, 2026-11-06)
        let reached: Vec<Entry> = marks.iter().filter(|w| first_word(w[1]) == first_word(r[1]) && number(field(w, 3)) == year).filter_map(|w| {
            let goal = goals.iter().find(|q| field(q, 1) == field(w, 2))?;
            let country = field(w, 4);
            let who: &[u8] = if field(w, 5) == b"1" { b"ticked by the host" } else { b"the game" };
            let detail = if country.is_empty() { who.to_vec() } else { [country, b"  -  ", who].concat() };
            Some(Entry { icon: Some((ICON_STAR, MEDAL)), name: field(goal, 3).to_vec(), points: Some(number(field(goal, 4))), detail, muted: false })
        }).collect();
        if !reached.is_empty() {
            let at = end.max(list_top + line + 4) + 4;
            text_at(split, at, at + small, f(1), MUTED, b"CAREER GOALS", 0);
            entries((split - 4, at + small + 4, right, bottom), &reached, line, small);
        }
    }
}

/// A row of the Season tab: icon, name, points (None: not shown), a detail line, a muted name.
struct Entry {
    icon: Option<(u32, [u8; 3])>,
    name: Vec<u8>,
    points: Option<i32>,
    detail: Vec<u8>,
    muted: bool,
}

/// The rows in a box as roomy as they fit: two lines each (the detail under the name), else one
/// line each (the detail after it), else one line in two columns; what still does not fit is left.
/// The bottom of what was drawn.
unsafe fn entries((x1, y1, x2, y2): Rect, list: &[Entry], line: i32, small: i32) -> i32 {
    let full: i32 = list.iter().map(|e| line + if e.detail.is_empty() { 0 } else { small } + 10).sum();
    let one = line + 6;
    let (two_lines, columns) = if full <= y2 - y1 { (true, 1) } else if one * list.len() as i32 <= y2 - y1 { (false, 1) } else { (false, 2) };
    let per = if columns == 2 { ((y2 - y1) / one).max(1) as usize } else { list.len() };
    let cw = (x2 - x1 - (columns - 1) * 8) / columns;
    let (mut y, mut end) = (y1, y1);
    // (small text, so a season with many trophies still fits: user, 2026-11-06)
    let (big, tiny) = (f(2), f(1));
    for (k, e) in list.iter().enumerate() {
        let col = (k / per.max(1)) as i32;
        if col >= columns {
            break;
        }
        if k % per.max(1) == 0 && k > 0 {
            y = y1;
        }
        let (bx1, bx2) = (x1 + col * (cw + 8), x1 + col * (cw + 8) + cw);
        let h = if two_lines && !e.detail.is_empty() { line + small + 6 } else { line + 2 };
        if y + h > y2 {
            continue;
        }
        round((bx1, y, bx2, y + h), 4, CARD);
        if let Some((icon, colour)) = e.icon {
            draw_icon(icon, bx1 + 8, y + 1 + line / 6, line * 6 / 10, colour);
        }
        let tx = bx1 + line + 8;
        let value = e.points.map(|p| format!("{p:+}")).unwrap_or_default();
        let value_w = if value.is_empty() { 0 } else { text_width(big, value.as_bytes()) + 12 };
        let name_w = text_width(big, &e.name).min(bx2 - 12 - value_w - tx);
        text_at(tx, y + 1, y + 1 + line, big, if e.muted { MUTED } else { INK }, &clip_text(big, &e.name, bx2 - 12 - value_w - tx), 0);
        if let Some(p) = e.points {
            text_at(bx2 - 10, y + 1, y + 1 + line, big, if p < 0 { ROSE } else { CYAN }, value.as_bytes(), 2);
        }
        if !e.detail.is_empty() {
            if two_lines {
                text_at(tx, y + 1 + line, y + 1 + line + small, tiny, MUTED, &clip_text(tiny, &e.detail, bx2 - 10 - tx), 0);
            } else if columns == 1 {
                let dx = tx + name_w + 14;
                text_at(dx, y + 1, y + 1 + line, tiny, MUTED, &clip_text(tiny, &e.detail, bx2 - 12 - value_w - dx), 0);
            }
        }
        end = end.max(y + h);
        y += h + 4;
    }
    end
}

/// Where a challenge stands: its word, its colour and its group (0 active, 1 reached, 2 failed).
fn standing(row: &[&[u8]], now: i32) -> (&'static [u8], [u8; 3], usize) {
    let (done, state, year) = (row[4] == b"1", number(field(row, 7)) as u8, number(field(row, 6)));
    match () {
        _ if done => (b"Done", SUCCESS, 1),
        _ if state == track::BAD => (b"Failed", DANGER, 2),
        _ if year < now => (b"Not done", DANGER, 2),
        _ if state == track::GOOD => (b"Reached", SUCCESS, 1),
        _ => (b"Active", CYAN, 0),
    }
}

/// RAD: at the left every manager at a glance (a click shows his) and the wheel, at the right the
/// chosen manager's challenges of one season by where they stand.
#[allow(static_mut_refs)]
unsafe fn rads((x1, y1, x2, y2): Rect, rows: &Lines, tasks: &Lines, head: &[&[u8]], names: &[&[u8]]) {
    let host = field(head, 3) == b"1";
    let now = number(field(head, 4));
    let year = now - BACK_YEARS;
    let season = format!("{}/{}", year, year + 1);
    PICK = PICK.min(names.len().saturating_sub(1));
    let chosen = names.get(PICK).copied().unwrap_or(b"");
    let word = |s: &[u8]| s.split(|&c| c == b' ').next().unwrap_or(b"").to_vec();
    let of_season = |n: &[u8]| rows.iter().filter(|r| number(r[6]) == year && word(r[2]).eq_ignore_ascii_case(&word(n))).collect::<Vec<_>>();
    // what the wheel gave
    if SPIN.is_none() {
        if let Some(before) = NEW_FROM.take() {
            let top = rows.iter().map(|r| number(r[1])).max().unwrap_or(0);
            (NEW_ROW, NOTHING_LEFT) = (Some(top).filter(|&t| t > before), top <= before);
        }
    }
    let mid = x1 + (x2 - x1) * 36 / 100;
    let line = (y2 - y1) * 7 / 100;
    // every manager at a glance
    let left_end = mid - 6;
    card((x1, y1, left_end, y2));
    let (lx, lr) = (x1 + 12, left_end - 12);
    text_at(lx, y1 + 2, y1 + line, f(4), GOLD, &[b"Managers  ", season.as_bytes()].concat(), 0);
    let col = |k: i32| lr - (2 - k) * (lr - lx) / 7;
    let heads: [&[u8]; 3] = [b"Active", b"Done", b"Failed"];
    for (k, label) in heads.iter().enumerate() {
        text_at(col(k as i32), y1 + line, y1 + line * 3 / 2, f(2), MUTED, label, 2);
    }
    let mut y = y1 + line * 3 / 2;
    for (k, n) in names.iter().enumerate() {
        let r = (x1 + 4, y, left_end - 4, y + line - 2);
        let s = Spot::Manager(k);
        fill(r.0, r.1, r.2, r.3, if PICK == k { SELECTED } else if lit(&s) { CARD } else { BG });
        text_at(lx, r.1, r.3, f(3), if PICK == k { GOLD } else { INK }, &clip_text(f(3), n, (lr - lx) * 4 / 7 - 8), 0);
        let mut count = [0; 3];
        for row in of_season(n) {
            count[standing(row, now).2] += 1;
        }
        for (c, colour) in [(0, CYAN), (1, SUCCESS), (2, DANGER)] {
            text_at(col(c as i32), r.1, r.3, f(3), if count[c] > 0 { colour } else { MUTED }, count[c].to_string().as_bytes(), 2);
        }
        spot(r, s);
        y += line;
    }
    y += 8;
    // the wheel: always for the season running, for the manager chosen
    if host {
        let r = (lx, y, lr, y + line * 13 / 10);
        let busy = SPIN.is_some();
        fill(r.0, r.1, r.2, r.3, if busy { CARD } else if lit(&Spot::Spin) { SELECTED } else { BORDER });
        let who = [b"Spin for ", &word(chosen)[..]].concat();
        text_at((lx + lr) / 2, r.1 + 2, r.1 + line * 8 / 10, f(4), GOLD, &who, 1);
        text_at((lx + lr) / 2, r.1 + line * 7 / 10, r.3 - 2, f(2), MUTED, &[b"season ", field(head, 1)].concat(), 1);
        if !busy {
            spot(r, Spot::Spin);
        }
        y += line * 13 / 10 + 8;
    }
    let shown = (lx, y, lr, y2 - 10);
    fill(shown.0, shown.1, shown.2, shown.3, BG);
    let (cx, cy) = ((shown.0 + shown.2) / 2, (shown.1 + shown.3) / 2);
    let (tl, tr) = (shown.0 + 12, shown.2 - 12);
    let new = NEW_ROW.and_then(|id| rows.iter().find(|r| number(r[1]) == id));
    if let Some((since, name)) = &SPIN {
        let ms = since.elapsed().as_millis() as usize;
        let t = tasks[(ms / 90 * 7) % tasks.len().max(1)];
        text_at(cx, shown.1 + 6, shown.1 + line, f(3), GOLD, &[b"RAD spinning for ", &word(name)[..], b"..."].concat(), 1);
        assistant::report_text(tl, cy - line / 2, tr, shown.3 - 6, f(3), line * 7 / 10, t[2]);
    } else if let Some(r) = new {
        text_at(cx, shown.1 + 6, shown.1 + line, f(2), GOLD, b"NEW CHALLENGE", 1);
        text_at(cx, shown.1 + line, shown.1 + line * 2, f(3), INK, &[&word(r[2])[..], b"  -  season ", field(head, 1)].concat(), 1);
        assistant::report_text(tl, shown.1 + line * 2, tr, shown.3 - line, f(3), line * 7 / 10, r[5]);
        text_at(cx, shown.3 - line, shown.3 - 4, f(4), GOLD, &[b"+", r[3], b" points"].concat(), 1);
    } else {
        let says: &[u8] = match (NOTHING_LEFT, host, names.len()) {
            (true, _, _) => b"No challenge left",
            (_, true, 1) => b"Spin to set yourself a challenge",
            (_, true, _) => b"Pick a manager and spin",
            _ => b"The host spins and ticks off",
        };
        text_at(cx, cy - line, cy + line, f(3), MUTED, says, 1);
    }
    // his challenges
    card((mid + 6, y1, x2, y2));
    let (rx, rr) = (mid + 18, x2 - 12);
    text_at(rx, y1 + 2, y1 + line, f(4), GOLD, chosen, 0);
    let w = text_width(f(4), b"0000/0000") + 24;
    let step = line * 8 / 10;
    let first = rows.iter().map(|r| number(r[6])).min().unwrap_or(year);
    text_at(rr - step - w / 2, y1 + 2, y1 + line, f(4), INK, season.as_bytes(), 1);
    if year > first.min(now) {
        button((rr - 2 * step - w, y1 + 6, rr - step - w, y1 + 6 + step), b"<", Spot::Earlier, false);
    }
    if BACK_YEARS > 0 {
        button((rr - step, y1 + 6, rr, y1 + 6 + step), b">", Spot::Later, false);
    }
    fill(rx, y1 + line + 2, rr, y1 + line + 3, BORDER);
    let mine = of_season(chosen);
    // (a row the game reads reached counts too: only "at least" challenges get there, and they stay
    // reached; `track::judge`)
    let points: i32 = mine.iter().filter(|r| standing(r, now).2 == 1).map(|r| number(r[3])).sum();
    let rh = (y2 - y1 - line * 3) / 8;
    let mut y = y1 + line + 8;
    if mine.is_empty() {
        text_at(rx, y, y + line, f(3), MUTED, b"No challenges this season", 0);
    }
    for (group, title) in [(0, &b"ACTIVE"[..]), (1, b"DONE"), (2, b"FAILED")] {
        let rows: Vec<_> = mine.iter().filter(|r| standing(r, now).2 == group).collect();
        if rows.is_empty() {
            continue;
        }
        text_at(rx, y, y + line * 6 / 10, f(2), GOLD, format!("{} ({})", latin1(title), rows.len()).as_bytes(), 0);
        y += line * 6 / 10;
        for r in rows {
            if y + rh > y2 - line * 7 / 10 {
                break;
            }
            challenge_row((rx, y, rr, y + rh - 3), r, now, host);
            y += rh;
        }
    }
    let foot = if host { format!("{points} points in. A click on a row: done, then failed, then the game's again. The cross removes it.") } else { format!("{points} points in. The host spins and ticks off.") };
    text_at(rx, y2 - line * 7 / 10, y2 - 2, f(2), MUTED, foot.as_bytes(), 0);
}

/// A challenge: its box (the host's click ticks it), the text, what the game has read so far, the
/// points, where it stands; for the host a cross at the end removes it.
#[allow(static_mut_refs)]
unsafe fn challenge_row(r: Rect, row: &[&[u8]], now: i32, host: bool) {
    let (id, done, note, tracked, manual) = (number(row[1]), row[4] == b"1", field(row, 8), field(row, 9) == b"1", field(row, 10) == b"1");
    let h = r.3 - r.1;
    let tick = Spot::Tick(id as u32);
    let new = NEW_ROW == Some(id);
    fill(r.0, r.1, r.2, r.3, if new { SELECTED } else if lit(&tick) { CARD } else { BG });
    let (word, colour, group) = standing(row, now);
    fill(r.0, r.1, r.0 + 4, r.3, colour);
    let size = h * 50 / 100;
    let mark = (r.0 + 14, r.1 + (h - size) / 2, r.0 + 14 + size, r.1 + (h + size) / 2);
    // (ticked when done, and as soon as the game reads it reached: user, 2026-11-07)
    let ticked = done || group == 1;
    round(mark, 3, if ticked { SUCCESS } else { BORDER });
    if ticked {
        draw_icon(ICON_CHECK, (mark.0 + mark.2) / 2, mark.1 + size / 5, size * 6 / 10, INK);
    }
    let tx = mark.2 + 10;
    let badge = text_width(f(2), b"Not done") + 20;
    let room = r.2 - tx - badge - 96;
    text_at(tx, r.1 + 1, r.1 + h * 58 / 100, f(3), INK, &clip_text(f(3), row[5], room), 0);
    let says: Vec<u8> = if manual {
        b"Set by the host".to_vec()
    } else if !note.is_empty() {
        note.to_vec()
    } else if tracked {
        b"The game follows this".to_vec()
    } else {
        b"The host ticks it off".to_vec()
    };
    text_at(tx, r.1 + h * 54 / 100, r.3 - 1, f(2), MUTED, &says, 0);
    let end = r.2 - if host { 30 } else { 8 };
    let pill_at = (end - badge, r.1 + h / 4, end, r.3 - h / 4);
    round(pill_at, 4, CARD);
    text_at((pill_at.0 + pill_at.2) / 2, pill_at.1, pill_at.3, f(2), colour, if new { b"New" } else { word }, 1);
    text_at(pill_at.0 - 8, r.1, r.3, f(3), GOLD, &[b"+", row[3]].concat(), 2);
    if host {
        spot((r.0, r.1, r.2 - 30, r.3), tick);
        let del = Spot::Delete(id as u32);
        draw_icon(ICON_CROSS, r.2 - 15, r.1 + h / 3, h / 3, if lit(&del) { ROSE } else { MUTED });
        spot((r.2 - 28, r.1, r.2, r.3), del);
    }
}

/// Stats: the three records, then a manager: points by season, his numbers, palmares, RAD.
#[allow(static_mut_refs)]
unsafe fn stats((x1, y1, x2, y2): Rect, tiles: &Lines, cards: &Lines, names: &[&[u8]]) {
    let (w, h) = (x2 - x1, y2 - y1);
    let th = h * 15 / 100;
    let n = (tiles.len() as i32).max(1);
    let tw = (w - (n - 1) * 12) / n;
    for (k, t) in tiles.iter().enumerate() {
        let x = x1 + k as i32 * (tw + 12);
        tile((x, y1, x + tw, y1 + th), t[2], t[1], t[3]);
    }
    let cy = y1 + th + 10;
    let ch = h * 5 / 100;
    PICK = PICK.min(names.len().saturating_sub(1));
    chips(x1, cy, ch, names, PICK, Spot::Manager);
    // the season counted (the host counts it: a click asks again), all seasons first
    #[allow(static_mut_refs)]
    {
        let at = STATS_YEARS.iter().position(|&y| y == STATS_YEAR).map_or(0, |k| k + 1);
        let label = if STATS_YEAR == 0 { "All seasons".to_string() } else { format!("Season {STATS_YEAR}/{}", STATS_YEAR + 1) };
        stepper(x2, cy, ch, label.as_bytes(), (at < STATS_YEARS.len()).then_some(Spot::StatsEarlier), (at > 0).then_some(Spot::StatsLater));
    }
    let Some(c) = cards.get(PICK) else { return };
    let (top, x1i, x2i) = (cy + ch + 10, x1 + 16, x2 - 16);
    card((x1, top, x2, y2));
    let (rank, seasons, avg, best, worst, clubs, done, all, items, points, scorer) = (c[2], c[3], c[4], c[5], c[6], c[7], number(c[8]), number(c[9]), number(c[10]), number(c[11]), c[12]);
    text_at(x1i, top + 6, top + 6 + ch, f(5), PRIMARY, c[1], 0);
    let tag_x = x1i + text_width(f(5), c[1]) + 14;
    let tag = (tag_x, top + 12, tag_x + text_width(f(2), rank) + 22, top + ch);
    round(tag, 4, CYAN);
    text_at((tag.0 + tag.2) / 2, tag.1, tag.3, f(2), DARK, rank, 1);
    // the graph: a bar a season
    let inner = top + ch + 12;
    let graph = (x1i, inner, x1i + (x2i - x1i) * 58 / 100, inner + (y2 - inner) * 46 / 100);
    round(graph, 4, BG);
    text_at(graph.0 + 10, graph.1 + 2, graph.1 + 22, f(2), MUTED, b"PROGRESS", 0);
    let bars: Vec<(i32, i32)> = c[13].split(|&b| b == b',').filter_map(|b| {
        let p: Vec<&[u8]> = b.split(|&x| x == b'=').collect();
        (p.len() == 2).then(|| (number(p[0]), number(p[1])))
    }).collect();
    let top_value = bars.iter().map(|b| b.1).max().unwrap_or(1).max(10);
    let (gw, ground, room) = (graph.2 - graph.0 - 24, graph.3 - 22, graph.3 - graph.1 - 70);
    let slot = gw / (bars.len() as i32).max(1);
    for (k, (year, value)) in bars.iter().enumerate() {
        let x = graph.0 + 12 + k as i32 * slot;
        let bh = (room * value.max(&0) / top_value).max(2);
        round((x + slot / 5, ground - bh, x + slot * 4 / 5, ground), 5, PRIMARY);
        text_at(x + slot / 2, ground - bh - 24, ground - bh - 2, f(3), INK, value.to_string().as_bytes(), 1);
        text_at(x + slot / 2, ground, graph.3, f(1), MUTED, format!("'{year:02}").as_bytes(), 1);
    }
    // his numbers
    let (mx, mw) = (graph.2 + 10, (x2i - graph.2 - 22) / 2);
    let mh = (graph.3 - graph.1 - 16) / 3;
    let numbers: [(&[u8], &[u8]); 6] = [(seasons, b"Seasons"), (avg, b"Points/season"), (best, b"Best season"), (worst, b"Worst season"), (clubs, b"Unique clubs"), (scorer, b"Top scorer")];
    for (k, (value, label)) in numbers.iter().enumerate() {
        let (x, y) = (mx + (k as i32 % 2) * (mw + 8), graph.1 + (k as i32 / 2) * (mh + 8));
        let r = (x, y, x + mw, y + mh);
        round(r, 4, BG);
        let font = if text_width(f(5), value) <= mw - 12 { f(5) } else if text_width(f(3), value) <= mw - 12 { f(3) } else { f(2) };
        text_at(x + mw / 2, y + 2, y + mh * 62 / 100, font, PRIMARY, &clip_text(font, value, mw - 12), 1);
        text_at(x + mw / 2, y + mh * 60 / 100, r.3 - 2, f(2), MUTED, label, 1);
    }
    // palmares, RAD and how his points came
    let low = graph.3 + 10;
    let split = x1i + (x2i - x1i) * 52 / 100;
    text_at(x1i, low, low + 26, f(3), INK, b"Honours", 0);
    let row = text_width(f(2), b"Mg") * 2 / 3 + 8;
    let (mut y, mut club) = (low + 28, &b""[..]);
    let palmares: Vec<Vec<&[u8]>> = c[14].split(|&b| b == b'|').filter(|p| !p.is_empty()).map(|p| p.split(|&x| x == b';').collect()).collect();
    if palmares.is_empty() {
        text_at(x1i, y, y + row, f(2), MUTED, b"No trophies yet", 0);
    }
    for p in &palmares {
        if y + row * 2 > y2 - 6 {
            break;
        }
        if field(p, 0) != club {
            club = field(p, 0);
            text_at(x1i, y, y + row, f(2), MUTED, club, 0);
            y += row;
        }
        draw_icon(ICON_TROPHY, x1i + 12, y + 2, row - 6, MEDAL);
        text_at(x1i + 28, y, y + row, f(2), INK, field(p, 1), 0);
        y += row;
    }
    text_at(split, low, low + 26, f(3), INK, b"RAD Challenges", 0);
    let percent = if all > 0 { done * 100 / all } else { 0 };
    text_at((split + x2i) / 2, low + 26, low + 74, f(6), INK, format!("{percent}%").as_bytes(), 1);
    bar(split, x2i, low + 84, 10, percent, CYAN);
    text_at(split, low + 92, low + 116, f(2), MUTED, format!("Done: {done}").as_bytes(), 0);
    text_at(x2i, low + 92, low + 116, f(2), MUTED, format!("Total: {all}").as_bytes(), 2);
    let total = (items + points).max(1);
    text_at(split, low + 118, low + 142, f(2), MUTED, b"Points split", 0);
    let (dy, cut) = (low + 148, split + (x2i - split) * items.max(0) / total);
    fill(split, dy, cut, dy + 10, ROSE);
    fill(cut, dy, x2i, dy + 10, CYAN);
    text_at(split, dy + 12, dy + 36, f(2), ROSE, format!("Results ({}%)", items * 100 / total).as_bytes(), 0);
    text_at(x2i, dy + 12, dy + 36, f(2), CYAN, format!("RAD ({}%)", points * 100 / total).as_bytes(), 2);
}

/// 1st, 2nd, 3rd, 4th...
fn ordinal(n: usize) -> String {
    let end = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{end}")
}

/// Up and down buttons for a long list.
unsafe fn pager(x: i32, y: i32, size: i32, more_above: bool, more_below: bool) {
    if more_above {
        button((x - 2 * size - 6, y, x - size - 6, y + size), b"<", Spot::Up, false);
    }
    if more_below {
        button((x - size, y, x, y + size), b">", Spot::Down, false);
    }
}

/// History: every season, newest first, of everyone or one manager.
#[allow(static_mut_refs)]
unsafe fn past((x1, y1, x2, y2): Rect, rows: &Lines, names: &[&[u8]]) {
    let ch = (y2 - y1) * 5 / 100;
    let mut list: Vec<&[u8]> = vec![b"All managers"];
    list.extend(names.iter().copied());
    chips(x1, y1, ch, &list, FILTER, Spot::Filter);
    let top = y1 + ch + 10;
    card((x1, top, x2, y2));
    let first_word = |s: &[u8]| s.split(|&c| c == b' ').next().unwrap_or(b"").to_ascii_lowercase();
    let rows: Vec<&&Vec<&[u8]>> = rows.iter().filter(|r| FILTER == 0 || names.get(FILTER - 1).is_some_and(|n| first_word(n) == first_word(r[2]))).collect();
    let (x1i, w) = (x1 + 14, x2 - x1 - 28);
    let col = |p: i32| x1i + w * p / 1000;
    let rh = text_width(f(3), b"Mg") * 2 / 3 + 12;
    let shown = ((y2 - top - rh - 8) / rh).max(1) as usize;
    SCROLL = SCROLL.min(rows.len().saturating_sub(shown));
    let cols: [(&[u8], i32, i32); 8] = [(b"SEASON", 0, 0), (b"MANAGER", 85, 0), (b"CLUB", 170, 0), (b"TOP SCORER", 320, 0), (b"G", 480, 2), (b"C", 520, 2), (b"PTS", 575, 2), (b"RESULTS", 595, 0)];
    for c in cols {
        text_at(col(c.1), top + 4, top + rh, f(2), MUTED, c.0, c.2);
    }
    for (k, r) in rows.iter().skip(SCROLL).take(shown).enumerate() {
        let y = top + rh + 4 + k as i32 * rh;
        if k % 2 == 0 {
            round((x1 + 6, y, x2 - 6, y + rh), 4, BG);
        }
        for (i, c) in cols.iter().enumerate() {
            let room = cols.get(i + 1).map_or(x1i + w - 4, |n| col(n.1)) - col(c.1) - 8;
            let t = field(r, i + 1);
            let t = if c.2 == 0 { clip_text(f(2), t, room) } else { t.to_vec() };
            text_at(col(c.1), y, y + rh, if i == 6 { f(3) } else { f(2) }, if i == 6 { PRIMARY } else if i == 1 { INK } else { MUTED }, &t, c.2);
        }
    }
    pager(x2 - 8, y1, ch, SCROLL > 0, SCROLL + shown < rows.len());
}

/// Rules: what everything is worth: prizes, ranks and the RAD list, with what the game follows.
#[allow(static_mut_refs)]
unsafe fn rules((x1, y1, x2, y2): Rect, prizes: &Lines, ranks: &Lines, tasks: &Lines) {
    let ch = (y2 - y1) * 5 / 100;
    card((x1, y1, x2, y2));
    let (x1i, x2i) = (x1 + 16, x2 - 16);
    heading(x1i, y1 + 8, x2i, ch, 0xE8A5, b"RULES");
    // every line: a head or a row (text, value, trophy or followed by the game)
    let mut all: Vec<(Vec<u8>, Vec<u8>, bool, bool)> = Vec::new();
    let mut last: &[u8] = b"";
    for p in prizes {
        if p[1] != last {
            last = p[1];
            all.push((last.to_vec(), Vec::new(), false, true));
        }
        all.push((p[2].to_vec(), [if number(p[3]) > 0 { &b"+"[..] } else { b"" }, p[3]].concat(), p[4] == b"1", false));
    }
    all.push((b"Ranks".to_vec(), Vec::new(), false, true));
    for r in ranks {
        all.push((r[1].to_vec(), r[2].to_vec(), false, false));
    }
    last = b"";
    for t in tasks {
        if t[1] != last {
            last = t[1];
            all.push(([b"RAD: ", last].concat(), Vec::new(), false, true));
        }
        all.push((t[2].to_vec(), [b"+", t[3]].concat(), t[4] == b"1", false));
    }
    let rh = text_width(f(2), b"Mg") * 2 / 3 + 6;
    let top = y1 + ch + 14;
    let per = ((y2 - top - 8) / rh).max(1) as usize;
    // (two on 800x600: three cut most rules short)
    let columns = if x2 - x1 < 720 { 2usize } else { 3 };
    let (gap, cw) = (16, (x2i - x1i - (columns as i32 - 1) * 16) / columns as i32);
    // a rule too long for its column goes on on the next row
    let room = cw - 62;
    let mut k = 0;
    while k < all.len() {
        let fits = clip_text(f(1), &all[k].0, room).len();
        if fits < all[k].0.len() {
            let cut = all[k].0[..fits].iter().rposition(|&b| b == b' ').map_or(fits.max(1), |s| s + 1);
            let rest = all[k].0.split_off(cut);
            all.insert(k + 1, (rest, Vec::new(), false, false));
        }
        k += 1;
    }
    SCROLL = SCROLL.min(all.len().saturating_sub(per * columns));
    for (k, (text, value, mark, is_head)) in all.iter().skip(SCROLL).take(per * columns).enumerate() {
        let (c, row) = ((k / per) as i32, (k % per) as i32);
        let (x, y) = (x1i + c * (cw + gap), top + row * rh);
        if *is_head {
            text_at(x, y, y + rh, f(2), CYAN, text, 0);
            continue;
        }
        if *mark {
            let (icon, colour) = if value.first() == Some(&b'+') && text.len() > 0 && prizes.iter().any(|p| p[2] == &text[..]) { (ICON_TROPHY, MEDAL) } else { (ICON_CHECK, SUCCESS) };
            draw_icon(icon, x + 9, y + 3, rh - 8, colour);
        }
        // (a long rule a size down: a client's fonts run wider)
        let font = if value.is_empty() || clip_text(f(2), text, room).len() < text.len() { f(1) } else { f(2) };
        text_at(x + 22, y, y + rh, font, INK, text, 0);
        text_at(x + cw, y, y + rh, f(2), PRIMARY, value, 2);
    }
    text_at(x2i - 2 * ch - 20, y1 + 8, y1 + 8 + ch, f(2), MUTED, b"Tick: the game follows it itself", 2);
    pager(x2i, y1 + 10, ch - 4, SCROLL > 0, SCROLL + per * columns < all.len());
}

/// Goals: each manager's ten career goals. At the left the managers with their goal points, at the
/// right the chosen one's goals: while they can change, all of them to pick from (a click picks or
/// drops one, the chip of the three that ask a country opens the country list); after that his
/// ten, and the host's click ticks one off.
#[allow(static_mut_refs)]
unsafe fn goals((x1, y1, x2, y2): Rect, defs: &Lines, marks: &Lines, open: &Lines, nations: &Lines, head: &[&[u8]], names: &[&[u8]]) {
    let host = field(head, 3) == b"1";
    PICK = PICK.min(names.len().saturating_sub(1));
    let chosen = names.get(PICK).copied().unwrap_or(b"");
    let of = |n: &[u8]| marks.iter().filter(|m| first_word(m[1]) == first_word(n)).collect::<Vec<_>>();
    let worth = |m: &[&[u8]]| defs.get(number(m[2]) as usize).map_or(0, |d| number(d[4]));
    let reached = |m: &[&[u8]]| number(field(m, 3)) > 0;
    let picking = open.iter().find(|o| first_word(o[1]) == first_word(chosen)).is_none_or(|o| field(o, 2) == b"1");
    let mid = x1 + (x2 - x1) * 26 / 100;
    let line = (y2 - y1) * 7 / 100;
    // the managers
    card((x1, y1, mid - 6, y2));
    let (lx, lr) = (x1 + 12, mid - 18);
    text_at(lx, y1 + 2, y1 + line, f(4), GOLD, b"Managers", 0);
    let mut y = y1 + line;
    for (k, n) in names.iter().enumerate() {
        let r = (x1 + 4, y, mid - 10, y + line - 2);
        let s = Spot::Manager(k);
        fill(r.0, r.1, r.2, r.3, if PICK == k { SELECTED } else if lit(&s) { CARD } else { BG });
        let mine = of(n);
        let points: i32 = mine.iter().filter(|m| reached(m)).map(|m| worth(m)).sum();
        let h = r.3 - r.1;
        text_at(lx, r.1, r.1 + h * 58 / 100, f(3), if PICK == k { GOLD } else { INK }, &clip_text(f(3), n, lr - lx - 50), 0);
        let done = mine.iter().filter(|m| reached(m)).count();
        text_at(lx, r.1 + h * 50 / 100, r.3, f(2), MUTED, format!("{done} of {} reached", mine.len()).as_bytes(), 0);
        text_at(lr, r.1, r.3, f(4), GOLD, format!("+{points}").as_bytes(), 2);
        spot(r, s);
        y += line;
    }
    let says: &[u8] = match (host, picking) {
        (true, true) => b"Pick ten goals: a click picks or drops one. They are fixed when this season ends. Each counts from the season it is picked, once, for its points.",
        (true, false) => b"The game ticks off what it follows. Click a goal to tick off the rest.",
        (false, true) => b"The host picks the ten goals.",
        (false, false) => b"The host ticks off what the game does not follow.",
    };
    assistant::report_text(lx, y + 10, lr, y2 - 6, f(2), line * 45 / 100, says);
    // his goals
    card((mid + 6, y1, x2, y2));
    let (rx, rr) = (mid + 20, x2 - 14);
    let mine = of(chosen);
    let mark = |k: usize| mine.iter().find(|m| number(m[2]) == k as i32).copied();
    let title = if picking { format!("{} of 10 picked", mine.len()) } else { format!("{} of {} reached", mine.iter().filter(|m| reached(m)).count(), mine.len()) };
    text_at(rx, y1 + 2, y1 + line, f(4), GOLD, chosen, 0);
    text_at(rx + text_width(f(4), chosen) + 16, y1 + 2, y1 + line, f(3), MUTED, title.as_bytes(), 0);
    fill(rx, y1 + line + 2, rr, y1 + line + 3, BORDER);
    let top = y1 + line + 10;
    let step = line * 6 / 10;
    // the country list of a goal that asks one
    if let (Some(k), true) = (PICKING, host) {
        let goal = defs.get(k).map_or(&b""[..], |d| d[3]);
        text_at(rx, top, top + step, f(2), MUTED, &[b"Choose the country for: ", goal].concat(), 0);
        let cancel = text_width(f(3), b"Cancel") + 28;
        button((rr - cancel, y1 + 6, rr, y1 + 6 + step), b"Cancel", Spot::Cancel, false);
        let cols = ((rr - rx) / 170).max(1);
        let bw = (rr - rx - (cols - 1) * 8) / cols;
        let grid = top + step + 8;
        let fit = ((y2 - grid - 8) / (step + 6)).max(1) as usize;
        let rows = nations.len().div_ceil(cols as usize);
        SCROLL = SCROLL.min(rows.saturating_sub(fit));
        let now = mark(k).map_or(&b""[..], |m| field(m, 4));
        for (i, n) in nations.iter().enumerate().skip(SCROLL * cols as usize).take(fit * cols as usize) {
            let at = (i / cols as usize - SCROLL) as i32;
            let (bx, by) = (rx + (i as i32 % cols) * (bw + 8), grid + at * (step + 6));
            button((bx, by, bx + bw, by + step), &clip_text(f(3), field(n, 1), bw - 12), Spot::Nation(i), field(n, 1) == now);
        }
        pager(rr - cancel - 12, y1 + 6, step, SCROLL > 0, SCROLL + fit < rows);
        return;
    }
    // the rows: a category's head, a goal, or the rest of a goal's text
    let shown: Vec<usize> = (0..defs.len()).filter(|&k| picking || mark(k).is_some()).collect();
    if shown.is_empty() {
        text_at(rx, top, top + line, f(3), MUTED, b"No goals picked", 0);
        return;
    }
    let rh = text_width(f(2), b"Mg") * 2 / 3 + 10;
    let per = ((y2 - top - 8) / rh).max(1) as usize;
    let mut all: Vec<(Option<usize>, Vec<u8>, bool)> = Vec::new(); // goal, text, the first line of it
    let mut last: &[u8] = b"";
    for &k in &shown {
        let d = defs[k];
        if d[2] != last {
            last = d[2];
            all.push((None, last.to_vec(), true));
        }
        // (the three that ask a country: the country in their text once chosen)
        let country = mark(k).map_or(&b""[..], |m| field(m, 4));
        let country = if country.is_empty() { goals::CHOICE.to_string() } else { latin1(country).to_string() };
        let text = latin1(d[3]).replace(goals::CHOICE, &country);
        all.push((Some(k), to_latin1(&text), true));
    }
    let columns = if all.len() > per && rr - rx >= 900 { 2 } else { 1 };
    let (gap, cw) = (16, (rr - rx - (columns - 1) * 16) / columns);
    let (status, room) = (text_width(f(2), b"Choose a country") + 12, cw - 24 - text_width(f(2), b"Choose a country") - 70);
    let mut k = 0;
    while k < all.len() {
        let fits = clip_text(f(2), &all[k].1, room).len();
        if all[k].0.is_some() && fits < all[k].1.len() {
            let cut = all[k].1[..fits].iter().rposition(|&b| b == b' ').map_or(fits.max(1), |s| s + 1);
            let rest = all[k].1.split_off(cut);
            all.insert(k + 1, (all[k].0, rest, false));
        }
        k += 1;
    }
    // (a goal is not cut by the foot of a column: an empty row pushes it to the next; a group's
    // heading neither: "Special achievements" stood alone at the foot, night 6 E-3)
    let mut k = 0;
    while k + 1 < all.len() {
        let heading = all[k].0.is_none() && !all[k].1.is_empty();
        if k % per == per - 1 && (heading || all[k].0.is_some() && all[k].2 && !all[k + 1].2) {
            all.insert(k, (None, Vec::new(), true));
        }
        k += 1;
    }
    SCROLL = SCROLL.min(all.len().saturating_sub(per * columns as usize));
    for (n, (goal, text, first)) in all.iter().skip(SCROLL).take(per * columns as usize).enumerate() {
        let (c, row) = ((n / per) as i32, (n % per) as i32);
        let (x, y) = (rx + c * (cw + gap), top + row * rh);
        let Some(g) = goal else {
            text_at(x, y, y + rh, f(2), CYAN, text, 0);
            continue;
        };
        let m = mark(*g);
        let s = Spot::Goal(*g);
        if !first {
            text_at(x + 24, y - 4, y + rh - 4, f(2), if m.is_some() || !picking { INK } else { MUTED }, text, 0);
            continue;
        }
        let r = (x, y, x + cw, y + rh - 2);
        if host && (picking || m.is_some()) {
            if lit(&s) {
                fill(r.0, r.1, r.2, r.3, CARD);
            }
            // (left of the status column and the points: the country chip there has its own click)
            spot((r.0, r.1, r.2 - status - 56, r.3), s);
        }
        let done = m.is_some_and(|m| reached(m));
        let size = rh * 55 / 100;
        let mark_at = (x + 2, y + (rh - size) / 2 - 1, x + 2 + size, y + (rh + size) / 2 - 1);
        round(mark_at, 3, if done { SUCCESS } else if picking && m.is_some() { SELECTED } else { BORDER });
        if done || picking && m.is_some() {
            draw_icon(ICON_CHECK, (mark_at.0 + mark_at.2) / 2, mark_at.1 + size / 5, size * 6 / 10, if done { INK } else { GOLD });
        }
        text_at(x + 24, y, y + rh, f(2), if m.is_some() || !picking { INK } else { MUTED }, text, 0);
        text_at(r.2, y, y + rh, f(2), PRIMARY, &[b"+", defs[*g][4]].concat(), 2);
        // where it stands: the season reached, a country to choose, or the host's to tick
        let asks = latin1(defs[*g][3]).contains(goals::CHOICE);
        let at = r.2 - 44;
        if let Some(m) = m.filter(|m| reached(m)) {
            let y0 = number(field(m, 3));
            text_at(at, y, y + rh, f(2), SUCCESS, format!("{y0}/{:02}", (y0 + 1) % 100).as_bytes(), 2);
        } else if asks && m.is_some() && picking && host {
            let label: &[u8] = if field(m.unwrap(), 4).is_empty() { b"Choose a country" } else { b"Change country" };
            let chip = (at - text_width(f(2), label) - 12, y + 1, at, y + rh - 3);
            let cs = Spot::Country(*g);
            round(chip, 4, if lit(&cs) { SELECTED } else { CARD });
            text_at((chip.0 + chip.2) / 2, chip.1, chip.3, f(2), CYAN, label, 1);
            spot(chip, cs);
        } else if asks && m.is_some_and(|m| field(m, 4).is_empty()) {
            text_at(at, y, y + rh, f(2), MUTED, b"No country", 2);
        } else if defs[*g][5] != b"1" {
            text_at(at, y, y + rh, f(2), MUTED, b"Host ticks", 2);
        }
    }
    pager(rr, y1 + 6, step, SCROLL > 0, SCROLL + per * (columns as usize) < all.len());
}
