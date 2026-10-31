//! Assistant pick (switch `assistant_pick`; design in `assistant_pick.md`): an "Assistant" button
//! left of the squad screen's Clear Squad, with Best team, Most fit team, Squad rotation and
//! Clear team. The club's assistant manager fills the selection slots of the human's own club
//! as he sees the players: their ability through the game's own judging (0x6e86d0, his
//! Judging Ability sets the noise), position fit as his Tactics lets him read it (a fixed error
//! per assistant and player), condition and availability. He also names the captain and the
//! free kick and corner takers, and a line under the button tells what he did.

use crate::*;

const TACTICS: usize = 0xde1d08; // the Tactics store; team index 0 = the first team
const CLEAR_SELECTION: usize = 0x896fe0; // thiscall(club, team, use_undo, restore)
const SET_SLOT: usize = 0x896a90; // thiscall(club, slot 0..19, Staff*, team)
const GET_SLOT: usize = 0x8968f0; // thiscall(club, slot, team) -> Staff*
const SET_CAPTAIN_FROM_SLOT: usize = 0x897760; // thiscall(club, slot, team)
const SET_TAKER: usize = 0x897bb0; // thiscall(club, role 0..4, choice 0..1, Staff* or 0, team)
const CLUB_ASSISTANT: usize = 0x53cb80; // cdecl(club) -> Staff*
const JUDGE: usize = 0x6e86d0; // cdecl(judge, player, club, kind 2 current, reputation -1) -> short
const INJURY_STATUS: usize = 0x62c400; // thiscall(Injury*, Staff*) -> 0 fit, 1 short of fitness, 2 injured
pub(crate) const INJURIES: usize = 0xb63d28;
pub(crate) const DISCIPLINE: usize = 0xae28f0; // Discipline*
const COMP_TABLE: usize = 0xadadfc; // competitions by id, as the ban check reads them
const BANNED: usize = 0x55d270; // Discipline::is_player_banned(staff, comp, 1, 0) -> nonzero when banned
/// Player record attributes (as `scout.rs` has them): stored 1..20, or intrinsic (converted with
/// his CA the way the profile shows them).
const INFLUENCE: (usize, bool) = (0x2f, false);
/// The set piece roles of `Tactics` and the attribute each is picked by, as Team Instructions
/// shows them: 0 / 1 free kicks left / right (Set Pieces), 2 / 3 corners left / right (Corners).
/// (Role 4 is not shown there: left as it is.)
const ROLES: [(usize, bool); 4] = [(0x29, false), (0x29, false), (0x22, false), (0x22, false)];
/// Penalties (intrinsic) and the human's Penalty Takers list (places 0..49; the map calls it a
/// squad number table, 0x601200 / 0x601560; human 0 = the one at the screen).
const PENALTIES: (usize, bool) = (0x38, true);
const HUMAN_MANAGER: usize = 0xb63c98;
const SET_PENALTY_PLACE: usize = 0x601200; // thiscall(place, Staff*, human)
const CLEAR_PENALTY_PLACE: usize = 0x601560; // thiscall(place, human)

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Best,
    Fit,
    Rotation,
    Clear,
}
const ITEMS: [(Mode, &str); 4] = [(Mode::Best, "Best team"), (Mode::Fit, "Most fit team"), (Mode::Rotation, "Squad rotation"), (Mode::Clear, "Clear team")];

/// This frame's slot header texts (top, left, label) and the slots' labels in order (kept).
static mut SLOT_TEXTS: Vec<(i32, i32, Vec<u8>)> = Vec::new();
static mut SLOTS: Vec<Vec<u8>> = Vec::new();
/// Inf cells this frame (top, right, text), the players they mark as not available (this screen).
static mut INF_TEXTS: Vec<(i32, i32, Vec<u8>)> = Vec::new();
static mut UNAVAILABLE: Vec<usize> = Vec::new();
/// The Clear Squad button (box, font), the list's title band, the Assistant button, its menu.
static mut CLEAR_BUTTON: Option<((i32, i32, i32, i32), u32)> = None;
/// That button was drawn since the last full frame's flip: another tab or screen of the same
/// node has none, and the Assistant button goes with it.
static mut CLEAR_SEEN: bool = false;
static mut TITLE_BAND: Option<(i32, i32, i32, i32)> = None;
/// How wide the note may be at the band's left (its title stays clear).
static mut TITLE_FREE: i32 = 0;
static mut BUTTON: Option<(i32, i32, i32, i32)> = None;
static mut MENU_OPEN: bool = false;
static mut HOVER: Option<usize> = None;
/// What the assistant last did (shown under the buttons until the screen changes).
static mut NOTE: Vec<u8> = Vec::new();
/// A choice from the menu, for the club screen's input to carry out.
static mut PENDING: Option<Mode> = None;

// A network client's squad and selection are the host's: its choice goes to the host (request
// 'H': mode, request number, the players the client sees marked not available), whose club
// screen input makes the pick for that client's manager; the note comes back by request 'J'.
/// Client: a choice to send (by poll_input), the number of the last request, the one shown.
static mut CLIENT_SEND: Option<Mode> = None;
static mut ASKED: u32 = 0;
static mut NOTE_FOR: u32 = 0;
/// Host: a client's choice waiting for its screen input (channel, mode, not available, request
/// number), and the notes of the clients' picks (channel, request number, note).
static mut REMOTE_PENDING: Option<(i16, Mode, Vec<usize>, u32)> = None;
static mut REMOTE_NOTES: Vec<(i16, u32, Vec<u8>)> = Vec::new();

// The team report (switch `team_report`): a fifth menu item on the squad screen only, a full
// screen cmui draws over the game's until Back (the game sees no input meanwhile). What it shows
// is made once, here or for a network client by the host (key 'T'), as text: a line with club,
// assistant, shape and what he says, then one per player (`report_for`); `report` draws it.
const REPORT_ITEM: &str = "Team report";
const RIGHT_CLICK: usize = 0xb67a1c; // LEFT_CLICK's twin
/// The formations the assistant chooses from, the ten outfield slots as the original game's
/// presets have them. Not read from the game: Restructured 25-26 keeps the names on other layouts
/// (its "4-4-2" stands DC DC DMC MC MC MC AMC AMC AMC FC). The first nine are what the byte at
/// non-player record +0x43 prefers, 1 to 9, as the staff profile names them (+0x23, eight words,
/// is -1 throughout).
const FORMATIONS: [(&str, [&str; 10]); 11] = [
    ("5-3-2 Sweeper", ["SW", "DC", "DC", "WBL", "WBR", "MC", "MC", "MC", "FC", "FC"]),
    ("5-3-2", ["DC", "DC", "DC", "WBL", "WBR", "MC", "MC", "MC", "FC", "FC"]),
    ("4-4-2", ["DL", "DC", "DC", "DR", "ML", "MC", "MC", "MR", "FC", "FC"]),
    ("4-3-3", ["DL", "DC", "DC", "DR", "DMC", "MC", "MC", "AML", "AMR", "FC"]),
    ("4-2-4", ["DL", "DC", "DC", "DR", "MC", "MC", "AML", "AMR", "FC", "FC"]),
    ("3-5-2", ["DC", "DC", "DC", "DMC", "ML", "MC", "MC", "MR", "FC", "FC"]),
    ("4-1-2-1-2", ["DL", "DC", "DC", "DR", "DMC", "MC", "MC", "AMC", "FC", "FC"]),
    ("4-5-1", ["DL", "DC", "DC", "DR", "DMC", "ML", "MC", "MC", "MR", "FC"]),
    ("3-4-3", ["DC", "DC", "DC", "WBL", "WBR", "MC", "MC", "AML", "AMR", "FC"]),
    ("4-4-2 Diamond", ["DL", "DC", "DC", "DR", "DMC", "ML", "MR", "AMC", "FC", "FC"]),
    ("4-1-3-2", ["DL", "DC", "DC", "DR", "DMC", "MC", "MC", "MC", "FC", "FC"]),
];
static mut SQUAD_HERE: bool = false; // this is the squad screen (the tactics screen has no report)
static mut REPORT_OPEN: bool = false;
static mut REPORT: Vec<u8> = Vec::new(); // empty: a client's answer has not come yet
static mut REPORT_FONT: u32 = 0;
static mut REPORT_HOVER: bool = false;
static mut REPORT_KEY: Vec<u8> = Vec::new();
static mut REPORT_ASKS: u32 = 0;
static mut REPORT_LOOKED: Option<std::time::Instant> = None;
/// What is open is the scout's report on a club (switch `scout_screen`): the same screen, one
/// pitch and his text. Its chip on a "<club> scouting completed" story (box, club, font).
static mut REPORT_SCOUT: bool = false;
pub static mut SCOUT_CHIP: Option<((i32, i32, i32, i32), usize, u32)> = None;

/// The club screen's input or the tactics screen's (`tactics`), every poll: a pending choice
/// carried out, the host's own or a client's (when the screen manager serves that client's
/// channel). True: the screen is to be built again (its list shows the selection as it was when built).
#[allow(static_mut_refs)]
pub unsafe fn run_pending(tactics: bool) -> bool {
    if let Some(mode) = PENDING.take() {
        NOTE = pick(mode).unwrap_or_default();
        return true;
    }
    let serving = rd::<i16>(SCREEN_MANAGER + 0x382a);
    let Some((channel, mode, unavailable, number)) = REMOTE_PENDING.take_if(|p| p.0 == serving) else { return false };
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel.max(0) as usize);
    let humans = rd::<usize>(HUMAN_MANAGER);
    let note = if readable(manager, 0x3d) && mapped(humans, 4) {
        let club = rd::<usize>(manager + 0x39);
        // (the client on its tactics screen: that screen's own copy, as tactics_screen; the
        // parameters read are the served channel's)
        let ts = if tactics { screen_param(0) } else { 0 };
        let ts = (mapped(ts, 0x4a60) && rd::<usize>(ts) == club).then_some(ts);
        let formation = match ts {
            Some(ts) => ts + 0xe44,
            None => game::<extern "thiscall" fn(usize, usize) -> usize>(ACTIVE_TACTIC)(humans, manager),
        };
        if mapped(formation, 0x200) { pick_for(mode, club, manager, formation_slots(formation), unavailable, ts) } else { None }
    } else {
        None
    };
    REMOTE_NOTES.retain(|n| n.0 != channel);
    REMOTE_NOTES.push((channel, number, note.unwrap_or_else(|| b"The assistant could not pick a team.".to_vec())));
    true
}

/// Client: the request for the host when a choice was made ('H', mode, number, not available).
#[allow(static_mut_refs)]
pub unsafe fn client_request() -> Option<Vec<u8>> {
    let mode = CLIENT_SEND.take()?;
    ASKED += 1;
    let mut args = vec![ITEMS.iter().position(|i| i.0 == mode).unwrap_or(0) as u32, ASKED, UNAVAILABLE.len() as u32];
    args.extend(UNAVAILABLE.iter().map(|&s| s as u32));
    NOTE = b"Asking the assistant...".to_vec();
    Some(key(b'H', &args))
}

/// Host: a client's request 'H' (its words after the letter).
#[allow(static_mut_refs)]
pub unsafe fn remote_request(channel: i16, args: &[u32]) {
    let (Some(&mode), Some(&number), Some(&n)) = (args.first(), args.get(1), args.get(2)) else { return };
    let Some(&(mode, _)) = ITEMS.get(mode as usize) else { return };
    let unavailable = args.iter().skip(3).take(n.min(64) as usize).map(|&s| s as usize).filter(|&s| valid_staff(s)).collect();
    REMOTE_PENDING = Some((channel, mode, unavailable, number));
}

/// Host: the note of a client's pick ('J', its request number), None until it is made.
#[allow(static_mut_refs)]
pub unsafe fn remote_note(channel: i16, number: u32) -> Option<Vec<u8>> {
    REMOTE_NOTES.iter().find(|n| n.0 == channel && n.1 == number).map(|n| n.2.clone())
}

/// The eleven slots' labels of a formation, then nine bench slots.
unsafe fn formation_slots(formation: usize) -> Vec<Vec<u8>> {
    let mut slots: Vec<Vec<u8>> = (0..11).map(|k| label_of(game::<extern "thiscall" fn(usize, i32, i32) -> u32>(SLOT_POSITION)(formation, k, 0))).collect();
    slots.extend((1..=9).map(|n| format!("SB{n}").into_bytes()));
    slots
}

const SLOT_POSITION: usize = 0x5b2ae0; // Formation::get_slot_position(slot, attacking) -> u16 code
const ACTIVE_TACTIC: usize = 0x5fbd00; // HUMAN_MANAGER::get_active_tactic(human or 0) -> Formation*

/// A slot's label from its position code (checked against the squad screen's boxes): role bit
/// 0x1 GK, 0x2 SW, 0x4 D, 0x8 DM (on a side a wing back), 0x10 M, 0x20 AM, 0x40 F; side 0x80
/// left, 0x800 right, else centre (0x100 / 0x400 the two central spots).
fn label_of(code: u32) -> Vec<u8> {
    let side = if code & 0x80 != 0 { "L" } else if code & 0x800 != 0 { "R" } else { "C" };
    let label = match code & 0x7f {
        0x1 => "GK".to_string(),
        0x2 => "SW".to_string(),
        0x4 => format!("D{side}"),
        0x8 if side != "C" => format!("WB{side}"),
        0x8 => "DMC".to_string(),
        0x10 => format!("M{side}"),
        0x20 => format!("AM{side}"),
        0x40 => format!("F{side}"),
        _ => "MC".to_string(), // ponytail: an unknown role read as a centre midfielder
    };
    label.into_bytes()
}

/// The tactics screen's own object, before a match only (screen parameter 0; +0 its club).
unsafe fn tactics_screen() -> Option<usize> {
    if !crate::TACTICS || LIVE_TACTICS || SQUAD_CLUB == 0 {
        return None;
    }
    let ts = screen_param(0);
    (mapped(ts, 0x4a60) && rd::<usize>(ts) == SQUAD_CLUB).then_some(ts)
}

/// A new screen: what was seen of the last one is dropped.
#[allow(static_mut_refs)]
pub unsafe fn forget() {
    SLOTS.clear();
    UNAVAILABLE.clear();
    CLEAR_BUTTON = None;
    TITLE_BAND = None;
    BUTTON = None;
    MENU_OPEN = false;
    NOTE.clear();
    SQUAD_HERE = false;
    REPORT_OPEN = false;
}

fn slot_label(t: &[u8]) -> bool {
    (t.len() >= 3 && t.starts_with(b"SB") && t[2..].iter().all(u8::is_ascii_digit)) || SPOTS.iter().any(|s| s.4.as_bytes() == t) || [&b"DM"[..], b"AM", b"ST"].contains(&t)
}

/// A text of the squad screen (the human's own club only; called from the text hook).
#[allow(static_mut_refs)]
pub unsafe fn note_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, flags: u32, t: &[u8]) {
    let (w, h) = (x2 - x1, y2 - y1);
    let (sw, sh) = screen();
    // where the button goes: left of Clear Squad (squad screen; Undo Clear on that button while
    // the squad is cleared, 0x458a47) or of Last Match at the top (tactics screen; the squad
    // screen's own Last Match is at the bottom), in the same gap
    let squad = t == b"Clear Squad" || t == b"Undo Clear";
    if squad || (t == b"Last Match" && y1 < sh / 5) {
        CLEAR_BUTTON = Some(((x1, y1, x2, y2), font));
        CLEAR_SEEN = true;
        SQUAD_HERE = squad;
    } else if flags == 0xc && (40..=90).contains(&h) && y1 < sh / 3 && x1 < sw / 4 && (500..1000).contains(&w) {
        // the tactics list's band (the formation's name in the middle): the note at its left
        TITLE_BAND = Some((x1, y1, x2, y2));
        TITLE_FREE = (w - text_width(font, t)) / 2 - 24;
    } else if slot_label(t) && h < 50 && w < 160 {
        if SLOT_TEXTS.len() < 64 {
            SLOT_TEXTS.push((y1, x1, t.to_vec()));
        }
    } else if [&b"Inj"[..], b"Sus", b"Ban", b"Int"].contains(&t) && h < 40 {
        if INF_TEXTS.len() < 64 {
            INF_TEXTS.push((y1, x2, t.to_vec()));
        }
    } else if h < 40 && INF_TEXTS.iter().any(|i| i.0 == y1 && i.1 <= x1 + 4 && x1 - i.1 < 120) {
        // the name right of an Inf cell, in any view (the squad list's own name check only looks
        // at the narrow name cells of its first view)
        if let Some(staff) = squad_member(t) {
            note_member(y1, x1, staff);
        }
    } else if flags == 0xc && w > 1000 && (40..=90).contains(&h) && y1 < sh / 2 {
        TITLE_BAND = Some((x1, y1, x2, y2));
        TITLE_FREE = (w - text_width(font, t)) / 2 - 40;
    }
}

/// A squad member's name cell (top, left): an Inf cell just left of it on the row marks him.
#[allow(static_mut_refs)]
pub unsafe fn note_member(y1: i32, x1: i32, staff: usize) {
    if INF_TEXTS.iter().any(|i| i.0 == y1 && i.1 <= x1 + 4 && x1 - i.1 < 120) && !UNAVAILABLE.contains(&staff) && UNAVAILABLE.len() < 64 {
        UNAVAILABLE.push(staff);
    }
}

/// The flip pass: the slots' labels from this frame's header row (the row of position boxes that
/// starts with GK), then the button, its menu and the note.
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    // (taken on every frame: a part frame's Clear Squad must not count for the next full frame)
    if !std::mem::take(&mut CLEAR_SEEN) && full_frame {
        CLEAR_BUTTON = None;
    }
    let texts = std::mem::take(&mut SLOT_TEXTS);
    INF_TEXTS.clear();
    let mut rows: Vec<i32> = texts.iter().map(|t| t.0).collect();
    rows.sort();
    rows.dedup();
    // (the boxes are not in slot order: each is a widget of kind 0xa whose +0x48 is its slot, as
    // the squad screen's drop reads it, 0x468b90)
    let display = rd::<usize>(CHANNEL_DISPLAY + 0x300 * channel());
    let count = if mapped(display + 0x12e9a0, 2) { rd::<i16>(display + 0x12e9a0).clamp(0, 1200) as usize } else { 0 };
    let boxes: Vec<([i32; 4], usize)> = if mapped(display + 0xba95e, count * 0x18c) {
        (0..count)
            .map(|k| display + 0xba95e + k * 0x18c)
            .filter(|&w| rd::<i16>(w + 8) == 0xa)
            .map(|w| ([0, 1, 2, 3].map(|i| rd::<i32>(w + 0x10 + 4 * i)), rd::<u32>(w + 0x48) as usize & 0xff))
            .collect()
    } else {
        Vec::new()
    };
    for y in rows {
        let row: Vec<&(i32, i32, Vec<u8>)> = texts.iter().filter(|t| t.0 == y).collect();
        if row.len() < 12 || !row.iter().any(|t| t.2 == b"GK") {
            continue;
        }
        let mut slots: Vec<Vec<u8>> = vec![Vec::new(); 20];
        for t in row {
            if let Some(&(_, slot)) = boxes.iter().find(|b| (b.0[0]..=b.0[2]).contains(&(t.1 + 2)) && (b.0[1]..=b.0[3]).contains(&(t.0 + 2)) && b.1 < 20) {
                slots[slot] = t.2.clone();
            }
        }
        if slots[..11].iter().all(|s| !s.is_empty()) {
            let used = slots.iter().rposition(|s| !s.is_empty()).map_or(0, |p| p + 1);
            slots.truncate(used);
            SLOTS = slots;
        }
    }
    // the tactics screen (before a match) has no slot boxes: its formation's slots. The squad
    // screen's boxes give the size of the bench, the labels come from its formation too: a box's
    // text can leave out the side of a wide DM or AM slot (0x5b7810 line only), and a box under
    // the mouse is drawn another way
    let formation = match tactics_screen() {
        Some(ts) => Some(ts + 0xe44),
        None if (SQUAD_HERE || SLOTS.len() < 11) && CLEAR_BUTTON.is_some() && !remote::CLIENT => {
            let humans = rd::<usize>(HUMAN_MANAGER);
            let f = if mapped(humans, 4) { game::<extern "thiscall" fn(usize, usize) -> usize>(ACTIVE_TACTIC)(humans, 0) } else { 0 };
            mapped(f, 0x200).then_some(f)
        }
        None => None,
    };
    if let Some(f) = formation {
        let keep = if SLOTS.len() > 11 { SLOTS.len() } else { 20 };
        SLOTS = formation_slots(f);
        SLOTS.truncate(keep);
    }
    let own = human_club_ptr().is_some_and(|c| c == SQUAD_CLUB && c != 0);
    // (a network client: the squad screen and the tactics screen before a match; the host knows
    // its slots and assistant)
    let ready = if remote::CLIENT { !LIVE_TACTICS } else { SLOTS.len() >= 11 };
    let Some((clear, font)) = CLEAR_BUTTON.filter(|_| own && ready) else {
        BUTTON = None;
        return;
    };
    // a client's note from the host, once its pick is made
    if remote::CLIENT && ASKED > NOTE_FOR {
        if let Some(v) = ask_host(&key(b'J', &[ASKED])).filter(|v| !v.is_empty()) {
            NOTE = v;
            NOTE_FOR = ASKED;
        }
    }
    let w = clear.2 - clear.0;
    let b = (clear.0 - 6 - w, clear.1, clear.0 - 6, clear.3);
    BUTTON = Some(b);
    let has = remote::CLIENT || assistant().is_some();
    // the game's own button look: face and bevel taken from Clear Squad as drawn this frame
    let at = |x: i32, y: i32, or: [u8; 3]| pixel_at(x, y).unwrap_or(or);
    let mid = (clear.1 + clear.3) / 2;
    let face = at(clear.0 + 6, clear.1 + 5, [0x88, 0x8c, 0x92]);
    let face = if MENU_OPEN { mix(face, [0, 0, 0], 85) } else { face };
    fill(b.0, b.1, b.2, b.3, face);
    fill(b.0, b.1, b.2, b.1 + 1, at(clear.0 + 6, clear.1, face));
    fill(b.0, b.1, b.0 + 1, b.3, at(clear.0, mid, face));
    fill(b.0, b.3 - 1, b.2, b.3, at(clear.0 + 6, clear.3 - 1, face));
    fill(b.2 - 1, b.1, b.2, b.3, at(clear.2 - 1, mid, face));
    let label = if has { "Assistant" } else { "No assistant" };
    // (the size Clear Squad comes out at beside it)
    let size = roomier_font(clear.0, clear.1, clear.2, clear.3, 0xc, font, b"Clear Squad\0".as_ptr() as *const i8, -1);
    put(b.0, b.1, b.2 - 18, b.3, 0xc, size, if has { WHITE } else { [0x5a, 0x60, 0x68] }, label);
    // a small chevron at the right, dark as on the game's dropdown buttons
    let (cx, cy) = (b.2 - 16, (b.1 + b.3) / 2);
    let ink = [0x14, 0x16, 0x1a];
    for i in 0..4 {
        fill(cx - 4 + i, cy - 2 + i, cx - 2 + i, cy + i, ink);
        fill(cx + 3 - i, cy - 2 + i, cx + 5 - i, cy + i, ink);
    }
    if MENU_OPEN {
        for k in 0..menu_len() {
            let name = ITEMS.get(k).map_or(REPORT_ITEM, |i| i.1);
            let r = item_rect(b, k);
            fill(r.0, r.1, r.2, r.3, if HOVER == Some(k) { SELECTED } else { [0x22, 0x25, 0x2a] });
            fill(r.0, r.3 - 1, r.2, r.3, [0x34, 0x38, 0x40]);
            put(r.0 + 14, r.1, r.2, r.3, 0x9, font, if HOVER == Some(k) { GOLD } else { LIGHT }, name);
        }
    }
    if let (false, Some(t)) = (NOTE.is_empty(), TITLE_BAND) {
        let small = smaller(font, 0);
        let parts: Vec<&[u8]> = NOTE.split(|&c| c == b'\n').map(|p| p.trim_ascii()).collect();
        let (head, why, takers) = (parts.first().copied().unwrap_or(b""), parts.get(1).copied().unwrap_or(b""), parts.get(2).copied().unwrap_or(b""));
        // the second line: why and who takes what, or only who takes what where that does not fit
        let both = [why, takers].iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(&b' ');
        let second = if text_width(small, &both) <= TITLE_FREE { both } else if !takers.is_empty() { takers.to_vec() } else { why.to_vec() };
        let lines: Vec<&[u8]> = [head, &second[..]].into_iter().filter(|l| !l.is_empty()).collect();
        let h = (t.3 - t.1) / lines.len().max(1) as i32;
        for (k, line) in lines.iter().enumerate() {
            let y = t.1 + k as i32 * h + if lines.len() > 1 { if k == 0 { 4 } else { -4 } } else { 0 };
            // (cut after a whole phrase: at its last ", " or ". ", else a whole word)
            let mut text = clip_text(small, line, TITLE_FREE.max(60));
            if text.len() < line.len() {
                let phrase = text.windows(2).rposition(|w| w == b", " || w == b". ");
                text.truncate(phrase.or_else(|| text.iter().rposition(|&c| c == b' ')).unwrap_or(text.len()));
                if phrase.is_some() {
                    text.push(b'.');
                }
            }
            text_at(t.0 + 16, y, y + h, small, if k == 0 { LIGHT } else { GREY }, &text, 0);
        }
    }
}

fn item_rect(b: (i32, i32, i32, i32), k: usize) -> (i32, i32, i32, i32) {
    let h = b.3 - b.1;
    (b.0, b.3 + 2 + k as i32 * h, b.2, b.3 + 2 + (k as i32 + 1) * h)
}

/// The menu's items: the four picks, and on the squad screen the team report.
unsafe fn menu_len() -> usize {
    ITEMS.len() + (SQUAD_HERE && !crate::TACTICS && !LIVE_TACTICS && on(TEAM_REPORT)) as usize
}

/// The report's Back button.
fn back_rect() -> (i32, i32, i32, i32) {
    let (sw, sh) = screen();
    let m = report_margin(sh);
    (sw - m - sw / 8, sh - m - sh / 20, sw - m, sh - m)
}

fn report_margin(sh: i32) -> i32 {
    (sh / 88).max(6)
}

/// The Team report item: the report made here, or asked of the host when the screen is drawn.
#[allow(static_mut_refs)]
unsafe fn open_report() {
    REPORT_OPEN = true;
    REPORT_SCOUT = false;
    REPORT_HOVER = false;
    REPORT.clear();
    REPORT_FONT = CLEAR_BUTTON.map_or(0, |b| b.1);
    if remote::CLIENT {
        REPORT_ASKS += 1;
        let mut args = vec![REPORT_ASKS, UNAVAILABLE.len() as u32];
        args.extend(UNAVAILABLE.iter().map(|&s| s as u32));
        REPORT_KEY = key(b'T', &args);
        REPORT_LOOKED = None;
    } else if let Some(club) = human_club_ptr() {
        REPORT = report_for(club, SLOTS.clone(), UNAVAILABLE.clone());
    }
}

/// Host: a client's request 'T' (its words after the letter: a number, not available), while the
/// screen manager serves that channel: the report for its manager.
pub unsafe fn remote_report(channel: i16, args: &[u32]) -> Vec<u8> {
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel.max(0) as usize);
    let humans = rd::<usize>(HUMAN_MANAGER);
    if !readable(manager, 0x3d) || !mapped(humans, 4) {
        return b"\n".to_vec();
    }
    let n = args.get(1).copied().unwrap_or(0).min(64) as usize;
    let unavailable = args.iter().skip(2).take(n).map(|&s| s as usize).filter(|&s| valid_staff(s)).collect();
    let formation = game::<extern "thiscall" fn(usize, usize) -> usize>(ACTIVE_TACTIC)(humans, manager);
    let slots = if mapped(formation, 0x200) { formation_slots(formation) } else { Vec::new() };
    report_for(rd::<usize>(manager + 0x39), slots, unavailable)
}

#[cfg(test)]
#[test]
fn staff_opinions_read_right() {
    let say = |now, future, edge| String::from_utf8(opinion(b"Kok", "coach", b"Asante", b"Bos", now, future, edge)).unwrap();
    assert_eq!(say((70.0, 60.0), (70.0, 60.0), None), "*Kok* (coach): *Asante* is clearly the better player.");
    assert_eq!(say((60.0, 64.0), (60.0, 64.0), None), "*Kok* (coach): *Bos* is a little better than *Asante*.");
    assert_eq!(say((70.0, 60.0), (70.0, 80.0), None), "*Kok* (coach): *Asante* is clearly the better player, but *Bos* has more to come.");
    assert_eq!(say((60.0, 60.5), (60.0, 60.0), Some((&b"Bos"[..], "on the ball"))), "*Kok* (coach): there is little between *Asante* and *Bos*, but on the ball I would take *Bos*.");
    assert_eq!(say((70.0, 60.0), (70.0, 60.0), Some((&b"Asante"[..], "in attitude"))), "*Kok* (coach): *Asante* is clearly the better player, also in attitude I would take *Asante*.");
}

#[cfg(test)]
#[test]
fn scout_words() {
    assert_eq!(line_of(b"DMC"), 1);
    assert_eq!(line_of(b"DL"), 0);
    assert_eq!(line_of(b"WBR"), 0);
    assert_eq!(line_of(b"AMC"), 2);
    assert_eq!(style_text(16, 16), "direct, attacking football");
    assert_eq!(style_text(10, 10), "balanced football");
    assert_eq!(side_word(5.0), "stronger than us");
    assert_eq!(side_word(-2.0), "about our level");
}

/// The scout's report chip: made here, or asked of the host when the screen is drawn.
#[allow(static_mut_refs)]
unsafe fn open_scout(club: usize, font: u32) {
    REPORT_OPEN = true;
    REPORT_SCOUT = true;
    REPORT_HOVER = false;
    REPORT.clear();
    REPORT_FONT = font;
    if remote::CLIENT {
        REPORT_ASKS += 1;
        REPORT_KEY = key(b'B', &[REPORT_ASKS, club as u32]);
        REPORT_LOOKED = None;
    } else if let Some(own) = human_club_ptr() {
        REPORT = scout_for(club, own);
    } else {
        REPORT = b"\n".to_vec(); // (no club: nothing to write, not "Asking the scout...")
    }
}

/// Host: a client's request 'B': the report for that client's club.
pub unsafe fn remote_scout(channel: i16, club: usize) -> Vec<u8> {
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel.max(0) as usize);
    if readable(manager, 0x3d) { scout_for(club, rd::<usize>(manager + 0x39)) } else { b"\n".to_vec() }
}

/// Who judges for a club: its best scout by Judging Ability (club +0x1b3: seven), else its assistant.
unsafe fn best_judge(club: usize) -> Option<usize> {
    if !readable(club, 0x1b3 + 28) {
        return None;
    }
    let scouts = (0..7).map(|k| rd::<usize>(club + 0x1b3 + 4 * k)).filter(|&s| readable(s, 0x6e) && readable(rd::<usize>(s + 0x69), 68));
    scouts.max_by(|&x, &y| coaching(x, 0x17).total_cmp(&coaching(y, 0x17))).or_else(|| assistant_of(club))
}

/// The manager's football, as the game's own report words it (0x7be670): his Attacking and
/// Directness (non-player +0xe, +0x13) at 15 or more, or 5 or less.
fn style_text(attacking: i8, direct: i8) -> &'static str {
    match (attacking >= 15, attacking <= 5, direct >= 15, direct <= 5) {
        (true, _, true, _) => "direct, attacking football",
        (true, _, _, true) => "patient, attacking football",
        (true, _, _, _) => "attacking football",
        (_, true, true, _) => "cautious football with long balls forward",
        (_, true, _, _) => "cautious football",
        (_, _, true, _) => "direct football",
        (_, _, _, true) => "patient football",
        _ => "balanced football",
    }
}

/// How the scout rates a side against his own, as the game's report does: `gap` their best
/// eleven's score less ours (half a star is about 5).
fn side_word(gap: f64) -> &'static str {
    match gap {
        g if g > 8.0 => "a lot stronger than us",
        g if g > 3.0 => "stronger than us",
        g if g >= -3.0 => "about our level",
        g if g >= -8.0 => "weaker than us",
        _ => "a lot weaker than us",
    }
}

/// Defence, midfield or attack by a slot's label.
fn line_of(label: &[u8]) -> usize {
    match label {
        l if l == b"GK" || l.starts_with(b"SW") || l.starts_with(b"D") && !l.starts_with(b"DM") || l.starts_with(b"WB") => 0,
        l if l.starts_with(b"AM") || l.starts_with(b"F") || l.starts_with(b"ST") => 2,
        _ => 1,
    }
}

/// What the scout writes: `first` the expected eleven by slot, `out` those missing with the reason.
fn scout_text(club: &[u8], manager: &[u8], form: &[u8], style: &str, level: &str, labels: &[&[u8]], first: &[Option<Man>], out: &[(Vec<u8>, &str)]) -> Vec<u8> {
    let side: Vec<(usize, &Man)> = first.iter().enumerate().filter_map(|(s, m)| Some((s, m.as_ref()?))).collect();
    let mut text = [b"*", club, b"* should line up in a *", form, b"*, the shape *", manager, b"* prefers, and play ", style.as_bytes(), b"."].concat();
    let mut say = |parts: &[&[u8]]| {
        text.extend_from_slice(b" | ");
        parts.iter().for_each(|p| text.extend_from_slice(p));
    };
    if side.is_empty() {
        return text;
    }
    say(&[b"I rate them ", level.as_bytes(), b"."]);
    let mut best = side.clone();
    best.sort_by(|x, y| y.1 .1.total_cmp(&x.1 .1));
    if let [a, b, ..] = best[..] {
        say(&[b"Watch out for *", &a.1 .0, b"* (", labels[a.0], b") and *", &b.1 .0, b"* (", labels[b.0], b")."]);
    }
    let lines: Vec<f64> = (0..3)
        .map(|l| {
            let scores: Vec<f64> = side.iter().filter(|m| line_of(labels[m.0]) == l).map(|m| m.1 .1).collect();
            if scores.is_empty() { f64::MAX } else { scores.iter().sum::<f64>() / scores.len() as f64 }
        })
        .collect();
    let weakest = (0..3).min_by(|&x, &y| lines[x].total_cmp(&lines[y])).unwrap_or(0);
    let target = side.iter().filter(|m| line_of(labels[m.0]) == weakest).min_by(|x, y| x.1 .1.total_cmp(&y.1 .1));
    if let Some(t) = target {
        say(&[b"Their ", ["defence", "midfield", "attack"][weakest].as_bytes(), b" is the weakest part of the side, and *", &t.1 .0, b"* at ", labels[t.0], b" is the man to target."]);
    }
    if out.is_empty() {
        say(&[b"They have no one of note missing."]);
    } else {
        let names: Vec<Vec<u8>> = out.iter().map(|(n, why)| [b"*", n.as_slice(), b"* (", why.as_bytes(), b")"].concat()).collect();
        say(&[b"Missing: ", &names.join(&b", "[..]), b"."]);
    }
    text
}

/// The scout's report on a club, in `report_for`'s format: the eleven he expects in the shape its
/// manager prefers (the game's own report names the same one, 0x7be670: club +0xcf, else the
/// assistant +0xd3, non-player +0x43), those banned from its league matches or injured left out,
/// as `own`'s best judge rates them. Nothing is written to the selection or the tactic.
unsafe fn scout_for(club: usize, own: usize) -> Vec<u8> {
    if !readable(club, 0x1e0 + 80) || !readable(own, 0x1b3 + 28) {
        return b"\n".to_vec(); // (no club to read)
    }
    let boss = [0xcf, 0xd3].into_iter().map(|at| rd::<usize>(club + at)).find(|&b| readable(b, 0x6e) && readable(rd::<usize>(b + 0x69), 68));
    let (Some(judge), Some(boss)) = (best_judge(own), boss) else { return b"\n".to_vec() };
    let np = rd::<usize>(boss + 0x69);
    let (form, spots) = (rd::<i8>(np + 0x43) as usize).checked_sub(1).and_then(|k| FORMATIONS[..9].get(k)).unwrap_or(&FORMATIONS[2]);
    let labels: Vec<&[u8]> = std::iter::once("GK").chain(spots.iter().copied()).map(str::as_bytes).collect();
    let eye = Eye { mode: Mode::Best, id: rd::<i32>(judge), reading: coaching(judge, 0x21) };
    // (as `report_for`: the ban check takes a banned player out of the club's selection)
    let selection = ((club + 0x1e0) as *const [usize; 20]).read_unaligned();
    let pool = candidates_in(judge, club, rd::<usize>(club + 0x57), &[], &[]);
    ((club + 0x1e0) as *mut [usize; 20]).write_unaligned(selection);
    let first = eleven(&eye, &pool, &labels);
    let first: Vec<Option<usize>> = first.iter().map(|c| c.map(|i| pool[i].staff)).collect();
    let rest: Vec<Candidate> = pool.iter().copied().filter(|c| !first.contains(&Some(c.staff))).collect();
    let second = eleven(&eye, &rest, &labels);
    let second: Vec<Option<usize>> = second.iter().map(|c| c.map(|i| rest[i].staff)).collect();
    let (rows, missing) = team_rows(club, judge, &pool, &labels, &first, &second);
    let injuries = rd::<usize>(INJURIES);
    let hurt = |staff: usize| mapped(injuries, 4) && game::<extern "thiscall" fn(usize, usize) -> i32>(INJURY_STATUS)(injuries, staff) & 0xff == 2;
    let out: Vec<(Vec<u8>, &str)> = missing.iter().take(4).map(|m| (short_name(m.0), if hurt(m.0) { "injured" } else { "banned" })).collect();
    let men: Vec<Option<Man>> = (0..11).map(|s| first[s].map(|staff| (short_name(staff), scout::seen_score(staff, judge, Some(labels[s])).unwrap_or(0.0)))).collect();
    let (name, by) = (game_text(club + 4, 51), staff_name(judge));
    let style = style_text(rd::<i8>(np + 0xe), rd::<i8>(np + 0x13));
    let level = side_word(scout::strength(club) - scout::strength(own));
    let says = scout_text(&name, &staff_name(boss), form.as_bytes(), style, level, &labels, &men, &out);
    let head: [&[u8]; 4] = [&name, &by, form.as_bytes(), &says];
    let mut lines = vec![head.join(&b'\t')];
    lines.extend(rows);
    lines.join(&b'\n')
}

/// What a coach looks at most, by the coaching attribute he is best at (non-player offset, as the
/// game keeps it or converted, what he would say) and the players' attributes that go with it.
const SPECIALITIES: [(usize, bool, &str, [&str; 3]); 4] = [
    (0x12, false, "on the ball", ["Technique", "Dribbling", "Passing"]),
    (0xe, false, "in front of goal", ["Finishing", "Off The Ball", "Long Shots"]),
    (0x21, true, "in reading the game", ["Anticipation", "Decisions", "Positioning"]),
    (0x1b, true, "in attitude", ["Determination", "Work Rate", "Teamwork"]),
];

/// One staff member's sentence on two players. `now` and `future` their scores as he sees them,
/// `edge` who he prefers in what he looks at most, and that phrase.
fn opinion(who: &[u8], job: &str, a: &[u8], b: &[u8], now: (f64, f64), future: (f64, f64), edge: Option<(&[u8], &str)>) -> Vec<u8> {
    let (better, worse, gap, rise) = if now.0 >= now.1 { (a, b, now.0 - now.1, future.1 - future.0) } else { (b, a, now.1 - now.0, future.0 - future.1) };
    let mut text = [b"*", who, b"* (", job.as_bytes(), b"): "].concat();
    let start: Vec<&[u8]> = match gap {
        g if g < 2.0 => vec![b"there is little between *", a, b"* and *", b, b"*"],
        g if g < 6.0 => vec![b"*", better, b"* is a little better than *", worse, b"*"],
        _ => vec![b"*", better, b"* is clearly the better player"],
    };
    text.extend_from_slice(&start.concat());
    if rise > 2.0 {
        text.extend_from_slice(&[b", but *", worse, b"* has more to come"].concat());
    } else if let Some((leader, what)) = edge {
        let join: &[u8] = if leader == better && gap >= 2.0 { b", also " } else { b", but " };
        text.extend_from_slice(&[join, what.as_bytes(), b" I would take *", leader, b"*"].concat());
    }
    text.push(b'.');
    text
}

/// What the assistant and the two best judges among the coaches of a human's club (club +0x19f:
/// five) say on two players, a line each.
pub unsafe fn staff_views(channel: i16, s1: usize, s2: usize) -> Vec<u8> {
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel.max(0) as usize);
    if !readable(manager, 0x3d) {
        return Vec::new();
    }
    let club = rd::<usize>(manager + 0x39);
    if !readable(club, 0x19f + 20) {
        return Vec::new(); // (no club)
    }
    let assistant = assistant_of(club);
    let mut coaches: Vec<usize> = (0..5).map(|k| rd::<usize>(club + 0x19f + 4 * k)).filter(|&c| Some(c) != assistant && readable(c, 0x6e) && readable(rd::<usize>(c + 0x69), 68)).collect();
    coaches.sort_by(|&x, &y| coaching(y, 0x17).total_cmp(&coaching(x, 0x17)));
    let staff = assistant.into_iter().map(|a| (a, "assistant")).chain(coaches.into_iter().map(|c| (c, "coach"))).take(3);
    let (a1, a2) = (attributes(s1), attributes(s2));
    let (n1, n2) = (short_name(s1), short_name(s2));
    let value = |a: &[u8; 31], names: &[&str; 3]| names.iter().filter_map(|n| ATTRIBUTE_NAMES.iter().position(|x| x == n)).map(|k| a[k] as i32).sum::<i32>();
    let mut lines = Vec::new();
    for (coach, job) in staff {
        let np = rd::<usize>(coach + 0x69);
        let level = |s: &(usize, bool, &str, [&str; 3])| if s.1 { coaching(coach, s.0) } else { rd::<i8>(np + s.0) as f64 };
        let liked = SPECIALITIES.iter().max_by(|x, y| level(x).total_cmp(&level(y))).unwrap_or(&SPECIALITIES[0]);
        let (v1, v2) = (value(&a1, &liked.3), value(&a2, &liked.3));
        let edge = if (v1 - v2).abs() >= 3 { Some((if v1 > v2 { n1.as_slice() } else { n2.as_slice() }, liked.2)) } else { None };
        let seen = |s: usize| scout::seen_score(s, coach, None).unwrap_or(0.0);
        let coming = |s: usize| scout::seen_future(s, coach).unwrap_or(0.0);
        lines.push(opinion(&short_name(coach), job, &n1, &n2, (seen(s1), seen(s2)), (coming(s1), coming(s2)), edge));
    }
    lines.join(&b'\n')
}

/// How strong a formation's best eleven looks to the assistant: his stars' score for each slot,
/// less what he reads as out of position, on average.
unsafe fn strength(eye: &Eye, a: usize, pool: &[Candidate], labels: &[&[u8]]) -> f64 {
    let team = eleven(eye, pool, labels);
    team.iter().zip(labels).filter_map(|(c, l)| Some((&pool[(*c)?], *l))).map(|(c, l)| scout::seen_score(c.staff, a, Some(l)).unwrap_or(0.0) * fit_factor(eye.seen_fit(c, l))).sum::<f64>() / 11.0
}

/// What an assistant adds to a formation's strength, in score points (half a star is about 5):
/// his preferred shape, the less the better his Tactics as he then lets the squad decide, and
/// his Attacking for how many men play up front (two are neutral).
fn liking(name: &[u8], labels: &[&[u8]], preferred: &str, attacking: f64, reading: f64) -> f64 {
    let shape = if !preferred.is_empty() && name.starts_with(preferred.as_bytes()) { 6.0 - reading * 0.2 } else { 0.0 };
    let up_front = labels.iter().filter(|l| l.starts_with(b"F") || l.starts_with(b"AM") || l.starts_with(b"ST")).count() as f64;
    shape + (attacking - 10.0) * 0.1 * (up_front - 2.0)
}

/// Two formations with the same slots, in any order.
fn same_slots(a: &[&[u8]], b: &[&[u8]]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort();
    b.sort();
    a == b
}

/// What he says about the formation: `own` the one in use, `chosen` his, `strongest` the one that
/// looks strongest to him, `preferred` the shape he likes.
fn formation_text(own: &[u8], chosen: &[u8], preferred: &str, strongest: &[u8]) -> Vec<u8> {
    let his = !preferred.is_empty() && chosen.starts_with(preferred.as_bytes());
    let parts: Vec<&[u8]> = if chosen == own && his {
        vec![b"Your *", own, b"* is the shape I believe in and it suits this squad, so I would keep it."]
    } else if preferred.is_empty() && chosen == own {
        vec![b"Your *", own, b"* gets the most out of this squad, so I would keep it."]
    } else if preferred.is_empty() {
        vec![b"This squad suits the *", chosen, b"* best, so I would play that instead of your ", own, b"."]
    } else if chosen == own {
        vec![b"I usually go for a ", preferred.as_bytes(), b", but your *", own, b"* gets more out of this squad, so I would keep it."]
    } else if his && chosen == strongest {
        vec![b"I would play the *", chosen, b"* instead of your ", own, b": it is the shape I believe in and it suits this squad best."]
    } else if his {
        vec![b"I would play the *", chosen, b"* instead of your ", own, b": it is the shape I believe in, although the ", strongest, b" looks a little stronger on paper."]
    } else {
        vec![b"I usually go for a ", preferred.as_bytes(), b", but this squad suits the *", chosen, b"* better, so I would play that instead of your ", own, b"."]
    };
    parts.concat()
}

/// The report as text (latin-1, as the game has the names): "club \t assistant \t his formation \t
/// what he says", then a line per player of that formation, the best eleven, the backup eleven,
/// then those not available who would have been in them, each by line of the pitch and as the
/// assistant rates them: "name \t short name \t positions \t role \t slot \t slot's label \t half
/// stars", role 1 the best eleven, 2 the backup eleven, x not available. Nothing is written to
/// the selection or the tactic.
unsafe fn report_for(club: usize, slots: Vec<Vec<u8>>, unavailable: Vec<usize>) -> Vec<u8> {
    let Some(a) = assistant_of(club).filter(|_| slots.len() >= 11) else { return b"\n".to_vec() };
    let eye = Eye { mode: Mode::Best, id: rd::<i32>(a), reading: coaching(a, 0x21) };
    // (the ban check drops a banned player from the club's selection, 20 slots at +0x1e0: put
    // back, a report changes nothing)
    let selection = ((club + 0x1e0) as *const [usize; 20]).read_unaligned();
    let pool = candidates(a, club, next_fixture(None, club), &unavailable, &[]);
    ((club + 0x1e0) as *mut [usize; 20]).write_unaligned(selection);
    // his formation, for this report only: of `FORMATIONS` and the one in use, the strongest in
    // his eyes as he likes it (the first of equals, so the one in use stays on a tie)
    let np = rd::<usize>(a + 0x69);
    let preferred = (rd::<i8>(np + 0x43) as usize).checked_sub(1).and_then(|k| FORMATIONS[..9].get(k)).map_or("", |f| f.0);
    let attacking = rd::<i8>(np + 0xe) as f64;
    let table: Vec<(Vec<u8>, Vec<&[u8]>)> = FORMATIONS.iter().map(|f| (f.0.as_bytes().to_vec(), std::iter::once("GK").chain(f.1).map(str::as_bytes).collect())).collect();
    let own: Vec<&[u8]> = slots[..11].iter().map(|s| s.as_slice()).collect();
    let own_form = table.iter().find(|f| same_slots(&f.1, &own)).map_or_else(|| shape(&own).into_bytes(), |f| f.0.clone());
    let rate = |name: &[u8], labels: &[&[u8]]| {
        let value = strength(&eye, a, &pool, labels);
        (value, value + liking(name, labels, preferred, attacking, eye.reading))
    };
    let rated: Vec<(&Vec<u8>, &Vec<&[u8]>, (f64, f64))> = table.iter().map(|f| (&f.0, &f.1, rate(&f.0, &f.1))).collect();
    let strongest = rated.iter().max_by(|x, y| x.2 .0.total_cmp(&y.2 .0)).map_or(own_form.clone(), |p| p.0.clone());
    let (form, starters, _) = rated.iter().fold((&own_form, &own, rate(&own_form, &own).1), |b, r| if r.2 .1 > b.2 + 0.01 { (r.0, r.1, r.2 .1) } else { b });
    let first = eleven(&eye, &pool, starters);
    let staff_of = |c: &Option<usize>, pool: &[Candidate]| c.map(|i| pool[i].staff);
    let first: Vec<Option<usize>> = first.iter().map(|c| staff_of(c, &pool)).collect();
    let rest: Vec<Candidate> = pool.iter().copied().filter(|c| !first.contains(&Some(c.staff))).collect();
    let second = eleven(&eye, &rest, &starters);
    let second: Vec<Option<usize>> = second.iter().map(|c| staff_of(c, &rest)).collect();
    let men = |team: &[Option<usize>]| -> Vec<Option<Man>> {
        (0..11).map(|s| team[s].map(|staff| (short_name(staff), scout::seen_score(staff, a, Some(starters[s])).unwrap_or(0.0)))).collect()
    };
    let (first_men, second_men) = (men(&first), men(&second));
    let (rows, missing) = team_rows(club, a, &pool, starters, &first, &second);
    let out: Vec<Man> = missing.iter().map(|m| (short_name(m.0), m.1)).collect();
    let (club_name, by) = (game_text(club + 4, 51), staff_name(a));
    let says = coach_text(&formation_text(&own_form, form, preferred, &strongest), starters, &first_men, &second_men, &out);
    let head: [&[u8]; 4] = [&club_name, &by, form, &says];
    let mut lines = vec![head.join(&b'\t')];
    lines.extend(rows);
    lines.join(&b'\n')
}

/// A report's player rows, the two elevens by line of the pitch, then those not in `pool` good
/// enough to be in them; and those players (staff, score), best first.
unsafe fn team_rows(club: usize, a: usize, pool: &[Candidate], starters: &[&[u8]], first: &[Option<usize>], second: &[Option<usize>]) -> (Vec<Vec<u8>>, Vec<(usize, f64)>) {
    let seen = |staff: usize, label: Option<&[u8]>| scout::seen_score(staff, a, label).unwrap_or(0.0);
    let score_in = |team: &[Option<usize>]| (0..11).filter_map(|s| team[s].map(|staff| seen(staff, Some(starters[s])))).collect::<Vec<f64>>();
    let lowest = score_in(first).into_iter().chain(score_in(second)).fold(f64::MAX, f64::min);
    let mut out: Vec<(usize, f64)> = Vec::new();
    let mut rows: Vec<(u8, usize, i32, Vec<u8>)> = Vec::new();
    for k in 0..50 {
        let staff = rd::<usize>(club + 0xd7 + 4 * k);
        if !readable(staff, 0x6e) || !plays_for(staff, club) || !readable(((staff + 0x61) as *const usize).read_unaligned(), 70) {
            continue;
        }
        let at = |team: &[Option<usize>]| team.iter().position(|s| *s == Some(staff));
        let (role, slot) = match (at(first), at(second)) {
            (Some(s), _) => (b'1', s),
            (_, Some(s)) => (b'2', s),
            _ if pool.iter().any(|c| c.staff == staff) => continue,
            _ => (b'x', 0),
        };
        let placed = role != b'x';
        let label: &[u8] = if placed { starters[slot] } else { b"" };
        let score = seen(staff, placed.then_some(label));
        if !placed && score < lowest {
            continue;
        }
        let positions = positions_text(staff);
        let line = line_rank(if placed { label } else { &positions });
        let (name, short, slot, halves) = (staff_name(staff), short_name(staff), slot.to_string(), scout::halves(score).to_string());
        if !placed {
            out.push((staff, score));
        }
        let fields: [&[u8]; 7] = [&name, &short, &positions, &[role], slot.as_bytes(), label, halves.as_bytes()];
        rows.push((role, line, -(score * 10.0) as i32, fields.join(&b'\t')));
    }
    rows.sort_by_key(|r| (r.0, r.1, r.2));
    out.sort_by(|x, y| y.1.total_cmp(&x.1));
    (rows.into_iter().map(|r| r.3).collect(), out)
}

/// A player in the assistant's text: the name he goes by, his score where he would play.
type Man = (Vec<u8>, f64);

/// What the assistant says about his choices, *..* around the key names: `opening` on the
/// formation, the two best men, the slot with the closest call, the one with the biggest drop to
/// the backup, the weakest starter, and the best man not available when he would start.
fn coach_text(opening: &[u8], labels: &[&[u8]], first: &[Option<Man>], second: &[Option<Man>], out: &[Man]) -> Vec<u8> {
    let mut text = opening.to_vec();
    let mut say = |parts: &[&[u8]]| {
        text.push(b' ');
        parts.iter().for_each(|p| text.extend_from_slice(p));
    };
    let mut side: Vec<(usize, &Man)> = first.iter().enumerate().filter_map(|(s, m)| Some((s, m.as_ref()?))).collect();
    side.sort_by(|x, y| y.1 .1.total_cmp(&x.1 .1));
    if let [best, next, ..] = side[..] {
        say(&[b"The team leans on *", &best.1 .0, b"* (", labels[best.0], b") and *", &next.1 .0, b"* (", labels[next.0], b")."]);
    }
    let gaps: Vec<(usize, &Man, &Man, f64)> = (0..first.len())
        .filter_map(|s| {
            let (x, y) = (first[s].as_ref()?, second.get(s)?.as_ref()?);
            Some((s, x, y, x.1 - y.1))
        })
        .collect();
    if let Some(c) = gaps.iter().min_by(|x, y| x.3.total_cmp(&y.3)).filter(|c| c.3 < 2.0) {
        say(&[b"It is close at ", labels[c.0], b": *", &c.1 .0, b"* starts, but *", &c.2 .0, b"* is pushing for the place."]);
    }
    if let Some(t) = gaps.iter().max_by(|x, y| x.3.total_cmp(&y.3)).filter(|t| t.3 >= 5.0) {
        say(&[b"We are thinnest at ", labels[t.0], b", where the drop from *", &t.1 .0, b"* to *", &t.2 .0, b"* is the biggest."]);
    }
    if let (true, Some(w)) = (side.len() > 2, side.last()) {
        say(&[b"I would look for an upgrade on *", &w.1 .0, b"* at ", labels[w.0], b"."]);
    }
    if let Some(m) = out.first().filter(|m| side.last().is_some_and(|w| m.1 > w.1 .1)) {
        say(&[b"We miss *", &m.0, b"*, who would be in this side."]);
    }
    text
}

/// Text from the top left of a box, word by word, *..* in gold.
pub unsafe fn report_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, line: i32, text: &[u8]) {
    let space = text_width(font, b"a a") - text_width(font, b"aa");
    let (mut x, mut y) = (x1, y1);
    for (k, part) in text.split(|&c| c == b'*').enumerate() {
        for word in part.split(|&c| c == b' ').filter(|w| !w.is_empty()) {
            // (a new paragraph, a little apart)
            if word == b"|" {
                (x, y) = (x1, y + line * 3 / 2);
                continue;
            }
            let w = text_width(font, word);
            // (punctuation stays on the name before it)
            let glued = b".,:)".contains(&word[0]) && x > x1;
            if glued {
                x -= space;
            } else if x + w > x2 && x > x1 {
                (x, y) = (x1, y + line);
            }
            if y + line > y2 {
                return;
            }
            text_at(x, y, y + line, font, if k % 2 == 1 { GOLD } else { LIGHT }, word, 0);
            x += w + space;
        }
    }
}

/// Where a position text ("D/DM RC") comes in the list, back to front.
fn line_rank(positions: &[u8]) -> usize {
    const LINES: [(&[u8], usize); 9] = [(b"GK", 0), (b"SW", 1), (b"DM", 3), (b"D", 2), (b"WB", 2), (b"M", 4), (b"AM", 5), (b"F", 6), (b"S", 6)];
    LINES.iter().find(|l| positions.starts_with(l.0)).map_or(7, |l| l.1)
}

/// Where each player of an eleven stands, from the slots' labels: parts of the pitch across and
/// down, his own goal at the bottom. The game's spots (`SPOTS`), several on one spot side by side.
fn places(labels: &[&[u8]]) -> Vec<(f32, f32)> {
    let name = |l: &[u8]| -> Vec<u8> {
        match l {
            b"DM" => b"DMC".to_vec(),
            b"AM" => b"AMC".to_vec(),
            b"ST" => b"FC".to_vec(),
            l => l.to_vec(),
        }
    };
    (0..labels.len())
        .map(|i| {
            let spot = name(labels[i]);
            let (depth, side) = SPOTS.iter().find(|s| s.4.as_bytes() == spot).map_or((0.52, 0.5), |s| (s.2, s.3));
            let same = labels.iter().filter(|l| name(l) == spot).count() as f32;
            let nth = labels[..i].iter().filter(|l| name(l) == spot).count() as f32;
            ((0.5 + (side - 0.5) * 1.25 + (nth - (same - 1.0) / 2.0) * 0.25).clamp(0.09, 0.91), 0.93 - depth * 0.9)
        })
        .collect()
}

/// A pitch as the tactics screen has it: dark stripes, thin lines, the goal at the bottom his own.
unsafe fn report_pitch(x1: i32, y1: i32, x2: i32, y2: i32) {
    let (w, h) = (x2 - x1, y2 - y1);
    for k in 0..10 {
        fill(x1, y1 + h * k / 10, x2, y1 + h * (k + 1) / 10, if k % 2 == 0 { PITCH_GRASS } else { PITCH_STRIPE });
    }
    let frame = |a: i32, b: i32, c: i32, d: i32| {
        fill(a, b, c, b + 1, PITCH_LINE);
        fill(a, d - 1, c, d, PITCH_LINE);
        fill(a, b, a + 1, d, PITCH_LINE);
        fill(c - 1, b, c, d, PITCH_LINE);
    };
    frame(x1, y1, x2, y2);
    fill(x1, y1 + h / 2, x2, y1 + h / 2 + 1, PITCH_LINE);
    circle(x1 + w / 2, y1 + h / 2, w / 8, 1.5, PITCH_LINE);
    for (side, depth) in [(w * 22 / 100, h * 14 / 100), (w * 37 / 100, h * 5 / 100)] {
        frame(x1 + side, y1, x2 - side, y1 + depth); // penalty area, goal area
        frame(x1 + side, y2 - depth, x2 - side, y2);
    }
}

/// The team report, over everything the game drew this frame.
#[allow(static_mut_refs)]
pub unsafe fn report() {
    if !REPORT_OPEN {
        return;
    }
    if remote::CLIENT && REPORT.is_empty() {
        if let Some(v) = ask_host(&REPORT_KEY) {
            REPORT = v;
        }
    }
    let (sw, sh) = screen();
    // the game's font numbers go by size; a small screen (800x600) two sizes down
    let shrink = if sh < 800 { 2 } else { 0 };
    let f = |n: u32| REPORT_FONT & !0xffff | n.saturating_sub(shrink).max(1);
    let (m, bar) = (report_margin(sh), sh * 8 / 100);
    // (the game's rectangle takes the last pixel, not one past it: sw itself draws nothing)
    fill(0, 0, sw - 1, sh - 1, BACKGROUND);
    fill(0, 0, sw - 1, bar, [0x22, 0x26, 0x2c]);
    fill(0, bar - 3, sw - 1, bar, GOLD);
    let b = back_rect();
    pill(b.0, b.1, b.2, b.3, if REPORT_HOVER { SELECTED } else { [0x3a, 0x40, 0x4a] });
    text_at((b.0 + b.2) / 2, b.1, b.3, f(3), if REPORT_HOVER { GOLD } else { WHITE }, b"Back", 1);
    let mut lines = REPORT.split(|&c| c == b'\n');
    let head: Vec<&[u8]> = lines.next().unwrap_or(b"").split(|&c| c == b'\t').collect();
    let rows: Vec<Vec<&[u8]>> = lines.map(|l| l.split(|&c| c == b'\t').collect()).filter(|r: &Vec<&[u8]>| r.len() == 7).collect();
    let scouting = REPORT_SCOUT;
    let title: &[u8] = if scouting { b"Scout Report" } else { b"Assistant Manager Team Report" };
    if rows.is_empty() {
        let text: &[u8] = match (scouting, REPORT.is_empty()) {
            (true, true) => b"Asking the scout...",
            (true, false) => b"The scout could not write a report.",
            (false, true) => b"Asking the assistant...",
            (false, false) => b"The assistant could not write a report.",
        };
        text_at(sw / 2, 0, bar, f(5), WHITE, title, 1);
        text_at(sw / 2, sh / 2 - 30, sh / 2 + 30, f(4), LIGHT, text, 1);
        return;
    }
    // the title band: crest, club and report, the assistant and the shape
    let club = head.first().copied().unwrap_or(b"");
    let size = bar * 70 / 100;
    let title_x = if draw_crest(club, m * 2, (bar - size) / 2, size) { m * 4 + size } else { m * 2 };
    text_at(title_x, 4, bar * 58 / 100, f(6), WHITE, &[club, b" - ", title].concat(), 0);
    let (job, verb): (&[u8], &[u8]) = if scouting { (b"Scout ", b"  -  expects ") } else { (b"Assistant manager ", b"  -  recommends ") };
    let by = [job, head.get(1).copied().unwrap_or(b""), verb, head.get(2).copied().unwrap_or(b"")].concat();
    text_at(title_x, bar * 54 / 100, bar - 6, f(3), LIGHT, &by, 0);
    // the players, with the numbers the shirts wear, and what the assistant says under them
    let (x1, y1, x2, y2) = (m, bar + m, sw * 22 / 100, b.1 - m);
    let top = (y2 - y1) / 24;
    let rh = (y2 - y1 - top - 6) / (rows.len() as i32).max(44);
    let list_end = y1 + top + 6 + rows.len() as i32 * rh;
    fill(x1, y1, x2, list_end, AREA);
    text_at(x1 + 14, y1, y1 + top, f(2), GOLD, b"PLAYERS", 0);
    let list_font = if rh >= 30 { f(3) } else if rh >= 18 { f(2) } else { f(1) };
    let size = rh * 60 / 100; // a star
    let stars_x = x2 - 10 - 5 * size - 4 * 4;
    for (k, row) in rows.iter().enumerate() {
        let y = y1 + top + 2 + k as i32 * rh;
        let role = row[3].first().copied().unwrap_or(b'x');
        let (mark, ink) = match role {
            b'1' => (P1, WHITE),
            b'2' => (GOLD, LIGHT),
            _ => (P2, GREY),
        };
        fill(x1 + 6, y + 3, x1 + 10, y + rh - 3, mark);
        text_at(x1 + 40, y, y + rh, list_font, GREY, (k + 1).to_string().as_bytes(), 2);
        // where he plays; his positions when he is not available
        // (the scout's next in line: what they play, not the slot left over for them)
        let place = if role == b'x' || (scouting && role == b'2') { row[2] } else { row[5] };
        let room = stars_x - 10 - text_width(list_font, place) - 10 - (x1 + 48);
        text_at(x1 + 48, y, y + rh, list_font, ink, &clip_text(list_font, row[0], room), 0);
        text_at(stars_x - 10, y, y + rh, list_font, if role == b'x' { GREY } else { GOLD }, place, 2);
        let halves = std::str::from_utf8(row[6]).ok().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        stars(stars_x, y + (rh - size) / 2, size, halves * 20, GOLD);
    }
    let says = list_end + m;
    if !scouting && y2 - says > top * 3 {
        fill(x1, says, x2, y2, AREA);
        text_at(x1 + 14, says, says + top, f(2), GOLD, b"THE ASSISTANT'S VIEW", 0);
        let font = if rows.len() <= 24 { f(3) } else { f(2) };
        report_text(x1 + 14, says + top, x2 - 14, y2 - 6, font, rh + 4, head.get(3).copied().unwrap_or(b""));
    }
    // the two teams
    let w = (sw - x2 - 2 * m) / 2 - m / 2;
    let kit = KIT_BAND.map(px_rgb).unwrap_or([0xf4; 3]);
    let number = if kit[0] as u32 * 3 + kit[1] as u32 * 6 + kit[2] as u32 > 1400 { [0x10, 0x12, 0x14] } else { WHITE };
    let titles: [&[u8]; 2] = if scouting { [b"EXPECTED XI", b"THE SCOUT'S REPORT"] } else { [b"BEST STARTING XI (RECOMMENDED)", b"BEST BACKUP XI"] };
    for (k, (title, role)) in [(titles[0], b'1'), (titles[1], b'2')].into_iter().enumerate() {
        let px1 = x2 + m + k as i32 * (w + m);
        let (px2, grass) = (px1 + w, y1 + top);
        fill(px1, y1, px2, y2, AREA);
        text_at(px1 + 14, y1, grass, f(2), GOLD, title, 0);
        if scouting && k == 1 {
            report_text(px1 + 14, grass + 8, px2 - 14, y2 - 8, f(3), (top * 7 / 10).max(16), head.get(3).copied().unwrap_or(b""));
            continue;
        }
        let g = (px1 + m, grass, px2 - m, y2 - m);
        report_pitch(g.0, g.1, g.2, g.3);
        let mut team: Vec<(usize, &Vec<&[u8]>)> = rows.iter().enumerate().filter(|(_, r)| r[3].first() == Some(&role)).collect();
        team.sort_by_key(|t| std::str::from_utf8(t.1[4]).ok().and_then(|s| s.parse::<i32>().ok()));
        let labels: Vec<&[u8]> = team.iter().map(|t| t.1[5]).collect();
        let (s, name_h, label_h) = ((g.3 - g.1) / 34, (g.3 - g.1) / 32, (g.3 - g.1) / 40); // s: half a shirt
        let (room, label_font) = ((g.2 - g.0) * 23 / 100, f(1));
        for ((number_in_list, row), (across, down)) in team.iter().zip(places(&labels)) {
            let (cx, cy) = (g.0 + ((g.2 - g.0) as f32 * across) as i32, g.1 + ((g.3 - g.1) as f32 * down) as i32);
            shirt_kit(cx - s, cy - s, cx + s, cy + s, row[5] == b"GK");
            text_at(cx, cy - s, cy + s, f(3), if row[5] == b"GK" { WHITE } else { number }, (number_in_list + 1).to_string().as_bytes(), 1);
            // the name in a dark pill under the shirt, inside the grass; the slot under it
            let name_font = if text_width(f(3), row[1]) > room { f(2) } else { f(3) };
            let name = clip_text(name_font, row[1], room);
            let half = text_width(name_font, &name) / 2 + 8;
            let nx = cx.clamp(g.0 + half + 2, g.2 - half - 2);
            let (ny1, ny2) = (cy + s + 2, cy + s + 2 + name_h);
            pill(nx - half, ny1, nx + half, ny2, [0x14, 0x18, 0x1c]);
            text_at(nx, ny1, ny2, name_font, WHITE, &name, 1);
            text_at(nx, ny2, ny2 + label_h, label_font, GOLD, row[5], 1);
        }
    }
    // what the marks in the list mean
    let mut x = m * 2;
    let (one, two) = if scouting { ("Expected XI", "Next in line") } else { ("Best XI", "Backup XI") };
    for (c, text) in [(P1, one), (GOLD, two), (P2, "Not available")] {
        fill(x, b.1 + (b.3 - b.1) / 2 - 7, x + 4, b.1 + (b.3 - b.1) / 2 + 7, c);
        text_at(x + 12, b.1, b.3, f(2), LIGHT, text.as_bytes(), 0);
        x += 12 + text_width(f(2), text.as_bytes()) + 30;
    }
}

/// The mouse (poll_input): None when the assistant's button and menu have nothing to do with it;
/// Some(redraw) when they take it (the game does not see it then).
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    let inside = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if REPORT_OPEN {
        // (the game's poll does not run: a click would still be waiting for it after Back)
        *(LEFT_CLICK as *mut u32) = 0;
        *(RIGHT_CLICK as *mut u32) = 0;
        let over = inside(back_rect());
        let mut redraw = std::mem::replace(&mut REPORT_HOVER, over) != over;
        if released && over {
            REPORT_OPEN = false;
            return Some(true);
        }
        // a client's report on its way: drawn when it comes, asked again if it stays out
        if remote::CLIENT {
            let arrived = std::mem::take(&mut remote::ARRIVED) | remote::hold_over();
            let due = REPORT_LOOKED.is_none_or(|t| t.elapsed().as_millis() > 500);
            #[allow(static_mut_refs)]
            if REPORT.is_empty() && (arrived || due) {
                REPORT_LOOKED = Some(std::time::Instant::now());
                redraw = true;
            }
        }
        return Some(redraw);
    }
    if let Some((_, club, font)) = SCOUT_CHIP.filter(|c| inside(c.0)) {
        if released {
            open_scout(club, font);
        }
        return Some(released);
    }
    let b = BUTTON?;
    if MENU_OPEN {
        let hover = (0..menu_len()).find(|&k| inside(item_rect(b, k)));
        let changed = hover != HOVER;
        HOVER = hover;
        if released {
            MENU_OPEN = false;
            HOVER = None;
            if hover == Some(ITEMS.len()) {
                open_report();
                return Some(true);
            }
            // (done by the club screen's input, which then has the screen built again; a
            // client's by the host's)
            if remote::CLIENT {
                CLIENT_SEND = hover.map(|k| ITEMS[k].0);
            } else {
                PENDING = hover.map(|k| ITEMS[k].0);
            }
            return Some(true);
        }
        return (hover.is_some() || inside(b)).then_some(changed).or(changed.then_some(true));
    }
    if inside(b) {
        if released && (remote::CLIENT || assistant().is_some()) {
            MENU_OPEN = true;
            return Some(true);
        }
        return Some(false);
    }
    None
}

/// The human's club's assistant manager.
unsafe fn assistant() -> Option<usize> {
    assistant_of(human_club_ptr()?)
}

unsafe fn assistant_of(club: usize) -> Option<usize> {
    let a = game::<extern "C" fn(usize) -> usize>(CLUB_ASSISTANT)(club);
    (readable(a, 0x6e) && readable(rd::<usize>(a + 0x69), 68)).then_some(a)
}

/// A coaching attribute of a staff member as shown (1..20): non-player record +0x69, CA at +4.
unsafe fn coaching(staff: usize, at: usize) -> f64 {
    let np = rd::<usize>(staff + 0x69);
    scout::shown(rd::<i8>(np + at), rd::<i16>(np + 4), true)
}

/// A player attribute 1..20 from his record (game calls by attribute id assert on some ids).
unsafe fn attribute(staff: usize, (at, intrinsic): (usize, bool)) -> i32 {
    let player = ((staff + 0x61) as *const usize).read_unaligned();
    if !readable(player, 70) {
        return 0;
    }
    if intrinsic { scout::shown(rd::<i8>(player + at), rd::<i16>(player + 5), true) as i32 } else { rd::<i8>(player + at) as i32 }
}

/// A fixed number in -1..1 for an assistant, a player and a label (the same every time).
fn hash(a: i32, b: i32, c: &[u8]) -> f64 {
    let mut h = ((a as u32 as u64) << 32 | b as u32 as u64) ^ c.iter().fold(0u64, |h, &x| h.wrapping_mul(31).wrapping_add(x as u64)).wrapping_mul(0x9e3779b97f4a7c15);
    h = (h ^ h >> 30).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ h >> 27).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// A player's fit (1..20) for a slot label ("DL", "GK", "AMC"; "DM", "AM", "ST" too).
unsafe fn fit(player: usize, label: &[u8]) -> i32 {
    let label: &[u8] = match label {
        b"DM" => b"DMC",
        b"AM" => b"AMC",
        b"ST" => b"FC",
        l => l,
    };
    let rating = |at: usize| rd::<i8>(player + at) as i32;
    SPOTS.iter().find(|s| s.4.as_bytes() == label).map_or(1, |s| if s.0 == GK { rating(GK) } else { rating(s.0).min(rating(s.1)) })
}

fn fit_factor(f: i32) -> f64 {
    match f {
        18.. => 1.0,
        15..=17 => 0.92,
        12..=14 => 0.78,
        9..=11 => 0.55,
        5..=8 => 0.3,
        _ => 0.05,
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    staff: usize,
    player: usize,
    judged: f64,
    condition: i32,
    short_of_fitness: bool,
    started: bool,
}

/// How an assistant rates players: for a mode, his id and how he reads position fit.
struct Eye {
    mode: Mode,
    id: i32,
    reading: f64,
}

impl Eye {
    /// How he reads a player's fit for a slot: off by up to (20 - Tactics) / 4, fixed.
    unsafe fn seen_fit(&self, c: &Candidate, label: &[u8]) -> i32 {
        let err = hash(self.id, rd::<i32>(c.staff), label) * (20.0 - self.reading).max(0.0) / 4.0;
        ((fit(c.player, label) as f64 + err).round() as i32).clamp(1, 20)
    }

    unsafe fn condition_factor(&self, c: &Candidate) -> f64 {
        let cond = c.condition as f64;
        let f = match self.mode {
            Mode::Fit => {
                if cond >= 92.0 { 1.0 + (cond - 92.0) * 0.01 } else { 0.35 + (cond - 60.0).max(0.0) * 0.01 }
            }
            // (a keeper is not rotated: the first choice stays in goal)
            Mode::Rotation => (1.0 - (90.0 - cond).max(0.0) * 0.02) * if c.started && rd::<i8>(c.player + GK) < 15 { 0.88 } else { 1.0 },
            _ => 1.0 - (90.0 - cond).max(0.0) * 0.015,
        };
        f.max(0.05) * if c.short_of_fitness { if self.mode == Mode::Fit { 0.4 } else { 0.75 } } else { 1.0 }
    }

    unsafe fn score(&self, c: &Candidate, label: &[u8]) -> f64 {
        c.judged * fit_factor(self.seen_fit(c, label)) * self.condition_factor(c)
    }
}

/// The pick itself: the selection written, the note returned.
#[allow(static_mut_refs)]
unsafe fn pick(mode: Mode) -> Option<Vec<u8>> {
    let club = human_club_ptr().filter(|&c| c == SQUAD_CLUB)?;
    pick_for(mode, club, 0, SLOTS.clone(), UNAVAILABLE.clone(), tactics_screen())
}

/// The next match: the club screen's parameter 3 is that fixture; the tactics screen keeps a copy
/// of its fixture, the next one or the live match's, at +0xd92, flagged by +0xde1
/// (Tactics_Screen::Tactics_Screen 0x8a7440). Its +0x4d low nibble is the substitutes named, as
/// the pre-match check 0x7260f0 counts them, its +0x14 the competition for bans. 0: none.
unsafe fn next_fixture(ts: Option<usize>, club: usize) -> usize {
    let fixture = match ts {
        None => screen_param(3),
        Some(t) if readable(t + 0xd92, 0x53) && rd::<u32>(t + 0xde1) != 0 => t + 0xd92,
        Some(_) => 0,
    };
    // (only a fixture of this club: +0x1c / +0x20 its two clubs, as the constructor compares
    // them; any other screen's parameter 3 is left alone)
    if readable(fixture, 0x50) && (rd::<usize>(fixture + 0x1c) == club || rd::<usize>(fixture + 0x20) == club) { fixture } else { 0 }
}

/// The players a club's assistant can choose from: not injured, banned or marked not available.
unsafe fn candidates(a: usize, club: usize, fixture: usize, unavailable: &[usize], before: &[usize]) -> Vec<Candidate> {
    candidates_in(a, club, if fixture != 0 { rd::<usize>(fixture + 0x14) } else { 0 }, unavailable, before)
}

/// `candidates` for a competition's bans.
unsafe fn candidates_in(a: usize, club: usize, comp: usize, unavailable: &[usize], before: &[usize]) -> Vec<Candidate> {
    let injuries = rd::<usize>(INJURIES);
    let judge = game::<extern "C" fn(usize, usize, usize, i32, i32) -> i32>(JUDGE);
    let discipline = rd::<usize>(DISCIPLINE);
    // (the ban check reads *comp as an index into the table at 0xadadfc)
    let comp_ok = mapped(comp, 4) && (0..10_000).contains(&rd::<i32>(comp)) && mapped(rd::<usize>(COMP_TABLE) + 4 * rd::<i32>(comp) as usize, 4);
    // (as the pre-match check asks it: banned for that match; it also takes him out of the club's
    // selection, which is written anew below)
    let banned = |staff: usize| comp_ok && mapped(discipline, 4) && game::<extern "thiscall" fn(usize, usize, usize, i32, i32) -> i32>(BANNED)(discipline, staff, comp, 1, 0) != 0;
    let mut pool: Vec<Candidate> = Vec::new();
    for k in 0..50 {
        let staff = rd::<usize>(club + 0xd7 + 4 * k);
        if !readable(staff, 0x6e) || !plays_for(staff, club) || unavailable.contains(&staff) || banned(staff) {
            continue;
        }
        let player = ((staff + 0x61) as *const usize).read_unaligned();
        if !readable(player, 70) {
            continue;
        }
        let status = if mapped(injuries, 4) { game::<extern "thiscall" fn(usize, usize) -> i32>(INJURY_STATUS)(injuries, staff) & 0xff } else { 0 };
        if status == 2 {
            continue;
        }
        pool.push(Candidate {
            staff,
            player,
            judged: (judge(a, staff, club, 2, -1) as i16).max(1) as f64,
            condition: condition(staff).unwrap_or(100),
            short_of_fitness: status == 1,
            started: before.contains(&staff),
        });
    }
    pool
}

/// The eleven (indices in `pool`, a slot's None when there is no one left): the slots fewest
/// players can fill first, then swaps while the total gains.
unsafe fn eleven(eye: &Eye, pool: &[Candidate], starters: &[&[u8]]) -> Vec<Option<usize>> {
    let score = |c: &Candidate, label: &[u8]| eye.score(c, label);
    let mut order: Vec<usize> = (0..11).collect();
    order.sort_by_key(|&s| pool.iter().filter(|c| eye.seen_fit(c, starters[s]) >= 15).count());
    let mut chosen: Vec<Option<usize>> = vec![None; 11];
    for &s in &order {
        chosen[s] = (0..pool.len())
            .filter(|i| !chosen.contains(&Some(*i)))
            .max_by(|&x, &y| score(&pool[x], starters[s]).total_cmp(&score(&pool[y], starters[s])));
    }
    for _ in 0..6 {
        let mut better = false;
        for s in 0..11 {
            for t in s + 1..11 {
                if let (Some(x), Some(y)) = (chosen[s], chosen[t]) {
                    if score(&pool[y], starters[s]) + score(&pool[x], starters[t]) > score(&pool[x], starters[s]) + score(&pool[y], starters[t]) + 0.01 {
                        chosen.swap(s, t);
                        better = true;
                    }
                }
            }
            for i in 0..pool.len() {
                if !chosen.contains(&Some(i)) && chosen[s].is_none_or(|x| score(&pool[i], starters[s]) > score(&pool[x], starters[s]) + 0.01) {
                    chosen[s] = Some(i);
                    better = true;
                }
            }
        }
        if !better {
            break;
        }
    }
    chosen
}

/// The pick for a club: `manager` its human (0: the one at the screen), the slots' labels, the
/// players not available, the tactics screen's own object when the pick is made there.
#[allow(static_mut_refs)]
unsafe fn pick_for(mode: Mode, club: usize, manager: usize, slots: Vec<Vec<u8>>, unavailable: Vec<usize>, ts: Option<usize>) -> Option<Vec<u8>> {
    let tactics = rd::<usize>(TACTICS);
    let a = assistant_of(club)?;
    let name = staff_name(a);
    let clear = game::<extern "thiscall" fn(usize, usize, i32, i32, i32) -> i32>(CLEAR_SELECTION);
    if mode == Mode::Clear {
        clear(tactics, club, 0, 1, 0);
        return Some([&name[..], b": team cleared."].concat());
    }
    if slots.len() < 11 || !mapped(tactics, 4) {
        return None;
    }
    let get = game::<extern "thiscall" fn(usize, usize, i32, i32) -> usize>(GET_SLOT);
    let before: Vec<usize> = (0..11).map(|k| get(tactics, club, k, 0)).collect();
    let (judging, reading) = (coaching(a, 0x17), coaching(a, 0x21));
    let fixture = next_fixture(ts, club);
    let pool = candidates(a, club, fixture, &unavailable, &before);
    let eye = Eye { mode, id: rd::<i32>(a), reading };
    let score = |c: &Candidate, label: &[u8]| eye.score(c, label);
    let starters: Vec<&[u8]> = slots[..11].iter().map(|s| s.as_slice()).collect();
    let chosen = eleven(&eye, &pool, &starters);
    // the bench: a keeper, then one for each line, then the best of the rest
    let named = if readable(fixture, 0x50) { (rd::<u8>(fixture + 0x4d) & 0xf) as usize } else { 0 };
    // (the fixture's count wins over the boxes the screen showed: the squad screen showed seven
    // before a nine-sub CL match, night2 D)
    let bench_slots = if (1..=9).contains(&named) { named } else { 9.min(slots.len() - 11) };
    if trace() {
        log(&format!("assistant: fixture {fixture:#x} allows {named} substitutes, {bench_slots} named"));
    }
    let mut bench: Vec<usize> = Vec::new();
    let taken = |i: usize, bench: &Vec<usize>| chosen.contains(&Some(i)) || bench.contains(&i);
    let best_for = |labels: &[&[u8]], bench: &Vec<usize>| -> Option<usize> {
        (0..pool.len()).filter(|&i| !taken(i, bench)).max_by(|&x, &y| {
            let s = |i: usize| labels.iter().map(|l| score(&pool[i], l)).fold(0.0, f64::max);
            s(x).total_cmp(&s(y))
        })
    };
    // (wing backs only where the tactic has them: their rating is apart from a defender's)
    let backs: Vec<&[u8]> = [&b"DC"[..], b"DL", b"DR"].into_iter().chain(starters.iter().copied().filter(|l| l.starts_with(b"WB"))).collect();
    let lines: [&[&[u8]]; 4] = [&[b"GK"], &backs, &[b"MC", b"ML", b"MR", b"DMC", b"AMC"], &[b"FC", b"AMC"]];
    let all: Vec<&[u8]> = starters.iter().copied().filter(|l| *l != b"GK").collect();
    for k in 0..bench_slots {
        let pick = if k < lines.len() { best_for(lines[k], &bench) } else { best_for(&all, &bench) };
        match pick {
            Some(i) => bench.push(i),
            None => break,
        }
    }
    // written: the old selection cleared (the game keeps an undo copy), then slot by slot
    clear(tactics, club, 0, 1, 0);
    let set = game::<extern "thiscall" fn(usize, usize, i32, usize, i32) -> i32>(SET_SLOT);
    for (s, c) in chosen.iter().enumerate() {
        if let Some(i) = c {
            set(tactics, club, s as i32, pool[*i].staff, 0);
        }
    }
    for (k, &i) in bench.iter().enumerate() {
        set(tactics, club, 11 + k as i32, pool[i].staff, 0);
    }
    // the captain (Influence) and the set piece takers (first and second choice by attribute)
    let eleven: Vec<(usize, usize)> = chosen.iter().enumerate().filter_map(|(s, c)| c.map(|i| (s, pool[i].staff))).collect();
    let captain = eleven.iter().max_by_key(|e| attribute(e.1, INFLUENCE)).copied();
    if let Some((slot, _)) = captain {
        game::<extern "thiscall" fn(usize, usize, i32, i32) -> i32>(SET_CAPTAIN_FROM_SLOT)(tactics, club, slot as i32, 0);
    }
    let mut penalty = None;
    let taker = game::<extern "thiscall" fn(usize, usize, i32, i32, usize, i32) -> i32>(SET_TAKER);
    for (role, &attr) in ROLES.iter().enumerate() {
        let mut by: Vec<usize> = eleven.iter().filter(|e| starters[e.0] != b"GK").map(|e| e.1).collect();
        by.sort_by_key(|&s| -attribute(s, attr));
        for (choice, &s) in by.iter().take(2).enumerate() {
            taker(tactics, club, role as i32, choice as i32, s, 0);
            // (the tactics screen keeps its own copy, written back on Ok: +0x4a2c, two a role)
            if let Some(ts) = ts {
                ((ts + 0x4a2c + (role * 2 + choice) * 4) as *mut usize).write_unaligned(s);
            }
        }
    }
    if let (Some(ts), Some((_, c))) = (ts, captain) {
        let base = rd::<usize>(STAFF_TABLE);
        ((ts + 0x4a20) as *mut i32).write_unaligned(((c - base) / 0x6e) as i32); // its captain, a staff index
    }
    // penalties: the human's Penalty Takers list (50 places, HUMAN_MANAGER [0xb63c98]; the squad
    // view and the tactics menu show it), which the match engine reads for a penalty (0x6cf0ff:
    // the highest place of a player on the pitch takes it): the squad by Penalties, keepers last
    let humans = rd::<usize>(HUMAN_MANAGER);
    if mapped(humans, 4) {
        let clear_place = game::<extern "thiscall" fn(usize, i32, usize) -> i32>(CLEAR_PENALTY_PLACE);
        let set_place = game::<extern "thiscall" fn(usize, i32, usize, usize) -> i32>(SET_PENALTY_PLACE);
        for place in 0..50 {
            clear_place(humans, place, manager);
        }
        let mut kickers: Vec<usize> = pool.iter().map(|c| c.staff).collect();
        kickers.sort_by_key(|&s| {
            let p = ((s + 0x61) as *const usize).read_unaligned();
            (rd::<i8>(p + GK) >= 15, -attribute(s, PENALTIES))
        });
        for (place, &s) in kickers.iter().take(50).enumerate() {
            set_place(humans, place as i32, s, manager);
        }
        penalty = kickers.iter().copied().find(|s| eleven.iter().any(|e| e.1 == *s));
    }
    // the note: formation, changes, then a reason or two
    let changes = eleven.iter().filter(|e| !before.contains(&e.1)).count();
    let mut note = format!("{}: {}, ", latin1(&name), shape(&starters));
    note += &match changes {
        0 => "no changes".to_string(),
        1 => "1 change".to_string(),
        n => format!("{n} changes"),
    };
    let mut reasons: Vec<String> = Vec::new();
    for &s in before.iter().filter(|s| **s != 0 && !eleven.iter().any(|e| e.1 == **s)) {
        if let Some(c) = pool.iter().find(|c| c.staff == s && c.condition < 90) {
            reasons.push(format!("rested {} ({}%)", latin1(&short_name(c.staff)), c.condition));
        } else if unavailable.contains(&s) {
            reasons.push(format!("{} not available", latin1(&short_name(s))));
        }
    }
    for &(slot, s) in &eleven {
        if let Some(c) = pool.iter().find(|c| c.staff == s) {
            if fit(c.player, starters[slot]) < 15 {
                reasons.push(format!("{} at {}", latin1(&short_name(s)), latin1(starters[slot])));
            }
        }
    }
    // the other changes: who came in for whom (in slot order, those not explained above)
    let explained: Vec<String> = reasons.clone();
    let ins: Vec<usize> = eleven.iter().map(|e| e.1).filter(|s| !before.contains(s)).collect();
    let outs: Vec<usize> = before.iter().copied().filter(|&s| s != 0 && !eleven.iter().any(|e| e.1 == s)).collect();
    for (i, o) in ins.iter().zip(outs.iter()) {
        let out = latin1(&short_name(*o)).into_owned();
        if !explained.iter().any(|r| r.contains(&out)) {
            reasons.push(format!("{} in for {out}", latin1(&short_name(*i))));
        }
    }
    if mode == Mode::Rotation && reasons.is_empty() {
        reasons.push("no one needed a rest".to_string());
    }
    // (three parts: what he did, why, who takes what; a narrow band shows the first and last)
    note += ".\n";
    for r in reasons.iter().take(2) {
        // (by char: a latin1 accented first letter is two bytes)
        let mut c = r.chars();
        if let Some(f) = c.next() {
            note.extend(f.to_uppercase());
            note.push_str(c.as_str());
        }
        note += ". ";
    }
    note += "\n";
    if let (Some((_, c)), Some(p)) = (captain, penalty) {
        note += &format!("Captain {}, penalties {}.", latin1(&short_name(c)), latin1(&short_name(p)));
    }
    if trace() {
        log(&format!("assistant pick: judging {judging} tactics {reading}: {note}"));
    }
    Some(to_latin1(&note))
}

/// The shape by lines, back to front: defenders, then DM / M / AM where the tactic has them
/// ("2-3-4-1"), forwards.
fn shape(starters: &[&[u8]]) -> String {
    let count = |p: &[&str]| starters[1..].iter().filter(|l| p.iter().any(|x| l.starts_with(x.as_bytes()))).count();
    let (dm, am) = (count(&["DM"]), count(&["AM"]));
    let lines = [count(&["SW", "D", "WB"]) - dm, dm, count(&["M"]), am, count(&["F", "ST"])];
    let parts: Vec<String> = lines.iter().enumerate().filter(|(i, &n)| n > 0 || *i == 0 || *i == 2 || *i == 4).map(|(_, n)| n.to_string()).collect();
    parts.join("-")
}

/// The name a player goes by (common name, else surname).
unsafe fn short_name(staff: usize) -> Vec<u8> {
    let name = |at: usize| {
        let p = rd::<usize>(staff + at);
        if readable(p, 1) { game_text(p, 51) } else { Vec::new() }
    };
    let common = name(0xc);
    if common.is_empty() { name(8) } else { common }
}

#[cfg(test)]
#[test]
fn report_places() {
    let team: [&[u8]; 11] = [b"GK", b"DL", b"DC", b"DC", b"DR", b"ML", b"MC", b"MC", b"MR", b"FC", b"FC"];
    let p = places(&team);
    assert!(p[0].1 > p[1].1 && p[1].1 > p[5].1 && p[5].1 > p[9].1, "keeper at the bottom, forwards at the top");
    let across: Vec<f32> = [1, 2, 3, 4].iter().map(|&i| p[i].0).collect();
    assert!(across.windows(2).all(|w| (w[1] - w[0] - 0.25).abs() < 0.001), "the back four evenly left to right: {across:?}");
    assert!((p[9].0 + p[10].0 - 1.0).abs() < 0.001 && p[9].0 < p[10].0, "two forwards either side of the middle");
    assert_eq!(line_rank(b"D/DM RC"), 2);
    assert_eq!(line_rank(b"DM C"), 3);
    assert_eq!(line_rank(b"AM/F RLC"), 5);
}

#[cfg(test)]
#[test]
fn report_coach_text() {
    let man = |n: &str, s: f64| Some((n.as_bytes().to_vec(), s));
    let labels: [&[u8]; 4] = [b"GK", b"DC", b"MC", b"FC"];
    let first = [man("Keeper", 60.0), man("Back", 58.0), man("Mid", 63.0), man("Top", 66.0)];
    let second = [man("Keeper2", 59.0), man("Back2", 50.0), None, man("Top2", 64.5)];
    let out = [("Hurt".as_bytes().to_vec(), 61.0), ("Worse".as_bytes().to_vec(), 40.0)];
    let text = String::from_utf8(coach_text(b"Hello.", &labels, &first, &second, &out)).unwrap();
    assert_eq!(
        text,
        "Hello. The team leans on *Top* (FC) and *Mid* (MC). It is close at GK: *Keeper* starts, but *Keeper2* is pushing for the place. We are thinnest at DC, where the drop from *Back* to *Back2* is the biggest. I would look for an upgrade on *Back* at DC. We miss *Hurt*, who would be in this side."
    );
    assert_eq!(text.matches('*').count() % 2, 0);
    assert_eq!(coach_text(b"Hello.", &labels, &[None, None, None, None], &[], &[]), b"Hello.");
}

#[cfg(test)]
#[test]
fn report_formation() {
    assert_eq!((label_of(0x088), label_of(0x808), label_of(0x208)), (b"WBL".to_vec(), b"WBR".to_vec(), b"DMC".to_vec()));
    // the table's labels are all known spots, its names add up to ten outfield men
    for (name, slots) in FORMATIONS {
        assert!(slots.iter().all(|s| SPOTS.iter().any(|p| p.4 == *s)), "{name}");
        let labels: Vec<&[u8]> = std::iter::once("GK").chain(slots).map(str::as_bytes).collect();
        let lines: Vec<usize> = name.split(' ').next().unwrap().split('-').map(|n| n.parse().unwrap()).collect();
        assert_eq!(lines.iter().sum::<usize>(), 10, "{name}");
        assert_eq!(places(&labels).len(), 11, "{name}");
    }
    assert_eq!(shape(&["GK", "DL", "DC", "DC", "DR", "ML", "MC", "MC", "MR", "FC", "FC"].map(str::as_bytes)), "4-4-2");
    assert_eq!(shape(&["GK", "DC", "DC", "DMC", "DMC", "MC", "MC", "MC", "AMC", "AMC", "FC"].map(str::as_bytes)), "2-2-3-2-1");
    assert!(same_slots(&[b"GK", b"DC", b"MC"], &[b"MC", b"GK", b"DC"]) && !same_slots(&[b"GK", b"DC"], &[b"GK", b"DL"]));
    let (four_four_two, four_two_four): ([&[u8]; 3], [&[u8]; 5]) = ([b"GK", b"FC", b"FC"], [b"GK", b"FC", b"FC", b"AML", b"AMR"]);
    // his shape counts for more with a poor tactician; a strong one lets the squad decide
    assert_eq!(liking(b"4-4-2", &four_four_two, "4-4-2", 10.0, 5.0), 5.0);
    assert_eq!(liking(b"4-4-2 Diamond", &four_four_two, "4-4-2", 10.0, 20.0), 2.0);
    assert_eq!(liking(b"4-5-1", &four_four_two, "4-4-2", 20.0, 20.0), 0.0);
    assert_eq!(liking(b"4-2-4", &four_two_four, "", 15.0, 20.0), 1.0);
    assert_eq!(liking(b"4-2-4", &four_two_four, "", 5.0, 20.0), -1.0);
    let say = |own: &[u8], chosen: &[u8], preferred, strongest: &[u8]| String::from_utf8(formation_text(own, chosen, preferred, strongest)).unwrap();
    assert_eq!(say(b"4-4-2", b"4-4-2", "4-4-2", b"4-5-1"), "Your *4-4-2* is the shape I believe in and it suits this squad, so I would keep it.");
    assert_eq!(say(b"4-5-1", b"4-5-1", "4-4-2", b"4-5-1"), "I usually go for a 4-4-2, but your *4-5-1* gets more out of this squad, so I would keep it.");
    assert_eq!(say(b"2-2-3-2-1", b"4-4-2", "4-4-2", b"4-4-2"), "I would play the *4-4-2* instead of your 2-2-3-2-1: it is the shape I believe in and it suits this squad best.");
    assert_eq!(say(b"2-2-3-2-1", b"4-4-2", "4-4-2", b"4-5-1"), "I would play the *4-4-2* instead of your 2-2-3-2-1: it is the shape I believe in, although the 4-5-1 looks a little stronger on paper.");
    assert_eq!(say(b"2-2-3-2-1", b"4-5-1", "4-4-2", b"4-5-1"), "I usually go for a 4-4-2, but this squad suits the *4-5-1* better, so I would play that instead of your 2-2-3-2-1.");
    assert_eq!(say(b"4-5-1", b"4-5-1", "", b"4-4-2"), "Your *4-5-1* gets the most out of this squad, so I would keep it.");
    assert_eq!(say(b"4-5-1", b"4-4-2", "", b"4-4-2"), "This squad suits the *4-4-2* best, so I would play that instead of your 4-5-1.");
}
