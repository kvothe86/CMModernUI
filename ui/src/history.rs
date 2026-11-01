//! A player's past on his profile: the last injuries and bans under the rows of Injuries & Bans
//! (switch `injury_history`), his moves under the rows of Transfer (switch `transfer_history`).
//! The same lists as the History tab (staff_screens_draw_history_tab 0x885820); a network client
//! asks the host for them (key 'I').

use crate::*;

const INJURY_LIST: usize = 0x62ec20; // thiscall(Injury*, Staff*) -> LLIST* or 0
const BAN_LIST: usize = 0x55cbc0; // thiscall(Discipline*, staff id) -> LLIST* or 0
pub const STAFF_RECORDS: usize = 0xdd7f00; // Staff_Records*
pub const STAFF_LIST: usize = 0x86de70; // thiscall(records, Staff*, mask) -> LLIST* or 0
const MOVES: u32 = 4; // the mask of kinds 100..149
const FORMAT_ENTRY: usize = 0x86e0b0; // thiscall(records, node, Text*)
const TEXT_NEW: usize = 0x4ae590; // thiscall(Text*, initial): +0 the string
const TEXT_FREE: usize = 0x4ae630; // thiscall(Text*)
const GET_PREV: usize = 0x68d1d0; // thiscall(LLIST*) -> item or 0
const LIST_FREE: usize = 0x68d040; // thiscall(LLIST*)
const DELETE: usize = 0x944c94; // cdecl(pointer)
const DATE_TEXT: usize = 0x54a650; // thiscall(Date*, out, size, 0xff: no time of day)
const INJURY_NAME: usize = 0x62ef30; // stdcall(type, out, size, with an article) (never reads ECX)
const DURATION_TEXT: usize = 0x7e5780; // cdecl(days, out, size, approximate)
const DESCRIBE_BAN: usize = 0x55cc20; // thiscall(Discipline*, ban, out, size)
const BAN_REASON: usize = 0x55d0a0; // thiscall(Discipline*, ban, out, size)
/// Injury record: +0 type, +0x20 date, +0x28 days out. Ban record: +4 date. History record: +0
/// date, +0xc kind, +0xd the other club's index in kinds 'f' and 'j'.
const LIMIT: usize = 12;

/// The items of a list the game made for this call, newest first.
pub unsafe fn newest(list: usize) -> Vec<usize> {
    let mut items = Vec::new();
    if list == 0 {
        return items;
    }
    ((list + 10) as *mut u32).write_unaligned(((list + 6) as *const u32).read_unaligned());
    let prev = game::<extern "thiscall" fn(usize) -> usize>(GET_PREV);
    while let Some(item) = Some(prev(list)).filter(|&i| i != 0) {
        items.push(item);
    }
    items
}

/// Frees a list `newest` read (its items go with it).
pub unsafe fn free(list: usize) {
    if list != 0 {
        game::<extern "thiscall" fn(usize)>(LIST_FREE)(list);
        game::<extern "C" fn(usize)>(DELETE)(list);
    }
}

fn line(buf: &[u8]) -> Vec<u8> {
    let mut t = buf.split(|&c| c == 0).next().unwrap_or(&[]).to_vec();
    if let Some(c) = t.first_mut() {
        *c = c.to_ascii_uppercase();
    }
    t
}

unsafe fn date_of(date: usize) -> Vec<u8> {
    let mut buf = [0u8; 40];
    game::<extern "thiscall" fn(usize, *mut u8, i16, i32) -> i32>(DATE_TEXT)(date, buf.as_mut_ptr(), 40, 0xff);
    line(&buf)
}

/// The game's own sentence for a history record.
pub unsafe fn sentence(records: usize, item: usize) -> Vec<u8> {
    let mut object = [0usize; 2];
    let text = object.as_mut_ptr() as usize;
    game::<extern "thiscall" fn(usize, usize) -> usize>(TEXT_NEW)(text, 0);
    game::<extern "thiscall" fn(usize, usize, usize)>(FORMAT_ENTRY)(records, item, text);
    // (not game_text: a short string on the heap's last bytes before a free page reads as none)
    let said = if object[0] != 0 { std::ffi::CStr::from_ptr(object[0] as *const i8).to_bytes().to_vec() } else { Vec::new() };
    game::<extern "thiscall" fn(usize)>(TEXT_FREE)(text);
    said
}

/// The rows as text: "kind \t date \t what \t more", newest first; kind I an injury (type, how long
/// out), B a ban (what, why), T a move (the game's own sentence, the club when it names one).
pub unsafe fn rows(staff: usize) -> Vec<u8> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut buf = [0u8; 300];
    let (injuries, discipline, records) = (rd::<usize>(assistant::INJURIES), rd::<usize>(assistant::DISCIPLINE), rd::<usize>(STAFF_RECORDS));
    // (no switch tests here: the machine that draws decides what it asks for)
    if mapped(injuries, 4) {
        let list = game::<extern "thiscall" fn(usize, usize) -> usize>(INJURY_LIST)(injuries, staff);
        for item in newest(list).into_iter().take(LIMIT) {
            game::<extern "stdcall" fn(i32, *mut u8, i32, i32) -> i32>(INJURY_NAME)(rd::<u8>(item) as i32, buf.as_mut_ptr(), 300, 0);
            let name = line(&buf);
            game::<extern "C" fn(i32, *mut u8, i32, i32) -> i32>(DURATION_TEXT)(rd::<i16>(item + 0x28) as i32, buf.as_mut_ptr(), 300, 0);
            out.push([&b"I"[..], &date_of(item + 0x20), &name, &line(&buf)].join(&b'\t'));
        }
        free(list);
    }
    if mapped(discipline, 4) {
        let list = game::<extern "thiscall" fn(usize, i32) -> usize>(BAN_LIST)(discipline, rd::<i32>(staff));
        for item in newest(list).into_iter().take(LIMIT) {
            game::<extern "thiscall" fn(usize, usize, *mut u8, i32) -> i32>(DESCRIBE_BAN)(discipline, item, buf.as_mut_ptr(), 300);
            let what = line(&buf);
            game::<extern "thiscall" fn(usize, usize, *mut u8, i32) -> i32>(BAN_REASON)(discipline, item, buf.as_mut_ptr(), 300);
            out.push([&b"B"[..], &date_of(item + 4), &what, &line(&buf)].join(&b'\t'));
        }
        free(list);
    }
    if mapped(records, 4) {
        let list = game::<extern "thiscall" fn(usize, usize, u32) -> usize>(STAFF_LIST)(records, staff, MOVES);
        for item in newest(list).into_iter().take(LIMIT) {
            let said = sentence(records, item);
            let index = rd::<i32>(item + 0xd);
            let named = [b'f', b'j'].contains(&rd::<u8>(item + 0xc)) && index >= 0 && index < rd::<i32>(CLUB_COUNT);
            let club = if named { game_text(rd::<usize>(CLUB_TABLE) + index as usize * CLUB_SIZE + 4, 51) } else { Vec::new() };
            out.push([&b"T"[..], &date_of(item), &said, &club].join(&b'\t'));
        }
        free(list);
    }
    out.join(&b'\n')
}

/// This frame's tab (1 Injuries & Bans, 2 Transfer), its first label and row height, the font, the
/// bottoms of its rows.
static mut TAB: (u8, i32, i32, i32, u32) = (0, 0, 0, 0, 0);
static mut BOTTOMS: Vec<i32> = Vec::new();

/// A text of the profile's middle band.
#[allow(static_mut_refs)]
pub unsafe fn note(x1: i32, y1: i32, y2: i32, font: u32, t: &[u8]) {
    // (the game's grid fills the band with empty cells: only rows that say something count)
    if y2 <= y1 || y2 - y1 >= 40 || t.trim_ascii().is_empty() {
        return;
    }
    let tab = match t {
        b"Injury" if on(INJURY_HISTORY) => 1,
        b"Availability" if on(TRANSFER_HISTORY) => 2,
        _ => 0,
    };
    if tab != 0 {
        TAB = (tab, x1, y1, y2 - y1, font);
    }
    if TAB.0 != 0 {
        BOTTOMS.push(y2);
    }
}

/// A list's title and its rows, as many as there is room for: date, crest, text, at the right end
/// what else the row says.
unsafe fn list(x: i32, right: i32, top: i32, bottom: i32, rh: i32, font: u32, title: &[u8], empty: &[u8], rows: &[Vec<&[u8]>]) {
    let head = rh + 8;
    text_at(x, top, top + head - 4, font, GOLD, title, 0);
    fill(x, top + head - 3, right, top + head - 2, [0x3a, 0x3f, 0x47]);
    if rows.is_empty() {
        text_at(x, top + head, top + head + rh, font, GREY, empty, 0);
    }
    let date_width = text_width(font, b"00.00.0000  ");
    for (k, r) in rows.iter().take(((bottom - top - head) / rh).max(0) as usize).enumerate() {
        let y = top + head + k as i32 * rh;
        let (what, last) = (r.get(2).copied().unwrap_or(b""), r.get(3).copied().unwrap_or(b""));
        text_at(x, y, y + rh, font, GREY, r[1], 0);
        let mut tx = x + date_width;
        // (T: the last field is the club of the sentence; the others say it at the right)
        let more = if r[0] == b"T" {
            if !last.is_empty() && draw_crest(last, tx, y + 1, rh - 2) {
                tx += rh + 4;
            }
            &b""[..]
        } else {
            last
        };
        let room = if more.is_empty() { 0 } else { (text_width(font, more) + 12).min((right - tx) / 2) };
        text_at(tx, y, y + rh, font, WHITE, &clip_text(font, what, right - tx - room), 0);
        text_at(right, y, y + rh, font, GOLD, &clip_text(font, more, room), 2);
    }
}

/// The tab's lists under its rows (the flip pass); nothing when this frame drew no such tab.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    let (tab, x, y, rh, font) = std::mem::take(&mut TAB);
    let x = x + 8; // (the game's labels stand that far inside their boxes)
    let font = if on(BIGGER_TEXT) { font_up(font, b"Ay\0".as_ptr() as *const i8, i32::MAX, rh, 1, 3) } else { font };
    let bottoms = std::mem::take(&mut BOTTOMS);
    // (first: screen parameters asked during day processing crash the game; a menu open over the
    // tab stays on top)
    if tab == 0 || news::MENU_OPEN {
        return;
    }
    let Some(staff) = profiled_staff().filter(|_| profiled_player().is_some()) else { return };
    let (_, sh) = screen();
    let end = if STATS_BUTTONS.0 > y { STATS_BUTTONS.0 } else { sh * 80 / 100 };
    let top = bottoms.iter().copied().filter(|&b| b < end).max().unwrap_or(y) + 12;
    let right = PANELS.iter().filter(|p| p.0 <= x && p.2 > x && p.1 <= y && p.3 >= y).map(|p| p.2).max().unwrap_or(TITLE.2).min(TITLE.2) - 14;
    // (on the host the staff pointer is checked too: any readable record was taken for a player)
    let text = if remote::CLIENT {
        ask_host(&key(b'I', &[staff as u32])).unwrap_or_default()
    } else if valid_staff(staff) {
        rows(staff)
    } else {
        return;
    };
    let parsed: Vec<Vec<&[u8]>> = text.split(|&c| c == b'\n').map(|l| l.split(|&c| c == b'\t').collect()).filter(|r: &Vec<&[u8]>| r.len() >= 3).collect();
    let of = |k: &[u8]| parsed.iter().filter(|r| r[0] == k).cloned().collect::<Vec<_>>();
    let bottom = end - 8;
    if tab == 1 {
        let mid = (x + right) / 2;
        list(x, mid - 16, top, bottom, rh, font, b"INJURY HISTORY", b"No injuries on record", &of(b"I"));
        list(mid + 16, right, top, bottom, rh, font, b"BAN HISTORY", b"No bans on record", &of(b"B"));
    } else {
        list(x, right, top, bottom, rh, font, b"TRANSFER HISTORY", b"No moves since the game began", &of(b"T"));
    }
}
