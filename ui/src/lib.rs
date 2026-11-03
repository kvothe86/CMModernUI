//! cmui: a modern look for CM 01/02 3.9.68, loaded into the game by cm0102_ui.exe.
//! Every change is a call-site patch, made only when the call is where the original exe has it.
//! Addresses come from re\function_names.tsv.
use std::{fs::OpenOptions, io::Write};

#[link(name = "kernel32")]
extern "system" {
    fn VirtualProtect(addr: *mut u8, size: usize, new: u32, old: *mut u32) -> i32;
    fn VirtualAlloc(addr: usize, size: usize, kind: u32, protect: u32) -> *mut u8;
    fn VirtualQuery(addr: usize, info: *mut [usize; 7], size: usize) -> usize;
    fn GetCurrentThreadId() -> u32;
    fn OpenThread(access: u32, inherit: i32, id: u32) -> usize;
    fn SuspendThread(thread: usize) -> u32;
    fn ResumeThread(thread: usize) -> u32;
    fn GetThreadContext(thread: usize, context: *mut [u32; 180]) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn PostMessageA(hwnd: usize, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn GetWindowInfo(hwnd: usize, info: *mut [i32; 15]) -> i32;
    fn MessageBoxA(hwnd: usize, text: *const u8, caption: *const u8, kind: u32) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueA(key: usize, subkey: *const u8, value: *const u8, flags: u32, kind: *mut u32, data: *mut u8, size: *mut u32) -> i32;
}

mod remote;
mod assistant;
mod challenge;
mod comp;
mod history;
mod news;
mod prefetch;
mod scout;

/// Whether `len` bytes at `addr` are committed, readable memory of this process.
fn mapped(addr: usize, len: usize) -> bool {
    // readable regions seen in the last 50 ms (a squad cell checked 50 players' names with a
    // VirtualQuery each, for every small text on screen: lists slowed to a crawl)
    // ponytail: 16 regions, round robin; memory freed within 50 ms of a look would be trusted
    static mut SEEN: ([(usize, usize); 16], usize, Option<std::time::Instant>) = ([(0, 0); 16], 0, None);
    if addr <= 0x10000 {
        return false;
    }
    unsafe {
        #[allow(static_mut_refs)]
        let seen = &mut SEEN;
        if seen.2.is_none_or(|t| t.elapsed().as_millis() > 50) {
            *seen = ([(0, 0); 16], 0, Some(std::time::Instant::now()));
        }
        let end = addr.saturating_add(len);
        if seen.0.iter().any(|&(a, b)| addr >= a && end <= b) {
            return true;
        }
        let mut info = [0usize; 7]; // MEMORY_BASIC_INFORMATION: base, alloc base, alloc protect, size, state, protect, type
        let ok = VirtualQuery(addr, &mut info, 28) != 0
            && info[4] == 0x1000 // MEM_COMMIT
            && info[5] & 0x66 != 0 // PAGE_READONLY | PAGE_READWRITE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE
            && info[5] & 0x100 == 0; // (not PAGE_GUARD)
        if ok {
            seen.0[seen.1] = (info[0], info[0] + info[3]);
            seen.1 = (seen.1 + 1) % 16;
        }
        ok && end <= info[0] + info[3]
    }
}

// Game data (records, tables, the game's globals) goes through `readable`, `rd`, `data_bytes` and
// `game_text`: this process's memory, or in a network client the host's (`remote`), where what is
// not there yet reads as missing and the screen is drawn again once it comes.

/// Whether `len` bytes of game data at `addr` can be read (game pointers are checked before they
/// are followed).
fn readable(addr: usize, len: usize) -> bool {
    unsafe {
        if !remote::CLIENT {
            return mapped(addr, len);
        }
        if addr <= 0x10000 {
            return false;
        }
        let (first, last) = (addr / remote::CHUNK, (addr + len.max(1) - 1) / remote::CHUNK);
        // ponytail: big ranges (whole tables) are lookups the host makes (`ask_host`)
        if last - first > 128 {
            return false;
        }
        (first..=last).map(|c| remote::chunk(c * remote::CHUNK).is_some()).fold(true, |a, b| a & b)
    }
}

/// `len` bytes of game data (zeros for what a network client does not have yet).
unsafe fn data_bytes(addr: usize, len: usize) -> Vec<u8> {
    if !remote::CLIENT {
        return std::slice::from_raw_parts(addr as *const u8, len).to_vec();
    }
    (addr..addr + len)
        .map(|a| a - a % remote::CHUNK)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .flat_map(|base| {
            let chunk = remote::chunk(base).unwrap_or_else(|| vec![0; remote::CHUNK]);
            let (from, to) = (addr.max(base) - base, (addr + len).min(base + remote::CHUNK) - base);
            chunk[from..to].to_vec()
        })
        .collect()
}

/// A value of game data at `addr` (checked with `readable` first where it is a pointer's target).
unsafe fn rd<T: Copy>(addr: usize) -> T {
    if !remote::CLIENT {
        return (addr as *const T).read_unaligned();
    }
    let bytes = data_bytes(addr, std::mem::size_of::<T>()); // kept until read
    (bytes.as_ptr() as *const T).read_unaligned()
}

/// The zero-terminated text at a checked game pointer.
unsafe fn game_text(addr: usize, max: usize) -> Vec<u8> {
    if !readable(addr, max) {
        return Vec::new();
    }
    data_bytes(addr, max).split(|&c| c == 0).next().unwrap().to_vec()
}

/// One of cmui's lookups on the game data (`host_answer`): made here, or in a network client by
/// the host (None until its answer comes, or when it has none).
unsafe fn ask_host(key: &[u8]) -> Option<Vec<u8>> {
    if remote::CLIENT { remote::get(key).flatten() } else { host_answer(0, key) }
}

fn key(kind: u8, args: &[u32]) -> Vec<u8> {
    let mut k = vec![kind];
    for a in args {
        k.extend(a.to_le_bytes());
    }
    k
}

fn word(v: &[u8]) -> Option<usize> {
    Some(u32::from_le_bytes(v.get(..4)?.try_into().ok()?) as usize)
}

static mut TRACE_LINES: u32 = 0;
/// Whether cmui.trace exists, looked at once a frame (by the flip: a look per text slowed lists).
static mut TRACE_ON: bool = false;

/// Trace lines are written while the file cmui.trace exists in the game folder (the game runs
/// with other rights than the shell, so it does not see the shell's environment), 200,000 at most.
fn trace() -> bool {
    unsafe {
        // (lines written count, not looks: a look per text used the 200,000 up in minutes)
        TRACE_ON && TRACE_LINES < 200_000
    }
}

fn log(msg: &str) {
    unsafe { TRACE_LINES += TRACE_ON as u32 };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open("cmui.log") {
        let _ = writeln!(f, "{msg}");
    }
}

// Game functions (cdecl) and globals.
const RGB_TO_PIXEL: usize = 0x5e4800; // (r, g, b, format or 0) -> pixel
const DRAW_RECT: usize = 0x5e3b50; // (left, top, right, bottom, style, colour), style 0 = fill, 2 = outline
const DRAW_LINE: usize = 0x5e3730; // (x1, y1, x2, y2, style, colour), style 2 = solid
const IS_NATIONAL_TEAM: usize = 0x53b900; // database_is_national_team(club) -> bool
const AREA_DRAW: usize = 0x402e70; // Area::draw, draws the grid lines of grid areas
const PALETTE_INIT: usize = 0x5e4560;
const GRAPHICS_INIT: usize = 0x5e2620;
const DRAW_SCREEN: usize = 0x55f300;
const BLIT_PICTURE: usize = 0x5e3fd0;
const DRAW_PANEL: usize = 0x5e5df0; // (x1, y1, x2, y2, flags, colour, extra)
const SCREEN_W: usize = 0xaebe08;
const SCREEN_H: usize = 0xaebdd0;

unsafe fn game<F>(addr: usize) -> F {
    std::mem::transmute_copy(&addr)
}

/// The slots of the game's GUI palette (filled by 0x5e4560) and the colour each one gets.
const THEME: &[(usize, [u8; 3])] = &[
    (0xaebe04, [0x3a, 0x3f, 0x47]), // dark blue: panels and title bars
    (0xaebdd6, [0x2a, 0x2e, 0x35]), // navy
    (0xaebdf0, [0x4a, 0x50, 0x5a]), // blue
    (0xaebdde, [0x5a, 0x62, 0x6e]), // azure
    (0xae3192, [0x30, 0x34, 0x3b]), // dark purple
    (0xae3148, [0x44, 0x49, 0x52]), // purple
    (0xaebdec, [0xe0, 0xb8, 0x50]), // yellow -> gold
    (0xaebdee, [0xf0, 0xd8, 0x98]), // pale yellow
    (0xae3152, [0xd8, 0xdc, 0xe2]), // cyan -> light text
    (0xaebde8, [0xb8, 0xc0, 0xca]), // pale cyan
    (0xae3150, [0x26, 0x29, 0x2f]), // dark green: dialogs (Game Settings)
];
const TAB_COLOUR_MADE: usize = 0xb0be34;
const TAB_COLOUR: usize = 0xb0be36;
const BACKGROUND: [u8; 3] = [0x2b, 0x2f, 0x36];
const SIDEBAR: [u8; 3] = [0x22, 0x26, 0x2c];
const BACKGROUND_CHANGES_OFF: usize = 0xdd6bda; // u32, nonzero: draw_screen takes the default picture (0x55f32a)
/// The azure slot is a dark panel colour here; the game also writes text in it (Friendly in a
/// club's fixtures, some contract screens): that text a readable blue.
const AZURE_SLOT: [u8; 3] = [0x5a, 0x62, 0x6e];
const AZURE_TEXT: [u8; 3] = [0x9c, 0xc4, 0xe8];

// Panel flag bits of gui_draw_panel.
const PANEL_DARKEN: u32 = 2;
const PANEL_HGRAD: u32 = 4;
const PANEL_VGRAD: u32 = 8;
const PANEL_FILL: u32 = 0x10;
const PANEL_CIRCLE: u32 = 0x10000;

static mut PANEL_ORIG: usize = 0;

/// gui_draw_panel with the theme's rules: no gradients; the vertical one (the left menu bar)
/// becomes the flat sidebar colour.
extern "C" fn panel(x1: i32, y1: i32, x2: i32, y2: i32, mut flags: u32, mut colour: u32, extra: u32) {
    let _timed = Timed(0, std::time::Instant::now());
    unsafe {
        if trace() {
            log(&format!("panel {x1},{y1}-{x2},{y2} flags {flags:#x} colour {colour:#06x} extra {extra:#x}"));
        }
        #[allow(static_mut_refs)]
        if PANELS.len() < 400 {
            PANELS.push((x1, y1, x2, y2));
        }
        // the tactics pitch as mike_appguy has it: dark striped grass, white lines, shirts
        let low = colour & 0xffff;
        if on(TACTICS_PITCH) && flags == 0x30 && is_colour(low, 0x0200) && x2 - x1 > sx(400) {
            PITCH = (x1, y1, x2, y2);
            if live_engine().is_some() {
                REDRAW |= !LIVE_TACTICS; // its list was drawn before the pitch
                LIVE_TACTICS = true;
                LIVE_SEEN = true;
            }
        }
        let p = PITCH;
        if p.2 > p.0 && x1 >= p.0 && x2 <= p.2 && y1 >= p.1 && y2 <= p.3 {
            if flags & PANEL_CIRCLE != 0 {
                let (cx, cy, r) = ((x1 + x2) / 2, (y1 + y2) / 2, (x2 - x1).min(y2 - y1) / 2);
                if flags & PANEL_FILL != 0 {
                    shirt(x1, y1, x2, y2);
                } else if is_colour(extra, 0x400) {
                    circle(cx, cy, r, 1.5, PITCH_LINE); // centre circle, penalty arcs
                }
                return; // (the markers' outlines)
            }
            for (game_colour, ours) in [(0x0200, PITCH_GRASS), (0x01c0, PITCH_STRIPE), (0x0400, PITCH_LINE)] {
                if is_colour(low, game_colour) {
                    colour = colour & !0xffff | rgb(ours) as u32;
                }
            }
        }
        // (buttons with a popup arrow are tall; table heads and pills are not)
        if flags & 0xff == 0x30 && x2 - x1 > 2 * (y2 - y1) && y2 - y1 > sy(45) {
            #[allow(static_mut_refs)]
            if BUTTONS.len() < 200 {
                BUTTONS.push((x1, y1, x2, y2));
            }
        }
        // the active tab: flat with a gold underline (and gold text) instead of a gold frame
        if on(TABS) && flags & 0x830 == 0x830 && x2 - x1 > 2 * (y2 - y1) {
            game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32)>(PANEL_ORIG)(x1, y1, x2, y2, flags & !0x800, colour, extra);
            fill(x1 + 1, y2 - 3, x2, y2, GOLD);
            ACTIVE_TAB = (x1, y1, x2, y2);
            return;
        }
        if (x1, y1, x2, y2) == ACTIVE_TAB {
            ACTIVE_TAB = (0, 0, 0, 0); // that tab is no longer the active one
        }
        // the subs' number cells (navy) as grey as the first eleven's
        let theme = on(THEME_ON);
        // the menu bar's button boxes: none (`sidebar_item` draws the buttons, the click areas stay
        // the game's boxes); an open or pressed one (0x1061) gets a lighter band with a gold edge
        let (sw, sh) = screen();
        if theme && on(MENU_FIXES) && x2 <= bar_end(sw) && (flags == 0x1021 || flags == 0x1061) {
            // (a closed one the bar's own colour: what the open state or a dropdown left is cleared)
            if flags == 0x1061 && y1 > sh * 15 / 100 {
                fill(x1, y1, x2 + 1, y2 + 1, [0x2c, 0x31, 0x38]);
                fill(x1, y1 + 6, x1 + 3, y2 - 5, GOLD);
            } else {
                fill(x1, y1, x2 + 1, y2 + 1, SIDEBAR);
            }
            return;
        }
        {
            // the match title's sides: their club colours for the possession bar
            let (w, h) = screen();
            if flags == 0x30 && y1 <= h / 30 && x2 - x1 > w * 35 / 100 && x2 - x1 < w / 2 {
                #[allow(static_mut_refs)]
                {
                    SIDE_COLOURS.retain(|s| s.0 != x1);
                    SIDE_COLOURS.push((x1, pack(unpack(low, screen_format() != Some(false)), true))); // kept as 565
                }
            }
        }
        // dropdown menus: dark rows (white and light grey) with light text, a dark frame
        // dropdown menus (any colour scheme: white and grey in the original, green in others):
        // a row is a short filled panel whose text starts with the five spaces left for the tick;
        // the text hook darkens it (`menu_row`). The frame is the last outline panel before it.
        if theme && flags & !0x100_0000 == 0x10 && y2 - y1 < 30 {
            ROW_PANEL = (x1, y1, x2, y2);
        } else if theme && flags & 0xff == 0x30 {
            LAST_FRAME = (x1, y1, x2, y2);
            FRAME_DRAWN = true;
        }
        if theme && flags == 0x30 && is_colour(low, 0x0008) && x2 - x1 < 100 {
            colour = colour & !0xffff | px565(0x39e8);
        }
        if theme && flags & PANEL_CIRCLE != 0 {
            // the pitch circles behind dialogs: the game's circle drawer gets their colour wrong
            // (blue) on patched resolutions, and they are only decoration
            return;
        }
        if theme && flags & PANEL_VGRAD != 0 {
            colour = rgb(SIDEBAR) as u32;
        }
        if flags == 0x2 {
            LAST_DARK = (x1, y1, x2, y2);
            #[allow(static_mut_refs)]
            if STORY.as_ref().is_some_and(|s| y1 >= s.0 .3 - 2) {
                STORY_BODY = Some((x1, y1, x2, y2));
            }
        }
        if theme {
            news::note_panel(x1, y1, x2, y2, flags);
        }
        if on(STATS_TABLE) && comp::panel(x1, y1, x2, y2, flags) {
            return;
        }
        // the story's line backgrounds (and its links' boxes) are not drawn: news::story lays the
        // text out again on the panel
        #[allow(static_mut_refs)]
        if STORY.as_ref().map(|s| s.0).is_some_and(|h| flags == 0x1 && y1 > h.3 && x1 >= h.0 && y2 - y1 < 60 && STORY_BODY.is_some_and(|b| y2 <= b.3)) {
            return;
        }
        let selected = theme && flags == PANEL_SELECTED;
        if selected {
            // the selected row of a list: mike_appguy's dark gold instead of red
            colour = colour & !0xffff | rgb(SELECTED) as u32;
        }
        if theme && flags & (PANEL_HGRAD | PANEL_VGRAD) != 0 && flags & PANEL_DARKEN == 0 {
            flags = flags & !(PANEL_HGRAD | PANEL_VGRAD) | PANEL_FILL;
        }
        // the game's grey widgets in steel blue: buttons and dropdowns keep their bevel (0x40:
        // pressed, lighter), the clickable cells of a list (dates, positions, numbers) a quieter
        // blue, column heads go flat, a stats cell without a value goes altogether
        // (not the scrollbars' narrow parts: `news::scrollbars` draws those)
        if theme && on(MODERN_BUTTONS) && flags & !0x40 == 0x30 {
            if is_colour(colour & 0xffff, 0x39e8) {
                colour = colour & !0xffff | rgb([0x2a, 0x3a, 0x4c]) as u32;
            }
            if is_colour(low, 0x31a7) {
                fill(x1, y1, x2 + 1, y2 + 1, AREA);
                return;
            }
            // (the exe's own button grey: 0x5acb patched darker, 0x8410 as shipped; a client draws the host's)
            if (is_colour(low, 0x5acb) || is_colour(low, 0x8410)) && x2 - x1 > sx(30) {
                if y2 - y1 < sy(30) {
                    fill(x1, y1, x2 + 1, y2 + 1, [0x26, 0x29, 0x2f]);
                    fill(x1, y2, x2 + 1, y2 + 1, [0x3a, 0x3f, 0x47]);
                    return;
                }
                colour = colour & !0xffff | rgb(if flags & 0x40 != 0 { [0x3a, 0x5c, 0x82] } else { [0x2b, 0x47, 0x66] }) as u32;
            }
        }
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32)>(PANEL_ORIG)(x1, y1, x2, y2, flags, colour, extra);
        note_title(x1, y1, x2, y2);
        if (x1, y1, x2, y2) == TITLE {
            KIT_BAND = Some(colour & 0xffff); // (a club's screen: its colours)
        }
        #[allow(static_mut_refs)]
        if let (true, Some((dx1, dy1, dx2, dy2, date, font))) = (selected, &NEWS_DATE) {
            if *dy1 == y1 {
                // a news row: its date cell joins the selection, with a gold marker at the left
                fill(*dx1, *dy1, *dx2 + 1, *dy2, SELECTED);
                fill(*dx1, *dy1, *dx1 + 3, *dy2, GOLD);
                put(*dx1 + 12, *dy1, *dx2, *dy2, 0x9, *font, LIGHT, &latin1(&news::short_date(date)));
                SELECTED_ROW = (y1, date.clone(), Vec::new());
            }
        }
    }
}

/// The menu bar's font: the largest every word of its items fits in.
static mut SIDEBAR_FONT: Option<u32> = None;
/// The font each column of this screen uses ((left, right, the game's font) -> font): the
/// smallest one a cell in it needed to fit.
static mut COLUMN_FONTS: Vec<((i32, i32, u32), u32)> = Vec::new();
/// The last short filled panel (a menu row, if its text says so) and the last outline panel (a
/// menu's frame); the last menu row darkened, and its index in its menu.
static mut ROW_PANEL: (i32, i32, i32, i32) = (0, 0, 0, 0);
static mut LAST_FRAME: (i32, i32, i32, i32) = (0, 0, 0, 0);
static mut MENU_ROW: (i32, i32, i32, i32) = (0, 0, 0, 0);
static mut MENU_INDEX: i32 = 0;
/// An outline panel drawn since the last menu row (the frame of a menu being drawn whole).
static mut FRAME_DRAWN: bool = false;
/// The rows of the open menus (left edge, top) in drawing order: a network client redraws only
/// the rows the pointer left and entered, which take their shade from their place here.
static mut MENU_ROWS: Vec<(i32, i32)> = Vec::new();

/// A dropdown menu row: dark (alternating), its frame dark under the first row drawn after the
/// frame; the text goes light.
#[allow(static_mut_refs)]
unsafe fn menu_row(x1: i32, y1: i32, x2: i32, y2: i32, colour: &mut u32) {
    (news::MENU_OPEN, news::MENU_SEEN) = (true, true);
    let fresh = std::mem::take(&mut FRAME_DRAWN);
    let next = MENU_ROW.0 == x1 && MENU_ROW.2 == x2 && (0..8).contains(&(y1 - MENU_ROW.3));
    let known = MENU_ROWS.iter().filter(|r| r.0 == x1).position(|r| r.1 == y1);
    MENU_INDEX = match (next, known) {
        (true, _) => MENU_INDEX + 1,
        (false, Some(i)) if !fresh => i as i32,
        _ => {
            MENU_ROWS.retain(|r| r.0 != x1);
            0
        }
    };
    if !MENU_ROWS.contains(&(x1, y1)) {
        MENU_ROWS.push((x1, y1));
    }
    let f = LAST_FRAME;
    if fresh && f.0 <= x1 && f.2 >= x2 && f.1 <= y1 && f.3 >= y2 {
        fill(f.0, f.1, f.2 + 1, f.3 + 1, [0x26, 0x29, 0x2f]);
    }
    fill(x1, y1, x2 + 1, y2 + 1, if MENU_INDEX % 2 == 0 { [0x2a, 0x2e, 0x35] } else { [0x30, 0x34, 0x3b] });
    MENU_ROW = (x1, y1, x2, y2);
    if *colour & 0xffff == 0 {
        *colour = rgb(LIGHT) as u32;
    }
}
/// The newest dropdown row is still on the screen: its two shades (menu_row's fills) at its left
/// edge. (A menu closes by restoring what was under it, without a full frame that would tell.)
pub(crate) unsafe fn menu_still_shown() -> bool {
    let (x1, y1, x2, y2) = MENU_ROW;
    x2 > x1 && pixel_at(x1 + 2, (y1 + y2) / 2).is_some_and(|p| {
        [[0x2a, 0x2e, 0x35], [0x30, 0x34, 0x3b]].iter().any(|c| (0..3).all(|i| (p[i] as i32 - c[i] as i32).abs() <= 8))
    })
}

/// The active tab drawn last (its label goes gold).
static mut ACTIVE_TAB: (i32, i32, i32, i32) = (0, 0, 0, 0);
/// The tactics pitch (grass panel) since the last full redraw.
static mut PITCH: (i32, i32, i32, i32) = (0, 0, 0, 0);
/// The tactics screen ("VVV Tactics" title), kept while full redraws show its title (the list is
/// drawn before the title and the pitch); seen in this full redraw.
static mut TACTICS: bool = false;
static mut TACTICS_SEEN: bool = false;
/// The tactics screen opened during a match (its pitch seen with a match engine), kept while
/// full redraws show it; seen in this full redraw.
static mut LIVE_TACTICS: bool = false;
static mut LIVE_SEEN: bool = false;
/// The left edge of the in-match tactics list's rating column (its cells: a short number right
/// of the list's name cells, drawn before each row's name).
static mut RATING_X: i32 = 0;
/// Pos / Cond moved right of the name cells (narrow lists); those cells' right end.
static mut LIVE_MOVED: bool = false;
static mut LIVE_NAME_X2: i32 = i32::MAX;
/// The last in-match list name drawn: its row and where its text ends.
static mut NAME_END: (i32, i32) = (0, 0);
const PITCH_GRASS: [u8; 3] = [0x1d, 0x3c, 0x2b];
const PITCH_STRIPE: [u8; 3] = [0x23, 0x47, 0x33];
const PITCH_LINE: [u8; 3] = [0xd8, 0xe0, 0xd8];

/// The club's colours as its screen's title band has them (band, title text; screen pixels): the
/// shirts on the tactics pitch are in them.
static mut KIT_BAND: Option<u32> = None;
static mut KIT_TEXT: Option<u32> = None;

/// A screen pixel as RGB (565; near enough in 555).
fn px_rgb(p: u32) -> [u8; 3] {
    [((p >> 11 & 31) * 8) as u8, ((p >> 5 & 63) * 4) as u8, ((p & 31) * 8) as u8]
}

/// The keeper on the tactics pitch: the marker in the bottom eighth (his own goal).
unsafe fn keeper_at(y1: i32, y2: i32) -> bool {
    let p = PITCH;
    (y1 + y2) / 2 > p.1 + (p.3 - p.1) * 88 / 100
}

/// The keeper's shirt: green, orange when the club's own colour is a green.
unsafe fn keeper_kit() -> [u8; 3] {
    let [r, g, b] = KIT_BAND.map(px_rgb).unwrap_or([0xf4; 3]);
    if g as i32 > r as i32 + 30 && g as i32 > b as i32 + 30 { [0xe8, 0x8a, 0x20] } else { [0x2a, 0x9a, 0x3a] }
}

/// A shirt in place of a player's marker (w x h box, centred): the club's colours (body in its
/// band colour, a thin trim in its text colour), the keeper's his own.
unsafe fn shirt(x1: i32, y1: i32, x2: i32, y2: i32) {
    shirt_kit(x1, y1, x2, y2, keeper_at(y1, y2));
}

unsafe fn shirt_kit(x1: i32, y1: i32, x2: i32, y2: i32, keeper: bool) {
    let h = (y2 - y1) * 11 / 10;
    let w = h * 11 / 10;
    let (left, top) = ((x1 + x2 - w) / 2, (y1 + y2 - h) / 2);
    let (body, trim) = if keeper {
        (rgb565(keeper_kit()) as u32, rgb565([0xf4; 3]) as u32)
    } else {
        (KIT_BAND.unwrap_or(rgb565([0xf4; 3]) as u32), KIT_TEXT.unwrap_or(rgb565([0x10, 0x12, 0x14]) as u32))
    };
    // shape in [-0.5, 0.5] x [0, 1]: sleeves to 0.42, body to 1, a V at the neck; 4x4 samples
    let inside = |x: f32, y: f32| {
        let ax = x.abs();
        let neck = y < 0.14 && ax < 0.14 - y * 0.6;
        let sleeve = y >= 0.02 + (ax - 0.22).max(0.0) * 0.35 && y <= 0.4 - (ax - 0.32).max(0.0) * 0.5 && ax <= 0.5;
        !neck && (sleeve || (ax <= 0.3 && y >= 0.02 && y <= 1.0))
    };
    // the trim: the whole shape, then the body over it a pixel in
    for (dx, dy, dw, dh, colour) in [(0, 0, w, h, trim), (1, 1, w - 2, h - 2, body)] {
        blend(left + dx, top + dy, dw, dh, |col, row| {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = (col as f32 + (sx as f32 + 0.5) / 4.0) / dw as f32 - 0.5;
                    let y = (row as f32 + (sy as f32 + 0.5) / 4.0) / dh as f32;
                    n += inside(x, y) as u32;
                }
            }
            (colour as u16, n * 255 / 16)
        });
    }
}

const PANEL_SELECTED: u32 = 0x210; // the selected row of a list (filled, dark red)
const SELECTED: [u8; 3] = [0x4c, 0x42, 0x24];
/// The last darkened panel (the story header on the news screen).
static mut LAST_DARK: (i32, i32, i32, i32) = (0, 0, 0, 0);
/// The last news date cell: box, text, font.
static mut NEWS_DATE: Option<(i32, i32, i32, i32, Vec<u8>, u32)> = None;
/// The selected news row: y, date, headline.
static mut SELECTED_ROW: (i32, Vec<u8>, Vec<u8>) = (i32::MIN, Vec::new(), Vec::new());
/// The story on the news screen this frame: header box, headline, date, font, story text.
static mut STORY: Option<((i32, i32, i32, i32), Vec<u8>, Vec<u8>, u32, Vec<u8>)> = None;

fn story_len_ok(body: &[u8]) -> bool {
    body.len() < 2000
}

/// "Sat 6th Apr EVE": a date in a news list (English game).
fn is_news_date(t: &[u8]) -> bool {
    let words: Vec<&[u8]> = t.split(|&c| c == b' ').collect();
    words.len() == 4
        && [b"Mon", b"Tue", b"Wed", b"Thu", b"Fri", b"Sat", b"Sun"].iter().any(|d| words[0] == *d)
        && [b"AM".as_slice(), b"PM", b"EVE"].contains(&words[3])
}

unsafe fn has_crest(key: &[u8]) -> bool {
    if !on(CRESTS_ON) {
        return false;
    }
    #[allow(static_mut_refs)]
    CRESTS.get_or_insert_with(load_crests).as_ref().is_some_and(|c| c.index.contains_key(key))
}

/// The crest a story gets, kept per headline (the search goes through the club table).
static mut STORY_CREST: Option<(Vec<u8>, Option<Vec<u8>>)> = None;

/// The club or competition a story is about. A story about a competition (its headline names a
/// league, cup, award, draw...) gets the competition's logo when the story names one by its full
/// name ("Dutch Premier Division"); else the first crest named (`crest_named`); else, when it
/// mentions the human manager or one of his players, his club's.
#[allow(static_mut_refs)]
unsafe fn story_crest(head: &[u8], body: &[u8]) -> Option<Vec<u8>> {
    if let Some((h, key)) = &STORY_CREST {
        if h.as_slice() == head {
            return key.clone();
        }
    }
    let words = ["League", "Division", "Cup", "Championship", "Trophy", "Shield", "Award", "Player of", "Manager of", "Team of", "draw", "Final", "Round", "Group", "Phase", "title", "champions", "promot", "relegat", "Association", "FA "];
    let about_comp = words.iter().any(|w| head.windows(w.len()).any(|x| x.eq_ignore_ascii_case(w.as_bytes())));
    let is_comp = |k: &[u8]| !k.starts_with(b"flag:") && club_named(k).is_none();
    let mut key = None;
    // the board's mood and the own scouts' reports are about the human's own club ("Board
    // delighted after Liverpool game" is PSV's board; "Oceanic search completed" his scout)
    let has = |w: &[u8]| head.windows(w.len()).any(|x| x.eq_ignore_ascii_case(w));
    // (whole words: not "aboard", "dashboard")
    let has_word = |w: &[u8]| {
        head.windows(w.len()).enumerate().any(|(i, x)| {
            x.eq_ignore_ascii_case(w)
                && (i == 0 || !head[i - 1].is_ascii_alphanumeric())
                && head.get(i + w.len()).is_none_or(|c| !c.is_ascii_alphanumeric())
        })
    };
    // (the board's stories name the opponent of the game they're about, "Board delighted after
    // NEC game", "Sparta match pleases board": the human's board all the same; a chairman or a
    // search only when the headline names no club, "Chelsea chairman ...")
    let own = has_word(b"board") || ((has_word(b"chairman") || has(b"search completed")) && crests_named(head).is_empty());
    if own {
        key = human_club().map(|(club, _)| club).filter(|c| has_crest(c));
    }
    if about_comp && key.is_none() {
        key = [head, body].iter().find_map(|t| crests_named(t).into_iter().find(|k| is_comp(k)));
    }
    if key.is_none() {
        // the story's own links (who it is really about: two players can share a name, a player
        // can be called like a club); a network client asks the host, whose display holds them,
        // and asks again until the answer is there
        let text = &body[..body.len().min(400)];
        key = if remote::CLIENT {
            match ask_host(&[b"Y", head, b"\0", text].concat()) {
                None => return None,
                Some(v) => (!v.is_empty()).then_some(v),
            }
        } else {
            story_link_crest(channel(), head, text)
        };
    }
    if key.is_none() {
        key = crests_named(head).into_iter().next().or_else(|| crests_named(body).into_iter().next());
    }
    if key.is_none() {
        // about the human's club without naming it: his name or a player of his
        if let Some((club, surname)) = human_club() {
            let mentions = |n: &[u8]| n.len() >= 4 && [head, body].iter().any(|t| t.windows(n.len()).any(|w| w == n));
            let squad = club_named(&club).filter(|&c| readable(c + 0xd7, 200)).map(|c| (0..50).map(|k| rd::<usize>(c + 0xd7 + 4 * k)).filter(|&s| readable(s, 0x6e)).collect::<Vec<_>>()).unwrap_or_default();
            // his surname (+8) or the name he goes by ("Butra", +0xc)
            let player_named = squad.iter().any(|&s| {
                [8, 0xc].iter().any(|&at| {
                    let p = rd::<usize>(s + at);
                    readable(p, 1) && mentions(&game_text(p, 51))
                })
            });
            if (mentions(&surname) || player_named) && has_crest(&club) {
                key = Some(club);
            }
        }
    }
    STORY_CREST = Some((head.to_vec(), key.clone()));
    key
}

/// The display of each channel (+0x300 apart); its widgets (0x18c bytes from +0xba95e, count i16
/// at +0x12e9a0): +8 the link kind, +0x48 the linked object, +0x80 the text (as
/// menubar_handle_object_link 0x7643d0 reads them).
const CHANNEL_DISPLAY: usize = 0xb74c4c;

/// The crest a story's own links give: a club (link 0x7d5 / 0x7d6) the headline names, else a
/// person (0x7d7..0x7e0, 0x7e7) it names: his national team when the story names it ("chosen for
/// the Netherlands squad"), else his club; then the same for the first links of the text. Read
/// where the display is (single player, the host).
unsafe fn story_link_crest(channel: usize, head: &[u8], body: &[u8]) -> Option<Vec<u8>> {
    let display = *((CHANNEL_DISPLAY + 0x300 * channel) as *const usize);
    if !mapped(display + 0x12e9a0, 2) {
        return None;
    }
    let count = (*((display + 0x12e9a0) as *const i16)).clamp(0, 1200) as usize;
    if !mapped(display + 0xba95e, count * 0x18c) {
        return None;
    }
    let names = |t: &[u8], n: &[u8]| n.len() >= 3 && t.windows(n.len()).any(|w| w == n);
    let crest = |club: usize| mapped(club, 0x5b).then(|| [game_text(club + 4, 51), game_text(club + 0x38, 26)].into_iter().find(|n| has_crest(n))).flatten();
    let (mut club, mut person) = ([None, None], [None, None]); // (named in the headline, in the text)
    for k in 0..count {
        let item = display + 0xba95e + k * 0x18c;
        let (kind, object) = (*((item + 8) as *const i16), *((item + 0x48) as *const usize));
        let text = std::slice::from_raw_parts((item + 0x80) as *const u8, 64).split(|&c| c == 0).next().unwrap().trim_ascii();
        if trace() && (0x7d0..=0x7e7).contains(&kind) {
            let ints: Vec<i32> = (0..16).map(|i| *((item + 0xc + 4 * i) as *const i32)).collect();
            log(&format!("story link {k} kind {kind:#x} {:?} +0xc.. {ints:?}", latin1(text)));
        }
        let place = if names(head, text) { 0 } else if names(body, text) { 1 } else { continue };
        match kind {
            0x7d5 | 0x7d6 => club[place] = club[place].or(Some(object)),
            0x7d7..=0x7e0 | 0x7e7 => person[place] = person[place].or(Some(object)),
            _ => {}
        }
    }
    for place in 0..2 {
        if let Some(key) = club[place].and_then(crest) {
            return Some(key);
        }
        if let Some(staff) = person[place].filter(|&s| mapped(s, 0x6e)) {
            let nation = rd::<usize>(staff + 0x24);
            let team = mapped(nation, 0x5b).then(|| game_text(nation + 4, 51)).filter(|n| names(head, n) || names(body, n));
            if let Some(key) = team.and_then(|_| crest(nation)).or_else(|| crest(rd::<usize>(staff + 0x39))) {
                return Some(key);
            }
        }
    }
    None
}

/// The crest keys a text names, longest runs of capitalised words first.
unsafe fn crests_named(text: &[u8]) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    for text in [text] {
        let words: Vec<&[u8]> = text.split(|&c| c == b' ').map(|w| w.trim_ascii_end().trim_ascii_end()).collect();
        let words: Vec<&[u8]> = words.iter().map(|w| w.strip_suffix(b",").or(w.strip_suffix(b".")).or(w.strip_suffix(b"'s")).unwrap_or(w)).collect();
        for n in (1..=5).rev() {
            for run in words.windows(n) {
                if run.iter().all(|w| w.first().is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'&')) {
                    let key = run.join(&b' ');
                    if key.len() >= 3 && has_crest(&key) && !found.contains(&key) {
                        found.push(key);
                    }
                }
            }
        }
    }
    found
}

/// mike_appguy's story header: the crest of the club or competition, the headline large in gold,
/// the date under it.
unsafe fn story_header(rect: (i32, i32, i32, i32), head: &[u8], date: &[u8], font: u32, body: &[u8]) {
    let (x1, y1, x2, y2) = rect;
    // the story text was moved down by STORY_SHIFT: the header takes that room too
    let y2 = y2 + story_shift(rect);
    let h = y2 - y1;
    let panel = pixel_at(x1 + 3, y2 + 6).unwrap_or(AREA);
    fill(x1, y1, x2, y2, panel);
    let mut tx = x1 + 16;
    let key = story_crest(head, body);
    if let Some(key) = &key {
        let tile = h - 22;
        if has_crest(key) {
            fill(tx, y1 + 10, tx + tile, y1 + 10 + tile, [0xf2, 0xf2, 0xf2]);
            draw_crest(key, tx + tile / 10, y1 + 10 + tile / 10, tile * 8 / 10);
            tx += tile + 16;
        }
    }
    let big = font & !0xffff | 7; // the title bar's font; a smaller one when it does not fit
    // (a low header: the size that stays clear of the date under it)
    let tall = |n: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)(n as i32);
    let big = if small_screen() { font & !0xffff | (2..=7).rev().find(|&n| tall(n) * 5 / 4 <= h * 47 / 100).unwrap_or(2) } else { big };
    put(tx, y1 + 8, x2 - 16, y1 + h * 55 / 100, 0x9, big, GOLD, &latin1(head));
    put(tx, y1 + h * 52 / 100, x2 - 16, y1 + h * 80 / 100, 0x9, smaller(font, 2), GREY, &latin1(date));
    let dx = tx + text_width(smaller(font, 2), date) + 20;
    news::category(dx, y1 + h * 54 / 100, y1 + h * 78 / 100, smaller(font, 3), head);
    fill(x1 + 16, y2 - 4, x2 - 16, y2 - 3, [0x3a, 0x3f, 0x47]); // the divider
    // the text across the panel, a transfer's card, the chips of the club and the links
    #[allow(static_mut_refs)]
    if let Some(b) = STORY_BODY.take() {
        news::story(head, rect, b, story_shift(rect), h * 45 / 100, font, key);
    }
}

/// The story's club row on the news screen and its club; a club a click on it asked for.
static mut NEWS_LINK: Option<((i32, i32, i32, i32), usize)> = None;
/// The club a click on the news club row asked for, and the channel it is for (a network client's
/// seat on the host, 0 here).
static mut PENDING_CLUB: (i16, usize) = (0, 0);
const NEWS_INPUT: usize = 0x7892b0; // news_screens_news_input(widget), cdecl, returns a screen code
const OPEN_CLUB: usize = 0x453d60; // open_club_squad_screen(club, reserves, restore, alt_view, locked)
const OBJECT_LINK: usize = 0x7643d0; // menubar_handle_object_link(short cell), cdecl, returns a screen code
static mut NEWS_INPUT_ORIG: usize = 0;

/// The news screen's input: a club asked for by the story's row opens as a club link does
/// (0x7643d0): open_club_squad_screen, then -4 so the screen loop builds it.
extern "C" fn news_input(widget: i32) -> i32 {
    unsafe {
        let serving = *((SCREEN_MANAGER + 0x382a) as *const i16);
        let (channel, club) = PENDING_CLUB;
        if club != 0 && channel == serving {
            PENDING_CLUB = (0, 0);
            game::<extern "C" fn(usize, i32, i32, i32, i32)>(OPEN_CLUB)(club, 0, 0, 0, 0);
            NEWS_LINK = None;
            return -4;
        }
        // a client's story link (its cell on this display): opened as the game opens a link
        if let Some((_, cell)) = news::PENDING_CELL.take_if(|p| p.0 == serving) {
            // (only a cell of this display's item list that holds a link: else the original input)
            let display = rd::<usize>(CHANNEL_DISPLAY + 0x300 * serving.max(0) as usize);
            let valid = mapped(display + 0x12e9a0, 2) && cell >= 0 && {
                let count = rd::<i16>(display + 0x12e9a0).clamp(0, 1200);
                let item = display + 0xba95e + cell as usize * 0x18c;
                cell < count && mapped(item, 0x18c) && (0x7d0..=0x7e7).contains(&rd::<i16>(item + 8))
            };
            if valid {
                if trace() {
                    log(&format!("news: channel {serving} opens link cell {cell}"));
                }
                return game::<extern "C" fn(i16) -> i32>(OBJECT_LINK)(cell);
            }
        }
        // Mark all read (news::mark_button), then the screen built again with the new flags
        if news::PENDING_MARK.take_if(|&mut c| c == serving).is_some() {
            news::mark_all_read(serving as usize);
            return -4;
        }
        game::<extern "C" fn(i32) -> i32>(NEWS_INPUT_ORIG)(widget)
    }
}

/// How far the story text moves down to make room for mike_appguy's taller header.
fn story_shift(header: (i32, i32, i32, i32)) -> i32 {
    (header.3 - header.1) * 2 / 3
}


/// The story's text panel (darkened, under the header) this frame.
static mut STORY_BODY: Option<(i32, i32, i32, i32)> = None;

/// A w x h block of the back buffer, row by row (one lock); 16-bit screens, all inside it.
unsafe fn region(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<[u8; 3]>> {
    let surface = *(BACK_BUFFER as *const *mut usize);
    let Some(f565) = screen_format() else { return None };
    if surface.is_null() {
        return None;
    }
    let vtable = *surface as *const usize;
    let lock: extern "system" fn(*mut usize, usize, *mut u8, u32, usize) -> i32 = std::mem::transmute(*vtable.add(25));
    let unlock: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(32));
    let mut desc = [0u8; 108];
    desc[0] = 108;
    if lock(surface, 0, desc.as_mut_ptr(), 1, 0) != 0 {
        return None;
    }
    let field = |at: usize| i32::from_le_bytes(desc[at..at + 4].try_into().unwrap());
    let (sh, sw, pitch, bits) = (field(8), field(0xc), field(0x10), field(0x24) as usize);
    let inside = x >= 0 && y >= 0 && x + w <= sw && y + h <= sh;
    let out = inside.then(|| {
        (0..h * w)
            .map(|k| {
                let p = *((bits + ((y + k / w) * pitch + (x + k % w) * 2) as usize) as *const u16) as u32;
                unpack(p, f565).map(|v| v as u8)
            })
            .collect()
    });
    unlock(surface, 0);
    out
}

/// One pixel of the back buffer; 16-bit screens.
unsafe fn pixel_at(x: i32, y: i32) -> Option<[u8; 3]> {
    let surface = *(BACK_BUFFER as *const *mut usize);
    let Some(f565) = screen_format() else { return None };
    if surface.is_null() {
        return None;
    }
    let vtable = *surface as *const usize;
    let lock: extern "system" fn(*mut usize, usize, *mut u8, u32, usize) -> i32 = std::mem::transmute(*vtable.add(25));
    let unlock: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(32));
    let mut desc = [0u8; 108];
    desc[0] = 108;
    if lock(surface, 0, desc.as_mut_ptr(), 1, 0) != 0 {
        return None;
    }
    let field = |at: usize| i32::from_le_bytes(desc[at..at + 4].try_into().unwrap());
    let (h, w, pitch, bits) = (field(8), field(0xc), field(0x10), field(0x24) as usize);
    let p = (x >= 0 && y >= 0 && x < w && y < h).then(|| *((bits + (y * pitch + x * 2) as usize) as *const u16) as u32);
    unlock(surface, 0);
    Some(unpack(p?, f565).map(|v| v as u8))
}

unsafe fn rgb(c: [u8; 3]) -> u16 {
    game::<extern "C" fn(u32, u32, u32, u32) -> u32>(RGB_TO_PIXEL)(c[0] as u32, c[1] as u32, c[2] as u32, 0) as u16
}

unsafe fn apply_palette() {
    for &(at, c) in THEME {
        (at as *mut u16).write_unaligned(rgb(c));
    }
    // the tab bars' and button rows' colour: gui_add_tab_bar (0x5ed780) makes it once (flag byte
    // 0xb0be34, colour 0xb0be36) and it came out purple, olive or black by resolution; set here,
    // the game keeps it
    *(TAB_COLOUR_MADE as *mut u8) = 1;
    (TAB_COLOUR as *mut u16).write_unaligned(rgb(SIDEBAR));
}

extern "C" fn palette_init() {
    unsafe {
        game::<extern "C" fn()>(PALETTE_INIT)();
        apply_palette();
    }
}

/// Takes the place of the background picture blit in Display::draw_screen: a flat fill.
extern "C" fn background(x: i32, y: i32, picture: usize) {
    unsafe {
        // (a list redrawn alone, a scroll, has no flip pass: its time is told here)
        let (text, calls) = std::mem::take(&mut TEXT_TIME);
        if TIMING && calls > 0 {
            let [panels, glyphs, results] = std::mem::take(&mut HOOK_TIME).map(|d| d.as_secs_f64() * 1e3);
            log(&format!("timing: {calls} texts {:.1} ms before a full frame, panels {panels:.1}, glyphs {glyphs:.1}, results {results:.1}", text.as_secs_f64() * 1e3));
        }
        FULL_FRAME = true;
        ROOMY_ROWS = false; // (the line "Born ..." comes before a profile tab's rows)
        #[allow(static_mut_refs)]
        NATION_HIT.clear(); // (the strip, if this screen has one, sets it again)
        TABLE_BEST.0 = 0;
        static mut LAID_OUT: bool = false;
        // (spent only when the layout is really decided: not on a first frame of an unknown size)
        if on(SQUAD_TABLE) && screen().0 >= 1280 && !std::mem::replace(&mut LAID_OUT, true) {
            squad_two_columns();
        }
        #[allow(static_mut_refs)]
        LAST_MINUTES.clear();
        news::MENU_SEEN = false; // (an open menu is drawn again in this frame)
        comp::full_frame();
        #[allow(static_mut_refs)]
        LABELS.clear(); // a script's labels: only what this screen draws
        #[allow(static_mut_refs)]
        PANELS.clear();
        INJURY_ROW = (i32::MIN, 0, false);
        CONDITION_ROW = (i32::MIN, 0);
        PITCH = (0, 0, 0, 0); // a new screen: the pitch is seen again if it has one
        // another screen (the screen manager's current node): what a screen kept is dropped, so
        // its first frame is not taken for the last screen's (a match title for a band...)
        // (a network client's screens come from the host: a new one each time its display is reset)
        let node = if remote::CLIENT { CLIENT_SCREEN } else if mapped(SCREEN_MANAGER + 0x7e2, 4) { ((SCREEN_MANAGER + 0x7e2) as *const usize).read_unaligned() } else { 0 };
        if node != LAST_NODE {
            LAST_NODE = node;
            MATCH = false;
            #[allow(static_mut_refs)]
            MATCH_TAB.clear();
            OVERVIEW = false;
            TACTICS = false;
            LIVE_TACTICS = false;
            SEARCH = false;
            SQUAD_ABILITY = false;
            assistant::forget();
            RATING_X = 0;
            #[allow(static_mut_refs)]
            {
                MATCH_SIDES.clear();
                MATCH_SCORES.clear();
                SQUAD_COLS.clear();
                COLUMN_FONTS.clear();
            }
            // (a network client's screens don't always come with a full redraw: the injury rows
            // are forgotten with the screen too, or a later screen's texts at that height got a cross)
            INJURY_ROW = (i32::MIN, 0, false);
            CONDITION_ROW = (i32::MIN, 0);
            if trace() {
                log(&format!("screen node {node:#x}"));
            }
        }
        if !on(THEME_ON) {
            return game::<extern "C" fn(i32, i32, usize)>(BLIT_PICTURE)(x, y, picture);
        }
        // ponytail: also catches a palette init that ran without the hook (another mod's own path)
        if (THEME[0].0 as *const u16).read_unaligned() != rgb(THEME[0].1) {
            apply_palette();
        }
        let (w, h) = (*(SCREEN_W as *const i32), *(SCREEN_H as *const i32));
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_RECT)(0, 0, w - 1, h - 1, 0, rgb(BACKGROUND) as u32);
        // Game Options' Background Changes on: the game's picture from pics.cfg as well (800x600,
        // so the flat fill first: the rest of a larger screen would keep the last screen's parts)
        if *(BACKGROUND_CHANGES_OFF as *const u32) == 0 {
            game::<extern "C" fn(i32, i32, usize)>(BLIT_PICTURE)(x, y, picture);
            if on(BACKGROUND_SHOW) {
                keep_picture();
            }
        } else {
            #[allow(static_mut_refs)]
            BEHIND.0.clear();
        }
    }
}

/// Background Changes on (`background_show`): the panels let the game's picture through. The
/// picture as it stood before the panels (pixels, width, height), and for each picture pixel what
/// a panel pixel over it becomes. Only pixels of exactly the panel colour change, at the flip, so
/// texts, lines and boxes stay as they are.
static mut BEHIND: (Vec<u16>, i32, i32) = (Vec::new(), 0, 0);
static mut THROUGH: Vec<u16> = Vec::new();
/// How much of the picture shows, in %: `background_opacity` in cmui.ini, looked at every half
/// second, in force from the next full frame.
static mut PICTURE_SHOWS: u32 = 30;

/// The back buffer locked: its pixels' address, pitch, width and height.
unsafe fn with_back_buffer(work: impl FnOnce(usize, i32, i32, i32)) {
    let surface = *(BACK_BUFFER as *const *mut usize);
    if surface.is_null() {
        return;
    }
    let vtable = *surface as *const usize;
    let lock: extern "system" fn(*mut usize, usize, *mut u8, u32, usize) -> i32 = std::mem::transmute(*vtable.add(25));
    let unlock: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(32));
    let mut desc = [0u8; 108]; // DDSURFACEDESC
    desc[0] = 108;
    if lock(surface, 0, desc.as_mut_ptr(), 1, 0) != 0 {
        return;
    }
    let field = |at: usize| i32::from_le_bytes(desc[at..at + 4].try_into().unwrap());
    work(field(0x24) as usize, field(0x10), field(0xc), field(8));
    unlock(surface, 0);
}

#[allow(static_mut_refs)]
unsafe fn keep_picture() {
    let Some(f565) = screen_format() else { return };
    static mut BUILT: u32 = u32::MAX;
    let shows = PICTURE_SHOWS;
    if BUILT != shows {
        BUILT = shows;
        let (area, flat) = (rgb(AREA), rgb(BACKGROUND));
        let a = unpack(area as u32, f565);
        THROUGH = (0..=0xffffu32)
            .map(|p| if p == flat as u32 { area } else { pack([0, 1, 2].map(|i| (unpack(p, f565)[i] * shows + a[i] * (100 - shows)) / 100), f565) })
            .collect();
    }
    with_back_buffer(|bits, pitch, w, h| {
        BEHIND.0.clear();
        for y in 0..h {
            BEHIND.0.extend_from_slice(std::slice::from_raw_parts((bits + (y * pitch) as usize) as *const u16, w as usize));
        }
        (BEHIND.1, BEHIND.2) = (w, h);
    });
}

#[allow(static_mut_refs)]
unsafe fn let_picture_through(left: i32, top: i32, right: i32, bottom: i32) {
    if BEHIND.0.is_empty() {
        return;
    }
    let area = rgb(AREA);
    with_back_buffer(|bits, pitch, w, h| {
        if (w, h) != (BEHIND.1, BEHIND.2) {
            return;
        }
        let whole = left < 0 || top < 0 || right <= left || bottom <= top;
        let (x1, y1, x2, y2) = if whole { (0, 0, w, h) } else { (left, top, (right + 1).min(w), (bottom + 1).min(h)) };
        for y in y1..y2 {
            let row = std::slice::from_raw_parts_mut((bits + (y * pitch) as usize) as *mut u16, w as usize);
            let under = &BEHIND.0[(y * w) as usize..((y + 1) * w) as usize];
            for x in x1 as usize..x2 as usize {
                if row[x] == area {
                    row[x] = THROUGH[under[x] as usize];
                }
            }
        }
    });
}

/// Points the one direct call to `target` inside [func, func + len) at `hook`.
unsafe fn patch_call(func: usize, len: usize, target: usize, hook: usize) {
    patch_calls(func, len, target, hook, 1);
}

/// Points the `count` direct calls to `target` inside [func, func + len) at `hook`, or none when
/// there are not exactly that many.
unsafe fn patch_calls(func: usize, len: usize, target: usize, hook: usize, count: usize) {
    let code = std::slice::from_raw_parts(func as *const u8, len);
    let sites: Vec<usize> = (0..len - 4)
        .filter(|&i| {
            let rel = i32::from_le_bytes(code[i + 1..i + 5].try_into().unwrap());
            code[i] == 0xe8 && (func + i + 5).wrapping_add(rel as usize) == target
        })
        .map(|i| func + i)
        .collect();
    if sites.len() != count {
        log(&format!("{func:#x}: {} calls to {target:#x}, not {count}, left alone", sites.len()));
        return;
    }
    for at in sites {
        let mut old = 0;
        VirtualProtect(at as *mut u8, 5, 0x40, &mut old);
        ((at + 1) as *mut i32).write_unaligned((hook as i32).wrapping_sub(at as i32 + 5));
        VirtualProtect(at as *mut u8, 5, old, &mut old);
        log(&format!("{at:#x}: call {target:#x} now goes to cmui"));
    }
}

/// Takes the place of the grid lines Area::draw puts between the rows and columns of a grid area:
/// mike_appguy's areas are one plain surface.
extern "C" fn no_grid_line(_x1: i32, _y1: i32, _x2: i32, _y2: i32, _style: u32, _colour: u32) {}

const DARKEN: usize = 0x5e42e0; // gui_darken_rect(x1, y1, x2, y2): 60% of what is there
static mut DARKEN_ORIG: usize = 0;

/// The screen size of this exe (800x600 unpatched, 1280x800, 1680x1050...).
fn screen() -> (i32, i32) {
    unsafe { (*(SCREEN_W as *const i32), *(SCREEN_H as *const i32)) }
}

/// gui_darken_rect on the flat background: the area colour, so a cell darkened on top of a
/// darkened area does not come out darker than the seams between cells. Grid cells leave a
/// one-pixel gap to the next, where the lighter screen shows as a line; the fill takes that pixel
/// in too. The full-screen darken behind dialogs stays a real darken, so the screen behind shows.
extern "C" fn darken(x1: i32, y1: i32, x2: i32, y2: i32) {
    let (w, h) = screen();
    if x2 - x1 >= w - 2 {
        unsafe { game::<extern "C" fn(i32, i32, i32, i32)>(DARKEN_ORIG)(x1, y1, x2, y2) }
    } else if x2 >= 0 && y2 >= 0 && x1 < w && y1 < h {
        fill(x1.max(0), y1.max(0), (x2 + 1).min(w - 1), (y2 + 1).min(h - 1), AREA);
        #[allow(static_mut_refs)]
        unsafe {
            if DARKENED.len() < 4096 {
                DARKENED.push((y1, y2 + 1));
            }
        }
    }
}

const DRAW_TEXT_BOX: usize = 0x5e6d80; // (x1, y1, x2, y2, flags, font, colour, text, caret)
/// The player profile tab's attribute table: 31 entries of a 100-byte name and a value byte.
const ATTRIBUTES: usize = 0xddf570;
static mut TEXT_BOX_ORIG: usize = 0;
/// y1 of the last text box drawn, and whether its text was an attribute name.
static mut LAST_TEXT: (i32, bool) = (i32::MIN, false);
/// Width and height of the label cells of the last attribute grid: labels of the same size are
/// attributes too (the hidden-attribute column of patched exes is not in the table).
static mut GRID: (i32, i32) = (0, 0);
/// Room right of a bar for the value.
const VALUE_W: i32 = 34;

unsafe fn is_attribute_name(text: &[u8]) -> bool {
    !text.is_empty() && readable(ATTRIBUTES, 31 * 0x65) && (0..31).any(|k| game_text(ATTRIBUTES + 0x65 * k, 100) == text)
}

/// One font for every attribute label in a box `width` wide: the one the longest name ("  Name",
/// as the game pads them) fits in, so small screens neither mix sizes nor wrap "Off The Ball".
static mut ATTRIBUTE_FONT: (u32, i32, u32) = (0, 0, 0);
unsafe fn attribute_font(font: u32, width: i32) -> u32 {
    if ATTRIBUTE_FONT.0 != font || ATTRIBUTE_FONT.1 != width {
        let names: Vec<Vec<u8>> = (0..31).map(|k| game_text(ATTRIBUTES + 0x65 * k, 100)).collect();
        if names.iter().any(|n| n.is_empty()) {
            return font; // a network client: not all names from the host yet
        }
        let longest = names.iter().map(|n| [b"  ".as_slice(), n].concat()).max_by_key(|n| text_width(font, n)).unwrap_or_default();
        let text = std::ffi::CString::new(longest).unwrap_or_default();
        ATTRIBUTE_FONT = (font, width, fit_font(font, text.as_ptr(), width));
    }
    ATTRIBUTE_FONT.2
}

fn fill(x1: i32, y1: i32, x2: i32, y2: i32, c: [u8; 3]) {
    unsafe { game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_RECT)(x1, y1, x2, y2, 0, rgb(c) as u32) }
}

/// A width or height measured at 1680x1050, in this screen's pixels (minimum sizes that tell a
/// screen's boxes apart).
fn sx(v: i32) -> i32 {
    v * screen().0.max(1) / 1680
}

fn sy(v: i32) -> i32 {
    v * screen().1.max(1) / 1050
}

fn rgb565(c: [u8; 3]) -> u16 {
    (c[0] as u16 >> 3) << 11 | (c[1] as u16 >> 2) << 5 | c[2] as u16 >> 3
}

// The screen is 16-bit RGB565 or RGB555 (which one depends on the display driver: the user's
// 2001 install with cmfix gets 565, the 25-26 install without it 555). cmui keeps its own colours
// in 565 and converts at the screen; the game's own colours are compared as 565.

/// The screen's 16-bit format: Some(true) 565, Some(false) 555, None for anything else.
fn screen_format() -> Option<bool> {
    match unsafe { *(GREEN_MASK as *const u32) } {
        0x7e0 => Some(true),
        0x3e0 => Some(false),
        _ => None,
    }
}

/// 8-bit channels of a pixel in 565 (`f565`) or 555.
fn unpack(p: u32, f565: bool) -> [u32; 3] {
    if f565 {
        [(p >> 11 & 31) * 255 / 31, (p >> 5 & 63) * 255 / 63, (p & 31) * 255 / 31]
    } else {
        [(p >> 10 & 31) * 255 / 31, (p >> 5 & 31) * 255 / 31, (p & 31) * 255 / 31]
    }
}

fn pack(c: [u32; 3], f565: bool) -> u16 {
    if f565 {
        ((c[0] >> 3) << 11 | (c[1] >> 2) << 5 | c[2] >> 3) as u16
    } else {
        ((c[0] >> 3) << 10 | (c[1] >> 3) << 5 | c[2] >> 3) as u16
    }
}

/// Whether a screen pixel is the game's 565 colour constant `c` (in 555 the game drops the low
/// green bit, so the constant is converted, not the pixel).
fn is_colour(p: u32, c: u32) -> bool {
    p & 0xffff == px565(c)
}

/// A 565 colour as a screen pixel.
fn px565(p: u32) -> u32 {
    match screen_format() {
        Some(false) => (p >> 11 & 31) << 10 | (p >> 6 & 31) << 5 | (p & 31),
        _ => p,
    }
}

/// mike_appguy's bar: a dark track over the cell, filled by the value (full at 20 and above) in
/// its colour, shaded from 75% to full along the bar.
fn bar(x1: i32, y1: i32, x2: i32, y2: i32, value: i32) {
    let (top, bottom) = (y1 + 2, y2 - 2);
    fill(x1, top, x2, bottom, [0x24, 0x27, 0x2d]);
    let (w, c) = ((x2 - x1) * value.clamp(0, 20) / 20, level_colour(value));
    unsafe {
        blend(x1, top, w, bottom - top, |col, _| {
            let shade = 0.75 + 0.25 * col as f32 / w.max(1) as f32;
            (rgb565(c.map(|v| (v as f32 * shade) as u8)), 255)
        })
    }
}

/// How a value cell in the attribute grid is shown.
enum Cell {
    Bar(i32),       // a number, a word or a percentage: bar by a 1..20 level, text on it
    Form(Vec<i32>), // "6-7-7-8-9": a coloured tile per match
    Plain,
}

/// Morale words of the English game, worst to best, and the level each gets.
const MORALE: &[(&[u8], i32)] = &[(b"Abysmal", 1), (b"Very Poor", 3), (b"Poor", 6), (b"Okay", 10), (b"Ok", 10),(b"Good", 13), (b"Very Good", 16), (b"Superb", 20)];

fn cell(s: &[u8]) -> Cell {
    let t = std::str::from_utf8(s.trim_ascii()).unwrap_or("");
    if let Ok(v) = t.parse::<i32>() {
        return if v > 0 { Cell::Bar(v) } else { Cell::Plain };
    }
    if let Some(p) = t.strip_suffix('%').and_then(|p| p.parse::<i32>().ok()) {
        return Cell::Bar((p / 5).clamp(1, 20));
    }
    let form: Vec<i32> = t.split('-').filter_map(|v| v.parse().ok()).collect();
    if form.len() > 1 && form.len() == t.split('-').count() {
        return Cell::Form(form);
    }
    MORALE.iter().find(|m| m.0 == t.as_bytes()).map_or(Cell::Plain, |m| Cell::Bar(m.1))
}

/// The colour of a level of 1..20 on mike_appguy's scale: red, orange, olive, green.
fn level_colour(value: i32) -> [u8; 3] {
    const STOPS: [(i32, [u8; 3]); 5] =
        [(1, [0xc8, 0x4e, 0x44]), (8, [0xd6, 0x86, 0x46]), (12, [0xc4, 0xb0, 0x4e]), (16, [0x7c, 0xb0, 0x58]), (20, [0x3e, 0xa8, 0x6a])];
    let v = value.clamp(1, 20);
    let k = STOPS.iter().position(|s| s.0 >= v).unwrap().max(1);
    let ((a, ca), (b, cb)) = (STOPS[k - 1], STOPS[k]);
    let t = (v - a) as f32 / (b - a) as f32;
    std::array::from_fn(|i| (ca[i] as f32 + (cb[i] as f32 - ca[i] as f32) * t) as u8)
}

/// gui_draw_text_box: a value in the attribute grid (the box after a grid label on the same row)
/// gets a bar, or form boxes, before its text.
/// Time spent in the text hook since the last flip, and its calls (logged by the flip while the
/// file cmui.timing exists in the game folder).
static mut TEXT_TIME: (std::time::Duration, u32) = (std::time::Duration::ZERO, 0);
static mut TIMING: bool = false;
/// Time in the panel, glyph and search results hooks since the last flip (glyphs are drawn
/// inside texts: counted in both).
static mut HOOK_TIME: [std::time::Duration; 3] = [std::time::Duration::ZERO; 3];
struct Timed(usize, std::time::Instant);
impl Drop for Timed {
    fn drop(&mut self) {
        unsafe { HOOK_TIME[self.0] += self.1.elapsed() }
    }
}

extern "C" fn text_box(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, colour: u32, text: *const i8, caret: i32) {
    let start = std::time::Instant::now();
    text_box_hooked(x1, y1, x2, y2, flags, font, colour, text, caret);
    unsafe {
        if TIMING && start.elapsed().as_micros() > 3000 && !text.is_null() {
            log(&format!("timing: slow text {:.1} ms at {x1},{y1}-{x2},{y2} flags {flags:#x} {:?}", start.elapsed().as_secs_f64() * 1e3, latin1(std::ffi::CStr::from_ptr(text).to_bytes())));
        }
        TEXT_TIME.0 += start.elapsed();
        TEXT_TIME.1 += 1;
    }
}

fn text_box_hooked(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, mut font: u32, mut colour: u32, text: *const i8, caret: i32) {
    unsafe {
        if trace() && !text.is_null() {
            log(&format!("in {x1},{y1}-{x2},{y2} flags {flags:#x} font {font:#x} colour {colour:#x} {:?}", latin1(std::ffi::CStr::from_ptr(text).to_bytes())));
        }
        if SCRIPTED && !text.is_null() {
            note_label(x1, y1, x2, y2, std::ffi::CStr::from_ptr(text).to_bytes());
        }
        if !text.is_null() {
            let s = std::ffi::CStr::from_ptr(text).to_bytes();
            let value = std::str::from_utf8(s).ok().and_then(|t| t.trim().parse::<i32>().ok());
            let size = (x2 - x1, y2 - y1);
            // the season stats table: flat headers, values coloured by column, Av R as a pill
            let t = s.trim_ascii();
            // (a dropdown's rows start with five spaces: "Passing" or "Form" there is no profile's)
            let dropdown = s.starts_with(b"     ");
            #[allow(static_mut_refs)]
            // (real game fonts only: 0xffff0001 and up; empty texts come with font 0, which draws
            // nothing and must never be picked to fit a text)
            if font >> 16 == 0xffff && font & 0xffff > 0 && !FONTS_SEEN.contains(&font) && FONTS_SEEN.len() < 32 {
                FONTS_SEEN.push(font);
            }
            // the news screen, as mike_appguy has it: flat dim dates, the story header redrawn in
            // the flip pass with a crest
            if on(NEWS) {
                news::note_text(x1, y1, x2, y2, flags, font, t);
            }
            if on(ASSISTANT_PICK) {
                assistant::note_text(x1, y1, x2, y2, font, flags, t);
            }
            if on(NEWS) && is_news_date(t) {
                let small = smaller(font, 2); // (also on a first draw, before any stats table)
                NEWS_DATE = Some((x1, y1, x2, y2, t.to_vec(), small));
                fill(x1, y1, x2 + 1, y2, AREA);
                put(x1 + 12, y1, x2, y2, 0x9, small, GREY, &latin1(&news::short_date(t)));
                return;
            }
            #[allow(static_mut_refs)]
            if y1 == SELECTED_ROW.0 && x1 > NEWS_DATE.as_ref().map_or(i32::MAX, |d| d.2) {
                SELECTED_ROW.2 = t.to_vec();
            }
            #[allow(static_mut_refs)]
            if on(NEWS) && !t.is_empty() && t == SELECTED_ROW.2.as_slice() && y2 - y1 > sy(40) && y1 > SELECTED_ROW.0 {
                STORY = Some((LAST_DARK, t.to_vec(), SELECTED_ROW.1.clone(), font, Vec::new()));
                return;
            }
            if on(NEWS) && news::row(x1, y1, x2, y2, font, colour, t) {
                return;
            }
            // a profile's other tabs have no list title: History's would stay and take their rows
            // for its list
            if flags == 1 && [&b"Injury"[..], b"Availability", b"Type"].contains(&t) {
                comp::leave_career();
            }
            history::note(x1, y1, y2, font, t);
            if on(STATS_TABLE) {
                font = comp::cell_font(y1, y2, font);
                if comp::text(x1, y1, x2, y2, flags, font, colour, s) {
                    return;
                }
            }
            #[allow(static_mut_refs)]
            if let Some(story) = STORY.as_mut().filter(|s| y1 > s.0 .3) {
                if story_len_ok(&story.4) {
                    story.4.extend_from_slice(t);
                    story.4.push(b' ');
                }
                // the story text: kept for the flip pass, which lays it out again under the
                // taller header (news::story)
                let head = story.0;
                if let Some(b) = STORY_BODY.filter(|b| x1 >= head.0 && y2 <= b.3) {
                    news::story_text(x1, y1, x2, y2, font, s, b.2 - b.0);
                    return;
                }
            }
            if (x1, y1, x2, y2) == ACTIVE_TAB {
                colour = rgb(GOLD) as u32;
                // (the match screen's open tab: its Latest Scores keeps the game's own rows)
                #[allow(static_mut_refs)]
                if MATCH && MATCH_TAB != t {
                    MATCH_TAB = t.to_vec();
                    REDRAW = true;
                }
            }
            if on(THEME_ON) && (x1, y1, x2, y2) == ROW_PANEL && s.starts_with(b"     ") {
                menu_row(x1, y1, x2, y2, &mut colour);
            }
            // (an event line starts with two spaces; a dropdown menu's rows with five, and a menu
            // opened over a match result is no list of events)
            if on(MATCH_SCREEN) && match_text(x1, y1, x2, y2, font, colour, t, s.starts_with(b"  ") && !s.starts_with(b"     ")) {
                return;
            }
            if ability_text(x1, y1, x2, y2, font, t) || squad_ability_text(x1, y1, x2, y2, font, t) {
                return;
            }
            if on(COMPARE_SCREEN) {
                compare_text(y1, t);
            }
            // on the tactics pitch: black shirt numbers, names in a dark pill with the fit under it
            let p = PITCH;
            // (by the box's middle: a wide player's name box at the touchline reaches past the grass)
            // (not a dropdown's rows, five spaces in, over the pitch)
            if p.2 > p.0 && (p.0..=p.2).contains(&((x1 + x2) / 2)) && y1 >= p.1 && y2 <= p.3 && !t.is_empty() && y2 - y1 < 80 && !s.starts_with(b"     ") {
                if t.iter().all(|c| c.is_ascii_digit()) {
                    // (in the club's text colour on its shirt; the keeper's white)
                    colour = if keeper_at(y1, y2) { rgb(WHITE) as u32 } else { KIT_TEXT.unwrap_or(rgb([0x10, 0x12, 0x14]) as u32) };
                } else {
                    let width = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, text);
                    let (cx, h) = ((x1 + x2) / 2, y2 - y1);
                    pill(cx - width / 2 - 8, y1 + 1, cx + width / 2 + 8, y2 - 1, [0x14, 0x18, 0x1c]);
                    // the game's own bytes (Latin-1: "Tó Madeira")
                    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(cx - width / 2 - 8, y1, cx + width / 2 + 8, y2, 0xc, font, rgb(WHITE) as u32, text, -1);
                    // (the human's own club only: an opponent's condition is not the human's to see)
                    let own = human_runs(SQUAD_CLUB);
                    // (during a match his live condition from the match engine)
                    let fit = pitch_member(t).filter(|_| own).and_then(|staff| match if LIVE_TACTICS { live_engine().and_then(|e| live_player(e, staff)) } else { None } {
                        Some(p) => Some(live_condition(p)),
                        None => condition(staff),
                    });
                    if let Some(fit) = fit {
                        let label = format!("{fit}%");
                        let small = if font & 0xffff > 1 { font - 1 } else { font };
                        let c = std::ffi::CString::new(label.clone()).unwrap();
                        let fw = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(small, c.as_ptr());
                        let top = y2 + if small_screen() { 5 } else { 2 };
                        let bottom = top + h * 85 / 100;
                        pill(cx - fw / 2 - 7, top, cx + fw / 2 + 7, bottom, if fit >= 90 { [0x2e, 0x7d, 0x52] } else { [0x9a, 0x7a, 0x30] });
                        put(cx - fw / 2 - 7, top, cx + fw / 2 + 7, bottom, 0xc, small, WHITE, &label);
                    }
                    return;
                }
            }
            // the tactics screen's list (left of the pitch): the position cell is kept for the
            // name; the name cell gets Av R, Tck and Drb at its right, the position cell fitness
            let (sw, sh) = screen();
            if on(TACTICS_PITCH) && y1 <= sh / 10 && t.ends_with(b" Tactics") {
                REDRAW |= KIT_TEXT != Some(colour & 0xffff); // (the pitch may come before the title)
                KIT_TEXT = Some(colour & 0xffff);
                // the club whose tactics these are (an opponent's in a match): its players are
                // the ones matched by name on this screen
                if let Some(club) = club_named(&t[..t.len() - 8]) {
                    REDRAW |= SQUAD_CLUB != club;
                    SQUAD_CLUB = club;
                }
                REDRAW |= !TACTICS;
                TACTICS = true;
                TACTICS_SEEN = true;
            }
            // the in-match list's rating cells ("6"): a one-digit number in a narrow cell in the
            // list's half of the screen, right of its name cells
            if LIVE_TACTICS && t.len() <= 2 && t.iter().all(|c| c.is_ascii_digit()) && x2 - x1 < sx(70) && y2 - y1 < 40 && x1 > sw * 30 / 100 && x2 <= sw * 55 / 100 {
                RATING_X = if RATING_X == 0 { x1 } else { RATING_X.min(x1) }; // (goals sit right of it)
            }
            // its status words ("capt", "inj 12", "sub 53") where Pos / Cond moved into their
            // column: at the right end of the name cell instead
            let status = t == b"capt" || [&b"inj "[..], b"sub ", b"on ", b"off "].iter().any(|p| t.starts_with(p) && t[p.len()..].iter().all(|c| c.is_ascii_digit()));
            if LIVE_TACTICS && LIVE_MOVED && status && x1 >= LIVE_NAME_X2 - 4 && y2 - y1 < 40 {
                // (a long name at 800x600 leaves no room for "capt": "(c)")
                let tight = NAME_END.0 == y1 && NAME_END.1 + text_width(font, t) + 6 > LIVE_NAME_X2 - 4;
                let shown: &[u8] = if tight && t == b"capt" { b"(c)" } else { t };
                text_at(LIVE_NAME_X2 - 4, y1, y2, font, if t == b"capt" { GOLD } else { GREY }, shown, 2);
                return;
            }
            if LIVE_TACTICS && LIVE_MOVED && x2 == LIVE_NAME_X2 && y2 - y1 < 40 {
                NAME_END = (y1, x1 + text_width(font, t));
            }
            // (the in-match tactics screen has no title: known by its screen parameters, asked only
            // once its pitch is seen; asked during day processing they crash the game)
            let in_match = LIVE_TACTICS;
            if on(TACTICS_PITCH) && (TACTICS || in_match) && x2 <= sw * 55 / 100 && y1 > sh * 22 / 100 && y2 - y1 < 40 {
                if is_position(t) {
                    TACTICS_POS = (y1, x1, x2);
                } else if x2 - x1 > sx(250) {
                    if let Some(staff) = squad_member(t) {
                        tactics_row(x1, y1, x2, y2, font, colour, text, staff);
                        return;
                    }
                }
            }
            // squad lists as mike_appguy has them: positions gold, fitness on the row coloured
            // (the Position(s) view's two wide columns: `squad_table`)
            let packed = on(SQUAD_TABLE) && POSITIONS_VIEW && sw >= 1280 && !SQUAD_ABILITY;
            if on(SQUAD_LIST) && (x2 - x1 < sx(260) || packed) && y2 - y1 < 40 {
                if let Some(staff) = squad_member(t) {
                    ROW_NAME = (y1, staff);
                    SQUAD_ROWS = true;
                    assistant::note_member(y1, x1, staff);
                    // (a name too long wraps and shows its first word: the column one smaller font)
                    if small_screen() && !packed {
                        font = column_font(x1, x2, font, fit_font(font, text, x2 - x1));
                    }
                    if packed {
                        // (the room a star player's star needs too)
                        let name = std::ffi::CString::new(s.iter().copied().filter(|&c| c != b'*').collect::<Vec<u8>>()).unwrap_or_default();
                        // (one size in both columns of players)
                        let f = fit_font(font, name.as_ptr(), x2 - x1 - 6 - (y2 - y1));
                        let f = if small_screen() { column_font(-1, 0, font, f) } else { f };
                        // (each name its own: the other column's row is at the same height)
                        TABLE_STAR = if star_player(SQUAD_CLUB, staff) { (y1, x1 + game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, name.as_ptr())) } else { (i32::MIN, 0) };
                        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1, x2, y2, flags, f, colour, name.as_ptr(), -1);
                        return;
                    }
                } else if is_position(t) && x2 - x1 >= sx(100) {
                    // (not the narrow picked-slot column); views without a condition column get
                    // the player's condition at the cell's right, as mike_appguy has it
                    POSITION_ROW = y1;
                    colour = rgb(GOLD) as u32;
                    // (the human's own club only: the condition of other clubs' players is not
                    // something the game shows in this view)
                    let own = human_runs(SQUAD_CLUB);
                    // (the wide cell: Squad Numbers has a list of this title too, a narrow one)
                    if packed && ROW_NAME.0 == y1 && x2 - x1 > sx(300) {
                        squad_table_row(x1, y1, x2, y2, font, text, ROW_NAME.1, own);
                        return;
                    }
                    if ROW_NAME.0 == y1 && own && !SQUAD_ABILITY {
                        let used = x1 + game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, text);
                        FIT_CELLS.push((used, y1, x2, y2, ROW_NAME.1, font));
                    }
                } else if let (true, Some(p)) = (y1 == POSITION_ROW, t.strip_suffix(b"%").and_then(|v| std::str::from_utf8(v).ok()?.parse::<i32>().ok())) {
                    colour = rgb(fit_colour(p)) as u32;
                    FIT_CELLS.retain(|c| c.1 != y1);
                }
            }
            if league_position(t).is_some() {
                LEAGUE_POS = Some((t.to_vec(), (x1, y1, x2, y2), font));
            }
            // Injuries & Bans: an injured player ("Injury" not "None") gets a red cross by his
            // condition, as the match events have it
            if on(PROFILE) && y2 - y1 < 40 {
                if t == b"Injury" {
                    INJURY_ROW = (y1, x2, false);
                } else if y1 == INJURY_ROW.0 && x1 >= INJURY_ROW.1 {
                    INJURY_ROW.2 = !t.is_empty() && t != b"None";
                } else if t == b"Condition" && INJURY_ROW.0 != i32::MIN {
                    CONDITION_ROW = (y1, x2);
                } else if y1 == CONDITION_ROW.0 && x1 >= CONDITION_ROW.1 && INJURY_ROW.2 && t.first().is_some_and(u8::is_ascii_digit) {
                    let (cy, r) = ((y1 + y2) / 2, (y2 - y1) * 30 / 100);
                    let (cx, a) = (x1 + r + 2, (r / 3).max(1));
                    fill(cx - r, cy - a, cx + r + 1, cy + a + 1, [0xd8, 0x30, 0x30]);
                    fill(cx - a, cy - r, cx + a + 1, cy + r + 1, [0xd8, 0x30, 0x30]);
                    let font = roomier_font(x1, y1, x2, y2, flags, font, text, caret);
                    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1 + 2 * r + 10, y1, x2, y2, flags, font, colour, text, caret);
                    return;
                }
            }
            if on(PROFILE) && t.starts_with(b"Born ") && y2 - y1 >= sy(30) {
                ROOMY_ROWS = true;
                if let Some(staff) = profiled_staff() {
                    // drawn in the flip pass, across the whole panel (this box only spans the line)
                    STRIP = Some((staff, latin1(t).into_owned(), x1, y1, x2, y2, font));
                    BORN_BOX = (x1, y1, x2, y2);
                    return;
                }
            }
            // the previous / next arrows of the menu bar: the game's own fonts have arrows at "("
            // and ")", fonts chosen in Game Settings have brackets there; the Traditional font's
            // text is "<<<" and ">>>"
            if on(MENU_FIXES) && [&b"("[..], b")", b"<<<", b">>>"].contains(&t) && x2 - x1 < 120 && y2 - y1 < 120 {
                arrow(x1, y1, x2, y2, t == b"(" || t == b"<<<", colour);
                return;
            }
            // the stats table's buttons: "*" back to the first columns, "+" the next ones ("<<" and
            // ">>" in the Traditional font)
            let small_box = y2 - y1 < 40 && x2 - x1 < 120;
            let star = t == b"*" || t == b"<<";
            // ("+" only right after a "*": the search filter's "-" / "+" are no stats table)
            let after_star = y1 == STATS_BUTTONS.0 && (x1 - STAR_END).abs() <= 10;
            if on(STATS_TABLE) && (star || ((t == b"+" || t == b">>") && after_star)) && small_box {
                if star {
                    STAR_END = x2;
                }
                STATS_BUTTONS = (y1, x2);
                fill(x1, y1, x2, y2, [0x3a, 0x3f, 0x47]);
                put(x1, y1, x2, y2, 0xc, font, LIGHT, if star { "-" } else { "+" });
                return;
            }
            // (only the profile's table: its heads share the row of the "-" / "+" buttons; the
            // squad list's Apps / Av R keep the game's headers like the other columns)
            let head_row = y1 == STATS_BUTTONS.0 && x1 > STATS_BUTTONS.1 && y2 - y1 < 40 && !t.is_empty();
            let head = head_row.then(|| STATS_HEADS.iter().position(|h| h.as_bytes() == t).unwrap_or(4)).filter(|_| on(STATS_TABLE));
            if let Some(k) = head.filter(|_| y2 - y1 < 40) {
                let k = if t == b"Av R" { 8 } else { k };
                SMALL_FONT = Some(font);
                if !STATS_SEEN {
                    STATS_COLS.clear();
                    STATS_SEEN = true;
                }
                fill(x1, y1, x2, y2, AREA);
                STATS_COLS.push((x1, x2, k));
                STATS_TOP = y2;
                colour = rgb(stats_colour(k)) as u32;
            } else if let Some(&(_, _, k)) = STATS_COLS.iter().find(|c| y1 >= STATS_TOP && x1 >= c.0 - 2 && x2 <= c.1 + 2) {
                if k == 8 {
                    let rating = std::str::from_utf8(t).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                    let c = match rating {
                        r if r >= 7.5 => [0x3e, 0xa8, 0x6a],
                        r if r >= 7.0 => [0xc8, 0xa8, 0x40],
                        r if r > 0.0 => [0xd0, 0x80, 0x40],
                        _ => [0x4a, 0x50, 0x5a],
                    };
                    fill(x1, y1, x2, y2, AREA);
                    if rating > 0.0 || !on(MODERN_BUTTONS) {
                        pill(x1 + 2, y1 + 2, x2 - 2, y2 - 2, c);
                        colour = rgb(WHITE) as u32;
                    }
                } else {
                    colour = rgb(stats_colour(k)) as u32;
                }
            }
            if on(MODERN_BUTTONS) && (t == b"----" || t == b"---") {
                colour = rgb(GREY) as u32;
            }
            // mike_appguy keeps form, morale and condition out of the grid, in a panel of their own:
            // their rows are not drawn, the values are kept for the panel
            let grid_sized = !dropdown && GRID.0 > 0 && (size.0 - GRID.0).abs() <= 2 && size.1 == GRID.1;
            if let Some(k) = STATUS_LABELS.iter().position(|l| l.as_bytes() == s.trim_ascii()).filter(|_| on(PROFILE)) {
                if grid_sized {
                    STATUS_LABEL = Some((k, y1));
                    STATUS_ROW.get_or_insert(y1);
                    return;
                }
            }
            if let Some((k, row)) = STATUS_LABEL.take() {
                if row == y1 {
                    STATUS[k] = s.trim_ascii().to_vec();
                    return;
                }
            }
            if LAST_TEXT == (y1, true) && x2 - x1 > sx(40) {
                let draw = game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG);
                let white = rgb([0xf0, 0xf2, 0xf4]) as u32;
                let kind = match cell(s) {
                    Cell::Bar(_) if !on(ATTRIBUTE_BARS) => Cell::Plain,
                    Cell::Form(_) if !on(STATUS_BARS) => Cell::Plain,
                    k => k,
                };
                match kind {
                    Cell::Bar(v) => {
                        bar(x1, y1, x2, y2, v);
                        VALUE_CELL = x2 - x1;
                        colour = white; // the text goes on the bar, centred as the game has it
                    }
                    Cell::Form(games) => {
                        let (gap, n) = (4, games.len() as i32);
                        let w = (x2 - x1 + gap) / n;
                        for (i, &g) in games.iter().enumerate() {
                            let bx = x1 + i as i32 * w;
                            fill(bx, y1 + 2, bx + w - gap, y2 - 2, level_colour(g * 2));
                            let digit = std::ffi::CString::new(g.to_string()).unwrap();
                            // alignment bits: 1 left, 2 right, 4 centre, 8 middle
                            draw(bx, y1, bx + w - gap, y2, flags & !0xf | 0xc, font, white, digit.as_ptr(), -1);
                        }
                        LAST_TEXT = (y1, false);
                        return;
                    }
                    Cell::Plain => {}
                }
            }
            // one word wider than its box is not drawn at all ("Competitions" on the menu bar with
            // a font from Game Settings): the largest font that fits
            let (sw, sh) = screen();
            // the menu bar's buttons: an icon over the label, no box
            if on(MENU_FIXES) && on(THEME_ON) && x1 <= sx(12).max(16) && x2 <= bar_end(sw) && y1 > sh * 15 / 100 && y2 - y1 > sh * 5 / 100 && !t.is_empty() && !t.iter().any(|c| c.is_ascii_digit()) {
                // (flags bit 0x20: a button the game has disabled here, "Add Manager" on the start
                // screen: dim)
                let colour = if flags & 0x20 != 0 { rgb([0x5a, 0x60, 0x68]) as u32 } else { colour };
                sidebar_item(x1, y1, x2, y2, font, colour, t);
                return;
            }
            if on(MENU_FIXES) && x2 <= bar_end(sw) && y1 > sh * 15 / 100 && !t.is_empty() && !t.iter().any(|c| c.is_ascii_digit()) {
                // the menu bar: one size for all its items, the largest every word fits in
                // ("Competitions" no longer smaller than the rest)
                let measure = |f: u32| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, c"Mg".as_ptr());
                for word in t.split(|c| c.is_ascii_whitespace()).filter(|w| !w.is_empty()) {
                    let word = std::ffi::CString::new(word).unwrap();
                    let f = fit_font(font, word.as_ptr(), x2 - x1 - 4);
                    if SIDEBAR_FONT.is_none_or(|s| measure(f) < measure(s)) {
                        REDRAW |= SIDEBAR_FONT.is_some();
                        SIDEBAR_FONT = Some(f);
                    }
                }
                font = SIDEBAR_FONT.unwrap_or(font);
            } else if on(MENU_FIXES)
                && value.is_none()
                && !dropdown
                && !s.trim_ascii().is_empty()
                // (the hidden attributes, "Big Matches"..., only by their grid-sized box)
                && (is_attribute_name(s.trim_ascii()) || (GRID.0 > 0 && (size.0 - GRID.0).abs() <= 2 && size.1 == GRID.1))
            {
                font = attribute_font(font, x2 - x1);
            // ("Non Competitive" wraps to "Non" on a small screen: the labels one smaller font)
            } else if on(MENU_FIXES) && !t.is_empty() && (!t.iter().any(|c| c.is_ascii_whitespace()) || (small_screen() && stats_label(t))) {
                // one font a column (cells with one left and right edge): "98%" as small as the
                // "100%" that needed it; a smaller one found later redraws the screen once
                font = column_font(x1, x2, font, fit_font(font, text, x2 - x1));
            }
            let named = value.is_none() && !dropdown && is_attribute_name(s.trim_ascii()); // labels are "  Name"
            if named {
                GRID = size;
            }
            let like_grid = !dropdown && (size.0 - GRID.0).abs() <= 2 && size.1 == GRID.1; // columns differ by a pixel
            LAST_TEXT = (y1, value.is_none() && (named || like_grid));
            if LAST_TEXT.1 && GRID.0 > 0 && !s.trim_ascii().is_empty() {
                // the empty rows under the attributes have grid-sized labels too
                note_grid_label(x1, y2, font, y2 - y1);
            }
            if trace() {
                let width = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, text);
                log(&format!("text {x1},{y1}-{x2},{y2} flags {flags:#x} font {font:#x} colour {colour:#06x} width {width} {:?} attr {}", latin1(s), LAST_TEXT.1));
            }
        }
        note_title(x1, y1, x2, y2);
        let t = TITLE;
        // the title text itself, not the rows of a dropdown list that opens over the bar
        let big = y2 - y1 >= (t.3 - t.1) * 4 / 10 && x2 - x1 >= (t.2 - t.0) / 5;
        // (a long name's box may run past the bar's right end: "14. Sidy Barhama Ndiaye
        // (Dortmund)" is 553-1679 in a 148-1658 bar at 1680x1050)
        let in_title = big && !text.is_null() && x1 >= t.0 && y1 >= t.1 && x1 < t.2 && y2 <= t.3 && t.2 > t.0;
        // a club's screens: the name goes up to make room for its league under it (flip pass)
        if on(COMPARE_SCREEN) && !text.is_null() && std::ffi::CStr::from_ptr(text).to_bytes().windows(3).any(|w| w == b" ~ ") && y1 <= screen().1 / 10 {
            COMPARE = Some((t, std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii().to_vec(), font));
        }
        // (only titles that are the club alone: a player's "Name (Club)" keeps its plain header)
        let alone = on(CLUB_HEADER) && in_title && x2 - x1 > (t.2 - t.0) * 8 / 10 && club_of(std::ffi::CStr::from_ptr(text).to_bytes()) == std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii();
        let whole = if alone { std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii() } else { &[] };
        let lift = match alone.then(|| header_club(title_name(whole))).flatten() {
            Some(club) => {
                HEADER_CLUB = Some((club, (x1, y1, x2, y2), colour));
                HEADER_PLAIN = title_name(whole).len() == whole.len();
                if HEADER_PLAIN {
                    // (the list is drawn before the title: once more for another club's squad, or
                    // when the squad's Ability view turns on or off)
                    REDRAW |= SQUAD_CLUB != club;
                    SQUAD_CLUB = club;
                    let ability = on(SEARCH_ABILITY) && screen_param(7) & SQUAD_ABILITY_BIT != 0;
                    REDRAW |= ability != SQUAD_ABILITY;
                    SQUAD_ABILITY = ability;
                }
                // (fonts do not shrink with the bar: a small bar (800x600) lifts the name more;
                // a national team has no league line under it)
                if national_team(club) { 0 } else { (t.3 - t.1) * if t.3 - t.1 < 90 { 20 } else { 14 } / 106 }
            }
            None => 0,
        };
        let own_title = alone && HEADER_CLUB.is_some_and(|h| h.1 == (x1, y1, x2, y2));
        let font = if in_title { font } else { roomier_font(x1, y1, x2, y2, flags, font, text, caret) };
        let colour = if on(THEME_ON) && colour & 0xffff == rgb(AZURE_SLOT) as u32 & 0xffff { rgb(AZURE_TEXT) as u32 } else { colour };
        // (flags bit 0x20: a disabled button's text, dim instead of the game's embossed look)
        let disabled = on(MODERN_BUTTONS) && flags & 0x20 != 0;
        let (flags, colour) = if disabled { (flags & !0x20, rgb([0x7c, 0x88, 0x96]) as u32) } else { (flags, colour) };
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1 - lift, x2, y2 - lift, flags, font, colour, text, caret);
        if !in_title && x1 > (t.0 + t.2) / 2 && x2 <= t.2 && y1 >= t.1 && y2 <= (t.1 + t.3) / 2 && (text.is_null() || std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii() != EXPORT_LABEL) {
            // a dropdown in the bar's right half (Print, Action): the right crest stays left of it
            // (Export stands clear of it; the upper half only: the first rows of an open menu
            // are in the bar too, "Squad", "Stats")
            TITLE_RIGHT = TITLE_RIGHT.min(x1);
        }
        if in_title {
            let title = std::ffi::CStr::from_ptr(text).to_bytes();
            // 80 pixels in the 106-high bar of 1680x1050, in proportion elsewhere; right of the
            // arrow button at the bar's left
            let size = (CREST_SIZE as i32).min((t.3 - t.1) * 80 / 106);
            let (cx, cy) = (t.0 + (t.3 - t.1) * 45 / 100, t.1 + (t.3 - t.1 - size) / 2);
            let key = title_name(club_of(title));
            // (a competition's history: the title has its short name, the logo goes by the full one)
            // (parameter 0 is the competition, its record at +4 as 0x498b42 reads it, name +4)
            let comp = if title.ends_with(b" History") { screen_param(0) } else { 0 };
            let record = if readable(comp, 8) { rd::<usize>(comp + 4) } else { 0 };
            let full = if readable(record, 60) { game_text(record + 4, 51) } else { Vec::new() };
            if own_title || (MATCH && x2 - x1 < screen().0 / 2) {
                // a club's own title: league logo here, crest at the right; a match side: the crest
                // by its score (flip pass)
            } else if !key.is_empty() && !draw_crest(key, cx, cy, size) && !draw_crest(&full, cx, cy, size) {
                // a screen of the manager ("<name> News"): the crest of the manager's club
                if let Some((club, surname)) = human_club() {
                    if !surname.is_empty() && title.windows(surname.len()).any(|w| w == surname) {
                        draw_crest(&club, cx, cy, size);
                    }
                }
            }
            // (player pictures right of the crest: shelved, ui\shelved\player_pictures)
        }
    }
}

// CA/PA stars under the attribute grid of a player profile, drawn in a pass before the flip.
const FLIP_RECT: usize = 0x5e2eb0; // graphics_flip_rect(left, top, right, bottom)
const GET_SCREEN_PARAM: usize = 0x7ff010; // ScreenManager::get_screen_param(this, index), thiscall
const SCREEN_MANAGER: usize = 0xb74450;
/// Labels of the grid rows that go to the player status panel, and their values this frame.
const STATUS_LABELS: [&str; 3] = ["Form", "Morale", "Condition"];
static mut STATUS: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
static mut STATUS_LABEL: Option<(usize, i32)> = None;
/// The first of those rows: the ability block takes their place.
static mut STATUS_ROW: Option<i32> = None;

/// Text in a box through the game's own text drawing; flags 1 left, 2 right, 4 centre, 8 middle.
/// One line: a smaller font when it does not fit.
unsafe fn put(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, colour: [u8; 3], s: &str) {
    let text = std::ffi::CString::new(to_latin1(s)).unwrap();
    if SCRIPTED {
        note_label(x1, y1, x2, y2, text.as_bytes());
    }
    let font = fit_font(font, text.as_ptr(), x2 - x1);
    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1, x2, y2, flags, font, rgb(colour) as u32, text.as_ptr(), -1);
}

const TEXT_WIDTH: usize = 0x5e5ad0; // font_text_width(font, text) -> pixels
/// The fonts the game has drawn with so far.
static mut FONTS_SEEN: Vec<u32> = Vec::new();

/// The font of a column of this screen (its left and right edge, the game's font): `f`, what this
/// cell needs, or the smaller one another cell needed; a smaller one found later redraws once.
#[allow(static_mut_refs)]
unsafe fn column_font(x1: i32, x2: i32, font: u32, f: u32) -> u32 {
    let measure = |f: u32| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, c"Mg".as_ptr());
    match COLUMN_FONTS.iter_mut().find(|c| c.0 == (x1, x2, font)) {
        Some(c) => {
            if measure(f) < measure(c.1) {
                c.1 = f;
                REDRAW = true;
            }
            c.1
        }
        None if f != font => {
            COLUMN_FONTS.push(((x1, x2, font), f));
            REDRAW = true;
            f
        }
        None => font,
    }
}

/// Below the flagship size: what is done only to make things fit on 800x600 and 1280x800 asks this,
/// so 1680x1050 and up stay as they are.
fn small_screen() -> bool {
    screen().0 < 1680
}

/// The right end of the menu bar's buttons: 8% of a wide screen, the bar's own width on a small one
/// (its picture is 126 wide, 90 at 800x600).
fn bar_end(sw: i32) -> i32 {
    (sw * 8 / 100).max(if sw > 800 { 130 } else { 95 })
}

/// `font` when the text fits in `width`, else the font seen so far that makes it widest while
/// still fitting (the smallest one when none fits).
unsafe fn fit_font(font: u32, text: *const i8, width: i32) -> u32 {
    let measure = |f: u32| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, text);
    if measure(font) <= width {
        return font;
    }
    #[allow(static_mut_refs)]
    let sizes: Vec<(i32, u32)> = FONTS_SEEN.iter().map(|&f| (measure(f), f)).collect();
    let fits = sizes.iter().filter(|s| s.0 > 0 && s.0 <= width).max();
    fits.or(sizes.iter().filter(|s| s.0 > 0).min()).map_or(font, |s| s.1)
}

const FONT_HEIGHT: usize = 0x5e5cd0; // cdecl(font) -> line height in pixels
/// The table in a Position(s) row's wide cell: where each column ends, in percent of the cell.
const TABLE_ENDS: [i32; 6] = [26, 45, 59, 73, 85, 99];
const TABLE_HEADS: [&str; 6] = ["Pos", "CA", "Av R", "Cond", "Gls", "Apps"];
/// The cell widths of the two columns of players: picked, Inf, name, the table.
const TABLE_WIDTHS: [u8; 8] = [4, 3, 17, 33, 4, 3, 17, 33];
/// The rows drawn this frame (the table's left, width, top, bottom, font, where a star player's
/// star goes, else 0), and the last star player's row and the end of his name.
static mut TABLE_ROWS: Vec<(i32, i32, i32, i32, u32, i32)> = Vec::new();
static mut TABLE_STAR: (i32, i32) = (i32::MIN, 0);
/// The star players of the club on screen, for this frame's rows.
static mut TABLE_BEST: (usize, Vec<usize>) = (0, Vec::new());

/// A club's three best by the intrinsic score (the host's in a network client).
#[allow(static_mut_refs)]
unsafe fn star_player(club: usize, staff: usize) -> bool {
    if TABLE_BEST.0 != club {
        let best = if remote::CLIENT {
            ask_host(&key(b'U', &[club as u32])).map(|v| v.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize).collect())
        } else {
            Some(scout::best_three(club))
        };
        match best {
            Some(b) => TABLE_BEST = (club, b),
            None => return false,
        }
    }
    TABLE_BEST.1.contains(&staff)
}

/// A row of the Position(s) view: the player's positions and, in fixed columns, his CA stars, the
/// season's average rating, condition (the human's own club only), goals and appearances. One font
/// for the column, whatever a row's positions are; only something longer still falls back.
#[allow(static_mut_refs)]
unsafe fn squad_table_row(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, text: *const i8, staff: usize, own: bool) {
    let w = x2 - x1;
    let end = |k: usize| x1 + w * TABLE_ENDS[k] / 100;
    // the role large and its sides small after it ("AM/F" "RLC"): the longest ones fit, and the
    // column's font stays the same on every row
    let room = end(0) - x1 - 8;
    let f = if on(BIGGER_TEXT) { font_up(font, b"AM/F\0".as_ptr() as *const i8, room - text_width(font, b"RLC") - 4, y2 - y1, 1, 3) } else { font };
    let small = if f != font { font } else { smaller(font, 1) };
    let whole = std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii();
    let (role, sides) = whole.iter().rposition(|&c| c == b' ').map_or((whole, &b""[..]), |at| (&whole[..at], &whole[at + 1..]));
    let lines = table_lines(x1, w);
    let mid = |k: usize| (lines[k] + lines[k + 1]) / 2;
    // (a very long role, "SW/D/DM", in the small size: the sides stay clear of it)
    let long = text_width(f, role) + text_width(small, sides) + 8 > room;
    text_at(lines[0] + 8, y1, y2, if long { small } else { f }, GOLD, role, 0);
    text_at(lines[1] - 6, y1, y2, small, LIGHT, sides, 2);
    if let Some((ca, _)) = ca_pa(rd::<usize>(staff + 0x61)).filter(|_| !HIDE_CA_PA) {
        let size = ((y2 - y1) * 60 / 100).min((end(1) - end(0) - 24) / 5);
        stars(end(0) + 6, y1 + (y2 - y1 - size) / 2, size, ca, GOLD);
    }
    let rating = season_stat(staff, 0x11);
    let shade = match std::str::from_utf8(&rating).ok().and_then(|v| v.parse::<f32>().ok()) {
        None => GREY,
        Some(v) if v >= 7.5 => [0x4c, 0xc0, 0x7a],
        Some(v) if v >= 7.0 => GOLD,
        Some(_) => LIGHT,
    };
    // (the numbers one font, the one "100%" fits its column in)
    let n = if small_screen() { fit_font(f, c"100%".as_ptr(), lines[4] - lines[3] - 6) } else { f };
    text_at(mid(2), y1, y2, n, shade, &rating, 1);
    if let Some(fit) = condition(staff).filter(|_| own) {
        text_at(mid(3), y1, y2, n, fit_colour(fit), format!("{fit}%").as_bytes(), 1);
    }
    text_at(mid(4), y1, y2, n, LIGHT, &season_stat(staff, 2), 1);
    text_at(mid(5), y1, y2, n, LIGHT, &all_apps(&season_stat(staff, 1)), 1);
    if TABLE_ROWS.len() < 120 {
        TABLE_ROWS.push((x1, w, y1, y2, font, if TABLE_STAR.0 == y1 { TABLE_STAR.1 } else { 0 }));
    }
}

/// The table's vertical lines: its left edge, between the columns, its right edge.
fn table_lines(x1: i32, w: i32) -> [i32; 7] {
    let end = |k: usize| x1 + w * TABLE_ENDS[k] / 100;
    [x1, end(0), end(1) + 2, end(2) + 6, end(3) + 6, end(4) + 6, x1 + w]
}

/// Appearances as one number: "25 (10)", starts and as a sub, is 35; anything else as it is.
fn all_apps(t: &[u8]) -> Vec<u8> {
    let parts: Option<Vec<u32>> = t.split(|c| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| std::str::from_utf8(p).ok()?.parse().ok()).collect();
    match parts {
        Some(p) if !p.is_empty() => p.iter().sum::<u32>().to_string().into_bytes(),
        _ => t.to_vec(),
    }
}

#[cfg(test)]
#[test]
fn appearances_add_up() {
    assert_eq!(all_apps(b"25 (10)"), b"35");
    assert_eq!(all_apps(b"0 (1)"), b"1");
    assert_eq!(all_apps(b"13"), b"13");
    assert_eq!(all_apps(b"-"), b"-");
}

/// Over each column of players (the flip pass): the table's heads just above its first row, and
/// thin lines between the rows, across picked, Inf and name too, and between the table's columns.
#[allow(static_mut_refs)]
unsafe fn squad_table_heads() {
    const LINE: [u8; 3] = [0x34, 0x38, 0x40];
    let mut rows = std::mem::take(&mut TABLE_ROWS);
    // (a full frame draws an open dropdown too, before this pass: not over it)
    if news::MENU_SEEN {
        return;
    }
    rows.sort_by_key(|r| (r.0, r.2));
    for column in rows.chunk_by(|a, b| a.0 == b.0) {
        let (first, last) = (column[0], column[column.len() - 1]);
        let (x1, w, y1, y2, font) = (first.0, first.1, first.2, last.3, first.4);
        let small = font & !0xffff | 1;
        let lines = table_lines(x1, w);
        // (no column shown, no head: CA hidden, another club's condition; with Background Changes
        // on the picture is under this strip)
        fill(x1 - w * (4 + 3 + 17) / 33, y1 - 16, x1 + w, y1 - 1, AREA);
        let own = human_runs(SQUAD_CLUB);
        for (k, head) in TABLE_HEADS.iter().enumerate().filter(|(k, _)| !(*k == 1 && HIDE_CA_PA) && !(*k == 3 && !own)) {
            text_at((lines[k] + lines[k + 1]) / 2, y1 - 16, y1 - 1, small, GREY, head.as_bytes(), 1);
        }
        // (the cells to the table's left, in `TABLE_WIDTHS` units)
        let left = x1 - w * (4 + 3 + 17) / 33;
        for r in column {
            // a star player: the whole row a little gold, a gold star after his name
            if r.5 != 0 {
                let tint = rgb565(GOLD);
                blend(left, r.2 - 1, x1 + w - left, r.3 - r.2 + 1, |_, _| (tint, 36));
                let size = (r.3 - r.2) * 62 / 100;
                stars_one(r.5 + 6, r.2 + (r.3 - r.2 - size) / 2, size, GOLD);
            }
            fill(left, r.2 - 2, x1 + w, r.2 - 1, LINE);
        }
        fill(left, y2 + 1, x1 + w, y2 + 2, LINE);
        // (the first: between the name and the table)
        for x in &lines[..6] {
            fill(*x, y1 - 16, *x + 1, y2 + 2, LINE);
        }
    }
}

/// The Position(s) view in two wide columns of players (at 1280 wide or more; switch
/// `squad_table`): the 800x600 code has two (8 cells, the column base up to 4), the exes here a
/// patch to three (12, 8, and a table of its own: `lea edx, [esp + 0xb4]` at 0x459a22 is `mov edx,
/// 0x601eeb` there). Two again, with widths of our own: picked, Inf, name, then the table's cell.
/// Changed only where the code is what is expected. Once, when the screen's size is known.
unsafe fn squad_two_columns() {
    static WIDTHS: [u8; 8] = TABLE_WIDTHS;
    let (table, cells, end) = (0x459a22usize, 0x459a3busize, 0x45c018usize);
    if ![0xba, 0x8d].contains(&*(table as *const u8)) || ![8, 12].contains(&*(cells as *const u8)) || ![4, 8].contains(&*(end as *const u8)) {
        log("squad list: not the expected layout code, left alone");
        return;
    }
    let mut code = [0xba, 0, 0, 0, 0, 0x90, 0x90];
    code[1..5].copy_from_slice(&(WIDTHS.as_ptr() as u32).to_le_bytes());
    for (at, new) in [(table, &code[..]), (cells, &[8u8][..]), (end, &[4u8][..])] {
        let mut old = 0;
        VirtualProtect(at as *mut u8, new.len(), 0x40, &mut old);
        std::ptr::copy_nonoverlapping(new.as_ptr(), at as *mut u8, new.len());
        VirtualProtect(at as *mut u8, new.len(), old, &mut old);
    }
}

/// A profile tab with rows of text (Injuries & Bans, Contract, Transfer, History) is on screen.
static mut ROOMY_ROWS: bool = false;
/// The Position(s) view was the squad list's last (a list comes before its title).
static mut POSITIONS_VIEW: bool = false;
/// A squad row was drawn this frame.
static mut SQUAD_ROWS: bool = false;

/// The left column of the season stats table under a profile.
fn stats_label(t: &[u8]) -> bool {
    [&b"Non Competitive"[..], b"League", b"Cup", b"Continental", b"International", b"Senior Club"].contains(&t)
}

/// The font `steps` sizes up, or as far as the text stays in `width` and `height` and the size at
/// `cap` (the game's font numbers go by size). Nothing can overlap or leave the screen: a text only
/// grows inside the box the game gave it, and cells cut short at links stay as they are.
unsafe fn font_up(font: u32, text: *const i8, width: i32, height: i32, steps: u32, cap: u32) -> u32 {
    if font >> 16 != 0xffff {
        return font;
    }
    let tall = game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT);
    let wide = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH);
    (1..=steps).rev().map(|s| font + s).find(|&f| f & 0xffff <= cap && tall((f & 0xffff) as i32) <= height && wide(f, text) <= width).unwrap_or(font)
}

/// The font for what the game draws at a large resolution (switch `bigger_text`): buttons, tabs
/// and bands two sizes up as they fit, the rows of a profile tab one. Not the menu bar, a text
/// being typed, or a match, whose parts are redrawn on their own.
unsafe fn roomier_font(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, text: *const i8, caret: i32) -> u32 {
    if !on(BIGGER_TEXT) || text.is_null() || caret >= 0 || MATCH || LIVE_TACTICS || screen().0 < 1280 || x1 < TITLE.0 || y1 < TITLE.3 {
        return font;
    }
    // (a dropdown's rows start with five spaces; every one is the same size)
    if std::ffi::CStr::from_ptr(text).to_bytes().starts_with(b"     ") {
        return font;
    }
    let (w, h) = (x2 - x1, y2 - y1);
    // (the stats table stands under every tab of a profile, the same on each, by its labels; a
    // Profile tab's attribute names keep their one font)
    let label = std::ffi::CStr::from_ptr(text).to_bytes().trim_ascii();
    #[allow(static_mut_refs)]
    let rows = (ROOMY_ROWS && GRID_SEEN.is_none() && !(LAST_TEXT.0 == y1 && LAST_TEXT.1)) || (x1 < screen().0 / 4 && stats_label(label));
    match (h >= 30 && flags & 4 != 0, rows && h < 30) {
        (true, _) => font_up(font, text, w - 12, h - 4, 2, 4),
        (_, true) => font_up(font, text, w - 4, h, 1, 3),
        _ => font,
    }
}

/// A triangle pointing left or right, centred in the box, in a game pixel colour.
fn arrow(x1: i32, y1: i32, x2: i32, y2: i32, left: bool, colour: u32) {
    let half = ((y2 - y1).min(x2 - x1) / 5).max(3); // half the height; the depth is the same
    let (cx, cy) = ((x1 + x2) / 2, (y1 + y2) / 2);
    let rect = unsafe { game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_RECT) };
    for dy in -half..=half {
        let w = half - dy.abs(); // the row's width from the base towards the tip
        let (a, b) = if left { (cx + half / 2 - w, cx + half / 2) } else { (cx - half / 2, cx - half / 2 + w) };
        rect(a, cy + dy, b, cy + dy, 0, colour);
    }
}

/// The season stats table's column heads; the first four are gold in mike_appguy's table.
const STATS_HEADS: [&str; 9] = ["Apps", "Gls", "Asts", "MoM", "Pass", "Tck", "Drb", "Sh Tar", "Av R"];
/// Its columns (x1, x2, head) and the bottom of the head row; forgotten after a full screen
/// without the table, so numbers on other screens keep their colours.
static mut STATS_COLS: Vec<(i32, i32, usize)> = Vec::new();
static mut STATS_TOP: i32 = 0;
static mut STATS_SEEN: bool = false;
/// A stats column's colour: goals green, the other three of mike_appguy's first four gold.
fn stats_colour(k: usize) -> [u8; 3] {
    match k {
        1 => [0x3e, 0xc0, 0x6a],
        0..=3 => GOLD,
        _ => LIGHT,
    }
}

/// The areas' background: the flat screen colour darkened to 60% by the game.
const AREA: [u8; 3] = [0x1a, 0x1c, 0x20];
const GOLD: [u8; 3] = [0xe0, 0xb8, 0x50];

/// The menu bar's one label font: the largest every button's lines fit in (smaller when a button
/// needs it, the bar drawn again).
static mut SIDEBAR_ITEM_FONT: Option<u32> = None;

/// A menu bar button without its box: an icon (Windows' own icon font) over its label in one
/// font for the bar, a little larger than the game's, using the bar's width up to the screen's
/// content. The click area stays the game's box.
#[allow(static_mut_refs)]
unsafe fn sidebar_item(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, colour: u32, t: &[u8]) {
    let (sw, _) = screen();
    let cx = (x1 + x2) / 2;
    let room = 2 * (cx - 2).min(sw * 87 / 1000 - cx);
    let lines: Vec<&[u8]> = t.split(|&c| c == b'\n').map(|l| l.trim_ascii()).filter(|l| !l.is_empty()).collect();
    let measure = |f: u32, s: &[u8]| text_width(f, s);
    // the largest font of this family every line of this button fits
    let mut fonts: Vec<u32> = FONTS_SEEN.iter().copied().filter(|f| f & !0xffff == font & !0xffff).collect();
    fonts.sort_by_key(|&f| std::cmp::Reverse(measure(f, b"Mg")));
    let fit = fonts.iter().copied().find(|&f| measure(f, b"Mg") <= measure(font, b"Mg") * 13 / 10 && lines.iter().all(|l| measure(f, l) <= room)).unwrap_or(font);
    if SIDEBAR_ITEM_FONT.is_none_or(|s| measure(fit, b"Mg") < measure(s, b"Mg")) {
        REDRAW |= SIDEBAR_ITEM_FONT.is_some();
        SIDEBAR_ITEM_FONT = Some(fit);
    }
    let f = SIDEBAR_ITEM_FONT.unwrap_or(fit);
    let c = colour as u16 as u32;
    let rgb = [((c >> 11 & 31) * 255 / 31) as u8, ((c >> 5 & 63) * 255 / 63) as u8, ((c & 31) * 255 / 31) as u8];
    let h = y2 - y1;
    let label = latin1(t).replace('\n', " ").to_lowercase();
    if on(CHALLENGE) {
        challenge::bar_item(x1, y1, x2, y2, font, colour, &label);
    }
    if let Some(code) = sidebar_icon(&label, t) {
        draw_icon(code, cx, y1 + h * 8 / 100, h * 32 / 100, rgb);
    }
    let (top, bottom) = (y1 + h * 46 / 100, y2 - h * 4 / 100);
    let n = lines.len().max(1) as i32;
    for (i, l) in lines.iter().enumerate() {
        let i = i as i32;
        text_at(cx, top + (bottom - top) * i / n, top + (bottom - top) * (i + 1) / n, f, rgb, l, 1);
    }
}

/// The icon of a menu bar button by its label (Segoe Fluent Icons; the trophy from Segoe UI
/// Symbol); the human manager's own button a person.
unsafe fn sidebar_icon(label: &str, t: &[u8]) -> Option<u32> {
    let has = |w: &str| label.contains(w);
    Some(match () {
        _ if has("pause") => 0xE769,
        _ if has("continue") || has("kick off") || has("re-start") || has("resume") => 0xE768,
        _ if has("competitions") => 0x1F3C6,
        _ if has("challenge") => 0xE734,
        _ if has("nations") => 0xE774,
        _ if label == "find" => 0xE721,
        _ if has("options") => 0xE713,
        _ if label == "chat" => 0xE8BD,
        _ if has("add") && has("manager") => 0xE8FA,
        _ if has("restart") => 0xE72C,
        _ if has("exit") => 0xE7E8,
        _ if has("waiting") => 0xE916,
        _ if has("tactics") => 0xEA37,
        _ if has("commentary") => 0xEC4A,
        _ if has("scores") => 0xE9D9,
        _ if human_club().is_some_and(|(_, surname)| !surname.is_empty() && t.windows(surname.len()).any(|w| w == surname)) => 0xE77B,
        _ => return None,
    })
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u16) -> usize;
    fn SelectObject(dc: usize, obj: usize) -> usize;
    fn DeleteObject(obj: usize) -> i32;
    fn SetBkMode(dc: usize, mode: i32) -> i32;
    fn SetTextColor(dc: usize, colour: u32) -> u32;
    fn SetTextAlign(dc: usize, align: u32) -> u32;
    fn TextOutW(dc: usize, x: i32, y: i32, text: *const u16, n: i32) -> i32;
}

/// One glyph of Windows' icon font, centred on cx with its top at `top`, drawn through the back
/// buffer's GDI DC (IDirectDrawSurface::GetDC 17 / ReleaseDC 26).
unsafe fn draw_icon(code: u32, cx: i32, top: i32, size: i32, colour: [u8; 3]) {
    let surface = *(BACK_BUFFER as *const *mut usize);
    if surface.is_null() {
        return;
    }
    let vtable = *surface as *const usize;
    let get_dc: extern "system" fn(*mut usize, *mut usize) -> i32 = std::mem::transmute(*vtable.add(17));
    let release_dc: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(26));
    let mut dc = 0usize;
    if get_dc(surface, &mut dc) != 0 || dc == 0 {
        return;
    }
    // (Windows 11's Segoe Fluent Icons, else Windows 10's Segoe MDL2 Assets: the same codes)
    let face = if code > 0xffff { "Segoe UI Symbol" } else if std::path::Path::new("C:\\Windows\\Fonts\\SegoeIcons.ttf").exists() { "Segoe Fluent Icons" } else { "Segoe MDL2 Assets" };
    let face: Vec<u16> = face.encode_utf16().chain([0]).collect();
    let gdi_font = CreateFontW(-size, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 4, 0, face.as_ptr()); // DEFAULT_CHARSET, ANTIALIASED_QUALITY
    let old = SelectObject(dc, gdi_font);
    SetBkMode(dc, 1); // TRANSPARENT
    SetTextColor(dc, colour[0] as u32 | (colour[1] as u32) << 8 | (colour[2] as u32) << 16);
    SetTextAlign(dc, 6); // TA_CENTER | TA_TOP
    let mut glyph = [0u16; 2];
    let glyph = char::from_u32(code).unwrap_or(' ').encode_utf16(&mut glyph);
    TextOutW(dc, cx, top, glyph.as_ptr(), glyph.len() as i32);
    SelectObject(dc, old);
    DeleteObject(gdi_font);
    release_dc(surface, dc);
}

/// A box with rounded corners.
unsafe fn pill(x1: i32, y1: i32, x2: i32, y2: i32, c: [u8; 3]) {
    let (w, h, r, colour) = (x2 - x1, y2 - y1, 4.0f32, rgb565(c));
    blend(x1, y1, w, h, |col, row| {
        let dx = (r - col as f32 - 0.5).max(col as f32 + 0.5 - (w as f32 - r)).max(0.0);
        let dy = (r - row as f32 - 0.5).max(row as f32 + 0.5 - (h as f32 - r)).max(0.0);
        (colour, ((r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * 255.0) as u32)
    });
}

const LIGHT: [u8; 3] = [0xd8, 0xdc, 0xe2];
const GREY: [u8; 3] = [0x9a, 0xa0, 0xa8];
const WHITE: [u8; 3] = [0xf0, 0xf2, 0xf4];

/// A ring segment from 12 o'clock clockwise for `part` of the circle, on a dim track.
unsafe fn ring(cx: i32, cy: i32, r: i32, thick: i32, part: f32, c: [u8; 3]) {
    let (on, off) = (rgb565(c), rgb565([0x34, 0x38, 0x40]));
    blend(cx - r, cy - r, 2 * r + 1, 2 * r + 1, |col, row| {
        let (dx, dy) = ((col - r) as f32, (row - r) as f32);
        let d = (dx * dx + dy * dy).sqrt();
        let edge = (r as f32 + 0.5 - d).min(d - (r - thick) as f32 + 0.5).clamp(0.0, 1.0);
        let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU);
        (if angle <= part * std::f32::consts::TAU { on } else { off }, (edge * 255.0) as u32)
    });
}

/// "5,799"
fn thousands(v: i32) -> String {
    let s = v.to_string();
    s.as_bytes().rchunks(3).rev().map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(",")
}

/// mike_appguy's reputation panel: world, current and home reputation (player +13, +11, +9) as
/// lines and as three rings.
unsafe fn reputation(player: usize, x: i32, y: i32, w: i32, h: i32, font: u32, row: i32) {
    let rep = |at: usize| rd::<u16>(player + at) as i32;
    put(x, y, x + w, y + row, 0x9, font, GREY, "  REPUTATION");
    let colours = [[0xe0, 0xb8, 0x50], [0x3e, 0xc0, 0x6a], [0xd0, 0x5c, 0x5c]]; // world gold, current green, home red
    // labels, values and the rings side by side, measured (at 800x600 the rings give way)
    let (lw, vw) = (text_width(font, b"Current"), text_width(font, b"10,000"));
    let roomy = w - 24 - lw - vw - 20 >= (h - row).min(w * 40 / 100); // (1680x1050: as before)
    let size = if roomy { (h - row).min(w * 40 / 100) } else { (w - 24 - lw - vw - 14).max(row) };
    let (cx, cy) = (x + w - size / 2 - 6, y + row + (h - row) / 2);
    for (i, (label, at)) in [("World", 13), ("Current", 11), ("Home", 9)].into_iter().enumerate() {
        let ry = y + (i as i32 + 1) * row + row / 2;
        dot(x + 16, ry + row / 2, 4, colours[i]);
        put(x + 24, ry, x + 24 + lw + 4, ry + row, 0x9, font, LIGHT, label);
        let vx2 = if roomy { x + w - size - 16 } else { x + 24 + lw + 10 + vw };
        put(x + 24 + lw + 8, ry, vx2, ry + row, 0xa, font, WHITE, &thousands(rep(at)));
        let r = size / 2 - i as i32 * (size / 8 + 2);
        ring(cx, cy, r, size / 8, rep(at) as f32 / 10000.0, colours[i]);
    }
}

/// Form ratings coloured as mike_appguy does: 6 orange, 7 gold, 8 olive, 9 green.
fn form_colour(rating: i32) -> [u8; 3] {
    level_colour(rating * 3 - 11)
}

/// mike_appguy's player status panel: form tiles, a morale bar and a condition bar.
unsafe fn player_status(status: &[Vec<u8>; 3], x: i32, y: i32, w: i32, font: u32, row: i32) {
    put(x, y, x + w, y + row, 0x9, font, GREY, "  PLAYER STATUS");
    let label_w = (w * 32 / 100).max(text_width(font, b"  Condition") + 8);
    let (bx1, bx2) = (x + label_w, x + w);
    for (i, label) in STATUS_LABELS.iter().enumerate() {
        let ry = y + (i as i32 + 1) * row + row / 2;
        put(x, ry, bx1, ry + row, 0x9, font, LIGHT, &format!("  {label}"));
        let value = latin1(&status[i]).into_owned();
        match cell(&status[i]) {
            Cell::Form(games) => {
                let (gap, n) = (4, games.len() as i32);
                let tw = (bx2 - bx1 + gap) / n;
                for (k, &g) in games.iter().enumerate() {
                    let tx = bx1 + k as i32 * tw;
                    fill(tx, ry + 2, tx + tw - gap, ry + row - 2, form_colour(g));
                    put(tx, ry, tx + tw - gap, ry + row, 0xc, font, WHITE, &g.to_string());
                }
            }
            Cell::Bar(level) => {
                bar(bx1, ry, bx2, ry + row, level);
                put(bx1, ry, bx2, ry + row, 0xc, font, WHITE, &value);
            }
            Cell::Plain => put(bx1, ry, bx2, ry + row, 0xc, font, WHITE, &value),
        }
    }
}

// Settings in cmui.ini in the game folder, one `key=value` a line.
const INI: &str = "cmui.ini";

fn setting(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(INI).ok()?;
    text.lines().find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string()))
}

fn set_setting(key: &str, value: &str) {
    let text = std::fs::read_to_string(INI).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().filter(|l| l.split_once('=').map_or(true, |(k, _)| k.trim() != key)).map(String::from).collect();
    lines.push(format!("{key}={value}"));
    let _ = std::fs::write(INI, lines.join("\r\n") + "\r\n");
}

/// Every feature has its own switch in cmui.ini (`name=0` turns it off; missing names are
/// written as on), so each can become a separate misc patch.
const FEATURES: [&str; 31] = [
    "theme", "tabs", "attribute_bars", "crests", "profile", "stats_table", "news", "club_header",
    "squad_list", "tactics_pitch", "match_screen", "compare_screen", "menu_fixes", "status_bars",
    "search_ability", "scout_stars", "assistant_pick", "team_report", "honours", "export_button",
    "injury_history", "transfer_history", "compare_report", "scout_screen", "bigger_text",
    "squad_table", "table_icons", "background_show", "modern_buttons", "challenge", "career_goals",
];
const SEARCH_ABILITY: usize = 14;
const SCOUT_STARS: usize = 15;
const ASSISTANT_PICK: usize = 16;
const TEAM_REPORT: usize = 17;
const HONOURS: usize = 18;
const EXPORT_BUTTON: usize = 19;
const INJURY_HISTORY: usize = 20;
const TRANSFER_HISTORY: usize = 21;
const COMPARE_REPORT: usize = 22;
const SCOUT_SCREEN: usize = 23;
const BIGGER_TEXT: usize = 24;
const SQUAD_TABLE: usize = 25;
const TABLE_ICONS: usize = 26;
const BACKGROUND_SHOW: usize = 27;
const MODERN_BUTTONS: usize = 28;
const CHALLENGE: usize = 29;
const CAREER_GOALS: usize = 30; // career goals on the CM Challenge page (`challenge\goals.rs`; needs `challenge`)
const THEME_ON: usize = 0;
const TABS: usize = 1;
const ATTRIBUTE_BARS: usize = 2;
const CRESTS_ON: usize = 3;
const PROFILE: usize = 4;
const STATS_TABLE: usize = 5;
const NEWS: usize = 6;
const CLUB_HEADER: usize = 7;
const SQUAD_LIST: usize = 8;
const TACTICS_PITCH: usize = 9;
const MATCH_SCREEN: usize = 10;
const COMPARE_SCREEN: usize = 11;
const MENU_FIXES: usize = 12;
const STATUS_BARS: usize = 13;
static mut ENABLED: u32 = u32::MAX;

fn on(feature: usize) -> bool {
    unsafe { ENABLED & 1 << feature != 0 }
}

/// The exe's PE header (cm0102.exe is always loaded at 0x400000).
unsafe fn nt_header() -> usize {
    0x400000 + *(0x40003c as *const u32) as usize
}

/// The feature switches a cmui exe carries: one byte each (0 = off) at +0x20 of the loader stub
/// the exe starts at, after "cmui.dll" and the mark "CMUI" (make_exe.py turns all on; the
/// CM0102Patcher misc patches of make_patches.py one each). None for an exe without them.
unsafe fn exe_switches() -> Option<&'static [u8]> {
    let stub = 0x400000 + *((nt_header() + 0x28) as *const u32) as usize;
    // the exe's own sections are always mapped (writable ones copy-on-write, which readable() leaves out)
    (std::slice::from_raw_parts((stub + 16) as *const u8, 16) == b"cmui.dll\0\0\0\0CMUI")
        .then(|| std::slice::from_raw_parts((stub + 0x20) as *const u8, FEATURES.len()))
}

/// The theme's flat menu bars: for each bar picture the exe names (the original game.mbr /
/// match.mbr, read as 555; a resolution patch's g800.mbr / m800.mbr, blitted as they are, 565),
/// Data\<cmui name> is written with the original's header (its size) in sidebar grey with a
/// lighter one-pixel edge on the right, and the exe's name (same length) swapped for it.
unsafe fn menu_bars() {
    const BARS: [(&[u8], &str, bool); 4] = [(b"game.mbr", "gcui.mbr", false), (b"match.mbr", "mcui_.mbr", false), (b"g800.mbr", "gcui.mbr", true), (b"m800.mbr", "mcui.mbr", true)];
    let nt = nt_header();
    let (count, optional) = (*((nt + 6) as *const u16) as usize, *((nt + 0x14) as *const u16) as usize);
    for (old, new, is565) in BARS {
        let mut hits = Vec::new();
        for s in 0..count {
            let section = nt + 0x18 + optional + s * 40;
            let (size, start) = (*((section + 8) as *const u32) as usize, 0x400000 + *((section + 12) as *const u32) as usize);
            let image = std::slice::from_raw_parts(start as *const u8, size);
            hits.extend(image.windows(old.len()).enumerate().filter(|(_, w)| *w == old).map(|(i, _)| start + i));
        }
        let Ok(head) = std::fs::read(format!("Data\\{}", std::str::from_utf8(old).unwrap())) else { continue };
        if hits.len() != 1 || head.len() < 48 {
            continue;
        }
        let (w, h) = (u32::from_le_bytes(head[0..4].try_into().unwrap()) as usize, u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize);
        let px = |(r, g, b): (u16, u16, u16)| if is565 { (r >> 3) << 11 | (g >> 2) << 5 | b >> 3 } else { (r >> 3) << 10 | (g >> 3) << 5 | b >> 3 };
        let (bar, edge) = (px((0x22, 0x26, 0x2c)), px((0x33, 0x38, 0x40)));
        let mut picture = head[..48].to_vec();
        for _ in 0..h {
            for x in 0..w {
                picture.extend((if x + 1 == w { edge } else { bar }).to_le_bytes());
            }
        }
        if std::fs::write(format!("Data\\{new}"), picture).is_ok() {
            let mut protect = 0;
            VirtualProtect(hits[0] as *mut u8, old.len(), 0x04, &mut protect);
            std::ptr::copy_nonoverlapping(new.as_ptr(), hits[0] as *mut u8, old.len());
            VirtualProtect(hits[0] as *mut u8, old.len(), protect, &mut protect);
            log(&format!("menu bar Data\\{new} ({w}x{h}) for {}", std::str::from_utf8(old).unwrap()));
        }
    }
}

fn load_features() {
    if let Some(switches) = unsafe { exe_switches() } {
        for (k, &s) in switches.iter().enumerate() {
            if s == 0 {
                unsafe { ENABLED &= !(1 << k) };
            }
        }
    }
    for (k, name) in FEATURES.iter().enumerate() {
        match setting(name).as_deref() {
            Some("0") => unsafe { ENABLED &= !(1 << k) },
            Some(_) => {}
            None => set_setting(name, "1"),
        }
    }
    log(&format!("features off: {:?}", FEATURES.iter().enumerate().filter(|(k, _)| !on(*k)).map(|f| f.1).collect::<Vec<_>>()));
}

/// The More/Less button of the ability block (where the profile's panels are an overlay) and
/// whether they are shown (kept in cmui.ini as profile_details).
static mut DETAILS_BUTTON: Option<(i32, i32, i32, i32)> = None;
static mut SHOW_DETAILS: bool = false;

/// The Hide/Show button of the ability block while it is on screen, and whether CA/PA are hidden.
static mut HIDE_BUTTON: Option<(i32, i32, i32, i32)> = None;
static mut HIDE_CA_PA: bool = false;

const POLL_INPUT: usize = 0x55f620; // Display::poll_input(this) -> clicked widget or -1, thiscall
const MOUSE_X: usize = 0xb67a20;
const MOUSE_Y: usize = 0xb67a14;
const LEFT_CLICK: usize = 0xb67a0c; // set by a left click, cleared by poll_input
static mut POLL_INPUT_ORIG: usize = 0;
const GAME_WINDOW: usize = 0xb67a2c; // HWND
const BUTTON_HELD: usize = 0xb67a34; // left button down, kept by the window procedure 0x68d630
static mut WAS_HELD: bool = false;

/// The left button is down (for trace lines on a click only).
#[allow(non_snake_case)]
pub(crate) fn BUTTON_DOWN_NOW() -> bool {
    unsafe { *(BUTTON_HELD as *const u32) != 0 }
}

/// Display::poll_input: a left click on the Hide/Show button is taken before the game sees it,
/// switches CA/PA, keeps the choice in cmui.ini and redraws the screen.
extern "thiscall" fn poll_input(display: usize) -> i16 {
    unsafe {
        // (a quiet screen draws no frames: the trace switch is looked at here too, at most every
        // 200 ms, so a click's handlers are traced)
        static mut LOOKED: Option<std::time::Instant> = None;
        if LOOKED.is_none_or(|t| t.elapsed().as_millis() > 200) {
            LOOKED = Some(std::time::Instant::now());
            TRACE_ON = std::path::Path::new("cmui.trace").exists();
        }
        // cmui's own buttons act when the left button is let go over them (the game's click flag
        // only stands for a widget of its own)
        let held = *(BUTTON_HELD as *const u32) != 0;
        let released = std::mem::replace(&mut WAS_HELD, held) && !held;
        let (mx, my) = (*(MOUSE_X as *const i32), *(MOUSE_Y as *const i32));
        let over = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
        // the CM Challenge button and page
        if on(CHALLENGE) {
            if let Some(redraw) = challenge::input(mx, my, released) {
                if redraw {
                    game::<extern "thiscall" fn(usize, i16, i16, i16, i16) -> i32>(DRAW_SCREEN)(display, -1, -1, -1, -1);
                }
                script();
                return -1;
            }
        }
        // the squad screen's Assistant button and its menu, the scout report's chip (the game does
        // not see what they take)
        if on(ASSISTANT_PICK) || on(SCOUT_SCREEN) {
            if let Some(redraw) = assistant::input(mx, my, released) {
                // a network client: the choice to the host, then a click its club screen input takes
                if let Some(request) = assistant::client_request() {
                    remote::send_now(&request);
                    let net = *((SCREEN_MANAGER + 0x3840) as *const usize);
                    game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SEND_CLICK)(display, net, 0);
                }
                if redraw {
                    game::<extern "thiscall" fn(usize, i16, i16, i16, i16) -> i32>(DRAW_SCREEN)(display, -1, -1, -1, -1);
                }
                script(); // (the test script goes on: its pointer may rest on the button)
                return -1;
            }
        }
        if released && DETAILS_BUTTON.is_some_and(over) {
            SHOW_DETAILS = !SHOW_DETAILS;
            set_setting("profile_details", if SHOW_DETAILS { "1" } else { "0" });
            game::<extern "thiscall" fn(usize, i16, i16, i16, i16) -> i32>(DRAW_SCREEN)(display, -1, -1, -1, -1);
            return -1;
        }
        if released && HIDE_BUTTON.is_some_and(over) {
            HIDE_CA_PA = !HIDE_CA_PA;
            set_setting("hide_ca_pa", if HIDE_CA_PA { "1" } else { "0" });
            game::<extern "thiscall" fn(usize, i16, i16, i16, i16) -> i32>(DRAW_SCREEN)(display, -1, -1, -1, -1);
            return -1;
        }
        if let (true, Some(r)) = (released, news::MARK_BUTTON) {
            if over(r) {
                if remote::CLIENT {
                    // the host marks them: the request first, then a click its news input takes
                    remote::send_now(&key(b'Q', &[]));
                    let net = *((SCREEN_MANAGER + 0x3840) as *const usize);
                    game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SEND_CLICK)(display, net, 0);
                } else {
                    news::PENDING_MARK = Some(0); // done by the news screen's input (news_input)
                }
                return -1;
            }
        }
        // a network client's story link or chip from the host's block: the host opens its cell,
        // by the click that follows (news_input)
        news::menu_check();
        if let (true, true, true, Some(click)) = (released, remote::CLIENT, !news::MENU_OPEN, news::clicked(mx, my)) {
            remote::send_now(&match click {
                news::Click::Cell(cell) => key(b'X', &[cell as u16 as u32]),
                news::Click::Club(club) => key(b'K', &[club as u32]),
            });
            let net = *((SCREEN_MANAGER + 0x3840) as *const usize);
            game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SEND_CLICK)(display, net, 0);
            return -1;
        }
        #[allow(static_mut_refs)]
        if let (true, Some((rect, club))) = (released, NEWS_LINK) {
            if over(rect) {
                if remote::CLIENT {
                    // the host opens it: the club first, then a click its news input takes
                    remote::send_now(&key(b'K', &[club as u32]));
                    let net = *((SCREEN_MANAGER + 0x3840) as *const usize);
                    game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SEND_CLICK)(display, net, 0);
                    return -1;
                }
                PENDING_CLUB = (0, club); // opened by the news screen's input (news_input)
                return -1;
            }
        }
        #[allow(static_mut_refs)]
        if let (true, Some((rect, (bx, by)))) = (released, LEAGUE_LINK) {
            if over(rect) {
                SCRIPT.push_back(format!("{bx} {by}")); // the league position button does the rest
            }
        }
        // (a network client: the host's answers came, drawn with them)
        REDRAW |= remote::CLIENT && (std::mem::take(&mut remote::ARRIVED) | remote::hold_over() | remote::silence_changed());
        // (a click while a dropdown counts as open: one full frame, which draws an open menu
        // again and none that closed, so news::MENU_OPEN is right again; without it the flag
        // stayed on after a menu and the story's links went without their remaps)
        // (on the poll after the click: the game handles the click first)
        // Safety net (2026-10-29): one full frame after every click. Until the typing fix, a
        // redraw loop gave a full frame every frame, and state cmui learns in full frames (menu
        // open, screen / tab titles, what a screen noted down) depended on it; clicks are what
        // change those. Typing is keys, so it isn't disturbed. FIXES.md "Typing / redraws".
        static mut CLICK_CHECK: bool = false;
        // (not while a dropdown shows: the full frame drew the open menu and then the news story
        // over its lower rows, "Nort", "Ocea" cut off, 2026-10-30)
        news::menu_check();
        REDRAW |= std::mem::replace(&mut CLICK_CHECK, released) && !news::MENU_OPEN;
        if std::mem::take(&mut REDRAW) {
            if trace() {
                log("cmui redraw (poll)");
            }
            game::<extern "thiscall" fn(usize, i16, i16, i16, i16) -> i32>(DRAW_SCREEN)(display, -1, -1, -1, -1);
        }
        LAST_POLL = Some(std::time::Instant::now());
        script();
        // the story is laid out by cmui (news::story): over one of its links or chips the game
        // sees the pointer on its own box for it, elsewhere over the story on none (and back
        // after, unless the window procedure moved it meanwhile)
        let target = news::pointer(mx, my).or_else(|| nation_link(display, mx, my));
        if let Some((tx, ty)) = target {
            (*(MOUSE_X as *mut i32), *(MOUSE_Y as *mut i32)) = (tx, ty);
        }
        let result = game::<extern "thiscall" fn(usize) -> i16>(POLL_INPUT_ORIG)(display);
        if target.is_some_and(|t| t == (*(MOUSE_X as *const i32), *(MOUSE_Y as *const i32))) {
            (*(MOUSE_X as *mut i32), *(MOUSE_Y as *mut i32)) = (mx, my);
        }
        result
    }
}

/// The profile's info strip hides the line "Born ... Greek (..)" and the game's link on the nation
/// in it (a club link, 0x7d5, its item in the game's box for the line). Over the strip's flag or
/// Nationality cell the game sees the pointer on that item, so its own click opens the national
/// team (a network client sends it to the host like any widget). The first such item of the line
/// is the first nationality; none (no nation, no item sent) leaves the click to nothing.
#[allow(static_mut_refs)]
unsafe fn nation_link(display: usize, mx: i32, my: i32) -> Option<(i32, i32)> {
    if news::MENU_OPEN || !NATION_HIT.iter().any(|r| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my)) {
        return None;
    }
    // (the display is this process's own, on a client too: not `rd`, which asks the host)
    let count = if mapped(display + 0x12e9a0, 2) { ((display + 0x12e9a0) as *const i16).read_unaligned().clamp(0, 1200) as usize } else { 0 };
    if !mapped(display + 0xba95e, count * 0x18c) {
        return None;
    }
    let b = BORN_BOX;
    (0..count)
        .map(|k| display + 0xba95e + k * 0x18c)
        .filter(|&w| ((w + 8) as *const i16).read_unaligned() == 0x7d5)
        .map(|w| -> [i32; 4] { std::array::from_fn(|i| ((w + 0x10 + 4 * i) as *const i32).read_unaligned()) })
        .filter(|r| r[2] > r[0] && r[3] > r[1] && r[0] >= b.0 - 4 && r[2] <= b.2 + 4 && r[1] >= b.1 - 4 && r[3] <= b.3 + 4)
        .min_by_key(|r| (r[1], r[0]))
        .map(|r| ((r[0] + r[2]) / 2, (r[1] + r[3]) / 2))
}

/// Test script from `cmui.click` (the shell cannot send the game input): one command a line,
/// run in order with `pause` ms after each: `x y` clicks there, `type text`, `key vk`, `wait ms`,
/// `pause ms` (default 1500), `shot file.bmp` saves the back buffer. Each command is logged.
static mut SCRIPT: std::collections::VecDeque<String> = std::collections::VecDeque::new();
static mut SCRIPT_NEXT: Option<std::time::Instant> = None;
static mut SCRIPT_PAUSE: u64 = 1500;
/// When poll_input last ran (the flip pass runs the script while it does not, in a live match).
static mut LAST_POLL: Option<std::time::Instant> = None;
/// The texts drawn since a test script came (newest last: box, lower-case trimmed text), so
/// `click <label>` / `hover <label>` find a button or row at any resolution.
static mut SCRIPTED: bool = false;
static mut LABELS: std::collections::VecDeque<(i32, i32, i32, i32, String)> = std::collections::VecDeque::new();

#[allow(static_mut_refs)]
unsafe fn note_label(x1: i32, y1: i32, x2: i32, y2: i32, s: &[u8]) {
    let t = latin1(s).trim().to_lowercase();
    if t.is_empty() {
        return;
    }
    if LABELS.len() >= 4000 {
        LABELS.pop_front();
    }
    LABELS.push_back((x1, y1, x2, y2, t));
}

/// A scripted mouse message at game pixel (x, y): a window shown larger than the game (cnc-ddraw
/// scaling, aspect kept and centred) takes window pixels, so the point is scaled into it. The
/// window's real size from GetWindowInfo: cnc-ddraw's GetClientRect answers the game's size.
unsafe fn post_mouse(msg: u32, wparam: usize, x: i32, y: i32) {
    let window = *(GAME_WINDOW as *const usize);
    let (gw, gh) = screen();
    let mut info = [0i32; 15]; // WINDOWINFO: size, window rect, client rect (screen), ...
    info[0] = 60;
    let real = GetWindowInfo(window, &mut info) != 0;
    let r = [0, 0, info[7] - info[5], info[8] - info[6]];
    let (mut x, mut y) = (x, y);
    if real && gw > 0 && gh > 0 && r[2] > 0 && r[3] > 0 && (r[2], r[3]) != (gw, gh) {
        let s = (r[2] as f32 / gw as f32).min(r[3] as f32 / gh as f32);
        x = ((r[2] as f32 - gw as f32 * s) / 2.0 + (x as f32 + 0.5) * s) as i32;
        y = ((r[3] as f32 - gh as f32 * s) / 2.0 + (y as f32 + 0.5) * s) as i32;
    }
    if trace() {
        log(&format!("mouse {msg:#x} at {x},{y} (client {}x{}, real {real}, game {gw}x{gh})", r[2], r[3]));
    }
    PostMessageA(window, msg, wparam, (x & 0xffff | y << 16) as isize);
}

/// The centre of the newest text that is `label` (else the newest that contains it).
#[allow(static_mut_refs)]
unsafe fn find_label(label: &str) -> Option<(i32, i32)> {
    let hit = find_box(label)?;
    Some(((hit.0 + hit.2) / 2, (hit.1 + hit.3) / 2))
}

#[allow(static_mut_refs)]
unsafe fn find_box(label: &str) -> Option<&'static (i32, i32, i32, i32, String)> {
    let l = label.trim().to_lowercase().replace("\\n", "\n"); // two-line buttons: "add\nmanager"
    LABELS.iter().rev().find(|b| b.4 == l).or_else(|| LABELS.iter().rev().find(|b| b.4.contains(&l)))
}

/// "click Name|slot": a squad's selection cell on that name's row, also an empty one: the nearest
/// column left of the name of three or more cells ("FC", "SB4") with one x range, at any layout.
#[allow(static_mut_refs)]
unsafe fn slot_cell(label: &str) -> Option<(i32, i32)> {
    let b = find_box(label)?;
    let (y, rows) = ((b.1 + b.3) / 2, 20 * (b.3 - b.1).max(8));
    let slot = |t: &str| matches!(t, "gk" | "sw" | "dl" | "dr" | "dc" | "wbl" | "wbr" | "dml" | "dmr" | "dmc" | "ml" | "mr" | "mc" | "aml" | "amr" | "amc" | "fl" | "fr" | "fc")
        || (t.len() == 3 && t.starts_with("sb") && t.as_bytes()[2].is_ascii_digit());
    let cells: Vec<_> = LABELS.iter().filter(|c| slot(&c.4) && c.2 < b.0 && ((c.1 + c.3) / 2 - y).abs() < rows).map(|c| (c.0, c.2)).collect();
    let column = cells.iter().filter(|&&c| cells.iter().filter(|&&d| d == c).count() >= 3).max_by_key(|c| c.1)?;
    Some(((column.0 + column.1) / 2, y))
}

#[allow(static_mut_refs)]
unsafe fn script() {
    if let Ok(text) = std::fs::read_to_string("cmui.click") {
        let _ = std::fs::remove_file("cmui.click");
        // the screen drawn before the first script: again, so its labels are known
        if !std::mem::replace(&mut SCRIPTED, true) {
            REDRAW = true;
            SCRIPT_NEXT = Some(std::time::Instant::now() + std::time::Duration::from_millis(800));
        }
        SCRIPT.extend(text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
    }
    let now = std::time::Instant::now();
    if SCRIPT_NEXT.is_some_and(|t| now < t) {
        return;
    }
    let Some(line) = SCRIPT.pop_front() else { return };
    let mut words = line.split_whitespace();
    let (a, b) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    let mut pause = SCRIPT_PAUSE;
    match (a, a.parse::<i32>(), b.parse::<i32>()) {
        (_, Ok(x), Ok(y)) => {
            // through the window procedure (0x68d630), which keeps the button held between a
            // down and an up: the up comes on a later poll
            post_mouse(0x200, 0, x, y); // WM_MOUSEMOVE
            post_mouse(0x201, 1, x, y); // WM_LBUTTONDOWN
            SCRIPT.push_front(format!("up {x} {y}"));
            pause = 200;
        }
        ("move", _, Ok(x)) => {
            let y = words.next().and_then(|w| w.parse::<i32>().ok()).unwrap_or(0);
            post_mouse(0x200, 0, x, y); // WM_MOUSEMOVE
        }
        ("down", _, Ok(x)) => {
            // a press without its release: with `move` and `up` a drag
            let y = words.next().and_then(|w| w.parse::<i32>().ok()).unwrap_or(0);
            post_mouse(0x200, 0, x, y); // WM_MOUSEMOVE
            post_mouse(0x201, 1, x, y); // WM_LBUTTONDOWN
        }
        ("up", _, Ok(x)) => {
            let y = words.next().and_then(|w| w.parse::<i32>().ok()).unwrap_or(0);
            post_mouse(0x202, 0, x, y); // WM_LBUTTONUP
        }
        ("type", ..) if line.len() > 5 => {
            // the game keeps one typed character a poll
            PostMessageA(*(GAME_WINDOW as *const usize), 0x102, line.as_bytes()[5] as usize, 1); // WM_CHAR
            if line.len() > 6 {
                SCRIPT.push_front(format!("type {}", &line[6..]));
            }
            pause = 150;
        }
        ("key", _, Ok(vk)) => _ = PostMessageA(*(GAME_WINDOW as *const usize), 0x100, vk as usize, 1), // WM_KEYDOWN
        ("peek", ..) => {
            let at = usize::from_str_radix(b.trim_start_matches("0x"), 16).unwrap_or(0);
            let v = if readable(at, 4) { format!("{:#x}", (at as *const u32).read_unaligned()) } else { "unreadable".into() };
            log(&format!("peek {at:#x} = {v}"));
        }
        // ("click Name|-60": 60 pixels left of the label's centre, a cell next to it;
        //  "click Name|-11.3%": a share of the screen width, the same cell at any resolution)
        ("click" | "hover", ..) => match line.get(6..).unwrap_or("").rsplit_once('|').map_or((line.get(6..).unwrap_or(""), ""), |(l, d)| (l, d.trim())) {
            (label, "slot") if a == "click" => match slot_cell(label) {
                Some((x, y)) => SCRIPT.push_front(format!("{x} {y}")),
                None => log(&format!("script: no selection cell for {label:?} on screen")),
            },
            (label, d) => match find_label(label).map(|(x, y)| (x + d.strip_suffix('%').map_or(d.parse::<i32>().unwrap_or(0), |p| (p.parse::<f32>().unwrap_or(0.0) * screen().0 as f32 / 100.0) as i32), y)) {
            Some((x, y)) if a == "click" => SCRIPT.push_front(format!("{x} {y}")),
            Some((x, y)) => post_mouse(0x200, 0, x, y), // WM_MOUSEMOVE
            None => log(&format!("script: no text {:?} on screen", line.get(6..).unwrap_or(""))),
            },
        },
        ("wait", _, Ok(ms)) => pause = ms as u64,
        ("pause", _, Ok(ms)) => SCRIPT_PAUSE = ms as u64,
        ("shot", ..) => save_back_buffer(b),
        _ => log(&format!("script: {line}? (x y, type text, key vk, wait ms, pause ms, shot file.bmp)")),
    }
    log(&format!("script: {line}"));
    SCRIPT_NEXT = Some(now + std::time::Duration::from_millis(pause));
}

/// Writes the back buffer as a 24-bit BMP (works with the window covered); 16-bit screens.
unsafe fn save_back_buffer(path: &str) {
    let surface = *(BACK_BUFFER as *const *mut usize);
    let Some(f565) = screen_format() else { return log("shot: not a 16-bit back buffer") };
    if surface.is_null() {
        return log("shot: no back buffer");
    }
    let vtable = *surface as *const usize;
    let lock: extern "system" fn(*mut usize, usize, *mut u8, u32, usize) -> i32 = std::mem::transmute(*vtable.add(25));
    let unlock: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(32));
    let mut desc = [0u8; 108];
    desc[0] = 108;
    if lock(surface, 0, desc.as_mut_ptr(), 1, 0) != 0 {
        return log("shot: lock failed");
    }
    let field = |at: usize| i32::from_le_bytes(desc[at..at + 4].try_into().unwrap());
    let (h, w, pitch, bits) = (field(8), field(0xc), field(0x10), field(0x24) as usize);
    let row = (w as usize * 3 + 3) & !3;
    let size = (54 + row * h as usize) as u32;
    let mut out = b"BM".to_vec();
    for v in [size, 0, 54, 40, w as u32, h as u32, 1 | 24 << 16, 0, 0, 0, 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for y in (0..h).rev() {
        let start = out.len();
        for x in 0..w {
            let p = *((bits + (y * pitch + x * 2) as usize) as *const u16) as u32;
            let [r, g, b] = unpack(p, f565);
            out.extend_from_slice(&[b as u8, g as u8, r as u8]);
        }
        out.resize(start + row, 0);
    }
    unlock(surface, 0);
    match std::fs::write(path, out) {
        Ok(()) => log(&format!("shot: {path} {w}x{h}")),
        Err(e) => log(&format!("shot: {path}: {e}")),
    }
}

/// mike_appguy's ability block in the third column: header with a Hide button, then CA in green
/// and PA in gold stars.
unsafe fn ability(x: i32, y: i32, w: i32, h: i32, font: u32, ca: i32, pa: i32, details: bool) {
    put(x, y, x + w * 48 / 100, y + h, 0x9, font, GREY, "  ABILITY");
    let (hx1, hx2) = (x + w * 75 / 100, x + w - 6);
    fill(hx1, y + 2, hx2, y + h - 2, [0x3a, 0x3f, 0x47]);
    put(hx1, y, hx2, y + h, 0xc, font, LIGHT, if HIDE_CA_PA { "Show" } else { "Hide" });
    HIDE_BUTTON = Some((hx1, y + 2, hx2, y + h - 2));
    DETAILS_BUTTON = None;
    if details {
        // the panels as an overlay over the stats table: More shows them, Less hides them
        let (dx1, dx2) = (x + w * 50 / 100, x + w * 73 / 100);
        fill(dx1, y + 2, dx2, y + h - 2, if SHOW_DETAILS { [0x4c, 0x42, 0x24] } else { [0x3a, 0x3f, 0x47] });
        put(dx1, y, dx2, y + h, 0xc, font, if SHOW_DETAILS { GOLD } else { LIGHT }, if SHOW_DETAILS { "Less" } else { "More" });
        DETAILS_BUTTON = Some((dx1, y + 2, dx2, y + h - 2));
    }
    if HIDE_CA_PA {
        return;
    }
    for (i, (label, value, colour)) in [("CA", ca, [0x3e, 0xc0, 0x6a]), ("PA", pa, [0xe0, 0xb8, 0x50])].into_iter().enumerate() {
        let ry = y + (i as i32 + 1) * (h + 2);
        put(x, ry, x + w, ry + h, 0x9, font, LIGHT, &format!("  {label}"));
        stars(x + w * 38 / 100, ry + 1, h - 2, value, colour);
    }
}

/// The attribute grid of this frame: label column x positions, lowest row bottom, label font,
/// row height.
static mut GRID_SEEN: Option<(Vec<i32>, i32, u32, i32)> = None;
/// Width of a value cell of the grid.
static mut VALUE_CELL: i32 = 145;

fn note_grid_label(x1: i32, y2: i32, font: u32, height: i32) {
    unsafe {
        #[allow(static_mut_refs)]
        let g = GRID_SEEN.get_or_insert_with(|| (Vec::new(), 0, font, height));
        if !g.0.contains(&x1) {
            g.0.push(x1);
        }
        (g.1, g.2, g.3) = (g.1.max(y2), font, height);
    }
}

/// The profiled person's staff record: screen parameter 0.
unsafe fn profiled_staff() -> Option<usize> {
    let staff = screen_param(0);
    readable(staff, 0x6e).then_some(staff)
}

/// The profiled person's player record: staff +0x61 (null for non-players).
unsafe fn profiled_player() -> Option<usize> {
    let player = rd::<usize>(profiled_staff()? + 0x61);
    readable(player, 27).then_some(player)
}

/// The font of the season stats table's heads, for small labels.
static mut SMALL_FONT: Option<u32> = None;
/// The "Born ..." line of this frame: staff, text, its box and font; the strip goes in its place.
static mut STRIP: Option<(usize, String, i32, i32, i32, i32, u32)> = None;
/// The game's own box of that line (its link to the nation is an item in it), and where the strip
/// shows the nation: the flag and the Nationality cell, a click on which goes to the game's link.
static mut BORN_BOX: (i32, i32, i32, i32) = (0, 0, 0, 0);
static mut NATION_HIT: Vec<(i32, i32, i32, i32)> = Vec::new();
/// Injuries & Bans: the Injury row (y, its label's right edge, injured) and the Condition row.
static mut INJURY_ROW: (i32, i32, bool) = (i32::MIN, 0, false);
static mut CONDITION_ROW: (i32, i32) = (i32::MIN, 0);
/// The panels of this screen (left, top, right, bottom), newest last: a text's band.
static mut PANELS: Vec<(i32, i32, i32, i32)> = Vec::new();
/// The stats table's "*" / "+" buttons: their row and right end; boxes right of them on that row
/// are the column heads, whatever page of columns is shown.
static mut STATS_BUTTONS: (i32, i32) = (i32::MIN, 0);
/// The right edge of the stats table's "*" button (its "+" follows it).
static mut STAR_END: i32 = -1000;
/// The rectangles the game darkened this frame (y1, y2).
static mut DARKENED: Vec<(i32, i32)> = Vec::new();
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A game date (day of year from 0, year, leap flag: whether 29 February counts) as day and month.
unsafe fn day_month(date: usize) -> (i32, usize, i32) {
    let (mut day, year, leap) = (rd::<i16>(date) as i32, rd::<i16>(date + 2) as i32, rd::<i32>(date + 4) != 0);
    for (m, len) in [31, 28 + leap as i32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31].into_iter().enumerate() {
        if day < len {
            return (day + 1, m, year);
        }
        day -= len;
    }
    (31, 11, year)
}

/// The main contract of a staff member: his state (0xae2d0c, 0x4f bytes each) +0 is its index in
/// the contract manager's (0xae1050) list of 0x50-byte contracts.
unsafe fn main_contract(staff: usize) -> Option<usize> {
    let id = rd::<i32>(staff);
    if id < 0 || id >= rd::<i32>(0xae2364) || !readable(0xae2d0c, 4) {
        return None;
    }
    let state = rd::<usize>(0xae2d0c) + id as usize * 0x4f;
    let manager = rd::<usize>(0xae1050);
    if !readable(state, 4) || !readable(manager, 4) {
        return None;
    }
    let k = rd::<i32>(state);
    if k < 0 {
        return None;
    }
    let contract = rd::<usize>(manager) + k as usize * 0x50;
    readable(contract, 0x50).then_some(contract)
}

/// His main contract's wage and his value as the game writes them, in the currency and wage
/// period of Game Settings (the host's words in a network client).
unsafe fn money_texts(staff: usize) -> (String, String) {
    if remote::CLIENT {
        let text = |kind: u8| ask_host(&key(kind, &[staff as u32])).map(|v| latin1(&v).into_owned()).unwrap_or_default();
        return (text(b'W'), text(b'V'));
    }
    let mut wage = [0u8; 256];
    if let Some(c) = main_contract(staff) {
        // STAFF_CONTRACT::format_wage_text(this, out, style 2 "p/w", cash flags 2 separators)
        game::<extern "thiscall" fn(usize, *mut u8, u32, u32)>(0x85e750)(c, wage.as_mut_ptr(), 2, 2);
    }
    let mut value = [0u8; 256];
    let mut cash = [0u32; 4];
    // Cash::Cash(this, amount), Cash::format_display_amount(this, out, size, flags 2 separators)
    game::<extern "thiscall" fn(*mut u32, i32) -> usize>(0x450a90)(cash.as_mut_ptr(), *((staff + 0x52) as *const i32));
    game::<extern "thiscall" fn(*mut u32, *mut u8, u32, u32)>(0x450aa0)(cash.as_mut_ptr(), value.as_mut_ptr(), 256, 2);
    (buffer_text(&wage), buffer_text(&value))
}

/// Text a game function wrote into a buffer.
fn buffer_text(buf: &[u8]) -> String {
    latin1(buf.split(|&c| c == 0).next().unwrap_or(&[])).trim().to_string()
}

/// The game's text is Latin-1 (cp1252: the euro sign is 0x80): bytes to chars one for one.
fn latin1(b: &[u8]) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Owned(b.iter().map(|&c| c as char).collect())
}

/// Back to the game's bytes (a char above 0xff, which game text never has, becomes '?').
fn to_latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| if (c as u32) < 0x100 { c as u8 } else { b'?' }).filter(|&b| b != 0).collect()
}

/// mike_appguy's strip in place of the line "Born 16.6.73 (Age 28). Greek (41 caps/12 goals).":
/// small grey heads over gold values. Age and nationality come from that line (the game's words);
/// birth date (staff +0x10, +0x18 the year when the date is unknown), caps (+0x22, +0x23) and
/// value (+0x52) from the staff record; wage and contract end from his main contract, the money
/// through the game's own formatting (currency and wage period of Game Settings).
unsafe fn info_strip(staff: usize, line: &str, x1: i32, y1: i32, x2: i32, y2: i32, font: u32) {
    let age = line.split("(Age ").nth(1).and_then(|r| r.split(')').next()).unwrap_or("-").to_string();
    let nationality = line.split("). ").nth(1).and_then(|r| r.split(" (").next()).unwrap_or("").trim_end_matches('.').to_string();
    let (d, m, y) = day_month(staff + 0x10);
    let born = if y == 1900 { rd::<u16>(staff + 0x18).to_string() } else { format!("{d:02}.{:02}.{y}", m + 1) };
    let (caps, goals) = (rd::<u8>(staff + 0x22), rd::<u8>(staff + 0x23));
    let expires = main_contract(staff).map_or("-".to_string(), |c| {
        let (_, m, y) = day_month(c + 0x2d);
        format!("{} {y}", MONTHS[m])
    });
    let (wage, value) = money_texts(staff);
    let fields = [
        ("Date of Birth", born, 12),
        ("Age", age, 6),
        ("Nationality", nationality, 14),
        ("Wage", wage, 20),
        ("Contract Expires", expires, 13),
        ("Value", value, 15),
        ("International", format!("{caps} caps / {goals} goals"), 20),
    ];
    let small = SMALL_FONT.unwrap_or(font);
    #[allow(static_mut_refs)]
    NATION_HIT.clear();
    // the flag of his nation (staff +0x1a, name at +4) first, from cmui_crests.bin ("flag:Greece")
    let nation = rd::<usize>(staff + 0x1a);
    let flag = (y2 - y1) * 8 / 10;
    let mut x1 = x1;
    if readable(nation, 55) {
        let key = [b"flag:".as_slice(), &game_text(nation + 4, 51)].concat();
        if draw_crest(&key, x1 + 8, y1 + (y2 - y1 - flag) / 2, flag) {
            #[allow(static_mut_refs)]
            NATION_HIT.push((x1 + 8, y1 + (y2 - y1 - flag) / 2, x1 + 8 + flag, y1 + (y2 - y1 - flag) / 2 + flag));
            x1 += flag + 12;
        }
    }
    let (w, h) = (x2 - x1, y2 - y1);
    // two lines (grey heads over gold values) where the line is tall enough for two fonts; else
    // one line of values in the row's font (800x600), the heads left out
    let two_lines = h >= 34;
    if !two_lines {
        // one font for all values, the largest that fits them side by side; columns as wide as
        // their texts, the room left shared out
        let texts: Vec<Vec<u8>> = fields.iter().enumerate().map(|(i, f)| to_latin1(&if i == 1 { format!("Age {}", f.1) } else { f.1.clone() })).collect();
        let gap = sx(24).max(10);
        let total = |f: u32| texts.iter().map(|t| text_width(f, t)).sum::<i32>() + gap * texts.len() as i32;
        #[allow(static_mut_refs)]
        let mut fonts: Vec<u32> = FONTS_SEEN.iter().copied().filter(|f| f & !0xffff == font & !0xffff).collect();
        fonts.sort_by_key(|&f| std::cmp::Reverse(total(f)));
        let f = fonts.iter().copied().find(|&f| total(f) <= w).or(fonts.last().copied()).unwrap_or(font);
        let extra = ((w - total(f)) / texts.len() as i32).max(0);
        let mut x = x1;
        for (i, t) in texts.iter().enumerate() {
            let cw = text_width(f, t) + gap + extra;
            if i == 2 {
                #[allow(static_mut_refs)]
                NATION_HIT.push((x, y1, x + cw, y2));
            }
            if i > 0 {
                fill(x, y1 + h / 6, x, y2 - h / 6, [0x34, 0x38, 0x40]);
            }
            text_at(x + cw / 2, y1, y2, f, GOLD, t, 1);
            x += cw;
        }
        return;
    }
    let mut x = x1;
    for (i, (head, text, part)) in fields.into_iter().enumerate() {
        let cw = w * part / 100;
        if i == 2 {
            #[allow(static_mut_refs)]
            NATION_HIT.push((x, y1, x + cw, y2));
        }
        if i > 0 {
            fill(x, y1 + h / 6, x, y2 - h / 6, [0x34, 0x38, 0x40]);
        }
        if two_lines {
            put(x + 4, y1 + 2, x + cw - 4, y1 + h * 45 / 100, 0xc, small, GREY, head);
            put(x + 4, y1 + h * 40 / 100, x + cw - 4, y2 - 2, 0xc, font, GOLD, &text);
        } else {
            let one = if i == 1 { format!("Age {text}") } else { text };
            put(x + 4, y1, x + cw - 2, y2, 0xc, font, GOLD, &one);
        }
        x += cw;
    }
}

/// CA and PA: player +5 and +7 (None for a person with no player record: non-player staff in a
/// search, e.g. with Interested on Yes).
unsafe fn ca_pa(player: usize) -> Option<(i32, i32)> {
    if !remote::CLIENT && !readable(player, 9) {
        return None;
    }
    let (ca, pa) = (rd::<i16>(player + 5), rd::<i16>(player + 7));
    // (built only for a real CA: a network client's not yet known record reads as anything)
    (1..=200).contains(&ca).then(|| (ca as i32, (pa as i32).clamp(ca as i32, 200)))
}

// Player record +15..+26: ratings 1..20 for GK, SW, D, DM, M, AM, F, WB, then the sides R, L, C.
const GK: usize = 15;
const SW: usize = 16;
const D: usize = 17;
const DM: usize = 18;
const M: usize = 19;
const AM: usize = 20;
const F: usize = 21;
const WB: usize = 22;
const RIGHT: usize = 23;
const LEFT: usize = 24;
const CENTRE: usize = 25;

/// The spots on mike_appguy's pitch (own goal left): position, side, x and y as parts of the
/// pitch, and the short name.
const SPOTS: &[(usize, usize, f32, f32, &str)] = &[
    (GK, CENTRE, 0.05, 0.5, "GK"), (SW, CENTRE, 0.13, 0.5, "SW"),
    (D, LEFT, 0.22, 0.2, "DL"), (D, CENTRE, 0.22, 0.5, "DC"), (D, RIGHT, 0.22, 0.8, "DR"),
    (WB, LEFT, 0.33, 0.08, "WBL"), (WB, RIGHT, 0.33, 0.92, "WBR"), (DM, CENTRE, 0.36, 0.5, "DMC"),
    (M, LEFT, 0.52, 0.2, "ML"), (M, CENTRE, 0.52, 0.5, "MC"), (M, RIGHT, 0.52, 0.8, "MR"),
    (AM, LEFT, 0.7, 0.2, "AML"), (AM, CENTRE, 0.7, 0.5, "AMC"), (AM, RIGHT, 0.7, 0.8, "AMR"),
    (F, LEFT, 0.86, 0.3, "FL"), (F, CENTRE, 0.88, 0.5, "FC"), (F, RIGHT, 0.86, 0.7, "FR"),
];

/// How a 1..20 fit is named and coloured.
fn fit_word(fit: i32) -> (&'static str, [u8; 3]) {
    match fit {
        18.. => ("Natural", [0x3e, 0xc0, 0x6a]),
        15..=17 => ("Accomplished", [0xa8, 0xc8, 0x50]),
        10..=14 => ("Competent", [0xe0, 0xb8, 0x50]),
        5..=9 => ("Unconvincing", [0xd6, 0x86, 0x46]),
        _ => ("Awkward", [0x6a, 0x70, 0x78]),
    }
}

/// A filled circle, smoothed at the edge.
unsafe fn dot(cx: i32, cy: i32, r: i32, c: [u8; 3]) {
    let colour = rgb565(c);
    blend(cx - r, cy - r, 2 * r + 1, 2 * r + 1, |col, row| {
        let d = (((col - r) * (col - r) + (row - r) * (row - r)) as f32).sqrt();
        (colour, ((r as f32 + 0.5 - d).clamp(0.0, 1.0) * 255.0) as u32)
    });
}

/// mike_appguy's positions panel: the lines Position, Rating, Role /20, Side /20 and a pitch
/// with a dot per spot coloured by how well the player fits it.
unsafe fn positions(player: usize, x: i32, y: i32, w: i32, h: i32, font: u32, row: i32) {
    let rating = |at: usize| rd::<i8>(player + at) as i32;
    // (a keeper has no side: GK by the role alone, as the compare screen does)
    let fits: Vec<i32> = SPOTS.iter().map(|s| if s.0 == GK { rating(GK) } else { rating(s.0).min(rating(s.1)) }).collect();
    let best = (0..SPOTS.len()).max_by_key(|&i| (fits[i], rating(SPOTS[i].0))).unwrap();
    let draw = game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG);
    let (light, grey) = (rgb([0xd8, 0xdc, 0xe2]) as u32, rgb([0x9a, 0xa0, 0xa8]) as u32);
    let text = |s: String| std::ffi::CString::new(to_latin1(&s)).unwrap();
    draw(x, y, x + w, y + row, 0x1, font, grey, c"  POSITIONS".as_ptr(), -1);
    let (word, word_colour) = fit_word(fits[best]);
    let lines = [
        ("Position", SPOTS[best].4.to_string(), light),
        ("Rating", word.to_string(), rgb(word_colour) as u32),
        ("Role /20", rating(SPOTS[best].0).to_string(), light),
        ("Side /20", if SPOTS[best].0 == GK { "-".to_string() } else { rating(SPOTS[best].1).to_string() }, light),
    ];
    let (label_w, value_w) = (w * 24 / 100, w * 19 / 100);
    for (i, (label, value, colour)) in lines.into_iter().enumerate() {
        let ry = y + (i as i32 + 1) * row;
        draw(x, ry, x + label_w, ry + row, 0x1, font, grey, text(format!("  {label}")).as_ptr(), -1);
        draw(x + label_w, ry, x + label_w + value_w, ry + row, 0x1, font, colour, text(value).as_ptr(), -1);
    }
    let (px1, py1) = (x + label_w + value_w + 8, y + 4);
    fit_pitch(player, px1, py1, x + w - 8, (y + h).min(py1 + ((x + w - 8 - px1) * 64 / 100)));
}

/// The pitch as FM draws it: grey lines on the dark area, a grey dot for each spot he does not
/// play, a dot by fit for the others, the best with a gold ring.
unsafe fn fit_pitch(player: usize, px1: i32, py1: i32, px2: i32, py2: i32) {
    let rating = |at: usize| rd::<i8>(player + at) as i32;
    // (a keeper has no side: GK by the role alone, as the compare screen does)
    let fits: Vec<i32> = SPOTS.iter().map(|s| if s.0 == GK { rating(GK) } else { rating(s.0).min(rating(s.1)) }).collect();
    let best = (0..SPOTS.len()).max_by_key(|&i| (fits[i], rating(SPOTS[i].0))).unwrap();
    let line = rgb([0x6a, 0x70, 0x78]) as u32;
    let rect = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_RECT);
    let seg = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_LINE);
    rect(px1, py1, px2, py2, 2, line);
    let (pw, ph) = (px2 - px1, py2 - py1);
    seg(px1 + pw / 2, py1, px1 + pw / 2, py2, 2, line);
    circle(px1 + pw / 2, py1 + ph / 2, ph * 15 / 100, 1.0, [0x6a, 0x70, 0x78]);
    for (depth, part) in [(pw * 16 / 100, ph * 22 / 100), (pw * 6 / 100, ph * 37 / 100)] {
        rect(px1, py1 + part, px1 + depth, py2 - part, 2, line); // penalty area, goal area
        rect(px2 - depth, py1 + part, px2, py2 - part, 2, line);
    }
    let r = (ph / 19).max(3);
    for (i, s) in SPOTS.iter().enumerate() {
        let (cx, cy) = (px1 + (s.2 * pw as f32) as i32, py1 + (s.3 * ph as f32) as i32);
        if i == best {
            dot(cx, cy, r + 2, [0x5a, 0xe8, 0x9a]);
            circle(cx, cy, r + 5, 1.5, GOLD);
        } else {
            dot(cx, cy, r, if fits[i] >= 5 { fit_word(fits[i]).1 } else { [0x2a, 0x2e, 0x34] });
        }
    }
}

/// A smoothed circle outline of `width` pixels.
unsafe fn circle(cx: i32, cy: i32, r: i32, width: f32, c: [u8; 3]) {
    let (colour, size) = (rgb565(c), r + 2);
    blend(cx - size, cy - size, 2 * size + 1, 2 * size + 1, |col, row| {
        let d = (((col - size) * (col - size) + (row - size) * (row - size)) as f32).sqrt();
        (colour, ((width / 2.0 + 0.5 - (d - r as f32).abs()).clamp(0.0, 1.0) * 255.0) as u32)
    });
}

/// Stars for an ability in a list cell: five stars where they fit, else one star and the count
/// ("3.5") for a narrow column.
unsafe fn star_cell(x: i32, y1: i32, x2: i32, y2: i32, font: u32, size: i32, ability: i32, colour: [u8; 3]) {
    if size >= 8 {
        return stars(x, y1 + (y2 - y1 - size) / 2, size, ability, colour);
    }
    let one = (y2 - y1) * 60 / 100;
    let halves = (ability * 10 + 100) / 200;
    stars_one(x, y1 + (y2 - y1 - one) / 2, one, colour);
    text_at(x + one + 4, y1, y2, font, colour, format!("{}{}", halves / 2, if halves % 2 == 1 { ".5" } else { "" }).as_bytes(), 0);
    let _ = x2;
}

/// A row of five stars in `colour` for an ability of 0..200, in halves.
unsafe fn stars(x: i32, y: i32, size: i32, ability: i32, colour: [u8; 3]) {
    let halves = (ability * 10 + 100) / 200; // 0..10, rounded
    let (gold, grey) = (rgb(colour), rgb([0x4a, 0x50, 0x5a]));
    for i in 0..5 {
        let sx = x + i * (size + 4);
        let mask = star_mask(size);
        blend(sx, y, size, size, |col, row| {
            let filled = 2 * i + 1 < halves || (2 * i + 1 == halves && col < size / 2);
            (if filled { gold } else { grey }, mask[(row * size + col) as usize] as u32)
        });
    }
}

/// One filled star.
unsafe fn stars_one(x: i32, y: i32, size: i32, colour: [u8; 3]) {
    let c = rgb565(colour);
    let mask = star_mask(size);
    blend(x, y, size, size, |col, row| (c, mask[(row * size + col) as usize] as u32));
}

/// A star's coverage at a size, 0..255 a pixel (4x4 samples each), made once a size: a row of
/// search results drew its stars in 3.7 ms when every sample worked out the star again.
fn star_mask(size: i32) -> &'static [u8] {
    static mut MASKS: Vec<(i32, &'static [u8])> = Vec::new();
    let size = size.max(1);
    #[allow(static_mut_refs)]
    unsafe {
        if let Some(m) = MASKS.iter().find(|m| m.0 == size) {
            return m.1;
        }
        let mut mask = Vec::with_capacity((size * size) as usize);
        for row in 0..size {
            for col in 0..size {
                let mut hits = 0u32;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let (px, py) = ((col as f32 + (sx as f32 + 0.5) / 4.0) / size as f32 * 2.0 - 1.0, (row as f32 + (sy as f32 + 0.5) / 4.0) / size as f32 * 2.0 - 1.0);
                        hits += in_star(px, py) as u32;
                    }
                }
                mask.push((hits * 255 / 16) as u8);
            }
        }
        // ponytail: one mask a size, never freed (a handful of sizes in a game)
        let mask: &'static [u8] = Box::leak(mask.into_boxed_slice());
        MASKS.push((size, mask));
        mask
    }
}

/// Whether (x, y) in -1..1 lies in a five-pointed star with one point up.
fn in_star(x: f32, y: f32) -> bool {
    let pts: [(f32, f32); 10] = std::array::from_fn(|k| {
        let r = if k % 2 == 0 { 1.0 } else { 0.42 };
        let a = std::f32::consts::PI * (k as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
        (r * a.cos(), r * a.sin())
    });
    let mut inside = false;
    for k in 0..10 {
        let ((x1, y1), (x2, y2)) = (pts[k], pts[(k + 1) % 10]);
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

/// Takes the place of the flip at the end of Display::draw_screen: overlays first.
extern "C" fn flip(left: i32, top: i32, right: i32, bottom: i32) {
    let start = std::time::Instant::now();
    flip_hooked(left, top, right, bottom);
    unsafe {
        let (text, calls) = std::mem::take(&mut TEXT_TIME);
        TRACE_ON = std::path::Path::new("cmui.trace").exists();
        TIMING = std::path::Path::new("cmui.timing").exists();
        let [panels, glyphs, results] = std::mem::take(&mut HOOK_TIME).map(|d| d.as_secs_f64() * 1e3);
        if TIMING {
            log(&format!("timing: {calls} texts {:.1} ms, flip {:.1} ms, panels {panels:.1}, glyphs {glyphs:.1}, results {results:.1}", text.as_secs_f64() * 1e3, start.elapsed().as_secs_f64() * 1e3));
        }
    }
}

fn flip_hooked(left: i32, top: i32, right: i32, bottom: i32) {
    unsafe {
        #[allow(static_mut_refs)]
        if !std::mem::take(&mut STATS_SEEN) {
            STATS_COLS.clear();
        }
        HIDE_BUTTON = None; // set again below when the ability block is drawn
        let darkened = std::mem::take(&mut DARKENED);
        let strip = STRIP.take();
        // (kept for the next time: only a change of view draws the list twice. Another screen's
        // title says nothing about the squad list: the other way only with its rows on screen)
        let titled = comp::view_is(b"Position(s)");
        if std::mem::take(&mut SQUAD_ROWS) | titled && FULL_FRAME {
            REDRAW |= titled != POSITIONS_VIEW;
            POSITIONS_VIEW = titled;
        }
        if STORY.is_none() && FULL_FRAME {
            NEWS_LINK = None; // a screen without a story
            assistant::SCOUT_CHIP = None;
            news::clear();
        }
        if let Some((rect, head, date, font, body)) = STORY.take() {
            story_header(rect, &head, &date, font, &body);
        }
        NEWS_DATE = None;
        fit_cells();
        squad_table_heads();
        if FULL_FRAME {
            TACTICS = std::mem::take(&mut TACTICS_SEEN);
            SEARCH = std::mem::take(&mut SEARCH_SEEN);
            LIVE_TACTICS = std::mem::take(&mut LIVE_SEEN);
        }
        tactics_labels();
        let full_frame = FULL_FRAME; // (match_overlays takes it)
        match_overlays();
        if on(NEWS) {
            news::mark_button(FULL_FRAME);
        }
        if on(ASSISTANT_PICK) {
            assistant::flip(full_frame);
        }
        news::frame_end(FULL_FRAME);
        if on(STATS_TABLE) {
            comp::flip(FULL_FRAME);
        }
        if on(THEME_ON) {
            news::scrollbars();
        }
        ROW_NAME = (i32::MIN, 0);
        POSITION_ROW = i32::MIN;
        league_line();
        history::flip();
        if let (Some((staff, line, x1, y1, x2, y2, font)), None) = (&strip, &GRID_SEEN) {
            // no grid to take the width from (Injuries & Bans, Contract, Transfer): the band it is
            // in: the widest panel around the line about as high as the line (170-1636; the game's
            // own box for the text, 623-1636, is a panel too)
            #[allow(static_mut_refs)]
            // (the Traditional font's box reaches past the band's right edge: 608-1649 in 170-1636)
            let band = PANELS.iter().filter(|p| p.0 <= *x1 && p.2 >= (x1 + x2) / 2 && p.1 <= *y1 && p.3 >= *y2 && p.3 - p.1 <= (y2 - y1) * 3 / 2).max_by_key(|p| p.2 - p.0);
            let (left, right) = band.map_or((*x1, *x2), |p| (p.0, p.2));
            info_strip(*staff, line, left, *y1, right, *y2, *font);
        }
        if let Some((mut cols, bottom, font, h)) = GRID_SEEN.take().filter(|_| on(PROFILE)) {
            let status = std::mem::take(&mut STATUS);
            let status_row = STATUS_ROW.take();
            cols.sort();
            let (left, right) = (cols[0], cols[cols.len() - 1] + GRID.0 + 2 + VALUE_CELL);
            if let Some((staff, line, _, y1, _, y2, font)) = &strip {
                info_strip(*staff, line, left, *y1, right, *y2, *font);
            }
            if let (Some(p), true) = (profiled_player(), cols.len() >= 3) {
                // mike_appguy's three panels under the attributes: reputation, status, positions.
                // Where they do not fit above the stats table (800x600 and the like) they are an
                // overlay over it, shown with the More button of the ability block.
                let (top, mut height, mut row, w) = (bottom + 6, 6 * (h + 2), h + 4, right - left);
                let stats_top = STATS_BUTTONS.0;
                let mut fits = stats_top == i32::MIN || stats_top < bottom || top + height <= stats_top - 2;
                let mut panel_font = font;
                // where the gap holds the panels' five lines a little closer (1280x800), they go
                // there in the next smaller font, and the stats table stays in view
                let gap = stats_top - 2 - top;
                if !fits && gap / 5 >= h * 3 / 4 {
                    (fits, row, height, panel_font) = (true, gap / 5, gap, smaller(font, 1));
                }
                if let (Some((ca, pa)), Some(y)) = (ca_pa(p), status_row) {
                    ability(cols[2], y, GRID.0 + 2 + VALUE_CELL, h, font, ca, pa, !fits);
                }
                if fits || SHOW_DETAILS {
                let (w1, w2) = (w * 27 / 100, w * 30 / 100);
                // one dark surface under all three panels, down to where the grid's own cells end
                // (they stop short of the pitch)
                // (only cell-sized rectangles: the stats table's whole area is darkened in one go)
                let cells = darkened.iter().filter(|d| d.1 - d.0 <= h + 4 && d.0 < top + height && d.1 > top);
                let mut end = cells.map(|d| d.1).max().unwrap_or(top + height);
                if !fits {
                    // the overlay covers the stats table down to its end
                    end = end.max(darkened.iter().filter(|d| d.0 <= stats_top && d.1 >= stats_top).map(|d| d.1 - 1).max().unwrap_or(top + height));
                }
                fill(left, bottom, right, end.max(top + height), AREA);
                reputation(p, left, top, w1 - 16, height, panel_font, row);
                player_status(&status, left + w1, top, w2 - 16, panel_font, row);
                positions(p, left + w1 + w2, top, w - w1 - w2, height, panel_font, row);
                }
            }
        }
        compare_overlay();
        if on(MENU_FIXES) {
            button_arrows();
        }
        assistant::report();
        if on(CHALLENGE) {
            challenge::flip(full_frame);
            challenge::overlay();
        }
        // a live match runs its own loop without poll_input: the test script goes on from here
        if LAST_POLL.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(700)) {
            static mut SAID: bool = false;
            if !SCRIPT.is_empty() && !std::mem::replace(&mut SAID, true) {
                log("script: going on from the flip pass (poll_input is not running)");
            }
            script();
        }
        BUTTONS.clear();
        if remote::CLIENT {
            remote::ask(); // what this frame missed, from the host
            if remote::host_silent() {
                let (sw, sh) = screen();
                let (y1, y2) = (sh * 3 / 4, sh * 3 / 4 + 40);
                fill(sw / 2 - 170, y1, sw / 2 + 170, y2, [0x26, 0x2a, 0x31]);
                text_at(sw / 2, y1, y2, SMALL_FONT.unwrap_or(0xffff0001), GOLD, b"Waiting for the host ...", 1);
            }
        }
        let_picture_through(left, top, right, bottom);
        game::<extern "C" fn(i32, i32, i32, i32)>(FLIP_RECT)(left, top, right, bottom);
    }
}

// Club crests in the title bar, from Data\cmui_crests.bin (ui\logos.py pack).
const BACK_BUFFER: usize = 0xaebde4; // IDirectDrawSurface* the game draws on
const GREEN_MASK: usize = 0xae316c; // 0x7e0 when the screen is RGB565
const CREST_SIZE: usize = 80;
/// The title bar: the wide box at the top of the screen; a crest goes at its left.
static mut TITLE: (i32, i32, i32, i32) = (0, 0, 0, 0);

fn note_title(x1: i32, y1: i32, x2: i32, y2: i32) {
    // at the top, about a tenth of the screen high, more than half of it wide: 148,17-1658,123
    // at 1680x1050; all in proportion, so 800x600 and 1280x800 work the same
    let (w, h) = screen();
    if y1 <= h / 30 && y2 - y1 >= h * 8 / 100 && x2 - x1 > w / 2 {
        unsafe { TITLE = (x1, y1, x2, y2) }
    }
}

const CURRENT_CHANNEL: usize = 0xb77c7a; // i16: the network channel whose human is at the screen
const CHANNEL_MANAGER: usize = 0xb74c26; // Staff* of each channel's human manager, 0x300 apart

/// The human manager's club name and surname: staff +0x39 is the club (name at +4), +8 the
/// second name record (text first).
/// The network channel of the human at this screen: the client's own seat on the host in a client.
unsafe fn channel() -> usize {
    (if remote::CLIENT { remote::CHANNEL } else { *(CURRENT_CHANNEL as *const i16) }) as usize
}

unsafe fn human_club_ptr() -> Option<usize> {
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let staff = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    // (a manager without a club has 0 there: no club, nothing to read)
    readable(staff, 0x3d).then(|| rd::<usize>(staff + 0x39)).filter(|&club| club != 0 && readable(club, 0x1b3 + 28))
}

/// Staff +0x39 his club, +0x24 the national team he is contracted to (clubs and national teams
/// share the club table).
unsafe fn plays_for(staff: usize, club: usize) -> bool {
    rd::<usize>(staff + 0x39) == club || rd::<usize>(staff + 0x24) == club
}

/// The club or national team on screen is the human manager's own.
unsafe fn human_runs(club: usize) -> bool {
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let staff = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    club != 0 && readable(staff, 0x3d) && plays_for(staff, club)
}

unsafe fn human_club() -> Option<(Vec<u8>, Vec<u8>)> {
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let staff = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    if !readable(staff, 0x61) {
        return None;
    }
    let club = rd::<usize>(staff + 0x39);
    let surname = game_text(rd::<usize>(staff + 8), 51);
    let name = game_text(club + 4, 51);
    (!name.is_empty()).then_some((name, surname))
}

/// The club of a club screen's title: screen parameter 0 when it is that club (squad), else the
/// human's club when the title names it. Club +4 name, +0x38 short name, +0x57 its league.
unsafe fn header_club(key: &[u8]) -> Option<usize> {
    let names = |club: usize| readable(club, 0x5b) && (game_text(club + 4, 51) == key || game_text(club + 0x38, 26) == key);
    let club = [screen_param(0), human_club_ptr().unwrap_or(0)].into_iter().find(|&c| !key.is_empty() && names(c))?;
    let league = rd::<usize>(club + 0x57);
    ((readable(league, 0x37) && !game_text(league + 4, 51).is_empty()) || national_team(club)).then_some(club)
}

/// A national team's record (the rows after the clubs, nat_club.dat), as the game tells.
unsafe fn national_team(club: usize) -> bool {
    if remote::CLIENT {
        return ask_host(&key(b'N', &[club as u32])).is_some_and(|v| v.first() == Some(&1));
    }
    game::<extern "C" fn(usize) -> u32>(IS_NATIONAL_TEAM)(club) & 0xff != 0
}

/// The row of the last position cell ("D/DM C") drawn; the row and staff of the last squad
/// member's name; position cells still to get a condition (end of the position text, top, right,
/// bottom, staff, font).
static mut POSITION_ROW: i32 = i32::MIN;
static mut ROW_NAME: (i32, usize) = (i32::MIN, 0);
/// The club of the last club title drawn (kept: a partial redraw leaves the title alone).
static mut SQUAD_CLUB: usize = 0;
static mut FIT_CELLS: Vec<(i32, i32, i32, i32, usize, u32)> = Vec::new();
const INJURIES: usize = 0xb63d28; // the injury manager (per-staff fitness, 31 bytes each)
const CONDITION: usize = 0x62c590; // (this, staff, percent) -> condition, 0..100 when percent

/// The condition the game gives a staff member, 0..100 (the host's in a network client).
unsafe fn condition(staff: usize) -> Option<i32> {
    if remote::CLIENT {
        return ask_host(&key(b'F', &[staff as u32])).and_then(|v| word(&v)).map(|v| v as i32);
    }
    let manager = *(INJURIES as *const usize);
    mapped(manager, 4).then(|| game::<extern "thiscall" fn(usize, usize, i32) -> u32>(CONDITION)(manager, staff, 1) as i16 as i32)
}

fn fit_colour(p: i32) -> [u8; 3] {
    match p {
        90.. => [0x4c, 0xc0, 0x7a],
        60.. => GOLD,
        _ => [0xe0, 0x50, 0x48],
    }
}

/// The squad member of the club on screen a list cell names: "Jacobs, W", "Wim Jacobs" or the
/// common name ("Tó Madeira"); a trailing "*" is left out. Club +0xd7: 50 staff.
unsafe fn squad_member(t: &[u8]) -> Option<usize> {
    let club = SQUAD_CLUB;
    let t = t.strip_suffix(b"*").unwrap_or(t);
    if t.len() < 3 || t.iter().any(|c| c.is_ascii_digit()) || !readable(club + 0xd7, 200) {
        return None;
    }
    let name = |staff: usize, at: usize| {
        let p = rd::<usize>(staff + at);
        if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
    };
    (0..50).map(|k| rd::<usize>(club + 0xd7 + 4 * k)).find(|&staff| {
        // (the Position(s) table and the Ability view have a row for a player on loan here too)
        if !readable(staff, 0x6e) || !(plays_for(staff, club) || POSITIONS_VIEW || SQUAD_ABILITY) {
            return false;
        }
        let (first, second, common) = (name(staff, 4), name(staff, 8), name(staff, 0xc));
        let initial = [&second[..], b", ", &first[..first.len().min(1)]].concat();
        let full = [&first[..], b" ", &second[..]].concat();
        t == initial || t == full || (!common.is_empty() && t == common)
    })
}

/// The tactics list's last position cell (row, x1, x2) and its columns for the labels over the
/// list (first row's top, name cell x1 / x2, position cell x1 / x2).
static mut TACTICS_POS: (i32, i32, i32) = (i32::MIN, 0, 0);
static mut TACTICS_HEAD: Option<(i32, i32, i32, i32, i32, u32)> = None;
const PLAYER_STATS: usize = 0xb74350; // PlayerStats*
const FORMAT_STAT: usize = 0x7c25b0; // thiscall(stats, player, cat, field, buf, len, subs, apps, zero)

/// This season's value as the profile's stats table writes it ("7.48", "1.1"); `field` 0x11 Av R,
/// 0xe Tck, 0xf Drb; category 6 = all senior club games (5 for a national team's player).
unsafe fn season_stat(staff: usize, field: i32) -> Vec<u8> {
    if remote::CLIENT {
        return ask_host(&key(b'S', &[staff as u32, field as u32])).unwrap_or_default();
    }
    let stats = *(PLAYER_STATS as *const usize);
    let player = ((staff + 0x61) as *const usize).read_unaligned();
    if !readable(stats, 0x34) || !readable(player, 70) {
        return Vec::new();
    }
    let mut buf = [0u8; 0x40];
    game::<extern "thiscall" fn(usize, usize, i32, i32, *mut u8, i32, i32, i32, i32) -> i32>(FORMAT_STAT)(stats, player, 6, field, buf.as_mut_ptr(), 0x33, 1, 0, 1);
    buf.split(|&c| c == 0).next().unwrap_or(&[]).trim_ascii().to_vec()
}

/// One row of the tactics list: the name in the left half of its cell, then Av R, Tck, Drb; the
/// player's fitness at the right of the row's position cell.
#[allow(static_mut_refs)]
unsafe fn tactics_row(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, colour: u32, text: *const i8, staff: usize) {
    let w = x2 - x1;
    // (the original colours write the in-match list in black, for a light background: light here)
    let rgb_of = unpack(colour & 0xffff, screen_format() != Some(false));
    let colour = if rgb_of.iter().sum::<u32>() < 120 || LIVE_TACTICS { rgb(LIGHT) as u32 } else { colour };
    let (name_end, cols) = tactics_columns(x1, x2, font, LIVE_TACTICS);
    let f = fit_font(font, text, name_end - x1 - 6);
    let f = if small_screen() { column_font(x1, x2, font, f) } else { f };
    let measure = |t: *const i8| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, t);
    let surname = std::ffi::CString::new(game_text(rd::<usize>(staff + 8), 51)).unwrap_or_default();
    // (the game draws nothing that does not fit: the surname then)
    let room = name_end - x1 - 6;
    let clipped = std::ffi::CString::new(clip_text(f, surname.as_bytes(), room)).unwrap_or_default();
    let shown = if measure(text) <= room || surname.as_bytes().is_empty() { text } else if measure(surname.as_ptr()) <= room { surname.as_ptr() } else { clipped.as_ptr() };
    if trace() {
        log(&format!("tactics_row live {LIVE_TACTICS} cell {x1},{y1}-{x2},{y2} name_end {name_end} room {room} font {font:#x} f {f:#x} w(text) {} w(surname) {} shown {:?} cols {cols:?}", measure(text), measure(surname.as_ptr()), latin1(std::ffi::CStr::from_ptr(shown).to_bytes())));
    }
    if LIVE_TACTICS {
        // (in a match the box call draws nothing here: the plain text routine, cut to fit; grey
        // when he is not on the pitch: a sub not yet on, a starter gone off)
        let bytes = clip_text(f, std::ffi::CStr::from_ptr(shown).to_bytes(), room);
        let off = live_off_pitch(staff);
        text_at(x1 + 2, y1, y2, f, if off { GREY } else { LIGHT }, &bytes, 0);
    } else {
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(x1, y1, name_end, y2, 0x1, f, colour, shown, -1);
    }
    let small = smaller(font, 1);
    let dim = colour & 0xffff != rgb(WHITE) as u32 & 0xffff && !is_colour(colour, 0xe71c); // the subs
    // during a match: the position he plays and his live condition (the game's own column at the
    // right shows his match rating)
    if let Some(e) = if LIVE_TACTICS { live_engine() } else { None } {
        if let Some(p) = live_player(e, staff).filter(|_| human_runs(SQUAD_CLUB)) {
            let k = (p - (e + 0x4796)) / 0x1be;
            let cond = live_condition(p);
            // (off the pitch: a sub not yet on by the positions he plays, as before the match,
            // not his bench slot; a starter who has gone off "-", not "SB0")
            let mut pos = live_position(e, p, k);
            if pos.starts_with(b"SB") {
                pos = if k % 20 >= 11 { clip_text(small, &positions_text(staff), text_width(small, b"DMCR") + sx(14).max(6)) } else { b"-".to_vec() };
            }
            text_at(cols[0], y1, y2, small, if dim { GREY } else { GOLD }, &pos, 1);
            text_at(cols[1], y1, y2, small, fit_colour(cond), format!("{cond}%").as_bytes(), 1);
        }
        if TACTICS_HEAD.is_none_or(|h| y1 < h.0) {
            TACTICS_HEAD = Some((y1, x1, x2, -1, -1, font));
        }
        return;
    }
    for (k, field) in [0x11, 0xe, 0xf].into_iter().enumerate().filter(|&(k, _)| cols[k] >= 0) {
        let v = season_stat(staff, field);
        let cx = cols[k];
        let c = if dim { GREY } else if k == 0 { GOLD } else { LIGHT };
        text_at(cx, y1, y2, small, c, if v.is_empty() { b"-" } else { &v }, 1);
    }
    let (_, px1, px2) = TACTICS_POS;
    if TACTICS_HEAD.is_none_or(|h| y1 < h.0) {
        TACTICS_HEAD = Some((y1, x1, x2, px1, px2, font));
    }
}

/// The match engine when the tactics screen is opened during a match (screen parameter 0 is the
/// Tactics_Screen, +4 the engine; parameter 10 is 6 in a match, 2 outside). `ui\helper_live.md`.
unsafe fn live_engine() -> Option<usize> {
    let t = screen_param(0);
    if screen_param(10) != 6 || !readable(t, 8) {
        return None;
    }
    let e = rd::<usize>(t + 4);
    // (each player row points back to its engine at +0x19e: no stray pointer passes this)
    (readable(e + 0x4796, 40 * 0x1be) && rd::<usize>(e + 0x4796 + 0x19e) == e).then_some(e)
}

/// In a match, the player is not on the pitch: by the tactics screen's own copy of its side's rows
/// (+0x2735, 20 of 0x1be bytes, the substitutions queued on the screen in it), as the screen's
/// 0x8aa9d0 has it (slot >= 0, or -2 while the engine's +0x8eb3 is clear); else by the engine.
unsafe fn live_off_pitch(staff: usize) -> bool {
    let Some(e) = live_engine() else { return false };
    let ts = screen_param(0);
    let copy = (0..20).map(|k| ts + 0x2735 + k * 0x1be).find(|&r| readable(r, 0x6d) && rd::<usize>(r + 0x69) == staff);
    match copy.or_else(|| live_player(e, staff)) {
        Some(r) => {
            let slot = rd::<i8>(r + 0x19);
            !(slot >= 0 || (slot == -2 && (!readable(e + 0x8eb3, 1) || rd::<u8>(e + 0x8eb3) == 0)))
        }
        None => false,
    }
}

/// A player's row in the match engine (0x1be bytes; +0x69 Staff*, +0x4d live condition 0..10000
/// as a float, +0x1b rating, +0x19 pitch slot, +0x27 side).
unsafe fn live_player(e: usize, staff: usize) -> Option<usize> {
    (0..40).map(|k| e + 0x4796 + k * 0x1be).find(|&p| rd::<i8>(p + 0x19) != -5 && rd::<usize>(p + 0x69) == staff)
}

unsafe fn live_condition(p: usize) -> i32 {
    (rd::<f32>(p + 0x4d) * 0.01) as i32
}

/// "DC", "AMR" as the match screens write a slot's position (0x5b7810); "SB3" on the bench.
unsafe fn live_position(e: usize, p: usize, k: usize) -> Vec<u8> {
    let slot = rd::<i8>(p + 0x19);
    if !(0..=10).contains(&slot) {
        return format!("SB{}", (k % 20).saturating_sub(10)).into_bytes();
    }
    let at = e + 0x8ebc + rd::<u8>(p + 0x27) as usize * 4;
    if !readable(at, 4) {
        return Vec::new();
    }
    let table = rd::<usize>(at);
    if !readable(table, 22) {
        return Vec::new();
    }
    let code = rd::<u16>(table + slot as usize * 2) as u32;
    let mut buf = [0u8; 16];
    game::<extern "C" fn(u32, *mut u8, i32, i32)>(0x5b7810)(code, buf.as_mut_ptr(), 10, 0);
    buf.split(|&c| c == 0).next().unwrap().to_vec()
}

/// The columns cmui adds to a tactics list row's name cell, from its right end, as wide as their
/// widest text in the row font ("8.88", or "AM RLC" / "100%" in a match): where the name ends and
/// each column's centre. Measured, so 800x600 fits as well as 1680x1050.
unsafe fn tactics_columns(x1: i32, x2: i32, font: u32, live: bool) -> (i32, [i32; 3]) {
    let small = smaller(font, 1);
    let width = |s: &std::ffi::CStr| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(small, s.as_ptr());
    let gap = sx(14).max(6);
    if live {
        let (pos, cond) = (width(c"DMCR") + gap, width(c"100%") + gap); // live codes: "DMC", "SB3"
        let c1 = x2 - cond / 2 - 2;
        let c0 = c1 - cond / 2 - pos / 2;
        let name_end = (c0 - pos / 2).max(x1 + (x2 - x1) / 3);
        // where a long name does not fit that way (800x600), Pos and Cond move right, into the
        // free room left of the game's rating column, and the name keeps its whole cell
        let long = std::ffi::CString::new("Harry Anderson").unwrap();
        let name_w = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, long.as_ptr());
        if name_w > name_end - x1 - 6 && RATING_X > x2 + pos + cond {
            let c1 = RATING_X - cond / 2 - 4;
            (LIVE_MOVED, LIVE_NAME_X2) = (true, x2);
            return (x2 - 2, [c1 - cond / 2 - pos / 2, c1, c1]);
        }
        LIVE_MOVED = false;
        return (name_end, [c0, c1, c1]);
    }
    let col = width(c"8.88") + gap;
    let c2 = x2 - col / 2 - 2;
    if (x2 - x1) - 3 * col < (x2 - x1) * 55 / 100 {
        // a narrow cell (800x600): Av R only, the name keeps the room
        return (c2 - col / 2, [c2, -1, -1]);
    }
    let (c1, c0) = (c2 - col, c2 - 2 * col);
    ((c0 - col / 2).max(x1 + (x2 - x1) / 3), [c0, c1, c2])
}

/// The labels over the tactics list (flip pass).
#[allow(static_mut_refs)]
unsafe fn tactics_labels() {
    let Some((top, x1, x2, px1, px2, row_font)) = TACTICS_HEAD.take() else { return };
    let w = x2 - x1;
    let (y1, y2) = (top - sy(22), top - 2);
    let font = smaller(row_font, 2); // the labels' font
    label_row_inside(x1 + 4, y1, top);
    if px1 < 0 {
        // during a match: Pos and Cond by the name, the game's rating column at the right
        let (_, cols) = tactics_columns(x1, x2, row_font, true);
        text_at(cols[0], y1, y2, font, GREY, b"Pos", 1);
        text_at(cols[1], y1, y2, font, GREY, b"Cond", 1);
        text_at(x1 + w * 181 / 100, y1, y2, font, GREY, b"Rat", 1);
        return;
    }
    let (_, cols) = tactics_columns(x1, x2, row_font, false);
    for (cx, label) in [(cols[0], "Av R"), (cols[1], "Tck"), (cols[2], "Drb")].into_iter().filter(|c| c.0 >= 0) {
        text_at(cx, y1, y2, font, GREY, label.as_bytes(), 1);
    }
    text_at(px1 + 2, y1, y2, font, GREY, b"Pos", 0);
    text_at(px2 + (px2 - px1) / 3 + 10, y1, y2, font, GREY, b"Cond", 1);
}

/// Makes a labels row from `y1` to the first row (`top`) part of the list panel under it: the
/// thin seam of page background between the panel above (the tactic's name) and the list, when
/// it runs through the labels, moves up to just above them, across the list panel's width.
unsafe fn label_row_inside(x: i32, y1: i32, top: i32) {
    let reach = top - y1 + sy(16);
    let Some(column) = region(x, top - reach, 1, reach) else { return };
    let panel = column[column.len() - 2]; // the list's own colour just above its first row
    // the seam: the nearest rows above the first row that are not the panel's colour
    let Some(bottom) = (0..column.len() - 1).rev().find(|&k| column[k] != panel) else { return };
    let height = (0..=bottom).rev().take_while(|&k| column[k] != panel).count() as i32;
    let bottom = top - reach + bottom as i32;
    if bottom < y1 || height > sy(12).max(3) {
        return; // above the labels already (moved, or a roomy layout), or not a seam
    }
    let seam = column[(bottom - (top - reach)) as usize];
    // the list panel's width: its colour along the row under the seam
    let width = screen().0;
    let Some(row) = region(0, bottom + 1, width, 1) else { return };
    let left = (0..x.min(width) as usize).rev().take_while(|&k| row[k] == panel).last().unwrap_or(x as usize) as i32;
    let right = (x.max(0) as usize..width as usize).take_while(|&k| row[k] == panel).last().unwrap_or(x as usize) as i32;
    // (the game's rectangle includes its right and bottom edge)
    fill(left, bottom - height + 1, right, bottom, panel);
    fill(left, y1 - height, right, y1 - 1, seam);
}

/// The squad member a pitch label names: "Butra", "de Vries", "R.Evers", "Tó Madeira".
unsafe fn pitch_member(t: &[u8]) -> Option<usize> {
    let club = SQUAD_CLUB;
    if !readable(club + 0xd7, 200) {
        return None;
    }
    let name = |staff: usize, at: usize| {
        let p = rd::<usize>(staff + at);
        if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
    };
    (0..50).map(|k| rd::<usize>(club + 0xd7 + 4 * k)).find(|&staff| {
        // (the Position(s) table and the Ability view have a row for a player on loan here too)
        if !readable(staff, 0x6e) || !(plays_for(staff, club) || POSITIONS_VIEW || SQUAD_ABILITY) {
            return false;
        }
        let (first, second, common) = (name(staff, 4), name(staff, 8), name(staff, 0xc));
        let initial = [&first[..first.len().min(1)], b".", &second[..]].concat();
        t == second || t == initial || (!common.is_empty() && t == common)
    })
}

/// The conditions owed to position cells this frame, right-aligned in the cell, all in one font:
/// the largest every row has room for after its position ("D/DM RC" as much as "GK").
#[allow(static_mut_refs)]
unsafe fn fit_cells() {
    let cells: Vec<_> = std::mem::take(&mut FIT_CELLS).into_iter()
        .filter_map(|(used, y1, x2, y2, staff, font)| Some((used, y1, x2, y2, std::ffi::CString::new(format!("{}%", condition(staff)?)).unwrap(), font)))
        .collect();
    let width = |f: u32, t: &std::ffi::CString| game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(f, t.as_ptr());
    let fits = |c: &(i32, i32, i32, i32, std::ffi::CString, u32), f: u32| c.2 - 6 - width(f, &c.4) >= c.0 + 6;
    // ponytail: the game's fonts are numbered by size (low word), so a step down is one smaller;
    // a row redrawn on its own picks its own font
    let Some(font) = cells.iter().map(|c| {
        let mut f = c.5;
        while f & 0xffff > 1 && !fits(c, f) {
            f -= 1;
        }
        f
    }).min_by_key(|f| f & 0xffff) else { return };
    for c in &cells {
        if !fits(c, font) {
            continue; // no room after the position even in the smallest font: left out
        }
        let p: i32 = c.4.to_str().unwrap_or("").trim_end_matches('%').parse().unwrap_or(0);
        game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(c.2 - 6 - width(font, &c.4), c.1, c.2, c.3, 0x9, font, rgb(fit_colour(p)) as u32, c.4.as_ptr(), -1);
    }
}

/// A player's positions as the game writes them: "GK", "D/DM C", "AM RLC", "F C".
fn is_position(t: &[u8]) -> bool {
    let text = latin1(t);
    let mut parts = text.split(' ');
    let roles = parts.next().unwrap_or("");
    let sides = parts.next();
    let known = |r: &str| ["GK", "SW", "D", "DM", "M", "AM", "F", "S", "WB"].contains(&r);
    parts.next().is_none()
        && roles.split('/').all(known)
        && match sides {
            None => roles == "GK",
            Some(s) => !s.is_empty() && s.chars().all(|c| "RLC".contains(c)),
        }
}

/// "1st in D1": the league position button of a club's screens; the place ("1st").
fn league_position(t: &[u8]) -> Option<&[u8]> {
    let at = t.windows(4).position(|w| w == b" in ")?;
    let place = &t[..at];
    let digits = place.iter().take_while(|c| c.is_ascii_digit()).count();
    (digits > 0 && [&b"st"[..], b"nd", b"rd", b"th"].contains(&&place[digits..])).then_some(place)
}

/// The club whose title is on screen (title box, text colour) and the league position button seen this
/// frame (text, box, font); the league line under the name and where it sends a click.
static mut HEADER_CLUB: Option<(usize, (i32, i32, i32, i32), u32)> = None;
/// That title is the club alone.
static mut HEADER_PLAIN: bool = true;
static mut LEAGUE_POS: Option<(Vec<u8>, (i32, i32, i32, i32), u32)> = None;
static mut LEAGUE_LINK: Option<((i32, i32, i32, i32), (i32, i32))> = None;
/// The left edge of the dropdowns in the title bar's right half this frame (Print, Action).
static mut TITLE_RIGHT: i32 = i32::MAX;

/// mike_appguy's club header: league logo at the left, crest at the right, and under the name
/// "1st in Dutch First Division", which opens what the league position button opens.
#[allow(static_mut_refs)]
unsafe fn league_line() {
    LEAGUE_LINK = None;
    let right = std::mem::replace(&mut TITLE_RIGHT, i32::MAX);
    let (Some((club, (x1, y1, x2, y2), colour)), pos) = (HEADER_CLUB.take(), LEAGUE_POS.take()) else { return };
    // (a History, Tactics or Training title: no position button there, what looks like one is a
    // record's text, "9th in ...")
    let pos = pos.filter(|_| HEADER_PLAIN);
    let t = TITLE;
    let bar = t.3 - t.1;
    let size = (CREST_SIZE as i32).min(bar * 80 / 106);
    let (cx, cy) = (t.0 + bar * 45 / 100, t.1 + (bar - size) / 2);
    let crest_x = (t.2 - (cx - t.0) - size).min(right - 12 - size);
    let team = game_text(club + 4, 51);
    draw_crest(&team, crest_x, cy, size);
    if national_team(club) {
        // its country's flag, big, at the left where a club has its league's logo (flags are
        // kept at 160 pixels): the flag itself as tall as the badge, whatever its shape
        let nation = rd::<usize>(club + 0x53);
        let country = if readable(nation, 55) { game_text(nation + 4, 51) } else { team };
        let key = [b"flag:".as_slice(), &country].concat();
        if let Some((first, end, n)) = crest_rows(&key) {
            let side = (size * n / (end - first).max(1)).min(size * 2); // the square it sits in
            let top = cy - first * side / n; // the flag's own top level with the badge's
            draw_crest(&key, cx, top, side);
        }
        return;
    }
    let league = rd::<usize>(club + 0x57);
    let name = latin1(&game_text(league + 4, 51)).into_owned();
    draw_crest(name.as_bytes(), cx, cy, size);
    // (no stats table seen yet: font 0 would draw nothing)
    let font = pos.as_ref().map_or(SMALL_FONT.unwrap_or(0xffff0002), |p| p.2);
    let text = match pos.as_ref().and_then(|p| league_position(&p.0)) {
        Some(place) => format!("{} in {name}", latin1(place)),
        None => name.clone(),
    };
    let c = std::ffi::CString::new(to_latin1(&text)).unwrap();
    let (h, top) = (bar * 26 / 106, (y1 + y2) / 2 + bar * if bar < 80 { 10 } else { 6 } / 106);
    // (between the two logos)
    // (from one size: what a screen had seen before must not change it)
    let font = if on(BIGGER_TEXT) && screen().0 >= 1280 { font_up(0xffff0002, c.as_ptr(), (t.2 - t.0) * 6 / 10, h, 2, 4) } else { font };
    let width = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, c.as_ptr());
    let left = (x1 + x2 - width) / 2;
    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(left - 2, top, left + width + 2, top + h, 0xc, font, colour, c.as_ptr(), -1);
    if let Some((_, (bx1, by1, bx2, by2), _)) = pos {
        LEAGUE_LINK = Some(((left, top, left + width, top + h), ((bx1 + bx2) / 2, (by1 + by2) / 2)));
    }
}

// The match screen as mike_appguy has it. Its title is two side boxes (name, score square); the
// state lives until the next full redraw, since a live match redraws parts only.
static mut MATCH: bool = false;
/// The screen manager's current node at the last full redraw (+0x7e2 of 0xb74450).
static mut LAST_NODE: usize = 0;
/// This frame redraws the whole screen; the match sides were seen in it; a redraw is owed (the
/// game draws the events before the sides, so the first frame of a match screen misses them).
static mut FULL_FRAME: bool = false;
static mut SIDES_SEEN: bool = false;
/// The Match Overview tab (its stadium band) is on screen; seen in this frame.
static mut OVERVIEW: bool = false;
static mut OVERVIEW_SEEN: bool = false;
/// The match screen's open tab ("Match Overview", "Latest Scores" ...), as last drawn.
static mut MATCH_TAB: Vec<u8> = Vec::new();
static mut REDRAW: bool = false;
static mut MATCH_SIDES: Vec<(Vec<u8>, (i32, i32, i32, i32))> = Vec::new();
static mut MATCH_SCORES: Vec<((i32, i32, i32, i32), Vec<u8>)> = Vec::new();
/// The aggregate of a two-legged tie ("1-2", this match's home side first), taken from the title
/// band (the game writes "Agg 1-2 " under the away crest); kept until the screen changes.
static mut MATCH_AGG: Vec<u8> = Vec::new();
/// Stadium band, date and "First Division / HT 3-1": box, text, font.
static mut MATCH_HEAD: [Option<((i32, i32, i32, i32), Vec<u8>, u32)>; 3] = [None, None, None];
const MOM_SLOT: usize = 0xaebde8; // the palette slot the man of the match's name is written in
const MOM_TINT: [u8; 3] = [0x4d, 0x45, 0x2e]; // the list's colour with a quarter of gold over it
/// The Player Ratings cells this frame (left, top, right), the man of the match's name cell.
static mut RATING_CELLS: Vec<(i32, i32, i32)> = Vec::new();
static mut MOM_CELL: Option<(i32, i32, i32, i32)> = None;

/// The man of the match's half of his row (two sides in one list) lit gold, a gold bar at its
/// edge, over the cells of this full frame.
#[allow(static_mut_refs)]
unsafe fn light_man_of_match() {
    let cells = std::mem::take(&mut RATING_CELLS);
    let Some((x1, y1, x2, y2)) = MOM_CELL.take() else { return };
    let mut row: Vec<_> = cells.iter().filter(|c| c.1 == y1).map(|c| (c.0, c.2)).collect();
    row.sort();
    row.dedup();
    let (from, to) = half_of_row(&row, x1, sx(200)).unwrap_or((x1, x2));
    let tint = rgb565(GOLD);
    blend(from, y1, x1 - from, y2 - y1 + 1, |_, _| (tint, 64));
    blend(x2 + 1, y1, to - x2, y2 - y1 + 1, |_, _| (tint, 64));
    fill(from, y1, from + 3, y2 + 1, GOLD);
}

/// The half of a Player Ratings row (cells left to right: left, right) that holds the name at
/// `x1`: the second half starts at the number left of its name, names are wider than `name`.
fn half_of_row(row: &[(i32, i32)], x1: i32, name: i32) -> Option<(i32, i32)> {
    let wide = |c: &(i32, i32)| c.1 - c.0 > name;
    let (first, last) = (row.iter().position(wide)?, row.iter().rposition(wide)?);
    let half = if first == last { row } else if x1 < row[last].0 { &row[..last - 1] } else { &row[last - 1..] };
    Some((half.first()?.0, half.iter().map(|c| c.1).max()?))
}

#[test]
fn man_of_match_half() {
    let row = [(10, 40), (50, 300), (310, 340), (350, 380), (390, 640), (650, 680)];
    assert_eq!(half_of_row(&row, 50, 200), Some((10, 340)));
    assert_eq!(half_of_row(&row, 390, 200), Some((350, 680)));
    assert_eq!(half_of_row(&row[3..], 390, 200), Some((350, 680)));
    assert_eq!(half_of_row(&row[..3], 50, 200), Some((10, 340)));
}

const CLUB_TABLE: usize = 0xae23b4;
const CLUB_COUNT: usize = 0xae235c;
const CLUB_SIZE: usize = 581;

/// A club by its full or short name.
unsafe fn club_named(key: &[u8]) -> Option<usize> {
    if remote::CLIENT {
        return if key.is_empty() { None } else { ask_host(&[b"C", key].concat()).and_then(|v| word(&v)) };
    }
    let (base, count) = (*(CLUB_TABLE as *const usize), *(CLUB_COUNT as *const i32));
    if key.is_empty() || count <= 0 {
        return None;
    }
    // every name once per club table (a table's rows ask each frame: reading all clubs each time
    // made scrolling and switching screens slow)
    // ponytail: names read when the table is first asked; a club renamed later is found by its old name
    static mut INDEX: (usize, i32, Option<std::collections::HashMap<Vec<u8>, usize>>) = (0, 0, None);
    #[allow(static_mut_refs)]
    if INDEX.0 != base || INDEX.1 != count || INDEX.2.is_none() {
        if !readable(base, count as usize * CLUB_SIZE) {
            return None;
        }
        let name = |at: usize, max: usize| std::slice::from_raw_parts(at as *const u8, max).split(|&c| c == 0).next().unwrap().to_vec();
        let mut index = std::collections::HashMap::new();
        // (the first club of a name wins, as the scan did; full names before short ones)
        for k in (0..count as usize).rev() {
            let c = base + k * CLUB_SIZE;
            index.insert(name(c + 0x38, 26), c);
        }
        for k in (0..count as usize).rev() {
            let c = base + k * CLUB_SIZE;
            index.insert(name(c + 4, 51), c);
        }
        index.remove(&b""[..]);
        INDEX = (base, count, Some(index));
    }
    #[allow(static_mut_refs)]
    INDEX.2.as_ref().and_then(|i| i.get(key).copied())
}

/// Text from the game's own bytes (Latin-1) at x: `align` 0 left, 1 centred on x, 2 ending at x.
unsafe fn text_at(x: i32, y1: i32, y2: i32, font: u32, colour: [u8; 3], s: &[u8], align: i32) {
    let c = std::ffi::CString::new(s.iter().copied().filter(|&b| b != 0).collect::<Vec<u8>>()).unwrap();
    let w = game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, c.as_ptr());
    let left = x - w * align / 2;
    game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG)(left, y1, left + w + 4, y2, 0x9, font, rgb(colour) as u32, c.as_ptr(), -1);
}

unsafe fn text_width(font: u32, s: &[u8]) -> i32 {
    let c = std::ffi::CString::new(s.to_vec()).unwrap_or_default();
    game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, c.as_ptr())
}

/// As much of a text as fits in `width` pixels.
unsafe fn clip_text(font: u32, s: &[u8], width: i32) -> Vec<u8> {
    let mut t = s.to_vec();
    while !t.is_empty() {
        let c = std::ffi::CString::new(t.clone()).unwrap_or_default();
        if game::<extern "C" fn(u32, *const i8) -> i32>(TEXT_WIDTH)(font, c.as_ptr()) <= width {
            break;
        }
        t.pop();
    }
    t
}

fn smaller(font: u32, by: u32) -> u32 {
    if font & 0xffff > by { font - by } else { font }
}

/// A match event's icon centred at cx, cy (radius r), by the game's own words in its text
/// ("<name> injured", "sent off", "m/pen"; a goal has none, an own goal "og"), not the colour: a
/// client and the latest scores get other colours. Gives the colour its text goes in.
unsafe fn event_icon(cx: i32, cy: i32, r: i32, name: &[u8]) -> [u8; 3] {
    let has = |w: &[u8]| name.windows(w.len()).any(|x| x == w);
    let sent_off = has(b" sent off");
    // a missed penalty: "m/pen" in the match, "missed" in a shoot-out
    let missed = has(b" m/pen") || has(b" missed");
    let goal = !sent_off && !missed && !has(b" injured");
    let c = event_colour(name);
    if goal {
        ball(cx, cy, r + 1);
    } else if sent_off {
        // a red card
        let hw = r * 7 / 10;
        fill(cx - hw, cy - r, cx + hw, cy + r + 1, [0xd8, 0x30, 0x30]);
    } else if missed {
        // an X, drawn as two thick diagonals
        for d in -r..=r {
            for t in -1..=1 {
                fill(cx + d + t, cy + d, cx + d + t + 1, cy + d + 1, c);
                fill(cx + d + t, cy - d, cx + d + t + 1, cy - d + 1, c);
            }
        }
    } else {
        let a = (r / 3).max(1);
        fill(cx - r, cy - a, cx + r + 1, cy + a + 1, c);
        fill(cx - a, cy - r, cx + a + 1, cy + r + 1, c);
    }
    c
}

/// The colour an event's text goes in, by the game's words: a goal green, a missed penalty grey, a
/// red card the card's red, an injury salmon.
fn event_colour(name: &[u8]) -> [u8; 3] {
    let has = |w: &[u8]| name.windows(w.len()).any(|x| x == w);
    if has(b" sent off") {
        [0xd8, 0x30, 0x30]
    } else if has(b" m/pen") || has(b" missed") {
        [0xa0, 0xa6, 0xae]
    } else if has(b" injured") {
        [0xe0, 0x70, 0x60]
    } else {
        [0x4c, 0xc0, 0x7a]
    }
}

/// A minutes cell of a match event: "14", "19, 58", "pen 82", "og 30", "45+2", and the pairs the
/// game wraps with a comma after the first, "7, pen 25," (a "pen" or "og" before any of the minutes).
fn is_minutes(t: &[u8]) -> bool {
    let mut digits = false;
    for part in t.split(|&c| c == b',') {
        let part = part.trim_ascii();
        let part = part.strip_prefix(b"pen ").or(part.strip_prefix(b"og ")).unwrap_or(part);
        if !part.iter().all(|c| c.is_ascii_digit() || b" +".contains(c)) {
            return false;
        }
        digits |= part.iter().any(u8::is_ascii_digit);
    }
    digits
}

/// A scorer's next minutes joined to the ones on his row: "7, pen 25," + "pen 45" is
/// "7, pen 25, pen 45".
fn join_minutes(minutes: &[u8], next: &[u8]) -> Vec<u8> {
    let m = minutes.trim_ascii_end();
    [m.strip_suffix(b",").unwrap_or(m), b", ", next.trim_ascii()].concat()
}

/// The font a minutes text goes in: the row's a step smaller, and one more step down when it does
/// not fit in `room` pixels (the cell less its margins).
unsafe fn minutes_font(font: u32, room: i32, t: &[u8]) -> u32 {
    let f = smaller(font, 1);
    if text_width(f, t) <= room { f } else { smaller(font, 2) }
}

/// The match screen's draws taken over by cmui (true: not drawn by the game).
#[allow(static_mut_refs)]
unsafe fn match_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, colour: u32, t: &[u8], indented: bool) -> bool {
    let (w, h) = screen();
    // the aggregate: shown in the header band as a slot of its own
    if y1 < h * 15 / 100 && t.starts_with(b"Agg ") {
        MATCH_AGG = t[4..].trim_ascii().to_vec();
        return true;
    }
    // a side of the title: top band, a bit under half the screen wide
    if y1 <= h / 30 && y2 - y1 >= h * 8 / 100 && x2 - x1 > w * 35 / 100 && x2 - x1 < w / 2 {
        if t.iter().all(|c| c.is_ascii_digit()) && !t.is_empty() {
            return false;
        }
        if club_named(t).is_some() {
            REDRAW |= !MATCH;
            MATCH = true;
            SIDES_SEEN = true;
            MATCH_SIDES.retain(|s| s.1 != (x1, y1, x2, y2));
            MATCH_SIDES.push((t.to_vec(), (x1, y1, x2, y2)));
        }
        return false;
    }
    if !MATCH {
        return false;
    }
    if y1 <= h / 30 + 20 && !t.is_empty() && t.iter().all(|c| c.is_ascii_digit()) && ((x2 - x1) - (y2 - y1)).abs() < (y2 - y1) / 3 {
        MATCH_SCORES.retain(|s| s.0 != (x1, y1, x2, y2));
        MATCH_SCORES.push(((x1, y1, x2, y2), t.to_vec()));
        return false;
    }
    // the Latest Scores tab: the day's matches in the game's own rows (a client's friends saw its
    // results and scorers drawn as this match's events, with balls, crosses and red text)
    #[allow(static_mut_refs)]
    if MATCH_TAB == b"Latest Scores" {
        return false;
    }
    // Player Ratings: the man of the match. The game only writes his name in the pale cyan slot
    // (0x723350), next to no difference from the others in the theme: gold, with a star, and his
    // half of the row lit (`match_overlays`). (The ratings in that slot are centred numbers, in a
    // narrower cell.)
    #[allow(static_mut_refs)]
    if MATCH_TAB == b"Player Ratings" {
        if !t.is_empty() {
            RATING_CELLS.push((x1, y1, x2));
        }
    }
    #[allow(static_mut_refs)]
    if MATCH_TAB == b"Player Ratings" && colour & 0xffff == *(MOM_SLOT as *const u16) as u32 && x2 - x1 > sx(200) && !t.is_empty() && !t[0].is_ascii_digit() {
        MOM_CELL = Some((x1, y1, x2, y2));
        fill(x1, y1, x2 + 1, y2 + 1, MOM_TINT);
        text_at(x1 + 4, y1, y2, font, GOLD, t, 0);
        let size = (y2 - y1) * 7 / 10;
        stars_one(x1 + 4 + text_width(font, t) + 8, y1 + (y2 - y1 - size) / 2, size, GOLD);
        return true;
    }
    let content = x2 - x1 > w * 8 / 10;
    // (below the title bar: on the first frame of the next screen MATCH still holds, and its
    // full-width title must not be taken for the band)
    if content && y1 > h * 15 / 100 && y1 < h * 30 / 100 && MATCH_HEAD[0].is_none() {
        MATCH_HEAD[0] = Some(((x1, y1, x2, y2), t.to_vec(), font));
        REDRAW |= !OVERVIEW;
        OVERVIEW = true;
        OVERVIEW_SEEN = true;
        return true;
    }
    let days = [&b"Monday "[..], b"Tuesday ", b"Wednesday ", b"Thursday ", b"Friday ", b"Saturday ", b"Sunday "];
    if y1 < h * 35 / 100 && days.iter().any(|d| t.starts_with(d)) && t.len() > 12 {
        MATCH_HEAD[1] = Some(((x1, y1, x2, y2), t.to_vec(), font));
        return true;
    }
    if y1 < h * 35 / 100 && t.windows(6).any(|w| w == b" / HT ") {
        MATCH_HEAD[2] = Some(((x1, y1, x2, y2), t.to_vec(), font));
        return true;
    }
    // the events: scorer with a ball, green; minute gold at the right
    // minutes: "14", "19, 58", "pen 82", "og 30", "45+2"
    let digits = is_minutes(t);
    // the ground in the bottom bar of a live match ("De Koel, Venlo"); left as the game draws it
    if content && y1 > h * 88 / 100 && !t.is_empty() {
        MATCH_GROUND = t.to_vec();
    }
    // (the League Table tab has the band too, and an old event's row must not take its cells)
    #[allow(static_mut_refs)]
    if !OVERVIEW || MATCH_TAB == b"League Table" {
        return false;
    }
    // (an event's name comes with two leading spaces; its minute on the same row after it)
    if indented && y2 - y1 < 40 && !digits && !t.is_empty() {
        EVENT_ROW = y1;
        let small = smaller(font, 1);
        let name = t.trim_ascii();
        fill(x1, y1, x2 + 2, y2, [0x26, 0x2b, 0x31]);
        let r = (y2 - y1) * 30 / 100;
        let c = event_icon(x1 + 14 + r, (y1 + y2) / 2, r, name);
        text_at(x1 + 28 + 2 * r, y1, y2, small, c, name, 0);
        return true;
    }
    if y1 == EVENT_ROW && y2 - y1 < 40 && digits {
        fill(x1 - 2, y1, x2, y2, [0x26, 0x2b, 0x31]);
        text_at(x2 - 14, y1, y2, minutes_font(font, x2 - x1 - 12, t.trim_ascii()), GOLD, t.trim_ascii(), 2);
        // (one cell a side: the other side's event on the same row comes between this and its
        // third minute, Real Madrid v Ajax 2026-10-29)
        #[allow(static_mut_refs)]
        {
            LAST_MINUTES.retain(|m| (m.0 .0, m.0 .2) != (x1, x2));
            LAST_MINUTES.push(((x1, y1, x2, y2), t.trim_ascii().to_vec(), y2));
        }
        return true;
    }
    // a scorer's third goal on a row of its own under him (the game puts two minutes a row): on
    // his row instead, this one left empty
    #[allow(static_mut_refs)]
    if let Some(((mx1, my1, mx2, my2), minutes, bottom)) = LAST_MINUTES.iter_mut().find(|m| (m.0 .0, m.0 .2) == (x1, x2)) {
        if digits && x1 == *mx1 && x2 == *mx2 && y1 > *bottom && y1 - *bottom <= 8 && y2 - y1 < 40 {
            let joined = join_minutes(minutes, t);
            *minutes = joined;
            *bottom = y2;
            fill(*mx1 - 2, *my1, *mx2, *my2, [0x26, 0x2b, 0x31]);
            text_at(*mx2 - 14, *my1, *my2, minutes_font(font, *mx2 - *mx1 - 12, minutes), GOLD, minutes, 2);
            // (emptied in what is beside it on its row: the plain background when that side has no
            // event on the row, not an event row's colour)
            let beside = pixel_at(x1 - 12, (y1 + y2) / 2).unwrap_or([0x26, 0x2b, 0x31]);
            fill(x1 - 2, y1, x2, y2, beside);
            return true;
        }
    }
    // referee, attendance, weather: a small grey label over a gold value
    for (label, align) in [(&b"Referee - "[..], 0), (b"Attendance - ", 1), (b"Weather - ", 2)] {
        if let Some(value) = t.strip_prefix(label) {
            let value = if align == 1 {
                std::str::from_utf8(value).ok().and_then(|v| v.parse::<i32>().ok()).map_or(value.to_vec(), |v| thousands(v).into_bytes())
            } else {
                value.to_vec()
            };
            let x = [x1 + 40, (x1 + x2) / 2, x2 - 40][align as usize];
            let mid = (y1 + y2) / 2;
            REF_TOP = y1;
            let caps = latin1(&label[..label.len() - 3]).to_uppercase();
            // (a low band has no room for two lines: label and value side by side)
            let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
            if small_screen() && (y2 - y1) * 4 < (tall(font) + tall(smaller(font, 2))) * 5 {
                let (lw, vw) = (text_width(smaller(font, 2), caps.as_bytes()) + 8, text_width(font, &value));
                let left = [x, x - (lw + vw) / 2, x - lw - vw][align as usize];
                text_at(left, y1, y2, smaller(font, 2), GREY, caps.as_bytes(), 0);
                text_at(left + lw, y1, y2, font, GOLD, &value, 0);
                return true;
            }
            text_at(x, y1 + 4, mid, smaller(font, 2), GREY, caps.as_bytes(), align);
            text_at(x, mid - 2, y2 - 4, font, GOLD, &value, align);
            return true;
        }
    }
    false
}

/// The match screen's overlays: a crest by each score, and the header band (league logo tile,
/// competition, ground, date; half time and full time at the right).
#[allow(static_mut_refs)]
unsafe fn match_overlays() {
    if std::mem::take(&mut FULL_FRAME) {
        light_man_of_match();
        if !std::mem::take(&mut SIDES_SEEN) {
            MATCH = false; // another screen
            MATCH_SIDES.clear();
            MATCH_SCORES.clear();
            MATCH_AGG.clear();
        }
        OVERVIEW = std::mem::take(&mut OVERVIEW_SEEN);
    }
    if !MATCH {
        return;
    }
    for (name, (x1, y1, x2, y2)) in MATCH_SIDES.iter() {
        let Some(&((sx1, ..), _)) = MATCH_SCORES.iter().find(|s| s.0 .0 > *x1 && s.0 .2 <= *x2) else { continue };
        let size = (CREST_SIZE as i32).min((y2 - y1) * 75 / 100);
        draw_crest(name, sx1 - 16 - size, (y1 + y2 - size) / 2, size);
    }
    let [Some((band, ground, font)), date, comp] = std::mem::take(&mut MATCH_HEAD) else { return };
    let bottom = date.as_ref().map_or(band.3, |d| d.0 .3).max(comp.as_ref().map_or(band.3, |c| c.0 .3));
    let (x1, y1, x2, y2) = (band.0, band.1, band.2, bottom);
    fill(x1, y1, x2, y2, [0x20, 0x24, 0x2a]);
    // the competition: "First Division" is the league's short name; its full name keys the logo
    let (short, ht) = match comp.as_ref().map(|c| &c.1).and_then(|c| c.windows(6).position(|w| w == b" / HT ").map(|at| (c[..at].to_vec(), c[at + 6..].to_vec()))) {
        Some((s, h)) => (s, h),
        None => (Vec::new(), Vec::new()),
    };
    // the competition of the match itself (league, cup or European tie): the fixture's +0x14 is
    // its record (0x6b bytes, full name +4); the fixture is the match object +4 when live, screen
    // parameter 0x1f afterwards (as draw_match_overview_panel 0x71c270 reads it)
    let record = match_record();
    let live = record.is_some_and(|r| r.1);
    let fixture = match record {
        Some((p0, true, _)) => p0 + 4,
        _ => screen_param(0x1f),
    };
    let comp = if readable(fixture + 0x14, 4) { rd::<usize>(fixture + 0x14) } else { 0 };
    let full = readable(comp, 0x55).then(|| game_text(comp + 4, 51)).filter(|n| !n.is_empty());
    // a live match: its band's first line is the period ("Kick Off"), the ground is in the bottom bar
    let (ground, period) = if live { (MATCH_GROUND.clone(), Some(ground)) } else { (ground, None) };
    let date = date.map(|(r, d, f)| (r, match &period {
        Some(p) => [&d[..], b"   ", &p[..]].concat(),
        None => d,
    }, f));
    let tile = y2 - y1;
    let has_logo = full.as_ref().is_some_and(|f| has_crest(f));
    if has_logo {
        fill(x1, y1, x1 + tile, y2, [0xf2, 0xf2, 0xf2]);
        draw_crest(full.as_ref().unwrap(), x1 + tile / 10, y1 + tile / 10, tile * 8 / 10);
    }
    let tx = x1 + if has_logo { tile } else { 0 } + 20;
    let third = tile / 3;
    // (the League Table tab's band has no date: two lines in it)
    let (line, down) = if date.is_some() { (third, 4) } else { (tile / 2, 1) };
    text_at(tx, y1 + down, y1 + line + down, font, WHITE, full.as_deref().unwrap_or(&short), 0);
    text_at(tx, y1 + line, y1 + 2 * line, smaller(font, 1), LIGHT, &ground, 0);
    if let Some((_, d, df)) = &date {
        text_at(tx, y1 + 2 * third - 4, y2 - 4, *df, GREY, d, 0);
    }
    let ft: Vec<u8> = match MATCH_SCORES.as_slice() {
        [a, b, ..] => [&a.1[..], b"-", &b.1[..]].concat(),
        _ => Vec::new(),
    };
    // a live match: half time once the first half is over, the score as it stands until the end
    // (over when the game says so: "Full Time", or "End of 90 Minutes", period 0x483 of a league
    // match, which the number alone does not tell from extra time)
    let over = period.as_ref().is_some_and(|t| [&b"Full Time"[..], b"End of"].iter().any(|w| t.windows(w.len()).any(|x| x == *w)));
    let state = match_record().map(|(_, live, m)| (live, rd::<i16>(m + 0x4f)));
    // (mike_appguy's labels throughout; the score as it stands is in the boxes at the top, so the
    // full time score is "-" until the match is over, the half time one until the break)
    let (ht, ft) = match state {
        _ if over => (ht, ft),
        Some((true, 0x1ef)) => (b"-".to_vec(), b"-".to_vec()),
        Some((true, p)) if p != 0 && p != 1 => (ht, b"-".to_vec()),
        _ => (ht, ft),
    };
    // after extra time the band goes on past the half time score: "1-0 FT 1-1 ET 2-1" (and
    // penalties): the half time score is its first word, the rest are slots of their own
    let words: Vec<&[u8]> = ht.split(|&c| c == b' ' || c == b'/').filter(|w| !w.is_empty()).collect();
    let ht = words.first().map_or(Vec::new(), |w| w.to_vec());
    let mut slots = vec![(b"HALF TIME".to_vec(), ht), (b"FULL TIME".to_vec(), ft)];
    for pair in words.get(1..).unwrap_or(&[]).chunks(2) {
        if let [name, score] = pair {
            match name.to_ascii_uppercase().as_slice() {
                b"FT" => slots[1].1 = score.to_vec(),
                b"ET" => slots.push((b"EXTRA TIME".to_vec(), score.to_vec())),
                b"PENS" => slots.push((b"PENALTIES".to_vec(), score.to_vec())),
                other => slots.push((other.to_vec(), score.to_vec())),
            }
        }
    }
    if !MATCH_AGG.is_empty() {
        slots.push((b"AGGREGATE".to_vec(), MATCH_AGG.clone()));
        // level after the match and no shoot-out: away goals decided it. The home side's away goals
        // are its aggregate less tonight's; the away side's are tonight's.
        let pair = |s: &[u8]| -> Option<(i32, i32)> {
            let (a, b) = std::str::from_utf8(s).ok()?.split_once('-')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        };
        let mut scores = MATCH_SCORES.clone();
        scores.sort_by_key(|s| s.0 .0);
        let mut sides = MATCH_SIDES.clone();
        sides.sort_by_key(|s| s.1 .0);
        let tonight = match scores.as_slice() {
            [a, b, ..] => pair(&[&a.1[..], b"-", &b.1[..]].concat()),
            _ => None,
        };
        let shootout = slots.iter().any(|s| s.0 == b"PENALTIES");
        if let (Some((ah, aa)), Some((h, a)), [home, away, ..], false, true) = (pair(&MATCH_AGG), tonight, sides.as_slice(), shootout, !live || over) {
            if ah == aa && ah - h != a {
                let winner = if ah - h > a { &home.0 } else { &away.0 };
                slots.push((b"AWAY GOALS".to_vec(), winner.clone()));
            }
        }
    }
    let label = smaller(font, 2);
    let n = slots.len() as i32;
    for (k, (caps, score)) in slots.iter().enumerate() {
        let x = x2 - 30 - (n - 1 - k as i32) * 150;
        text_at(x, y1 + 6, y1 + tile / 2, label, GREY, caps, 2);
        text_at(x, y1 + tile / 2 - 6, y2 - 6, font, WHITE, score, 2);
    }
    key_stats(x1, x2, smaller(font, 1));
}

/// The match record shown (0xd7a bytes): live, screen parameter 0 is the match and its +0x5f the
/// record; after the match, parameter 0x20. (param 0 or 0x20, live, record)
unsafe fn match_record() -> Option<(usize, bool, usize)> {
    let p0 = screen_param(0);
    let (live, m) = if p0 != 0 && readable(p0 + 0x5f, 4) { (true, rd::<usize>(p0 + 0x5f)) } else { (false, screen_param(0x20)) };
    (readable(m, 0xd7a)).then_some((p0, live, m))
}

/// The row of the last match event's name (its minute follows on the same row).
static mut EVENT_ROW: i32 = i32::MIN;
/// The last event row's minute cell, its minutes and the bottom of the rows they span.
static mut LAST_MINUTES: Vec<((i32, i32, i32, i32), Vec<u8>, i32)> = Vec::new();
/// The ground of a live match, from its bottom bar.
static mut MATCH_GROUND: Vec<u8> = Vec::new();
/// The top of the referee row this frame; the title sides' colours (x, RGB565).
static mut REF_TOP: i32 = 0;
static mut SIDE_COLOURS: Vec<(i32, u16)> = Vec::new();

/// mike_appguy's key stats strip over the bottom of the events panel: shots, on target, pass %,
/// tackles %, yellows for each side, and the last-5-minutes possession bar in the club colours.
/// Record offsets from `ui\helper_match.md` (away = home + 0x625).
#[allow(static_mut_refs)]
unsafe fn key_stats(x1: i32, x2: i32, font: u32) {
    let Some((_, _, m)) = match_record() else { return };
    let bottom = std::mem::take(&mut REF_TOP) - 10;
    if bottom <= 0 {
        return;
    }
    let (_, h) = screen();
    let top = bottom - h * 95 / 1050;
    fill(x1, top, x2, bottom, [0x20, 0x24, 0x2a]);
    let byte = |at: usize| rd::<u8>(m + at) as i32;
    let short = |at: usize| rd::<i16>(m + at) as i32;
    let pct = |won: usize, all: usize| if short(all) > 0 { format!("{}%", short(won) * 100 / short(all)) } else { "-".to_string() };
    let third = (x2 - x1) / 3;
    let mid = (top + bottom) / 2;
    let small = smaller(font, 1);
    for (side, sx) in [(0usize, x1), (0x625, x2 - third)] {
        let values = [byte(0x127 + side).to_string(), byte(0x128 + side).to_string(), pct(0x132 + side, 0x130 + side), pct(0x136 + side, 0x134 + side), byte(0x12e + side).to_string()];
        let col = third / 5;
        // short labels when the long ones do not fit their column (800x600)
        let fits = text_width(small, b"Tackles %") <= col - 4;
        let labels = if fits { ["Shots", "On Target", "Pass %", "Tackles %", "Yellows"] } else { ["Sh", "OnT", "Pas%", "Tkl%", "Yel"] };
        for (k, (label, v)) in labels.iter().zip(values).enumerate() {
            let cx = sx + col * k as i32 + col / 2;
            text_at(cx, top + 6, mid, small, GREY, label.as_bytes(), 1);
            text_at(cx, mid - 4, bottom - 6, font, GOLD, v.as_bytes(), 1);
        }
    }
    // the bar: home share M+0x142 (50 when either side has none)
    let p = if byte(0x142) == 0 || byte(0x767) == 0 { 50 } else { byte(0x142).clamp(1, 99) };
    let (bx1, bx2) = (x1 + third + 20, x2 - third - 20);
    text_at((bx1 + bx2) / 2, top + 6, mid - 6, small, GREY, b"Last 5 Mins", 1);
    let (by1, by2) = (mid, mid + (bottom - mid) * 55 / 100);
    let split = bx1 + (bx2 - bx1) * p / 100;
    let colour = |k: usize, fallback: [u8; 3]| {
        let mut sides = SIDE_COLOURS.clone();
        sides.sort();
        sides.get(k).map_or(fallback, |c| { let p = c.1 as u32; [((p >> 11 & 31) * 255 / 31) as u8, ((p >> 5 & 63) * 255 / 63) as u8, ((p & 31) * 255 / 31) as u8] })
    };
    let (home, away) = (colour(0, P1), colour(1, P2));
    fill(bx1, by1, split, by2, home);
    fill(split, by1, bx2, by2, away);
    let ink = |c: [u8; 3]| if c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32 > 1200 { [0x10, 0x12, 0x14] } else { WHITE };
    text_at((bx1 + split) / 2, by1, by2, small, ink(home), format!("{p}%").as_bytes(), 1);
    text_at((split + bx2) / 2, by1, by2, small, ink(away), format!("{}%", 100 - p).as_bytes(), 1);
}

const DRAW_GLYPH: usize = 0x5e3ce0; // gui_draw_glyph(x, y, colour, glyph: w, h, w*h mask bytes)
const GUIO_DRAW: usize = 0x5ee590;
/// The game's four small popup arrows (4x7 and 7x4 bitmaps).
const ARROW_GLYPHS: [usize; 4] = [0x9cd67c, 0x9cd69c, 0x9cd6bc, 0x9cd6dc];

/// gui_draw_glyph from Guio::draw: the popup arrows become smooth chevrons, a bit larger, pointing
/// the way the bitmap points; other glyphs (ticks) as the game draws them.
extern "C" fn glyph(x: i32, y: i32, colour: u32, g: *const u8) {
    let _timed = Timed(1, std::time::Instant::now());
    unsafe {
        static mut SEEN: Vec<usize> = Vec::new();
        #[allow(static_mut_refs)]
        if !SEEN.contains(&(g as usize)) {
            SEEN.push(g as usize);
            log(&format!("glyph {:#x} {}x{} at {x},{y}", g as usize, *g, *g.add(1)));
        }
        if !on(MENU_FIXES) || !ARROW_GLYPHS.contains(&(g as usize)) {
            return game::<extern "C" fn(i32, i32, u32, *const u8)>(DRAW_GLYPH)(x, y, colour, g);
        }
        let (w, h) = (*g as i32, *g.add(1) as i32);
        let mask = |c: i32, r: i32| *g.add(2 + (r * w + c) as usize) != 0;
        let count = |cells: &mut dyn Iterator<Item = (i32, i32)>| cells.filter(|&(c, r)| mask(c, r)).count();
        // which way: the wide end of the triangle is the back
        let dir = if w < h {
            if count(&mut (0..h).map(|r| (0, r))) > count(&mut (0..h).map(|r| (w - 1, r))) { 0 } else { 2 } // right, left
        } else if count(&mut (0..w).map(|c| (c, 0))) > count(&mut (0..w).map(|c| (c, h - 1))) { 1 } else { 3 }; // down, up
        let (cx, cy) = (x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0);
        let s = w.max(h) as f32 * 0.75;
        // (a dark button's arrow light; the arrows on a white or club-coloured title band stay)
        let dark = |p: [u8; 3]| (p[0] as i32 + p[1] as i32 + p[2] as i32) < 330;
        let c565 = if on(MODERN_BUTTONS) && pixel_at(x, y).is_some_and(dark) { rgb565(LIGHT) } else { colour as u16 };
        // a chevron: two strokes meeting at the tip
        let (tip, a, b) = match dir {
            0 => ((cx + s * 0.45, cy), (cx - s * 0.45, cy - s), (cx - s * 0.45, cy + s)),
            2 => ((cx - s * 0.45, cy), (cx + s * 0.45, cy - s), (cx + s * 0.45, cy + s)),
            1 => ((cx, cy + s * 0.45), (cx - s, cy - s * 0.45), (cx + s, cy - s * 0.45)),
            _ => ((cx, cy - s * 0.45), (cx - s, cy + s * 0.45), (cx + s, cy + s * 0.45)),
        };
        let seg = |p: (f32, f32), q: (f32, f32), x: f32, y: f32| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let t = (((x - p.0) * dx + (y - p.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((x - p.0 - t * dx).powi(2) + (y - p.1 - t * dy).powi(2)).sqrt()
        };
        let (bx, by) = ((cx - s - 2.0) as i32, (cy - s - 2.0) as i32);
        let size = (2.0 * s + 5.0) as i32;
        blend(bx, by, size, size, |col, row| {
            let (px, py) = (bx as f32 + col as f32 + 0.5, by as f32 + row as f32 + 0.5);
            let d = seg(tip, a, px, py).min(seg(tip, b, px, py));
            (c565, ((1.9 - d).clamp(0.0, 1.0) * 255.0) as u32)
        });
    }
}

/// The buttons drawn this frame (for their popup arrows).
static mut BUTTONS: Vec<(i32, i32, i32, i32)> = Vec::new();

/// The small two-tone arrow some buttons carry at their right edge (the squad screen's Tactics,
/// Training...: not drawn through gui_draw_glyph) becomes a chevron like the dropdowns': found
/// by its pixels at the edge, covered with the button's colour.
#[allow(static_mut_refs)]
unsafe fn button_arrows() {
    for (x1, y1, x2, y2) in std::mem::take(&mut BUTTONS) {
        let _ = (x1, y1);
        let cy = (y1 + y2) / 2;
        // x2 - 16 .. x2 - 1, cy - 6 ..= cy + 6 in one lock
        let Some(area) = region(x2 - 16, cy - 6, 16, 13) else { continue };
        let at = |x: i32, y: i32| area[((y - (cy - 6)) * 16 + x - (x2 - 16)) as usize];
        let bg = at(x2 - 16, cy);
        let lum = |c: [u8; 3]| (c[0] as i32 * 3 + c[1] as i32 * 6 + c[2] as i32) / 10;
        let (mut hits, mut left, mut right, mut top, mut bottom) = (0, i32::MAX, 0, i32::MAX, 0);
        for y in cy - 6..=cy + 6 {
            for x in x2 - 13..x2 - 1 {
                if lum(at(x, y)) > lum(bg) + 50 {
                    hits += 1;
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
        // an arrow: small, and clear of the scan's top and bottom rows (a line through it is not)
        if !(6..=30).contains(&hits) || right - left > 5 || top <= cy - 5 || bottom >= cy + 5 {
            continue; // no arrow there (text, an edge, a line, nothing)
        }
        fill(left - 1, cy - 6, right + 2, cy + 7, bg);
        let s = 5.0f32;
        let (cx, cyf) = ((left + right) as f32 / 2.0, cy as f32 + 0.5);
        let (tip, a, b) = ((cx + s * 0.45, cyf), (cx - s * 0.45, cyf - s), (cx - s * 0.45, cyf + s));
        let seg = |p: (f32, f32), q: (f32, f32), x: f32, y: f32| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let t = (((x - p.0) * dx + (y - p.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((x - p.0 - t * dx).powi(2) + (y - p.1 - t * dy).powi(2)).sqrt()
        };
        let colour = rgb565(LIGHT);
        let (bx, by) = ((cx - s - 2.0) as i32, (cyf - s - 2.0) as i32);
        blend(bx, by, 15, 15, |col, row| {
            let (px, py) = (bx as f32 + col as f32 + 0.5, by as f32 + row as f32 + 0.5);
            let d = seg(tip, a, px, py).min(seg(tip, b, px, py));
            (colour, ((1.7 - d).clamp(0.0, 1.0) * 255.0) as u32)
        });
    }
}

/// A football: white with a black pentagon in the middle and five half patches at the edge.
unsafe fn ball(cx: i32, cy: i32, r: i32) {
    let (white, black) = (rgb565([0xf4, 0xf4, 0xf4]), rgb565([0x14, 0x16, 0x18]));
    let pentagon = |x: f32, y: f32, px: f32, py: f32, size: f32, turn: f32| {
        // inside a regular pentagon around (px, py): the largest projection on its five normals
        (0..5).all(|k| {
            let a = turn + k as f32 * std::f32::consts::TAU / 5.0;
            (x - px) * a.cos() + (y - py) * a.sin() <= size
        })
    };
    let rf = r as f32;
    blend(cx - r - 1, cy - r - 1, 2 * r + 3, 2 * r + 3, |col, row| {
        let (mut inside, mut dark) = (0u32, 0u32);
        for s in 0..9 {
            let x = (col - r - 1) as f32 + (s % 3) as f32 / 3.0 + 1.0 / 6.0;
            let y = (row - r - 1) as f32 + (s / 3) as f32 / 3.0 + 1.0 / 6.0;
            let d = (x * x + y * y).sqrt();
            if d > rf {
                continue;
            }
            inside += 1;
            let up = -std::f32::consts::FRAC_PI_2;
            let mut patch = d > rf - 1.2 || pentagon(x, y, 0.0, 0.0, rf * 0.3, up + std::f32::consts::PI / 5.0);
            for k in 0..5 {
                let a = up + k as f32 * std::f32::consts::TAU / 5.0;
                patch |= pentagon(x, y, a.cos() * rf * 0.95, a.sin() * rf * 0.95, rf * 0.28, a);
            }
            dark += patch as u32;
        }
        let colour = if dark * 2 > inside { black } else { white };
        (colour, inside * 255 / 9)
    });
}

// The player compare screen ("Wim Jacobs ~ Paul van Boekel") as mike_appguy has it, drawn over
// the game's own (which shows only the differences) in the flip pass.
/// Title box and text this frame; the game's lines kept for it: "Age 37. Dutch ~ Age 26. Dutch",
/// "GK ~ D R", the season rows (label, "33 - 21") and the attributes it masks ("-").
static mut COMPARE: Option<((i32, i32, i32, i32), Vec<u8>, u32)> = None;
static mut COMPARE_LINES: Vec<Vec<u8>> = Vec::new();
static mut COMPARE_SEASON: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
static mut COMPARE_LABEL: (i32, Vec<u8>) = (i32::MIN, Vec::new());
static mut COMPARE_MASKED: Vec<Vec<u8>> = Vec::new();
const ATTRIBUTE_NAMES: [&str; 31] = [
    "Acceleration", "Aggression", "Agility", "Anticipation", "Balance", "Bravery", "Creativity",
    "Crossing", "Decisions", "Determination", "Dribbling", "Finishing", "Flair", "Handling",
    "Heading", "Influence", "Jumping", "Long Shots", "Marking", "Off The Ball", "Pace", "Passing",
    "Positioning", "Reflexes", "Set Pieces", "Stamina", "Strength", "Tackling", "Teamwork",
    "Technique", "Work Rate",
];
const PLAYER_ATTRIBUTE: usize = 0x543540; // (staff, player, 1..31) -> 1..20, cdecl

/// A player's 31 attributes (1..20) as the game gives them, the host's in a network client (0
/// while not known).
unsafe fn attributes(staff: usize) -> [u8; 31] {
    let mut out = [0u8; 31];
    if remote::CLIENT {
        for (o, b) in out.iter_mut().zip(ask_host(&key(b'A', &[staff as u32])).unwrap_or_default()) {
            *o = b;
        }
        return out;
    }
    let player = rd::<usize>(staff + 0x61);
    for (i, o) in out.iter_mut().enumerate() {
        *o = game::<extern "C" fn(usize, usize, i32) -> u32>(PLAYER_ATTRIBUTE)(staff, player, i as i32 + 1) as u8;
    }
    out
}
/// The radar's groups: name and attributes (indexes into ATTRIBUTE_NAMES).
const RADAR: [(&str, &[usize]); 8] = [
    ("Defending", &[18, 27, 22]), ("Physical", &[26, 25, 4]), ("Speed", &[20, 0, 2]),
    ("Vision", &[6, 21, 12]), ("Attacking", &[11, 19, 10]), ("Technique", &[29, 7, 24]),
    ("Aerial", &[14, 16]), ("Mental", &[8, 3, 9, 28, 30]),
];
const P1: [u8; 3] = [0x4c, 0xc0, 0x7a];
const P2: [u8; 3] = [0xe0, 0x60, 0x58];

/// The name a staff member goes by: the common name, else first and second.
unsafe fn staff_name(staff: usize) -> Vec<u8> {
    let name = |at: usize| {
        let p = rd::<usize>(staff + at);
        if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
    };
    let common = name(0xc);
    if !common.is_empty() { common } else { [name(4), b" ".to_vec(), name(8)].concat() }
}

const STAFF_TABLE: usize = 0xae23bc;
const STAFF_COUNT: usize = 0xae2364;
const SCOUT_MANAGER: usize = 0xb74448;
static mut COMPARE_FOUND: Option<(Vec<u8>, usize, usize)> = None;

/// The two players of the compare title, found by name among the players in the staff table
/// (the screen keeps them elsewhere); kept for the title.
#[allow(static_mut_refs)]
unsafe fn compare_players(title: &[u8]) -> Option<(usize, usize)> {
    if let Some((t, a, b)) = &COMPARE_FOUND {
        if t.as_slice() == title {
            return Some((*a, *b));
        }
    }
    let at = title.windows(3).position(|w| w == b" ~ ")?;
    let (a, b) = (title[..at].trim_ascii(), title[at + 3..].trim_ascii());
    // the pair the human set (two players can share a name): SCOUT_MANAGER::get_compare_player
    // 0x7f7920 keeps them per human manager, +0x68 / +0xa8, indexed by his staff id's low byte
    // less the staff count's, plus 16
    let human = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel());
    let scouts = rd::<usize>(SCOUT_MANAGER);
    if readable(human, 4) && readable(scouts, 0xe8) {
        let k = rd::<u8>(human).wrapping_sub(rd::<u8>(STAFF_COUNT)).wrapping_add(0x10) as i8;
        if (0..16).contains(&k) {
            let (p, q) = (rd::<usize>(scouts + 0x68 + 4 * k as usize), rd::<usize>(scouts + 0xa8 + 4 * k as usize));
            if readable(p, 0x6e) && readable(q, 0x6e) && staff_name(p) == a && staff_name(q) == b {
                COMPARE_FOUND = Some((title.to_vec(), p, q));
                return Some((p, q));
            }
        }
    }
    if remote::CLIENT {
        let v = ask_host(&[b"M", title].concat())?;
        return Some((word(&v)?, word(v.get(4..)?)?));
    }
    let (base, count) = (*(STAFF_TABLE as *const usize), *(STAFF_COUNT as *const i32));
    if count <= 0 || !readable(base, count as usize * 0x6e) {
        return None;
    }
    let player = |s: usize| ((s + 0x61) as *const usize).read_unaligned();
    let find = |n: &[u8]| (0..count as usize).map(|k| base + k * 0x6e).find(|&s| readable(player(s), 70) && staff_name(s) == n);
    let found = (find(a)?, find(b)?);
    COMPARE_FOUND = Some((title.to_vec(), found.0, found.1));
    Some(found)
}

/// Keeps what the compare screen's own lines say (called for every text on that screen).
#[allow(static_mut_refs)]
unsafe fn compare_text(y1: i32, t: &[u8]) {
    if t.windows(3).any(|w| w == b" ~ ") {
        COMPARE_LINES.push(t.to_vec());
    } else if ["Goals", "Appearances", "Assists", "Man of Match", "Avg. Rating"].iter().any(|l| l.as_bytes() == t) {
        COMPARE_LABEL = (y1, t.to_vec());
    } else if y1 == COMPARE_LABEL.0 && t.windows(3).any(|w| w == b" - ") {
        COMPARE_SEASON.push((COMPARE_LABEL.1.clone(), t.to_vec()));
    } else if ATTRIBUTE_NAMES.iter().any(|n| n.as_bytes() == t) {
        COMPARE_LABEL = (y1, t.to_vec());
    } else if y1 == COMPARE_LABEL.0 && t == b"-" {
        COMPARE_MASKED.push(COMPARE_LABEL.1.clone());
    }
}

fn mix(a: [u8; 3], b: [u8; 3], part: u32) -> [u8; 3] {
    [0, 1, 2].map(|i| ((a[i] as u32 * part + b[i] as u32 * (100 - part)) / 100) as u8)
}

/// A filled polygon (even-odd), blended at `alpha`.
unsafe fn polygon(points: &[(f32, f32)], c: [u8; 3], alpha: u32) {
    let (x1, y1) = (points.iter().map(|p| p.0).fold(f32::MAX, f32::min) as i32, points.iter().map(|p| p.1).fold(f32::MAX, f32::min) as i32);
    let (x2, y2) = (points.iter().map(|p| p.0).fold(f32::MIN, f32::max) as i32, points.iter().map(|p| p.1).fold(f32::MIN, f32::max) as i32);
    let colour = rgb565(c);
    blend(x1, y1, x2 - x1 + 1, y2 - y1 + 1, |col, row| {
        let (x, y) = ((x1 + col) as f32 + 0.5, (y1 + row) as f32 + 0.5);
        let mut inside = false;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                inside = !inside;
            }
        }
        (colour, if inside { alpha } else { 0 })
    });
}

#[allow(static_mut_refs)]
unsafe fn compare_overlay() {
    let lines = std::mem::take(&mut COMPARE_LINES);
    let season = std::mem::take(&mut COMPARE_SEASON);
    let masked = std::mem::take(&mut COMPARE_MASKED);
    let Some(((tx1, ty1, tx2, ty2), title, font)) = COMPARE.take() else { return };
    let Some((s1, s2)) = compare_players(&title) else {
        static mut SAID: bool = false;
        if !std::mem::replace(&mut SAID, true) {
            let params: Vec<String> = (0..4).map(|k| format!("{:#x}", screen_param(k))).collect();
            log(&format!("compare: no players for {:?}, screen params {params:?}", latin1(&title)));
        }
        return;
    };
    let (_, h) = screen();
    // the game's font numbers go by size; a small screen (800x600) two sizes down
    let shrink = if h < 800 { 2 } else { 0 };
    let f = |n: u32| font & !0xffff | n.saturating_sub(shrink).max(1);
    let (l, r, top, bottom) = (tx1, tx2, ty2 + 12, h * 92 / 100);
    let draw_line = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_LINE);
    // the header: crest, name, club and "Age 37. Dutch | GK" for each, the second mirrored
    fill(tx1, ty1, tx2, ty2, [0x22, 0x26, 0x2c]);
    fill(l, top - 8, r, bottom, BACKGROUND);
    let split = |line: Option<&Vec<u8>>, k: usize| -> Vec<u8> {
        line.and_then(|t| t.split(|&c| c == b'~').nth(k).map(|p| p.trim_ascii().to_vec())).unwrap_or_default()
    };
    let ages = lines.iter().find(|t| t.starts_with(b"Age "));
    let spots = lines.iter().find(|t| t.split(|&c| c == b'~').all(|p| is_position(p.trim_ascii())));
    let bar = ty2 - ty1;
    let size = bar * 70 / 100;
    for (k, staff) in [s1, s2].into_iter().enumerate() {
        let club = rd::<usize>(staff + 0x39);
        let club_name = if readable(club, 60) { game_text(club + 4, 51) } else { Vec::new() };
        // "Age 37. Dutch" and "GK": "Age 37 | Dutch | GK"
        let age = split(ages, k).split(|&c| c == b'.').map(|p| p.trim_ascii().to_vec()).collect::<Vec<_>>().join(&b" | "[..]);
        let info = [age, b" | ".to_vec(), split(spots, k)].concat();
        let (crest_x, text_x, align) = if k == 0 { (tx1 + 20, tx1 + 40 + size, 0) } else { (tx2 - 20 - size, tx2 - 40 - size, 2) };
        draw_crest(&club_name, crest_x, ty1 + (bar - size) / 2, size);
        text_at(text_x, ty1 + 6, ty1 + bar * 45 / 100, f(6), WHITE, &staff_name(staff), align);
        text_at(text_x, ty1 + bar * 42 / 100, ty1 + bar * 68 / 100, f(3), LIGHT, &club_name, align);
        text_at(text_x, ty1 + bar * 66 / 100, ty2 - 6, f(2), GREY, &info, align);
    }
    fill(tx1, ty2 - 3, (tx1 + tx2) / 2, ty2, P1);
    fill((tx1 + tx2) / 2, ty2 - 3, tx2, ty2, P2);
    // the attributes: three columns of label, P1 and P2 boxes coloured by level, the lower dimmed
    let (a1, a2) = (attributes(s1), attributes(s2));
    let (v1, v2): (Vec<i32>, Vec<i32>) = (0..31).map(|i| (a1[i] as i32, a2[i] as i32)).unzip();
    let (aw, ah) = ((r - l) * 66 / 100, (bottom - top) * 58 / 100);
    let head = ah / 13;
    fill(l, top, l + aw, top + ah, AREA);
    text_at(l + 14, top, top + head, f(2), GOLD, b"ATTRIBUTES", 0);
    let rh = (ah - head - 8) / 11;
    let cw = aw / 3;
    // one font for the names: the largest the longest fits in ("Determination" at 800x600)
    let room = cw * 55 / 100 - 16;
    let nf = [f(3), f(2), f(1)].into_iter().find(|&nf| ATTRIBUTE_NAMES.iter().all(|n| text_width(nf, n.as_bytes()) <= room)).unwrap_or(f(1));
    for (i, name) in ATTRIBUTE_NAMES.iter().enumerate() {
        let (col, row) = ((i / 11) as i32, (i % 11) as i32);
        let (x, y) = (l + col * cw, top + head + 4 + row * rh);
        if row == 0 {
            text_at(x + cw * 64 / 100, top + head - 8, top + head + 6, f(2), P1, b"P1", 1);
            text_at(x + cw * 84 / 100, top + head - 8, top + head + 6, f(2), P2, b"P2", 1);
        }
        let shown = if text_width(nf, name.as_bytes()) <= room {
            name.as_bytes().to_vec()
        } else {
            [clip_text(nf, name.as_bytes(), room - text_width(nf, b".")), b".".to_vec()].concat() // "Determin."
        };
        text_at(x + 14, y, y + rh, nf, LIGHT, &shown, 0);
        let hidden = masked.iter().any(|m| m.as_slice() == name.as_bytes());
        for (k, (v, other)) in [(v1[i], v2[i]), (v2[i], v1[i])].into_iter().enumerate() {
            let bx = x + cw * (56 + 20 * k as i32) / 100;
            let (b1, b2) = (bx, bx + cw * 17 / 100);
            let c = if hidden || !(1..=20).contains(&v) { [0x3a, 0x3f, 0x47] } else if v < other { mix(level_colour(v), AREA, 45) } else { level_colour(v) };
            fill(b1, y + 3, b2, y + rh - 3, c);
            let label = if hidden || !(1..=20).contains(&v) { "-".to_string() } else { v.to_string() };
            text_at((b1 + b2) / 2, y, y + rh, f(3), WHITE, label.as_bytes(), 1);
        }
    }
    // the radar: eight groups, P1 green over P2 red; their values under it
    let (rx1, rx2) = (l + aw + 16, r);
    fill(rx1, top, rx2, top + ah, AREA);
    text_at(rx1 + 14, top, top + head, f(2), GOLD, b"ATTRIBUTE RADAR", 0);
    let group = |vals: &[i32], g: &[usize]| g.iter().map(|&i| vals[i].clamp(0, 20) as f32).sum::<f32>() / g.len() as f32;
    let radius = ((rx2 - rx1) as f32 * 0.26).min(ah as f32 * 0.27);
    let (cx, cy) = (((rx1 + rx2) / 2) as f32, (top + head) as f32 + radius + 24.0);
    let corner = |k: usize, v: f32| {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 4.0;
        (cx + a.cos() * radius * v / 20.0, cy + a.sin() * radius * v / 20.0)
    };
    let grid = rgb([0x4a, 0x50, 0x5a]) as u32;
    for ring in [10.0, 15.0, 20.0] {
        for k in 0..8 {
            let (a, b) = (corner(k, ring), corner((k + 1) % 8, ring));
            draw_line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32, 2, grid);
        }
    }
    for (k, (name, _)) in RADAR.iter().enumerate() {
        let (ax, ay) = corner(k, 20.0);
        draw_line(cx as i32, cy as i32, ax as i32, ay as i32, 2, grid);
        let (lx, ly) = corner(k, 24.5);
        let align = if (lx - cx).abs() < 4.0 { 1 } else if lx < cx { 2 } else { 0 };
        text_at(lx as i32, ly as i32 - 12, ly as i32 + 12, f(2), GREY, name.as_bytes(), align);
    }
    for (vals, c) in [(&v2, P2), (&v1, P1)] {
        let points: Vec<(f32, f32)> = RADAR.iter().enumerate().map(|(k, (_, g))| corner(k, group(vals, g))).collect();
        polygon(&points, c, 70);
        for k in 0..8 {
            let (a, b) = (points[k], points[(k + 1) % 8]);
            draw_line(a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32, 2, rgb(c) as u32);
        }
    }
    let ty = (cy + radius + 34.0) as i32;
    let trow = ((top + ah - ty - 8) / 5).max(10);
    // two columns of "name  P1  P2": the values measured, right-aligned at the column's end; whole
    // numbers where a narrow column (800x600) leaves the names too little room for decimals
    let cw = (rx2 - rx1 - 14) / 2;
    let fits = |s: &[u8]| cw - 2 * (text_width(f(2), s) + 8) - 6 >= text_width(f(2), b"Physical");
    let decimals = if fits(b"20.0") { 1 } else { 0 };
    let vw = text_width(f(2), if decimals == 1 { b"20.0" } else { b"20" }) + 8;
    for (k, (name, g)) in RADAR.iter().enumerate() {
        let (col, row) = ((k / 4) as i32, (k % 4) as i32);
        let (x, y) = (rx1 + 14 + col * cw, ty + row * trow);
        let room = cw - 2 * vw - 6;
        let shown = if text_width(f(2), name.as_bytes()) <= room {
            name.as_bytes().to_vec()
        } else {
            [clip_text(f(2), name.as_bytes(), room - text_width(f(2), b".")), b".".to_vec()].concat()
        };
        text_at(x, y, y + trow, f(2), GREY, &shown, 0);
        text_at(x + cw - 6 - vw, y, y + trow, f(2), P1, format!("{:.*}", decimals, group(&v1, g)).as_bytes(), 2);
        text_at(x + cw - 6, y, y + trow, f(2), P2, format!("{:.*}", decimals, group(&v2, g)).as_bytes(), 2);
    }
    let ly = ty + 4 * trow;
    text_at((rx1 + rx2) / 2 - 8, ly, ly + trow, f(2), P1, &[b"P1 ".as_slice(), &staff_name(s1)].concat(), 2);
    text_at((rx1 + rx2) / 2 + 8, ly, ly + trow, f(2), P2, &[b"P2 ".as_slice(), &staff_name(s2)].concat(), 0);
    // positional suitability: a pitch and the best spots for each
    let (by1, by2) = (top + ah + 14, bottom);
    let pw = (r - l) * 66 / 100;
    fill(l, by1, l + pw, by2, AREA);
    text_at(l + 14, by1, by1 + head, f(2), GOLD, b"POSITIONAL SUITABILITY", 0);
    for (k, (staff, c)) in [(s1, P1), (s2, P2)].into_iter().enumerate() {
        let x = l + k as i32 * pw / 2 + 14;
        let player = rd::<usize>(staff + 0x61);
        if !readable(player, 27) {
            continue;
        }
        text_at(x, by1 + head, by1 + 2 * head, f(3), c, &staff_name(staff), 0);
        let (px1, py1) = (x, by1 + 2 * head + 6);
        let px2 = x + pw / 2 * 50 / 100;
        let py2 = (py1 + (px2 - px1) * 64 / 100).min(by2 - 10);
        fit_pitch(player, px1, py1, px2, py2);
        let rating = |at: usize| rd::<i8>(player + at) as i32;
        // (a keeper has no side)
        let mut fits: Vec<(i32, &str)> = SPOTS.iter().map(|s| (if s.0 == GK { rating(GK) } else { rating(s.0).min(rating(s.1)) }, s.4)).filter(|f| f.0 >= 10).collect();
        fits.sort_by(|a, b| b.0.cmp(&a.0));
        let row = (py2 - py1) / 6;
        for (i, (fit, spot)) in fits.iter().take(6).enumerate() {
            let (word, wc) = fit_word(*fit);
            let y = py1 + i as i32 * row;
            text_at(px2 + 14, y, y + row, f(2), wc, format!("{spot} {word}").as_bytes(), 0);
        }
    }
    // under the pitches: what the club's staff say
    if on(COMPARE_REPORT) {
        let views = if remote::CLIENT { ask_host(&key(b'D', &[s1 as u32, s2 as u32])).unwrap_or_default() } else { assistant::staff_views(channel() as i16, s1, s2) };
        let y = by1 + 2 * head + 6 + (pw / 2 * 50 / 100) * 64 / 100 + 8;
        let line = ((by2 - y - 6) / 3).min(head);
        if line >= 12 {
            for (k, view) in views.split(|&c| c == b'\n').take(3).enumerate() {
                let y = y + k as i32 * line;
                // (too long for its line: the sentence ends at its first half, not mid-way)
                let cut = view.windows(2).rposition(|w| w == b", ").filter(|_| small_screen() && text_width(f(2), view) > pw - 28);
                let short = [&view[..cut.unwrap_or(view.len())], if cut.is_some() { b"." } else { b"" }].concat();
                assistant::report_text(l + 14, y, l + pw - 14, y + line, f(2), line, &short);
            }
        }
    }
    // the current season, from the game's own rows ("33 - 21")
    let sx1 = l + pw + 16;
    fill(sx1, by1, r, by2, AREA);
    text_at(sx1 + 14, by1, by1 + head, f(2), GOLD, b"CURRENT SEASON", 0);
    let sw = r - sx1;
    text_at(sx1 + sw * 64 / 100, by1 + head, by1 + 2 * head, f(2), P1, b"P1", 1);
    text_at(sx1 + sw * 86 / 100, by1 + head, by1 + 2 * head, f(2), P2, b"P2", 1);
    let srow = (by2 - by1 - 2 * head - 10) / 5;
    let names = [("Appearances", "Appearances"), ("Goals", "Goals"), ("Assists", "Assists"), ("Man of Match", "Man of the Match"), ("Avg. Rating", "Average rating")];
    for (i, (key, shown)) in names.iter().enumerate() {
        let y = by1 + 2 * head + 4 + i as i32 * srow;
        text_at(sx1 + 14, y, y + srow, f(3), LIGHT, &clip_text(f(3), shown.as_bytes(), sw * 52 / 100), 0);
        let Some((_, v)) = season.iter().find(|s| s.0.as_slice() == key.as_bytes()) else { continue };
        // (split at the first " - ": a player without a rating has "-" of his own, "- - 7.12")
        let parts: Vec<&[u8]> = match v.windows(3).position(|w| w == b" - ") {
            Some(at) => vec![v[..at].trim_ascii(), v[at + 3..].trim_ascii()],
            None => v.split(|&c| c == b'-').map(|p| p.trim_ascii()).collect(),
        };
        for (k, p) in parts.iter().take(2).enumerate() {
            let x = sx1 + sw * (64 + 22 * k as i32) / 100;
            if i == 4 {
                let rating = std::str::from_utf8(p).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let c = if rating >= 7.5 { [0x3e, 0xa8, 0x6a] } else if rating >= 7.0 { [0xc8, 0xa8, 0x40] } else { [0x4a, 0x50, 0x5a] };
                pill(x - sw * 9 / 100, y + 4, x + sw * 9 / 100, y + srow - 4, c);
            }
            text_at(x, y, y + srow, f(3), if i == 4 { WHITE } else { [P1, P2][k] }, p, 1);
        }
    }
}

// mike_appguy's "Ability" view of the player search: an 8th View item (bit 0x4000, kept together
// with Other's 0x20 so the game builds the Other layout), whose Nat. / Caps / Form / Morale cells
// become Club / Pos / Current / Potential (stars) and I.Gls / Cond. stay empty. Addresses from
// `ui\helper_search.md`.
const ADD_ITEM: usize = 0x55f990; // Display::add_item, thiscall, 18 stack args
const WIDGET_INIT: usize = 0x5ee150; // Widget::init, thiscall, 21 stack args, its one caller add_item
/// Dropdown rows in this font at 1680 wide and more: a popup's rows, width and height all come
/// from the font (0x5ee1d2), in real pixels, so they were sized for 800x600 (fonts 0 and 1).
const DROPDOWN_FONT: i32 = 3;
const DROPDOWN_ROOM: usize = 0x5ee341; // lea eax, [eax + edi + 6]: a row's height over its font
const DROPDOWN_PADDING: i32 = 12;

/// A row of a popup area (its owner widget at +0x20e): a larger font, while the popup stays on the
/// screen (the game asserts when one doesn't, so the rows past that stay small).
#[allow(clippy::too_many_arguments)]
extern "thiscall" fn widget_init(
    this: usize, areas: usize, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32, a10: u32,
    a11: u32, a12: u32, a13: u32, a14: u32, a15: u32, font: i32, a17: u32, a18: u32, a19: u32, a20: u32,
    a21: u32, area: i32,
) -> u32 {
    unsafe {
        let mut font = font;
        let a = areas + (area as i16 as usize) * 0xbf1;
        if area as i16 != -1 && ((a + 0x20e) as *const i16).read_unaligned() != -1 {
            // (by the screen the rows are for: a host builds its clients' screens)
            let viewer = comp::viewer_height();
            let big = viewer >= 1050;
            // (the font comes from the host on a client, the rows are measured there)
            let larger = if !remote::CLIENT && big && font & 0xffff < DROPDOWN_FONT { font & !0xffff | DROPDOWN_FONT } else { font };
            let height = ((a + 0xc) as *const i32).read_unaligned();
            let tall = game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)(larger & 0xffff) + DROPDOWN_PADDING;
            let room = big && height + tall < viewer - 40;
            if room {
                font = larger;
            }
            static mut WRITABLE: bool = false;
            if !std::mem::replace(&mut *(&raw mut WRITABLE), true) {
                let mut old = 0;
                VirtualProtect(DROPDOWN_ROOM as *mut u8, 1, 0x40, &mut old);
            }
            *(DROPDOWN_ROOM as *mut u8) = if room { DROPDOWN_PADDING as u8 } else { 6 };
        }
        type Init = extern "thiscall" fn(usize, usize, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, i32, u32, u32, u32, u32, u32, i32) -> u32;
        game::<Init>(WIDGET_INIT)(this, areas, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, font, a17, a18, a19, a20, a21, area)
    }
}
const OTHER_ITEM_CALL: usize = 0x80a774; // the add_item call of the View popup's "Other"
const SEARCH_INPUT: usize = 0x80eb00; // search_screens_handle_player_search_input(widget), cdecl
const SEARCH_VIEW: usize = 0xdd6cc8; // u32* to the current tab's view bits
const FORMAT_POSITIONS: usize = 0x53ee50; // database_format_player_positions(player, buf, size, style)

/// A player's positions as the game writes them ("D/DM RC"), the host's words in a network client.
unsafe fn positions_text(staff: usize) -> Vec<u8> {
    if remote::CLIENT {
        return ask_host(&key(b'O', &[staff as u32])).unwrap_or_default();
    }
    let player = if readable(staff + 0x61, 4) { rd::<usize>(staff + 0x61) } else { 0 };
    if !readable(player, 27) {
        return Vec::new();
    }
    let mut buf = [0u8; 0xc8];
    game::<extern "C" fn(usize, *mut u8, i32, i32) -> i32>(FORMAT_POSITIONS)(player, buf.as_mut_ptr(), 0xc8, 1);
    buf.split(|&c| c == 0).next().unwrap().to_vec()
}
const ABILITY_VIEW: u32 = 0x4000;
static mut SEARCH_INPUT_ORIG: usize = 0;
static ABILITY_TEXT: &[u8] = b"  Ability\0";
/// The search screen (its title) in the last full redraw; seen in this one.
static mut SEARCH: bool = false;
static mut SEARCH_SEEN: bool = false;
/// The Other view's header columns this screen (label, x1, x2) and the row's staff (y, Staff*).
static mut SEARCH_COLS: Vec<(&'static [u8], i32, i32)> = Vec::new();
static mut SEARCH_ROW: (i32, usize) = (i32::MIN, 0);
static mut NAMES: Option<std::collections::HashMap<Vec<u8>, usize>> = None;

/// The current search tab's view bits (from the session, screen parameter 0).
unsafe fn search_view_bits() -> Option<*mut u32> {
    let s = screen_param(0);
    if !readable(s, 1) {
        return None;
    }
    let p = s + rd::<u8>(s) as usize * 0x2b0 + 0x294;
    readable(p, 4).then_some(p as *mut u32)
}

/// The View popup's "Other" item, then "Ability" after it (ticked instead of Other when on).
#[allow(clippy::too_many_arguments)]
extern "thiscall" fn other_item(
    this: usize, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32,
    a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32, a16: u32, a17: u32, a18: u32,
) -> u32 {
    unsafe {
        type AddItem = extern "thiscall" fn(usize, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) -> u32;
        let add = game::<AddItem>(ADD_ITEM);
        let ability = search_view_bits().is_some_and(|f| *f & ABILITY_VIEW != 0);
        let r = add(this, if ability { 2 } else { a1 }, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17, a18);
        if on(SEARCH_ABILITY) {
            add(this, if ability { 0x2002 } else { 2 }, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, ABILITY_TEXT.as_ptr() as u32, a15, a16, ABILITY_VIEW, a18);
        }
        r
    }
}

/// The search screen's input: the Ability item sets its bit (with Other's); any other view clears it.
extern "C" fn search_input(widget: i32) -> i32 {
    unsafe {
        let w = widget as i16;
        if w >= 0 {
            let slot = 0xb74c4c + *(CURRENT_CHANNEL as *const i16) as usize * 0x300;
            let chan = if readable(slot, 4) { *(slot as *const usize) } else { 0 };
            let at = chan + w as usize * 0x18c;
            if readable(at + 0xba966, 0x44) && *((at + 0xba966) as *const i16) == 0x12 {
                let data = ((at + 0xba9a6) as *const u32).read_unaligned();
                if let Some(f) = search_view_bits() {
                    if data == ABILITY_VIEW {
                        *f = (*f & !0x207f) | 0x20 | ABILITY_VIEW;
                        return -0xb; // the game's code for "build this screen again"
                    }
                    if [1, 2, 4, 8, 0x10, 0x20, 0x40, 0x2000].contains(&data) {
                        *f &= !ABILITY_VIEW;
                    }
                }
            }
        }
        game::<extern "C" fn(i32) -> i32>(SEARCH_INPUT_ORIG)(widget)
    }
}

const RESULTS_CALL: usize = 0x808e47; // the build's call to get_current_results
const GET_RESULTS: usize = 0x804a40; // SearchEditSession::get_current_results(this, list, count, compare)

/// A search result's player: entry +0xc indexes the staff table, staff +0x61 the player.
unsafe fn result_player(entry: usize) -> Option<usize> {
    if !readable(entry, 0x2d) {
        return None;
    }
    let staff = *(STAFF_TABLE as *const usize) + *((entry + 0xc) as *const i32) as usize * 0x6e;
    let player = if readable(staff, 0x6e) { ((staff + 0x61) as *const usize).read_unaligned() } else { 0 };
    readable(player, 70).then_some(player)
}

/// Higher first; results without a player last.
unsafe fn compare_by(a: usize, b: usize, at: usize) -> i32 {
    match (result_player(a), result_player(b)) {
        (None, _) => 1,
        (_, None) => -1,
        (Some(pa), Some(pb)) => ((pb + at) as *const i16).read_unaligned() as i32 - ((pa + at) as *const i16).read_unaligned() as i32,
    }
}

extern "C" fn by_current(a: usize, b: usize) -> i32 {
    unsafe { compare_by(a, b, 5) }
}

extern "C" fn by_potential(a: usize, b: usize) -> i32 {
    unsafe { compare_by(a, b, 7) }
}

/// The result list of the search screen: in the Ability view the Current / Potential heads (the
/// Form / Morale sort keys 0x10000 / 0x20000) sort by ability instead.
extern "thiscall" fn get_results(this: usize, list: usize, count: usize, compare: usize) -> u32 {
    let _timed = Timed(2, std::time::Instant::now());
    unsafe {
        let bits = *(SEARCH_VIEW as *const usize);
        let mut compare = compare;
        scout::restar(this);
        if trace() && readable(bits, 8) {
            log(&format!("get_results: view {:#x} sort {:#x} compare {compare:#x} count {count}", rd::<u32>(bits), rd::<u32>(bits + 4)));
        }
        if on(SEARCH_ABILITY) && readable(bits, 8) && *(bits as *const u32) & ABILITY_VIEW != 0 {
            match *((bits + 4) as *const u32) {
                0x10000 => compare = by_current as usize,
                0x20000 => compare = by_potential as usize,
                _ => {}
            }
        }
        game::<extern "thiscall" fn(usize, usize, usize, usize) -> u32>(GET_RESULTS)(this, list, count, compare)
    }
}

/// The player a row of this screen's search names: among the search's own results first (two
/// players can share a name: the first of the whole table showed the other one's stars, so the
/// Current / Potential sorts looked wrong), else by `staff_by_row_name`. Index kept per result list.
#[allow(static_mut_refs)]
unsafe fn search_staff(t: &[u8], rect: (i32, i32, i32, i32)) -> Option<usize> {
    // best: the person the row's name links to (its display widget, as news links are read)
    if let Some(staff) = linked_person(rect) {
        return Some(staff);
    }
    static mut IN_RESULTS: (usize, i32, Option<std::collections::HashMap<Vec<u8>, usize>>) = (0, 0, None);
    let s = screen_param(0);
    if !remote::CLIENT && readable(s, 1) {
        let search = s + 1 + rd::<u8>(s) as usize * 0x2b0;
        let (list, n) = (rd::<usize>(search + 0x1ee), rd::<i32>(search + 0x1f6));
        if n > 0 && (IN_RESULTS.0, IN_RESULTS.1) != (list, n) && readable(list, n as usize * 0x2d) {
            let (base, count) = (*(STAFF_TABLE as *const usize), *(STAFF_COUNT as *const i32));
            let mut map = std::collections::HashMap::new();
            for e in (0..n as usize).map(|k| list + k * 0x2d) {
                let k = rd::<i32>(e + 0xc);
                if (0..count).contains(&k) {
                    let staff = base + k as usize * 0x6e;
                    for name in row_names(staff) {
                        map.entry(name).or_insert(staff);
                    }
                }
            }
            IN_RESULTS = (list, n, Some(map));
        }
        if let Some(staff) = IN_RESULTS.2.as_ref().filter(|_| IN_RESULTS.0 == list).and_then(|m| m.get(t.trim_ascii())) {
            return Some(*staff);
        }
    }
    staff_by_row_name(t)
}

/// The staff record a person link drawn in this box points to (display widgets of this channel:
/// +8 kind, +0x10..+0x1c its box, +0x48 the object; persons 0x7d7..0x7e0 and 0x7e7).
unsafe fn linked_person((x1, y1, x2, y2): (i32, i32, i32, i32)) -> Option<usize> {
    if remote::CLIENT {
        return None;
    }
    let display = rd::<usize>(CHANNEL_DISPLAY + 0x300 * channel());
    if !mapped(display + 0x12e9a0, 2) {
        return None;
    }
    let count = rd::<i16>(display + 0x12e9a0).clamp(0, 1200) as usize;
    if !mapped(display + 0xba95e, count * 0x18c) {
        return None;
    }
    (0..count).map(|k| display + 0xba95e + k * 0x18c).find_map(|w| {
        let kind = rd::<i16>(w + 8);
        let r: [i32; 4] = std::array::from_fn(|i| rd::<i32>(w + 0x10 + 4 * i));
        let near = (r[0] - x1).abs() <= 3 && (r[1] - y1).abs() <= 3 && (r[2] - x2).abs() <= 3 && (r[3] - y2).abs() <= 3;
        let object = rd::<usize>(w + 0x48);
        (near && ((0x7d7..=0x7e0).contains(&kind) || kind == 0x7e7) && valid_staff(object)).then_some(object)
    })
}

/// The names a search row may show for a player: "Second, First" and his common name.
unsafe fn row_names(s: usize) -> Vec<Vec<u8>> {
    let name = |at: usize| {
        let p = ((s + at) as *const usize).read_unaligned();
        if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
    };
    if !readable(s, 0x6e) || !readable(((s + 0x61) as *const usize).read_unaligned(), 70) {
        return Vec::new();
    }
    let (first, second, common) = (name(4), name(8), name(0xc));
    let mut names = vec![[&second[..], b", ", &first[..]].concat()];
    if !common.is_empty() {
        names.push(common);
    }
    names
}

/// A staff member by the name a search row shows ("Cziommer, Simon"); the index is built once.
#[allow(static_mut_refs)]
unsafe fn staff_by_row_name(t: &[u8]) -> Option<usize> {
    if remote::CLIENT {
        return if t.trim_ascii().is_empty() { None } else { ask_host(&[b"G", t.trim_ascii()].concat()).and_then(|v| word(&v)) };
    }
    let names = NAMES.get_or_insert_with(|| {
        let (base, count) = (*(STAFF_TABLE as *const usize), *(STAFF_COUNT as *const i32));
        let mut map = std::collections::HashMap::new();
        let mut shared = 0u32;
        if count > 0 && readable(base, count as usize * 0x6e) {
            // the rows' style: "Second, First", or the common name ("Agustín"); built here, the
            // game's formatter (0x53dba0) asserts on some records
            let name = |s: usize, at: usize| {
                let p = ((s + at) as *const usize).read_unaligned();
                if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
            };
            for k in 0..count as usize {
                let s = base + k * 0x6e;
                if !readable(((s + 0x61) as *const usize).read_unaligned(), 70) {
                    continue;
                }
                let (first, second, common) = (name(s, 4), name(s, 8), name(s, 0xc));
                if !common.is_empty() {
                    map.entry(common).or_insert(s);
                }
                let full = [&second[..], b", ", &first[..]].concat();
                shared += map.contains_key(&full) as u32;
                map.entry(full).or_insert(s);
            }
            log(&format!("search: {} player names indexed, {shared} players share a name with an earlier one", map.len()));
        }
        map
    });
    names.get(t.trim_ascii()).copied()
}

/// The Ability view's texts (true: drawn by cmui instead).
#[allow(static_mut_refs)]
unsafe fn ability_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, t: &[u8]) -> bool {
    let (_, h) = screen();
    if y1 <= h / 10 && t == b"Player & Staff Search" {
        REDRAW |= !SEARCH;
        SEARCH = true;
        SEARCH_SEEN = true;
    }
    // Continue Game: the day's progress is drawn over this screen, and asking the screen manager
    // for the view (screen parameter 0) while the game processes a day ends it with "Error"; the
    // search is back when its title is drawn again
    if t.starts_with(b"Processing, Please Wait") {
        SEARCH = false;
    }
    // (the view bits of this screen's own search session: a network client's on the host)
    if !on(SEARCH_ABILITY) || !SEARCH || search_view_bits().is_none_or(|p| rd::<u32>(p as usize) & ABILITY_VIEW == 0) {
        return false;
    }
    const HEADS: [(&[u8], &[u8]); 6] = [(b"Nat.", b"Club"), (b"Caps", b"Pos"), (b"I.Gls", b""), (b"Form", b"Current"), (b"Morale", b"Potential"), (b"Cond.", b"")];
    if let Some((head, shown)) = HEADS.iter().find(|h| h.0 == t) {
        SEARCH_COLS.retain(|c| c.0 != *head);
        SEARCH_COLS.push((head, x1, x2));
        // (a head wider than its column shortened: "Potential" -> "Pot")
        let shown: &[u8] = if text_width(font, shown) > x2 - x1 - 4 && shown.len() > 3 { &shown[..3] } else { shown };
        text_at((x1 + x2) / 2, y1, y2, font, LIGHT, shown, 1);
        return true;
    }
    if let Some(staff) = search_staff(t, (x1, y1, x2, y2)).filter(|_| y2 - y1 < 40) {
        SEARCH_ROW = (y1, staff);
        return false;
    }
    let Some(&(col, ..)) = SEARCH_COLS.iter().find(|c| x1 >= c.1 - 3 && x2 <= c.2 + 3 && y1 > 0) else { return false };
    if SEARCH_ROW.0 != y1 {
        return false;
    }
    let staff = SEARCH_ROW.1;
    let player = rd::<usize>(staff + 0x61);
    let small = smaller(font, 0);
    match col {
        b"Nat." => {
            let club = rd::<usize>(staff + 0x39);
            let name = if readable(club, 0x55) { game_text(club + 0x38, 26) } else if remote::CLIENT && !readable(staff + 0x39, 4) { Vec::new() } else { b"No Club".to_vec() };
            text_at(x1 + 6, y1, y2, small, LIGHT, &clip_text(small, &name, x2 - x1 - 10), 0);
        }
        b"Caps" => text_at(x1 + 6, y1, y2, small, GOLD, &positions_text(staff), 0),
        b"Form" | b"Morale" => {
            // (hidden with the profile's Hide button, like its CA / PA)
            if let Some((ca, pa)) = ca_pa(player).filter(|_| !HIDE_CA_PA) {
                let size = ((y2 - y1) * 70 / 100).min((x2 - x1 - 28) / 5); // (five stars, 4 px apart, in the column)
                let (v, c) = if col == b"Form" { (ca, [0x4c, 0xc0, 0x7a]) } else { (pa, GOLD) };
                star_cell(x1 + 6, y1, x2, y2, font, size, v, c);
            }
        }
        _ => {} // I.Gls, Cond.: left empty
    }
    true
}

// The squad screen's "Ability" view (`ui\helper_squad.md`): view bit 0x2000 in screen parameter 7,
// kept with Selection's 0x1000 so the game builds the Selection Info layout; its Form / Morale /
// Cond. cells become Age / Current / Potential (stars), Apps and Av R stay empty.
const SELECTION_ITEM_CALL: usize = 0x456f7f; // add_item of the squad View popup's "Selection"
const CLUB_INPUT: usize = 0x4671f0; // the club screen's input handler (cdecl, short widget)
const SET_SCREEN_PARAM: usize = 0x7ff270; // ScreenManager::set_screen_param(this, index, value, 0)
const SQUAD_ABILITY_BIT: usize = 0x2000;
static mut CLUB_INPUT_ORIG: usize = 0;
static SQUAD_ABILITY_TEXT: &[u8] = b"  Ability\0";
/// The squad list shows the Ability view (read at the club title, kept per screen).
static mut SQUAD_ABILITY: bool = false;

// A dragged widget's outline (Display::drag_widget 0x561210): the rect at display +0x12f511
// (x1, y1, x2, y2), the dragged widget +0x12f531 (i16), the screen under it saved by 0x5e3dd0
// before the dashed box is drawn there, restored at that rect on the next move. Mouse 0xb67a20 /
// 0xb67a14.
const DRAG_SAVE_CALL: usize = 0x561388;
const SAVE_SCREEN_IMAGE: usize = 0x5e3dd0; // cdecl(x1, y1, x2, y2, 0) -> Image*
/// The drag: widget, its size, the grab offset, last seen.
static mut DRAG: Option<(i16, i32, i32, i32, i32, std::time::Instant)> = None;

/// The outline of a dragged position box over a player's slot is that slot's box (a squad's
/// position boxes are larger than the slots they are dropped on); elsewhere the box's own size at
/// the grab point. Set before the screen under it is saved, so the game draws and restores that.
extern "C" fn drag_save(x1: i32, y1: i32, x2: i32, y2: i32, a5: u32) -> usize {
    unsafe {
        let save = game::<extern "C" fn(i32, i32, i32, i32, u32) -> usize>(SAVE_SCREEN_IMAGE);
        let display = rd::<usize>(CHANNEL_DISPLAY + 0x300 * channel());
        let at = display + 0x12f511;
        if remote::CLIENT || !mapped(at, 0x22) || [0, 1, 2, 3].map(|i| rd::<i32>(at + 4 * i)) != [x1, y1, x2, y2] {
            return save(x1, y1, x2, y2, a5);
        }
        let (mx, my, widget) = (rd::<i32>(0xb67a20), rd::<i32>(0xb67a14), rd::<i16>(display + 0x12f531));
        let now = std::time::Instant::now();
        #[allow(static_mut_refs)]
        let d = match DRAG {
            Some(d) if d.0 == widget && d.5.elapsed().as_millis() < 2000 => (d.0, d.1, d.2, d.3, d.4, now),
            _ => {
                // (the size of the dragged widget itself: the drag rect may still hold the last
                // drag's slot)
                let w = display + 0xba95e + widget.max(0) as usize * 0x18c;
                let own = if mapped(w + 0x10, 16) { [0, 1, 2, 3].map(|i| rd::<i32>(w + 0x10 + 4 * i)) } else { [x1, y1, x2, y2] };
                (widget, own[2] - own[0], own[3] - own[1], x1 - mx, y1 - my, now)
            }
        };
        DRAG = Some(d);
        // the smallest widget under the mouse a little smaller than the box (not the box itself)
        let count = rd::<i16>(display + 0x12e9a0).clamp(0, 1200) as usize;
        if trace() && mapped(display + 0xba95e, count * 0x18c) {
            for k in 0..count {
                let w = display + 0xba95e + k * 0x18c;
                let r = [0, 1, 2, 3].map(|i| rd::<i32>(w + 0x10 + 4 * i));
                if (r[0]..=r[2]).contains(&mx) && (r[1]..=r[3]).contains(&my) {
                    log(&format!("drag over widget {k} kind {:#x} rect {r:?} (dragging {widget}, mouse {mx},{my})", rd::<i16>(w + 8)));
                }
            }
        }
        let target = if mapped(display + 0xba95e, count * 0x18c) {
            (0..count)
                .filter(|&k| k as i16 != widget)
                .map(|k| [0, 1, 2, 3].map(|i| rd::<i32>(display + 0xba95e + k * 0x18c + 0x10 + 4 * i)))
                .filter(|r| (r[0]..=r[2]).contains(&mx) && (r[1]..=r[3]).contains(&my))
                .filter(|r| {
                    let (w, h) = (r[2] - r[0], r[3] - r[1]);
                    w <= d.1 && h <= d.2 && (w, h) != (d.1, d.2) && w * 2 >= d.1 && h * 2 >= d.2
                })
                .min_by_key(|r| (r[2] - r[0]) * (r[3] - r[1]))
        } else {
            None
        };
        let r = target.unwrap_or([mx + d.3, my + d.4, mx + d.3 + d.1, my + d.4 + d.2]);
        let (sw, sh) = screen();
        let r = if r[0] < 0 || r[1] < 0 || r[2] >= sw || r[3] >= sh { [x1, y1, x2, y2] } else { r };
        for (i, v) in r.iter().enumerate() {
            ((at + 4 * i) as *mut i32).write_unaligned(*v);
        }
        save(r[0], r[1], r[2], r[3], a5)
    }
}

const SQUAD_SORT_CALL: usize = 0x484865; // build_filtered_sorted_squad_list's qsort call
const QSORT: usize = 0x945333; // qsort(base, count, size, compare)

/// A squad list's player (Staff** entry) and his CA / PA, None for an empty slot.
unsafe fn entry_ability(entry: usize) -> Option<(i32, i32)> {
    let staff = rd::<usize>(entry);
    let player = if readable(staff, 0x6e) { ((staff + 0x61) as *const usize).read_unaligned() } else { 0 };
    readable(player, 70).then(|| (rd::<i16>(player + 5) as i32, rd::<i16>(player + 7) as i32))
}

/// Higher first (the other value next), empty slots last.
unsafe fn compare_ability(a: usize, b: usize, potential: bool) -> i32 {
    match (entry_ability(a), entry_ability(b)) {
        (None, None) => 0,
        (None, _) => 1,
        (_, None) => -1,
        (Some((ca, pa)), Some((cb, pb))) if potential => (pb - pa).signum() * 2 + (cb - ca).signum(),
        (Some((ca, pa)), Some((cb, pb))) => (cb - ca).signum() * 2 + (pb - pa).signum(),
    }
}

extern "C" fn squad_by_current(a: usize, b: usize) -> i32 {
    unsafe { compare_ability(a, b, false) }
}

extern "C" fn squad_by_potential(a: usize, b: usize) -> i32 {
    unsafe { compare_ability(a, b, true) }
}

/// The squad list's sort: in the squad's Ability view its Age / Current / Potential heads (the
/// game's Form / Morale / Cond. sorts) sort by age, CA and PA instead.
extern "C" fn squad_sort(base: usize, count: usize, size: usize, compare: usize) {
    unsafe {
        if trace() {
            log(&format!("squad_sort: compare {compare:#x} ability view {SQUAD_ABILITY}"));
        }
        let compare = match compare {
            0x485fd0 if on(SEARCH_ABILITY) && SQUAD_ABILITY => 0x4868c0, // form -> age (the game's own)
            0x4860e0 if on(SEARCH_ABILITY) && SQUAD_ABILITY => squad_by_current as usize, // morale
            // (the Potential head spans the game's injury, fatigue and two stats columns)
            0x4862c0 | 0x4863c0 | 0x487030 | 0x487470 if on(SEARCH_ABILITY) && SQUAD_ABILITY => squad_by_potential as usize,
            c => c,
        };
        game::<extern "C" fn(usize, usize, usize, usize)>(QSORT)(base, count, size, compare)
    }
}
static mut SQUAD_COLS: Vec<(&'static [u8], i32, i32)> = Vec::new();

/// A parameter of the current screen (a network client's on the host: 0 until known).
unsafe fn screen_param(k: i32) -> usize {
    if remote::CLIENT {
        return ask_host(&key(b'P', &[k as u32])).and_then(|v| word(&v)).unwrap_or(0);
    }
    game::<extern "thiscall" fn(usize, i32) -> usize>(GET_SCREEN_PARAM)(SCREEN_MANAGER, k)
}

/// The View popup's "Selection" item, then "Ability" after it (ticked instead when on).
#[allow(clippy::too_many_arguments)]
extern "thiscall" fn selection_item(
    this: usize, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32,
    a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32, a16: u32, a17: u32, a18: u32,
) -> u32 {
    unsafe {
        type AddItem = extern "thiscall" fn(usize, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) -> u32;
        let add = game::<AddItem>(ADD_ITEM);
        let ability = on(SEARCH_ABILITY) && screen_param(7) & SQUAD_ABILITY_BIT != 0;
        let r = add(this, if ability { 2 } else { a1 }, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17, a18);
        if on(SEARCH_ABILITY) {
            add(this, if ability { 0x2002 } else { 2 }, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, SQUAD_ABILITY_TEXT.as_ptr() as u32, a15, a16, SQUAD_ABILITY_BIT as u32, a18);
        }
        r
    }
}

/// The club screen's input: the squad View's Ability item sets 0x2000 with Selection's 0x1000;
/// any other view item clears it first (the game ignores a click on a view already set).
const TACTICS_INPUT: usize = 0x89f710; // tactics_screen_handle_event(short widget)
static mut TACTICS_INPUT_ORIG: usize = 0;

/// The tactics screen's input: the Assistant's pick carried out, then the screen built again.
extern "C" fn tactics_input(widget: i32) -> i32 {
    unsafe {
        if on(ASSISTANT_PICK) && assistant::run_pending(true) {
            return -11;
        }
        game::<extern "C" fn(i32) -> i32>(TACTICS_INPUT_ORIG)(widget)
    }
}

extern "C" fn club_input(widget: i32) -> i32 {
    unsafe {
        if on(ASSISTANT_PICK) && assistant::run_pending(false) {
            return -11; // build the screen again with the assistant's selection
        }
        let w = widget as i16;
        if w >= 0 && on(SEARCH_ABILITY) {
            let slot = 0xb74c4c + *(CURRENT_CHANNEL as *const i16) as usize * 0x300;
            let chan = if readable(slot, 4) { *(slot as *const usize) } else { 0 };
            let at = chan + w as usize * 0x18c;
            if readable(at + 0xba966, 0x44) && *((at + 0xba966) as *const i16) == 7 {
                let data = ((at + 0xba9a6) as *const u32).read_unaligned() as usize;
                let view = screen_param(7);
                let set = |v: usize| game::<extern "thiscall" fn(usize, i32, usize, i32)>(SET_SCREEN_PARAM)(SCREEN_MANAGER, 7, v, 0);
                if data == SQUAD_ABILITY_BIT {
                    set(view & !0x103f & !SQUAD_ABILITY_BIT | 0x1000 | SQUAD_ABILITY_BIT);
                    return -11; // build the screen again
                }
                if view & SQUAD_ABILITY_BIT != 0 {
                    set(view & !SQUAD_ABILITY_BIT);
                    if data == 0x1000 {
                        return -11; // Selection was set already: the game would do nothing
                    }
                }
            }
        }
        game::<extern "C" fn(i32) -> i32>(CLUB_INPUT_ORIG)(widget)
    }
}

/// The Ability view's texts on the squad list (true: drawn by cmui instead).
#[allow(static_mut_refs)]
unsafe fn squad_ability_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, t: &[u8]) -> bool {
    if !SQUAD_ABILITY {
        return false;
    }
    const HEADS: [(&[u8], &[u8]); 5] = [(b"Form", b"Age"), (b"Morale", b"Current"), (b"Cond.", b"Potential"), (b"Apps", b""), (b"Av R", b"")];
    if let Some((head, shown)) = HEADS.iter().find(|h| h.0 == t) {
        SQUAD_COLS.retain(|c| c.0 != *head);
        SQUAD_COLS.push((head, x1, x2));
        // Potential takes the room of Cond., Apps and Av R (left out): its head over all three,
        // once the last of them is drawn
        let (pot_x1, pot_x2) = potential_span();
        match *head {
            b"Cond." | b"Apps" => {}
            b"Av R" => {
                // the three header buttons as one, in their own colour
                if let Some(bg) = pixel_at(pot_x1 + 3, (y1 + y2) / 2) {
                    fill(pot_x1, y1 + 1, pot_x2, y2 - 1, bg);
                }
                text_at((pot_x1 + pot_x2) / 2, y1, y2, font, LIGHT, b"Potential", 1)
            }
            _ => text_at((x1 + x2) / 2, y1, y2, font, LIGHT, shown, 1),
        }
        return true;
    }
    if y2 - y1 <= 40 && ROW_NAME.0 != y1 && x2 - x1 > sx(200) {
        if let Some(staff) = squad_member(t) {
            ROW_NAME = (y1, staff); // the row's name comes before its cells
            return false;
        }
    }
    // (by the cell's centre: the header row sits a few pixels left of the rows)
    let cx = (x1 + x2) / 2;
    let Some(&(col, ..)) = SQUAD_COLS.iter().find(|c| (c.1..=c.2).contains(&cx) && (x2 - x1 - (c.2 - c.1)).abs() < 30) else { return false };
    if ROW_NAME.0 != y1 || y2 - y1 > 40 {
        return false;
    }
    let staff = ROW_NAME.1;
    let player = rd::<usize>(staff + 0x61);
    match col {
        // (the Form cell's font is the small one: the row's own size for the age)
        b"Form" => text_at((x1 + x2) / 2, y1, y2, font + 1, LIGHT, rd::<i8>(staff + 0x18).to_string().as_bytes(), 1),
        b"Morale" | b"Av R" => {
            if let Some((ca, pa)) = ca_pa(player).filter(|_| !HIDE_CA_PA) {
                // Current in the Morale column, Potential across Cond., Apps and Av R (with the last
                // of their cells: the game draws each cell's background before its text)
                let (sx1, sx2) = if col == b"Morale" { (x1, x2) } else { potential_span() };
                let size = ((y2 - y1) * 70 / 100).min((sx2 - sx1 - 28) / 5); // (five stars, 4 px apart)
                let (v, c) = if col == b"Morale" { (ca, [0x4c, 0xc0, 0x7a]) } else { (pa, GOLD) };
                let left = sx1 + ((sx2 - sx1) - (5 * size + 16)).max(8) / 2; // centred
                star_cell(left, y1, sx2, y2, font, size, v, c);
            }
        }
        _ => {} // Apps, Av R: left empty
    }
    true
}

/// The squad Ability view's Potential column: from Cond. to the end of Av R (or of Cond. alone).
#[allow(static_mut_refs)]
unsafe fn potential_span() -> (i32, i32) {
    let col = |n: &[u8]| SQUAD_COLS.iter().find(|c| c.0 == n).map(|c| (c.1, c.2));
    match (col(b"Cond."), col(b"Av R")) {
        (Some(a), Some(b)) => (a.0, b.1),
        (Some(a), None) => a,
        _ => (0, 0),
    }
}

/// The club a title names: the part in the last brackets ("Name (Club)"), else all of it.
fn club_of(s: &[u8]) -> &[u8] {
    let s = s.trim_ascii();
    match s.iter().rposition(|&c| c == b'(') {
        Some(p) if s.ends_with(b")") => s[p + 1..s.len() - 1].trim_ascii(),
        _ => s,
    }
}

/// A club's or competition's name in a screen title: "MVV History", "VVV Reserve Tactics" -> the
/// name, whose crest and header that screen gets too.
fn title_name(s: &[u8]) -> &[u8] {
    [&b" History"[..], b" Reserve Tactics", b" Tactics", b" Training"].iter().find_map(|end| s.strip_suffix(*end)).unwrap_or(s)
}

struct Crests {
    file: std::fs::File,
    index: std::collections::HashMap<Vec<u8>, (u32, usize)>, // offset, side
    // (every picture read is kept, boxed so it stays put: a table draws twenty crests a frame)
    last: std::collections::HashMap<u32, Box<Vec<u8>>>,
}
static mut CRESTS: Option<Option<Crests>> = None; // outer None: not loaded yet

/// Data\cmui_crests.bin (logos.py pack): 'CMU2' with each picture's side (crests 80, flags 160),
/// or the older 'CMUC', all 80.
fn load_crests() -> Option<Crests> {
    load_pack("Data\\cmui_crests.bin")
}

fn load_pack(path: &str) -> Option<Crests> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    let count = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
    let entry = match &head[..4] {
        b"CMU2" => 60,
        b"CMUC" if u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize == CREST_SIZE => 56,
        _ => return None,
    };
    let mut entries = vec![0u8; count * entry];
    file.read_exact(&mut entries).ok()?;
    let index = entries
        .chunks(entry)
        .map(|e| {
            let side = if entry == 60 { u32::from_le_bytes(e[56..60].try_into().unwrap()) as usize } else { CREST_SIZE };
            (e[..52].split(|&c| c == 0).next().unwrap().to_vec(), (u32::from_le_bytes(e[52..56].try_into().unwrap()), side))
        })
        .collect();
    log(&format!("{count} names loaded from {path}"));
    Some(Crests { file, index, last: Default::default() })
}

/// A picture of cmui_crests.bin (side x side x (RGB565, alpha)) and its side (kept once read).
#[allow(static_mut_refs)]
unsafe fn crest_pixels(key: &[u8]) -> Option<(&'static Vec<u8>, usize)> {
    pack_pixels(CRESTS.get_or_insert_with(load_crests).as_mut()?, key)
}

unsafe fn pack_pixels(crests: &'static mut Crests, key: &[u8]) -> Option<(&'static Vec<u8>, usize)> {
    use std::io::{Read, Seek, SeekFrom};
    let &(at, side) = crests.index.get(key)?;
    if !crests.last.contains_key(&at) {
        let mut px = vec![0u8; side * side * 3];
        crests.file.seek(SeekFrom::Start(at as u64)).and_then(|_| crests.file.read_exact(&mut px)).ok()?;
        // ponytail: 400 pictures (about 8 MB), then start again
        if crests.last.len() >= 400 {
            crests.last.clear();
        }
        crests.last.insert(at, Box::new(px));
    }
    Some((&**crests.last.get(&at)?, side))
}

/// The rows of a picture that hold anything (a flag's own height in its square): first, end.
unsafe fn crest_rows(key: &[u8]) -> Option<(i32, i32, i32)> {
    let (px, side) = crest_pixels(key)?;
    let filled = |row: usize| px[row * side * 3..][..side * 3].chunks(3).any(|p| p[2] > 0);
    let first = (0..side).find(|&r| filled(r))?;
    let end = (0..side).rev().find(|&r| filled(r))? + 1;
    Some((first as i32, end as i32, side as i32))
}

/// Blends a crest onto the back buffer with its top left at (x, y); false when there is none.
unsafe fn draw_crest(key: &[u8], x: i32, y: i32, size: i32) -> bool {
    if !on(CRESTS_ON) {
        return false;
    }
    let Some((px, side)) = crest_pixels(key) else { return false };
    let n = side as i32;
    let flag = key.starts_with(b"flag:");
    // a crest that would vanish into what is behind it (the Champions League logo on the dark
    // theme, a club's colours on its own bar) gets a disc behind it: light on dark, dark on light
    let lum = |p: u16| {
        let p = p as u32;
        ((p >> 11 & 31) * 255 / 31 * 3 + (p >> 5 & 63) * 255 / 63 * 6 + (p & 31) * 255 / 31) / 10
    };
    // (what is behind: above the middle for a crest; left of the middle for a flag, whose square
    // reaches past the bar above and below it)
    // (a flag never: a plain rectangle needs no halo, and France's blue set one off)
    let behind = if flag { None } else { pixel_at(x + size / 2, y).or_else(|| pixel_at(x, y + size / 2)) };
    if let Some(bg) = behind {
        let bg_lum = (bg[0] as u32 * 3 + bg[1] as u32 * 6 + bg[2] as u32) / 10;
        let (mut solid, mut close) = (0u32, 0u32);
        for p in px.chunks(3).filter(|p| p[2] > 160) {
            solid += 1;
            close += (lum(u16::from_le_bytes([p[0], p[1]])) as i32 - bg_lum as i32).unsigned_abs().lt(&55) as u32;
        }
        if solid > 0 && close * 100 / solid > 45 {
            // a soft halo that follows the crest's own outline, a little lower right like a
            // bevel's shadow: light on dark, dark on light
            let halo = rgb565(if bg_lum < 128 { [0xec, 0xee, 0xf2] } else { [0x10, 0x12, 0x14] });
            let alpha = |cx: i32, cy: i32| {
                if cx < 0 || cy < 0 || cx >= size || cy >= size {
                    return 0u32;
                }
                px[(((cy * n / size) * n + cx * n / size) * 3 + 2) as usize] as u32
            };
            const R: i32 = 3;
            blend(x - R, y - R, size + 2 * R + 1, size + 2 * R + 1, |col, row| {
                let (cx, cy) = (col - R - 1, row - R - 1); // the halo sits one pixel lower right
                let mut best = 0f32;
                for dy in -R..=R {
                    for dx in -R..=R {
                        let d = ((dx * dx + dy * dy) as f32).sqrt();
                        if d <= R as f32 {
                            best = best.max(alpha(cx + dx, cy + dy) as f32 * (1.0 - d / (R as f32 + 1.0)));
                        }
                    }
                }
                (halo, (best * 0.75) as u32)
            });
        }
    }
    blend_picture(px, n, x, y, size, flag);
    true
}

/// A pack's n x n picture blended onto the back buffer at size x size; `sharp`: a flag's edges.
unsafe fn blend_picture(px: &[u8], n: i32, x: i32, y: i32, size: i32, sharp: bool) {
    let flag = sharp;
    blend(x, y, size, size, |col, row| {
        // the average of the source pixels under this one, each by how much of it lies under
        // (colours weighted by their alpha): an 80 pixel crest drawn at 77 stays even, not a
        // doubled row here and there, and small flags and crests keep thin lines and soft edges.
        // In units where a source pixel is `size` wide, this pixel spans [col * n, (col + 1) * n).
        let span = |d: i32| (d * n / size, ((d + 1) * n + size - 1) / size, d * n, (d + 1) * n);
        let ((x1, x2, lx, hx), (y1, y2, ly, hy)) = (span(col), span(row));
        let (mut r, mut g, mut b, mut a, mut count) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for sy in y1..y2.min(n) {
            let wy = (hy.min((sy + 1) * size) - ly.max(sy * size)).max(0) as u64;
            for sx in x1..x2.min(n) {
                let wx = (hx.min((sx + 1) * size) - lx.max(sx * size)).max(0) as u64;
                let p = &px[((sy * n + sx) * 3) as usize..][..3];
                let (c, w) = (u16::from_le_bytes([p[0], p[1]]) as u64, p[2] as u64 * wx * wy);
                r += (c >> 11 & 31) * w;
                g += (c >> 5 & 63) * w;
                b += (c & 31) * w;
                a += w;
                count += wx * wy;
            }
        }
        if a == 0 {
            return (0, 0);
        }
        // (a flag is a plain rectangle: its edges kept sharp, not a grey half-row on a dark bar)
        let alpha = (a / count.max(1)) as u32;
        let alpha = if flag { if alpha >= 128 { 255 } else { 0 } } else { alpha };
        ((((r + a / 2) / a) << 11 | ((g + a / 2) / a) << 5 | (b + a / 2) / a) as u16, alpha)
    });
}

/// Blends a w x h block onto the back buffer at (x, y); `pixel` gives each pixel's RGB565 colour
/// and alpha (0..255); 16-bit screens (565 or 555).
unsafe fn blend(x: i32, y: i32, bw: i32, bh: i32, pixel: impl Fn(i32, i32) -> (u16, u32)) {
    let surface = *(BACK_BUFFER as *const *mut usize);
    let Some(f565) = screen_format() else { return };
    if surface.is_null() {
        return;
    }
    let vtable = *surface as *const usize;
    let lock: extern "system" fn(*mut usize, usize, *mut u8, u32, usize) -> i32 = std::mem::transmute(*vtable.add(25));
    let unlock: extern "system" fn(*mut usize, usize) -> i32 = std::mem::transmute(*vtable.add(32));
    let mut desc = [0u8; 108]; // DDSURFACEDESC
    desc[0] = 108;
    if lock(surface, 0, desc.as_mut_ptr(), 1, 0) != 0 {
        return;
    }
    let field = |at: usize| i32::from_le_bytes(desc[at..at + 4].try_into().unwrap());
    let (h, w, pitch, bits) = (field(8), field(0xc), field(0x10), field(0x24) as usize);
    for row in 0..bh {
        for col in 0..bw {
            let (px, py) = (x + col, y + row);
            let (s, a) = pixel(col, row);
            if a == 0 || px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            let dst = (bits + (py * pitch + px * 2) as usize) as *mut u16;
            let (s, d) = (s as u32, *dst as u32);
            if f565 {
                let mix = |shift: u32, mask: u32| ((((s >> shift) & mask) * a + ((d >> shift) & mask) * (255 - a)) / 255) << shift;
                *dst = (mix(11, 31) | mix(5, 63) | mix(0, 31)) as u16;
            } else {
                let (sc, dc) = (unpack(s, true), unpack(d, false));
                *dst = pack([0, 1, 2].map(|i| (sc[i] * a + dc[i] * (255 - a)) / 255), false);
            }
        }
    }
    unlock(surface, 0);
}

const ADD_AREA: usize = 0x55fba0; // Display::add_area, thiscall
const PACKET: usize = 0x564080; // Display::process_command_packet: a network client builds its screen here

const FONT_STEP: usize = 0x55fa14; // add_item: mov edx, [esp + 0x24c] (the font argument)
const LOCATION_NAME: usize = 0x9f17a0; // the registry value the game asks for its install path
const DISPLAY_RESET: usize = 0x55f7c0; // Display::reset(this, release_items), thiscall
const CLIENT_LOOP: usize = 0x7fdc00; // ScreenManager::run_network_client(this), thiscall
const SELECT_GAME_INPUT: usize = 0x81fbd0; // the Select Network Game screen's input: joins, runs the client
static mut PACKET_ORIG: usize = 0;
/// A network client's screen count (a new one each time the host resets its display).
static mut CLIENT_SCREEN: usize = 0;

/// Display::process_command_packet: cmui's own packets (`remote`) are taken before the game sees
/// them, on the host (a client's questions) and on a client (the host's answers).
extern "thiscall" fn packet(display: usize, net: usize, slot: i32, buf: usize) -> i32 {
    unsafe {
        let mut r = one_packet(display, net, slot, buf);
        // The client loop 0x7fdc00 reads one packet a pass and sleeps in the pass (not found where,
        // not in the exe or in cmui), ~1,000 packets a second, and a host sends every widget of
        // Latest Scores as a packet: its send stood still for minutes on a Dutch Cup day (night 4
        // run F). So what has come is taken here, 20 ms at a time. (0x13, 0x14: the session's end)
        if remote::CLIENT && buf == CLIENT_BUFFER && !NO_DRAIN {
            let read = game::<extern "thiscall" fn(usize, usize, *mut i16) -> i32>(READ_PACKET);
            let began = std::time::Instant::now();
            loop {
                let data = *(buf as *const *const u8);
                if r == -2 || matches!(*data, 0x13 | 0x14) || began.elapsed().as_millis() >= 20 || read(net, data as usize, CLIENT_SLOT as *mut i16) == 0 {
                    return r;
                }
                if *data == 0 {
                    *(LAST_PACKET_FLAG as *mut u32) = 1; // (as the loop does)
                }
                r = one_packet(display, net, *(CLIENT_SLOT as *const i16) as i32, buf);
            }
        }
        r
    }
}

const READ_PACKET: usize = 0x77af20; // Network's read of a packet (this, data, *slot), thiscall: 0 when none came
const CLIENT_BUFFER: usize = SCREEN_MANAGER + 0x37fe; // the client loop's NetworkBuffer
const CLIENT_SLOT: usize = SCREEN_MANAGER + 0x382e; // the slot the read fills in
const LAST_PACKET_FLAG: usize = 0xdd6bf8; // the loop's flag for its 30 s ping 0x568470

/// Test switches, read when the client loop starts: `cmui.nodrain` one packet a pass as the game
/// does; `cmui.nettrace` logs the client's packets a second and the long texts of widget packets.
static mut NO_DRAIN: bool = false;
static mut NET_TRACE: Option<(std::time::Instant, std::time::Instant, u32)> = None;

#[allow(static_mut_refs)]
unsafe fn trace_packet(data: *const u8) {
    let Some((began, second, count)) = NET_TRACE.as_mut() else { return };
    *count += 1;
    if second.elapsed().as_millis() >= 1000 {
        log(&format!("nettrace {} ms: {count} packets in the last second", began.elapsed().as_millis()));
        (*second, *count) = (std::time::Instant::now(), 0);
    }
    if *data == 8 {
        let head = std::slice::from_raw_parts(data, 300);
        let text = head[1..].split(|c| !(32..127).contains(c)).max_by_key(|t| t.len()).unwrap_or(b"");
        if text.len() >= 12 {
            log(&format!("nettrace {} ms: widget {:02x?} \"{}\"", began.elapsed().as_millis(), &head[1..4], String::from_utf8_lossy(text)));
        }
    }
}

unsafe fn one_packet(display: usize, net: usize, slot: i32, buf: usize) -> i32 {
    {
        // NetworkBuffer: data, cursor, length
        if mapped(buf, 12) {
            if remote::CLIENT && buf == CLIENT_BUFFER {
                trace_packet(*(buf as *const *const u8));
            }
            let (data, len) = (*(buf as *const usize), *((buf + 8) as *const i32));
            if len >= 2 && mapped(data, len as usize) && *(data as *const u8) == remote::OPCODE {
                let bytes = std::slice::from_raw_parts(data as *const u8, len as usize).to_vec();
                remote::received(&bytes, slot as i16 as i32, &|channel, key| {
                    prefetch::asked(channel, key); // (sent with that kind of screen next time)
                    host_answer(channel, key)
                });
                return -1; // (no widget clicked)
            }
        }
        game::<extern "thiscall" fn(usize, usize, i32, usize) -> i32>(PACKET_ORIG)(display, net, slot, buf)
    }
}

const SEND_CLICK: usize = 0x568560; // Display::send_widget_click(this, net, widget), thiscall
const RIGHT_HELD: usize = 0xb67a38; // right button down (BUTTON_HELD's twin)

/// A widget's text on a display (items of 0x18c bytes from +0xba95e, text at +0x80).
unsafe fn widget_text(display: usize, w: i16) -> Vec<u8> {
    let at = display + 0xba95e + w as usize * 0x18c + 0x80;
    if w < 0 || !mapped(at, 64) {
        return Vec::new();
    }
    std::slice::from_raw_parts(at as *const u8, 64).split(|&c| c == 0).next().unwrap().to_vec()
}

/// Display::send_widget_click: a network client's click goes to the host. The click handler fires
/// a widget while the button is held over it, and only a screen built here lets go of the button
/// (ScreenManager::monitor clears the held flags): on a client the host's next screen came while
/// the button was still down, and the widget under the mouse there fired too (Tactics, then a
/// player of the tactics list; Back twice). The button counts as let go once the click is sent.
extern "thiscall" fn send_click(display: usize, net: usize, widget: i32) -> i32 {
    unsafe {
        if trace() {
            log(&format!("click sent: widget {} {:?}", widget as i16, latin1(&widget_text(display, widget as i16))));
        }
        remote::clicked();
        let r = game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SEND_CLICK)(display, net, widget);
        *(BUTTON_HELD as *mut u32) = 0;
        *(RIGHT_HELD as *mut u32) = 0;
        r
    }
}

const SCREEN_SENT: usize = 0x5622a0; // Display::log_screen_sent(this, net, dest seat), thiscall: sends a client its screen
const SCREEN_SENT_CALL: usize = 0x7fca5c; // in SCREEN_MANAGER::monitor, right after the screen is built

/// The host has sent a client the screen it just built: then cmui's block for it, worked out
/// from that display while it still holds the screen's items (`NETWORK_PLAN.md`).
extern "thiscall" fn screen_sent(display: usize, net: usize, dest: i32) -> i32 {
    unsafe {
        let r = game::<extern "thiscall" fn(usize, usize, i32) -> i32>(SCREEN_SENT)(display, net, dest);
        if let Some(channel) = (1..16).find(|&c| rd::<usize>(CHANNEL_DISPLAY + 0x300 * c) == display) {
            // (a click armed for the screen before this one must not fire on this one)
            if news::PENDING_CELL.is_some_and(|p| p.0 as usize == channel) {
                news::PENDING_CELL = None;
            }
            // (the answers its cmui will ask for there first: the block after them tells the
            // client they are in)
            prefetch::send(channel as i16, display, dest as i16 as i32);
            let mut block = news::screen_block(display);
            block.extend(comp::history_block(channel));
            block.extend(comp::form_block());
            if trace() {
                log(&format!("cmui block to channel {channel} (seat {}): {} bytes", dest as i16, block.len()));
            }
            let mut p = vec![remote::OPCODE, b'b'];
            p.extend(&block[..block.len().min(38_000)]);
            remote::send(&p, dest as i16 as i32);
        }
        r
    }
}

/// Display::reset from a packet: on a network client the host has begun a new screen.
extern "thiscall" fn display_reset(display: usize, release: i32) {
    unsafe {
        if remote::CLIENT {
            CLIENT_SCREEN += 1;
            remote::new_screen();
            #[allow(static_mut_refs)]
            {
                remote::BLOCK.clear();
                news::CLICKS.clear();
            }
        }
        game::<extern "thiscall" fn(usize, i32)>(DISPLAY_RESET)(display, release)
    }
}

/// The client loop's "Contacting Server..." box (0x7fe159..0x7fe1c0: panel, text, flip) is drawn at
/// 800x600 design rows 550..590, unscaled: at a bigger screen right over the menu bar's Find and
/// Game Options. Its pushes of those rows are made three quarters down the screen instead,
/// below the menu bar's items.
unsafe fn contacting_box_lower() {
    let (_, sh) = screen();
    let (y1, y2) = (sh * 3 / 4, sh * 3 / 4 + 40);
    // (push y2 imm32, push y1 imm32) of the panel, the text and the flip
    let sites = [(0x7fe162usize, 0x24e, y2), (0x7fe169, 0x226, y1), (0x7fe19a, 0x24e, y2), (0x7fe1a1, 0x226, y1), (0x7fe1ad, 0x24e, y2), (0x7fe1b4, 0x226, y1)];
    if sh <= 600 || !sites.iter().all(|&(at, old, _)| *(at as *const u8) == 0x68 && ((at + 1) as *const i32).read_unaligned() == old) {
        return; // (800x600: where it was meant; another build: left alone)
    }
    for (at, _, new) in sites {
        let mut old = 0;
        VirtualProtect((at + 1) as *mut u8, 4, 0x40, &mut old);
        ((at + 1) as *mut i32).write_unaligned(new);
        VirtualProtect((at + 1) as *mut u8, 4, old, &mut old);
    }
    log(&format!("network client: Contacting Server box at rows {y1}..{y2}"));
}

/// ScreenManager::run_network_client: while it runs, this game is a network client and cmui's
/// game data comes from the host.
extern "thiscall" fn client_loop(manager: usize) -> u32 {
    unsafe {
        remote::CLIENT = true;
        log("network client: game data from the host");
        NO_DRAIN = std::path::Path::new("cmui.nodrain").exists();
        let now = std::time::Instant::now();
        NET_TRACE = std::path::Path::new("cmui.nettrace").exists().then_some((now, now, 0));
        log(&format!("network client: one packet a pass {NO_DRAIN}, trace {}", NET_TRACE.is_some()));
        if on(MENU_FIXES) {
            contacting_box_lower();
        }
        let r = game::<extern "thiscall" fn(usize) -> u32>(CLIENT_LOOP)(manager);
        remote::CLIENT = false;
        remote::reset();
        r
    }
}

/// A staff record of the staff table (a pointer a network client sent is checked before use).
unsafe fn valid_staff(staff: usize) -> bool {
    let (base, count) = (*(STAFF_TABLE as *const usize), *(STAFF_COUNT as *const i32));
    count > 0 && staff >= base && staff < base + count as usize * 0x6e && (staff - base) % 0x6e == 0 && mapped(staff, 0x6e)
}

/// The screen node of `channel` when the screen manager is serving that channel and it has a
/// screen (its parameters asked otherwise stop the game).
unsafe fn served_node(channel: i16) -> Option<usize> {
    let current = *((SCREEN_MANAGER + 0x382a) as *const i16);
    let node = SCREEN_MANAGER + 0x300 * channel as usize + 0x7e2;
    (current == channel && mapped(node, 4) && *(node as *const usize) != 0).then(|| *(node as *const usize))
}

/// What the host gives a network client for a key of `remote`, from this process's memory, for the
/// client's channel (the one the screen manager is serving while the packet is handled).
unsafe fn host_answer(channel: i16, key: &[u8]) -> Option<Vec<u8>> {
    let arg = |k: usize| key.get(1 + 4 * k..5 + 4 * k).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    let le = |v: usize| (v as u32).to_le_bytes().to_vec();
    let staff = arg(0);
    match key.first()? {
        b'R' => mapped(staff, remote::CHUNK).then(|| std::slice::from_raw_parts(staff as *const u8, remote::CHUNK).to_vec()),
        b'P' => served_node(channel).map(|_| le(screen_param(arg(0) as i32))),
        b'C' => club_named(&key[1..]).map(le),
        b'N' => mapped(staff, 0x5b).then(|| vec![national_team(staff) as u8]),
        b'F' => valid_staff(staff).then(|| condition(staff)).flatten().map(|v| le(v as usize)),
        b'S' => valid_staff(staff).then(|| season_stat(staff, arg(1) as i32)),
        b'W' => valid_staff(staff).then(|| to_latin1(&money_texts(staff).0)),
        b'V' => valid_staff(staff).then(|| to_latin1(&money_texts(staff).1)),
        b'A' => (valid_staff(staff) && mapped(*((staff + 0x61) as *const usize), 70)).then(|| attributes(staff).to_vec()),
        b'O' => valid_staff(staff).then(|| positions_text(staff)),
        b'M' => compare_players(&key[1..]).map(|(a, b)| [le(a), le(b)].concat()),
        b'G' => staff_by_row_name(&key[1..]).map(le),
        // a story's crest from its links (empty: none)
        b'Y' => {
            let text = &key[1..];
            let at = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            Some(story_link_crest(channel as usize, &text[..at], text.get(at + 1..).unwrap_or(&[])).unwrap_or_default())
        }
        // the news club row clicked on the client: opened by the click that follows (news_input)
        b'L' => Some(news::link_list(channel as usize)),
        // a client's story link clicked: opened by the click that follows (news_input)
        b'X' => {
            news::PENDING_CELL = Some((channel, arg(0) as u16 as i16));
            Some(Vec::new())
        }
        // a client's story link found by its words: its name and crest key
        b'Z' => news::link_names(arg(0) as u16 as i16, arg(1)).map(|(name, key)| news::name_bytes(&name, &key)),
        b'Q' => {
            news::PENDING_MARK = Some(channel);
            Some(Vec::new())
        }
        // a click came: the answer says the host is there; the client's height for its rows
        b'p' => {
            comp::client_height(channel, arg(0) as i32);
            None
        }
        // the Assistant's pick for a client (mode, number, players not available) and its note
        b'H' => {
            let words: Vec<u32> = key[1..].chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
            assistant::remote_request(channel, &words);
            Some(Vec::new())
        }
        b'J' => assistant::remote_note(channel, arg(0) as u32),
        // the Assistant's team report for a client (a number, players not available); only while
        // its channel is served, as the report reads the screen's fixture
        b'T' => served_node(channel).map(|_| {
            let words: Vec<u32> = key[1..].chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
            assistant::remote_report(channel, &words)
        }),
        b'K' => mapped(staff, 0x5b).then(|| {
            PENDING_CLUB = (channel, staff);
            Vec::new()
        }),
        // a player's injuries, bans and moves; what the staff say on a compare pair; the scout's
        // report on a club (a number, the club)
        b'I' => valid_staff(staff).then(|| history::rows(staff)),
        b'D' => {
            let player = |s: usize| valid_staff(s) && mapped(rd::<usize>(s + 0x61), 70);
            (player(staff) && player(arg(1))).then(|| assistant::staff_views(channel, staff, arg(1)))
        }
        b'B' => prefetch::valid_club(arg(1)).then(|| assistant::remote_scout(channel, arg(1))),
        // a club's star players
        b'E' => Some(challenge::remote(channel)),
        b'U' => prefetch::valid_club(staff).then(|| scout::best_three(staff).iter().flat_map(|&s| (s as u32).to_le_bytes()).collect()),
        _ => None,
    }
}

/// A network client is sent the areas and widgets the host has built, and its packet handler adds
/// them with Display::add_area / add_item again, so patches in those two run twice:
/// - the resolution patches (CM0102Patcher's 1280x800, 1680x1050 ...): `call scale; <first
///   instruction>` at the start of both scales each rect from 800x600 design coordinates (1.75
///   times too big at 1680x1050), and add_item's hook also steps fonts 0 and 1 of some types up;
/// - a bigger-fonts patch: `jmp` at FONT_STEP to `mov edx, [esp + 0x24c]; add edx, 1` (to 7 at
///   most), one font size up (texts a size bigger on the client).
/// The client's two calls go to stubs that skip the scaling hook (run only its own first
/// instruction) and take the font argument one down beforehand where the font step will add it.
unsafe fn client_rects_as_sent() {
    let jump_target = |at: usize| match *(at as *const u8) {
        0xe8 | 0xe9 => Some((at + 5).wrapping_add(((at + 1) as *const i32).read_unaligned() as usize)),
        _ => None,
    };
    let starts = |at: usize, code: &[u8]| std::slice::from_raw_parts(at as *const u8, code.len()) == code;
    let font_step = jump_target(FONT_STEP).is_some_and(|t| starts(t, &[0x8b, 0x94, 0x24, 0x4c, 0x02, 0x00, 0x00, 0x83, 0xc2, 0x01]));
    // (function, where its body goes on after the hook, the hook's instruction: mov eax,
    // [0xae290c] / mov dx, [ecx + 0x12e9a0], the font step's fix-up: dec dword [esp + 0x30])
    let hooks: [(usize, usize, &[u8], &[u8]); 2] = [
        (ADD_AREA, 5, &[0xa1, 0x0c, 0x29, 0xae, 0x00], &[]),
        (ADD_ITEM, 7, &[0x66, 0x8b, 0x91, 0xa0, 0xe9, 0x12, 0x00], if font_step { &[0xff, 0x4c, 0x24, 0x30] } else { &[] }),
    ];
    for (func, body, moved, before) in hooks {
        let scaled = match jump_target(func) {
            None => false, // the game's own code
            Some(hook) if *(hook as *const u8) == 0xe8 && starts(hook + 5, moved) => true,
            Some(_) => {
                log(&format!("{func:#x}: hooked by something else, network client left alone"));
                continue;
            }
        };
        if !scaled && before.is_empty() {
            continue;
        }
        let mut code = before.to_vec();
        let goes_on = if scaled {
            code.extend_from_slice(moved);
            func + body
        } else {
            func
        };
        let stub = VirtualAlloc(0, 32, 0x3000, 0x40);
        let n = code.len();
        code.push(0xe9);
        code.extend_from_slice(&(goes_on as i32 - (stub as i32 + n as i32 + 5)).to_le_bytes());
        std::ptr::copy_nonoverlapping(code.as_ptr(), stub, code.len());
        patch_call(PACKET, 14912, func, stub as usize);
    }
}

/// The registry's install path is another folder than the game's own (the case and a closing
/// slash do not count).
fn other_install(location: &str, own: &str) -> bool {
    let clean = |path: &str| path.trim_end_matches(['\\', '/']).to_ascii_lowercase();
    !location.is_empty() && clean(location) != clean(own)
}

/// An exe that is not portable moves to the registry's install path as it starts and loads the
/// data found there (Restructured's exe ran on the original game's, NickDLL then missed every
/// club of its own): says so. The patcher's portable patch renames the value to "Aocation".
unsafe fn warn_not_portable() {
    if std::slice::from_raw_parts(LOCATION_NAME as *const u8, 8) != b"Location" {
        return;
    }
    let Some(own) = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_str()?.to_string())) else { return };
    let (mut data, mut size) = ([0u8; 260], 260u32);
    let key = b"SOFTWARE\\Sports Interactive Ltd\\Championship Manager 01-02\0";
    // HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ
    if RegGetValueA(0x80000002, key.as_ptr(), b"Location\0".as_ptr(), 2, std::ptr::null_mut(), data.as_mut_ptr(), &mut size) != 0 {
        return;
    }
    let location = String::from_utf8_lossy(&data[..size.saturating_sub(1) as usize]).into_owned();
    if !other_install(&location, &own) {
        return;
    }
    log(&format!("not portable: this exe runs from {location}, not {own}"));
    let text = format!("This exe is not portable, so the game runs from\n{location}\nand loads that folder's data instead of\n{own}\n\nIn CM0102Patcher tick \"Make Executable Portable\" and apply it to this exe. Then switch off Windows 95 compatibility mode for it (Properties > Compatibility), or the game crashes at start.\0");
    // MB_ICONWARNING | MB_SETFOREGROUND | MB_TOPMOST
    MessageBoxA(0, text.as_ptr(), b"cmui\0".as_ptr(), 0x50030);
}

#[cfg(test)]
#[test]
fn other_install_ignores_case_and_closing_slash() {
    assert!(!other_install("f:\\CM0102\\", "F:\\cm0102"));
    assert!(other_install("f:\\CM0102Orign", "F:\\CM0102 - Restructured 2025-2026"));
    assert!(!other_install("", "F:\\CM0102"));
}

#[cfg(test)]
#[test]
fn match_minutes_join_with_penalties() {
    // what the game puts in the minutes cell of a scorer: two minutes a row
    for t in [&b"7"[..], b"19, 58", b"pen 82", b"og 30", b"45+2", b"7, pen 25,", b"pen 7, og 25,", b"pen 45", b" 12, "] {
        assert!(is_minutes(t), "{}", String::from_utf8_lossy(t));
    }
    for t in [&b""[..], b",", b"pen", b"Smith", b"Smith 7", b"7 injured", b"pen x"] {
        assert!(!is_minutes(t), "{}", String::from_utf8_lossy(t));
    }
    // hat-trick with two penalties: a row of two and a row of one
    assert_eq!(join_minutes(b"7, pen 25,", b"pen 45"), b"7, pen 25, pen 45");
    // four goals: two rows of two, the second joined to the first
    assert_eq!(join_minutes(b"10, 33,", b"pen 51, 80"), b"10, 33, pen 51, 80");
    // no comma at the end of the first row
    assert_eq!(join_minutes(b"7 ", b" 80 "), b"7, 80");
}

#[cfg(test)]
#[test]
fn event_text_colours() {
    assert_eq!(event_colour(b"Smith sent off"), [0xd8, 0x30, 0x30]);
    assert_eq!(event_colour(b"Jones injured"), [0xe0, 0x70, 0x60]);
    assert_ne!(event_colour(b"Smith sent off"), event_colour(b"Jones injured"));
    assert_eq!(event_colour(b"Smith m/pen"), [0xa0, 0xa6, 0xae]);
    assert_eq!(event_colour(b"Smith missed"), [0xa0, 0xa6, 0xae]);
    assert_eq!(event_colour(b"Smith"), [0x4c, 0xc0, 0x7a]);
}

/// The Print dropdown of the club screens as Export (switch `export_button`).
const EXPORT_LABEL: &[u8] = b"Export";
/// (at, what is there, what goes there): the label's pointer in the club and competition screens,
/// the club screen's button left of the crest's place in 800x600 design coordinates (was
/// 710..785; the button keeps its height), and the printer choice in club_screen_input never
/// taken, so the file is all there is.
unsafe fn export_button() {
    static LABEL: [u8; 7] = *b"Export\0";
    let label = (LABEL.as_ptr() as u32).to_le_bytes();
    let patches: [(usize, &[u8], &[u8]); 5] = [
        (0x45641c, &[0xf4, 0xe5, 0x98, 0x00], &label),
        (0x490227, &[0xf4, 0xe5, 0x98, 0x00], &label),
        (0x456454, &[0x11, 0x03, 0x00, 0x00], &705u32.to_le_bytes()),
        (0x45645b, &[0xc6, 0x02, 0x00, 0x00], &633u32.to_le_bytes()),
        (0x4672db, &[0x74], &[0xeb]),
    ];
    for (at, old, new) in patches {
        if std::slice::from_raw_parts(at as *const u8, old.len()) != old {
            log(&format!("{at:#x}: not the 3.9.68 code, Export left alone there"));
            continue;
        }
        let mut was = 0;
        VirtualProtect(at as *mut u8, new.len(), 0x40, &mut was);
        std::ptr::copy_nonoverlapping(new.as_ptr(), at as *mut u8, new.len());
        VirtualProtect(at as *mut u8, new.len(), was, &mut was);
    }
}

/// Sends `func` to `hook` when it starts with `moved` (position-independent instructions) or
/// with another mod's jump (cmfix hooks some of the same functions); returns a stub that runs
/// the moved bytes and carries on in `func`, or goes on to the other mod's hook. 0 when left alone.
unsafe fn hook_entry(func: usize, moved: &[u8], hook: usize) -> usize {
    let code = std::slice::from_raw_parts(func as *const u8, moved.len());
    let stub = VirtualAlloc(0, 16, 0x3000, 0x40);
    if code == moved {
        let n = moved.len();
        std::ptr::copy_nonoverlapping(moved.as_ptr(), stub, n);
        *stub.add(n) = 0xe9;
        (stub.add(n + 1) as *mut i32).write_unaligned((func + n) as i32 - (stub as i32 + n as i32 + 5));
    } else if code[0] == 0xe9 {
        let other = (func + 5).wrapping_add(((func + 1) as *const i32).read_unaligned() as usize);
        *stub = 0xe9;
        (stub.add(1) as *mut i32).write_unaligned(other as i32 - (stub as i32 + 5));
        log(&format!("{func:#x}: chained in front of another mod's hook at {other:#x}"));
    } else {
        log(&format!("{func:#x}: not the 3.9.68 code ({code:02x?}), left alone"));
        return 0;
    }
    let mut old = 0;
    VirtualProtect(func as *mut u8, 5, 0x40, &mut old);
    *(func as *mut u8) = 0xe9;
    ((func + 1) as *mut i32).write_unaligned(hook as i32 - (func as i32 + 5));
    VirtualProtect(func as *mut u8, 5, old, &mut old);
    log(&format!("{func:#x}: now goes to cmui"));
    stub as usize
}

/// The club screen's input on a tab that is none (seen on the host when a client left by Exit
/// Network Game from a club screen; why the tab is wrong not found): its switch's default box
/// "club_screens.cpp 8053" stood in the host's way until that client was gone. Past the box the
/// game just goes on, so the switch's jump goes there.
unsafe fn no_club_tab_box() {
    let at = 0x467cc8 as *mut i32;
    if at.read_unaligned() == 0x4688c8 - 0x467ccc {
        let mut old = 0;
        VirtualProtect(at as *mut u8, 4, 0x40, &mut old);
        at.write_unaligned(0x468931 - 0x467ccc);
        VirtualProtect(at as *mut u8, 4, old, &mut old);
    }
}

/// A club's Landmarks: its rows a page come from the list's height, which a tall screen takes past
/// the 30 an area may have ("Area.cpp 507", 0x403425). At most 30, where the pages are counted and
/// where the list is made; the new code fits in the place of the old.
unsafe fn landmarks_rows() {
    const SITES: [(usize, &[u8], &[u8]); 2] = [
        // mov ecx, eax; and ecx, 0xff -> push 30; pop ecx; cmp eax, ecx; cmovl ecx, eax
        (0x46a604, &[0x8b, 0xc8, 0x81, 0xe1, 0xff, 0, 0, 0], &[0x6a, 0x1e, 0x59, 0x3b, 0xc1, 0x0f, 0x4c, 0xc8]),
        // mov ebx, eax; movsx eax, [..]; and ebx, 0xff -> push 30; pop ebx; cmp eax, ebx; cmovl ebx, eax; movsx eax, [..]
        (0x46a683, &[0x8b, 0xd8, 0x0f, 0xbf, 0x05, 0x7a, 0x7c, 0xb7, 0, 0x81, 0xe3, 0xff, 0, 0, 0], &[0x6a, 0x1e, 0x5b, 0x3b, 0xc3, 0x0f, 0x4c, 0xd8, 0x0f, 0xbf, 0x05, 0x7a, 0x7c, 0xb7, 0]),
    ];
    for (at, old, new) in SITES {
        if std::slice::from_raw_parts(at as *const u8, old.len()) == old {
            let mut protect = 0;
            VirtualProtect(at as *mut u8, new.len(), 0x40, &mut protect);
            std::ptr::copy_nonoverlapping(new.as_ptr(), at as *mut u8, new.len());
            VirtualProtect(at as *mut u8, new.len(), protect, &mut protect);
        }
    }
}

/// A file `cmui.stack` in the game folder asks where the game thread is: its EIP and the code
/// addresses on its stack go to cmui.log (a game started elevated opens to nothing outside).
/// ponytail: a scan, so old return addresses show too; a walk needs frames the exe doesn't keep.
unsafe fn log_game_thread(thread: usize, cmui: usize) {
    let mut context = [0u32; 180];
    context[0] = 0x10003; // CONTEXT_i386 | CONTROL | INTEGER
    let mut stack = [0u32; 4096];
    let mut words = 0;
    // (nothing allocated while it stands: it may hold the heap's lock)
    SuspendThread(thread);
    let ok = GetThreadContext(thread, &mut context) != 0;
    let esp = context[49] as usize; // Esp
    let mut info = [0usize; 7];
    if ok && VirtualQuery(esp, &mut info, 28) != 0 {
        words = ((info[0] + info[3] - esp) / 4).min(stack.len());
        std::ptr::copy_nonoverlapping(esp as *const u32, stack.as_mut_ptr(), words);
    }
    ResumeThread(thread);
    let code: Vec<String> = stack[..words]
        .iter()
        .map(|&v| v as usize)
        .filter(|v| (0x401000..0x980000).contains(v) || (cmui..cmui + 0x400000).contains(v))
        .take(32)
        .map(|v| if v < 0x980000 { format!("{v:x}") } else { format!("cmui+{:x}", v - cmui) })
        .collect();
    log(&format!("game thread: eip {:x}, esp {esp:x}, on its stack: {}", context[46], code.join(" ")));
    let sm = SCREEN_MANAGER;
    let word = |a: usize| *(a as *const i16);
    let long = |a: usize| *(a as *const u32);
    log(&format!(
        "monitor: channel {}, done {} {}, step callback {:x} on action {}, action now {}",
        word(sm + 0x382a), long(sm + 0x7ee), long(sm + 0x300 + 0x7ee), long(sm + 0x37ee), long(sm + 0x37f2), word(0xdd6bde)
    ));
}

#[no_mangle]
extern "system" fn DllMain(module: usize, reason: u32, _reserved: usize) -> i32 {
    if reason == 1 {
        log("cmui 0.1.0 loaded");
        // SUSPEND_RESUME | GET_CONTEXT | QUERY_INFORMATION (DllMain runs on the game thread)
        let thread = unsafe { OpenThread(0x4a, 0, GetCurrentThreadId()) };
        // a panic ends the game (panic = abort): where it happened goes to cmui.log first
        std::panic::set_hook(Box::new(|info| log(&format!("cmui panic: {info}"))));
        // ponytail: test switch, the game ignores WM_CLOSE and cannot be ended from outside here
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Some(v) = setting("background_opacity").and_then(|v| v.parse::<u32>().ok()) {
                unsafe { PICTURE_SHOWS = v.min(100) };
            }
            if std::fs::remove_file("cmui.stack").is_ok() {
                unsafe { log_game_thread(thread, module) };
            }
            // (a shot from here: the script stands still while the game does not poll)
            if let Ok(name) = std::fs::read_to_string("cmui.shot") {
                let _ = std::fs::remove_file("cmui.shot");
                unsafe { save_back_buffer(name.trim()) };
            }
            if std::fs::remove_file("cmui.peek").is_ok() {
                for at in [DRAW_PANEL, DRAW_TEXT_BOX] {
                    log(&format!("{at:#x} now starts {:02x?}", unsafe { std::slice::from_raw_parts(at as *const u8, 8) }));
                }
            }
            if std::fs::remove_file("cmui.quit").is_ok() {
                std::process::exit(0);
            }
        });
        // (a thread: no message box under the loader lock, and the game starts on behind it)
        std::thread::spawn(|| unsafe { warn_not_portable() });
        // Save Printout writes into <game folder>\print and fopen does not make the folder
        let _ = std::fs::create_dir("print");
        unsafe {
            load_features();
            client_rects_as_sent();
            if on(MENU_FIXES) {
                no_club_tab_box();
                landmarks_rows();
            }
            if on(EXPORT_BUTTON) {
                export_button();
            }
            // cmui's data for a network client (`remote`)
            patch_call(PACKET, 14912, DISPLAY_RESET, display_reset as usize);
            patch_call(SELECT_GAME_INPUT, 720, CLIENT_LOOP, client_loop as usize);
            patch_call(CLIENT_LOOP, 2208, SEND_CLICK, send_click as usize);
            patch_call(SCREEN_SENT_CALL, 5, SCREEN_SENT, screen_sent as usize);
            // mov eax, [0xae237c] / sub esp, 0x1c; push ebx; push ebp
            comp::ADD_HISTORY_ORIG = hook_entry(comp::ADD_HISTORY, &[0xa1, 0x7c, 0x23, 0xae, 0x00], comp::add_history as usize);
            comp::HISTORY_WINNERS_ORIG = hook_entry(comp::HISTORY_WINNERS, &[0x83, 0xec, 0x1c, 0x53, 0x55], comp::history_winners as usize);
            if on(STATS_TABLE) {
                comp::one_view();
            }
            comp::live_table_paced();
            if on(BIGGER_TEXT) {
                patch_call(ADD_ITEM, 528, WIDGET_INIT, widget_init as usize);
            }
            // mov eax, 0x2af8
            PACKET_ORIG = hook_entry(PACKET, &[0xb8, 0xf8, 0x2a, 0x00, 0x00], packet as usize);
            if on(THEME_ON) {
                patch_call(GRAPHICS_INIT, 1696, PALETTE_INIT, palette_init as usize);
                menu_bars();
            }
            patch_call(DRAW_SCREEN, 800, BLIT_PICTURE, background as usize);
            patch_call(DRAW_SCREEN, 800, FLIP_RECT, flip as usize);
            patch_calls(AREA_DRAW, 816, DRAW_LINE, no_grid_line as usize, 2);
            patch_calls(GUIO_DRAW, 1680, DRAW_GLYPH, glyph as usize, 5);
            patch_call(OTHER_ITEM_CALL, 5, ADD_ITEM, other_item as usize);
            patch_call(RESULTS_CALL, 5, GET_RESULTS, get_results as usize);
            patch_call(SELECTION_ITEM_CALL, 5, ADD_ITEM, selection_item as usize);
            patch_call(SQUAD_SORT_CALL, 5, QSORT, squad_sort as usize);
            patch_call(DRAG_SAVE_CALL, 5, SAVE_SCREEN_IMAGE, drag_save as usize);
            // push -1; mov eax, fs:[0]
            CLUB_INPUT_ORIG = hook_entry(CLUB_INPUT, &[0x6a, 0xff, 0x64, 0xa1, 0x00, 0x00, 0x00, 0x00], club_input as usize);
            // push -1; push 0x9635fe
            TACTICS_INPUT_ORIG = hook_entry(TACTICS_INPUT, &[0x6a, 0xff, 0x68, 0xfe, 0x35, 0x96, 0x00], tactics_input as usize);
            // sub esp, 0x10; movsx eax, word [0xb77c7a]
            NEWS_INPUT_ORIG = hook_entry(NEWS_INPUT, &[0x83, 0xec, 0x10, 0x0f, 0xbf, 0x05, 0x7a, 0x7c, 0xb7, 0x00], news_input as usize);
            // sub esp, 0x794
            SEARCH_INPUT_ORIG = hook_entry(SEARCH_INPUT, &[0x81, 0xec, 0x94, 0x07, 0x00, 0x00], search_input as usize);
            // mov eax, [0xaebe0c]
            DARKEN_ORIG = hook_entry(DARKEN, &[0xa1, 0x0c, 0xbe, 0xae, 0x00], darken as usize);
            // mov eax, [0xaebe0c]
            PANEL_ORIG = hook_entry(DRAW_PANEL, &[0xa1, 0x0c, 0xbe, 0xae, 0x00], panel as usize);
            HIDE_CA_PA = setting("hide_ca_pa").as_deref() == Some("1");
            SHOW_DETAILS = setting("profile_details").as_deref() == Some("1");
            if setting("background_opacity").is_none() {
                set_setting("background_opacity", "30");
            }
            // sub esp, 0x200
            POLL_INPUT_ORIG = hook_entry(POLL_INPUT, &[0x81, 0xec, 0x00, 0x02, 0x00, 0x00], poll_input as usize);
            // push -1; push 0x95a91e
            TEXT_BOX_ORIG = hook_entry(DRAW_TEXT_BOX, &[0x6a, 0xff, 0x68, 0x1e, 0xa9, 0x95, 0x00], text_box as usize);
        }
    }
    1
}
