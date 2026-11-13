//! Saved line-ups 1 / 2 / 3 and Fill subs (switch `saved_lineups`, the user's design of
//! 2026-11-07, `ui\FEATURES.md`): on the human's own squad screen a strip in the right part of
//! the list's title band (the assistant's note keeps the left part, the view's title the middle):
//! three slots and a Fill subs button. A slot keeps the eleven and the bench by staff id with the
//! formation's shape; loading puts them back, a starter who cannot play (gone, injured, banned,
//! marked not available, condition under 75 %) is replaced for his slot by the assistant's pick,
//! then the empty bench places are filled (`assistant::fill`). Fill subs keeps the selection and
//! fills only the empty bench places. In a network game the host carries out a client's actions
//! on that client's club and keeps its slots in the host's file (request 'H' codes, `code`; the
//! shapes by key 'l'), night 8.

use crate::*;

const FILE: &str = "cmui_lineups.txt";
const LOW_CONDITION: i32 = 75;

#[derive(Clone, Default)]
struct Slot {
    shape: String,
    ids: [i32; 20], // staff ids, -1 empty
}

/// What the strip asked for, carried out by the club screen's input (which builds the screen again).
#[derive(Clone, Copy)]
pub(crate) enum Action {
    Load(usize),
    Save(usize),
    Clear(usize),
    FillSubs,
}

static mut SLOTS: Option<(usize, [Option<Slot>; 3])> = None; // club, its three slots
static mut PENDING: Option<Action> = None;
static mut RECTS: Vec<((i32, i32, i32, i32), Action)> = Vec::new();
static mut LOADED: Option<(usize, usize)> = None; // slot, stand-ins
static mut ARMED: Option<usize> = None; // a full slot's save asked once: the next click saves over it

unsafe fn club_index(club: usize) -> usize {
    (club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE
}

/// The club's slots from the file (read once a club).
#[allow(static_mut_refs)]
unsafe fn slots(club: usize) -> &'static mut [Option<Slot>; 3] {
    if SLOTS.as_ref().is_none_or(|s| s.0 != club) {
        let mut out: [Option<Slot>; 3] = Default::default();
        let me = club_index(club).to_string();
        for line in std::fs::read_to_string(FILE).unwrap_or_default().lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 4 || f[0] != me {
                continue;
            }
            let Ok(k) = f[1].parse::<usize>() else { continue };
            let mut ids = [-1; 20];
            for (i, v) in f[3].split(',').take(20).enumerate() {
                ids[i] = v.parse().unwrap_or(-1);
            }
            if k < 3 {
                out[k] = Some(Slot { shape: f[2].to_string(), ids });
            }
        }
        SLOTS = Some((club, out));
    }
    &mut SLOTS.as_mut().unwrap().1
}

/// The club's slots back to the file (other clubs' lines kept).
#[allow(static_mut_refs)]
unsafe fn save_file(club: usize) {
    let me = club_index(club).to_string();
    let mut lines: Vec<String> = std::fs::read_to_string(FILE).unwrap_or_default().lines().filter(|l| l.split('\t').next() != Some(me.as_str())).map(str::to_string).collect();
    for (k, s) in slots(club).iter().enumerate() {
        if let Some(s) = s {
            let ids: Vec<String> = s.ids.iter().map(i32::to_string).collect();
            lines.push(format!("{me}\t{k}\t{}\t{}", s.shape, ids.join(",")));
        }
    }
    let _ = std::fs::write(FILE, lines.join("\r\n") + "\r\n");
}

fn shape_of(labels: &[Vec<u8>]) -> String {
    let count = |p: &[&str]| labels[1..11].iter().filter(|l| p.iter().any(|x| l.starts_with(x.as_bytes()))).count();
    let (dm, am) = (count(&["DM"]), count(&["AM"]));
    let lines = [count(&["SW", "D", "WB"]) - dm, dm, count(&["M"]), am, count(&["F", "ST"])];
    lines.iter().enumerate().filter(|(i, &n)| n > 0 || *i == 0 || *i == 2 || *i == 4).map(|(_, n)| n.to_string()).collect::<Vec<_>>().join("-")
}

#[cfg(test)]
#[test]
fn shapes() {
    let l = |s: &[&str]| s.iter().map(|x| x.as_bytes().to_vec()).collect::<Vec<_>>();
    assert_eq!(shape_of(&l(&["GK", "DL", "DC", "DC", "DR", "DMC", "MC", "MC", "AML", "AMR", "FC"])), "4-1-2-2-1");
    assert_eq!(shape_of(&l(&["GK", "DL", "DC", "DC", "DR", "ML", "MC", "MC", "MR", "FC", "FC"])), "4-4-2");
}

/// The club's player with this id (its squad, 50 places).
/// The club's saved line-ups for the dashboard export: (slot 0..2, shape, the eleven's staff, 0 for
/// one who left the club).
pub(crate) unsafe fn saved(club: usize) -> Vec<(usize, String, Vec<usize>)> {
    slots(club).iter().enumerate().filter_map(|(k, s)| s.as_ref().map(|s| (k, s.shape.clone(), s.ids[..11].iter().map(|&id| by_id(club, id)).collect()))).collect()
}

unsafe fn by_id(club: usize, id: i32) -> usize {
    (0..50).map(|k| rd::<usize>(club + 0xd7 + 4 * k)).find(|&s| id >= 0 && readable(s, 0x6e) && plays_for(s, club) && rd::<i32>(s) == id).unwrap_or(0)
}

/// An action as a request code for the host (a network client's strip, request 'H' after the
/// Assistant's four picks): 100 Fill subs, 110 + k load, 120 + k save, 130 + k clear.
fn code(a: Action) -> u32 {
    match a {
        Action::FillSubs => 100,
        Action::Load(k) => 110 + k as u32,
        Action::Save(k) => 120 + k as u32,
        Action::Clear(k) => 130 + k as u32,
    }
}

pub(crate) fn action(code: u32) -> Option<Action> {
    let k = (code % 10) as usize;
    match code {
        100 => Some(Action::FillSubs),
        110..=112 => Some(Action::Load(k)),
        120..=122 => Some(Action::Save(k)),
        130..=132 => Some(Action::Clear(k)),
        _ => None,
    }
}

#[cfg(test)]
#[test]
fn codes() {
    for a in [Action::FillSubs, Action::Load(2), Action::Save(0), Action::Clear(1)] {
        assert_eq!(action(code(a)).map(code), Some(code(a)));
    }
    assert!(action(3).is_none() && action(113).is_none());
}

/// The club screen's input: a pending action carried out. True: the screen is to be built again.
#[allow(static_mut_refs)]
pub unsafe fn run() -> bool {
    let Some(action) = PENDING.take() else { return false };
    let Some((club, labels, unavailable)) = assistant::squad_now() else { return false };
    match action {
        Action::Save(_) => ARMED = None,
        Action::Clear(k) if LOADED.is_some_and(|l| l.0 == k) => LOADED = None,
        Action::FillSubs => LOADED = None,
        _ => {}
    }
    let Some((note, ins)) = run_for(code(action), club, &labels, &unavailable) else { return false };
    if let Action::Load(k) = action {
        LOADED = Some((k, ins));
    }
    if !note.is_empty() {
        assistant::set_note(note);
    }
    true
}

/// An action (`code`) for a human's team, here or on the host for a network client: the note
/// and the stand-ins put in. None: nothing to do (an empty slot loaded).
#[allow(static_mut_refs)]
pub(crate) unsafe fn run_for(code: u32, club: usize, labels: &[Vec<u8>], unavailable: &[usize]) -> Option<(Vec<u8>, usize)> {
    let now = assistant::selection(club);
    let mut note = String::new();
    let fixed: [usize; 20] = match action(code)? {
        Action::Save(k) => {
            let ids = now.map(|s| if readable(s, 0x6e) { rd::<i32>(s) } else { -1 });
            slots(club)[k] = Some(Slot { shape: shape_of(labels), ids });
            save_file(club);
            return Some((to_latin1(&format!("Line-up saved as XI {}.", k + 1)), 0));
        }
        Action::Clear(k) => {
            slots(club)[k] = None;
            save_file(club);
            return Some((to_latin1(&format!("XI {} cleared.", k + 1)), 0));
        }
        // (the selection as it is; only the empty bench places)
        Action::FillSubs => now,
        Action::Load(k) => {
            let s = slots(club)[k].clone()?;
            let mut fixed = [0usize; 20];
            for (i, &id) in s.ids.iter().enumerate() {
                let staff = by_id(club, id);
                if staff != 0 && !assistant::injured(staff) && (i >= 11 || condition(staff).is_none_or(|c| c >= LOW_CONDITION)) {
                    fixed[i] = staff;
                }
            }
            let shape = shape_of(labels);
            if s.shape != shape {
                note = format!(" XI {} was saved for {}, the tactic is {}.", k + 1, s.shape, shape);
            }
            fixed
        }
    };
    Some(match assistant::fill_team(club, labels, unavailable, fixed) {
        Some((ins, _, n)) => ([n, to_latin1(&note)].concat(), ins.len()),
        None => (b"No assistant to fill the team.".to_vec(), 0),
    })
}

/// The three slots' shapes of a club (its index), "-" for an empty one, one a line: the host's
/// answer to a network client's key 'l'.
#[allow(static_mut_refs)]
pub(crate) unsafe fn remote_summary(index: usize) -> Option<Vec<u8>> {
    let club = rd::<usize>(CLUB_TABLE) + index * CLUB_SIZE;
    if index >= 0x10000 || !readable(club, 0x40) {
        return None;
    }
    Some(slots(club).iter().map(|s| s.as_ref().map_or("-".to_string(), |s| s.shape.clone())).collect::<Vec<_>>().join("\n").into_bytes())
}

/// The slots' shapes as the strip shows them: kept here, or the host's for a network client
/// (None while its answer is on its way).
unsafe fn slot_shapes(club: usize) -> Option<[Option<String>; 3]> {
    if !remote::CLIENT {
        return Some(slots(club).clone().map(|s| s.map(|s| s.shape)));
    }
    // (with the number of the last request the host has carried out: the answer to a question
    // asked while a save was on its way is not kept after it, night 8: "XI 1 empty" after Save)
    let v = ask_host(&key(b'l', &[club_index(club) as u32, assistant::requests_done()]))?;
    let mut out: [Option<String>; 3] = Default::default();
    for (k, line) in String::from_utf8_lossy(&v).lines().take(3).enumerate() {
        out[k] = (line != "-").then(|| line.to_string());
    }
    Some(out)
}

/// The flip pass on the squad screen: the strip in the band, the tags in the list.
#[allow(static_mut_refs)]
pub unsafe fn draw(font: u32) {
    RECTS.clear();
    let Some(club) = assistant::strip_team() else { return };
    let Some(saved) = slot_shapes(club) else { return };
    let Some(((_, by1, bx2, by2), free)) = assistant::band() else { return };
    let small = smaller(font, 0);
    let (white, muted) = ([0xf4, 0xf6, 0xf8], [0x8a, 0x94, 0xa4]);
    // the strip: right of the title, in the room there
    let right = bx2 - 12;
    let room = free.max(0);
    let compact = small_screen() || room < 560;
    let (y1, y2) = (by1 + 6, by2 - 6);
    // (compact: the slots share the room right of the title; at 800x600 they ran over it,
    // "Positic", night 8; "Fill", digits and "S" where even that is tight)
    let tight = compact && room < 320;
    let fill_label: &[u8] = if tight { b"Fill" } else { b"Fill subs" };
    let fill_w = if tight { text_width(small, fill_label) + 16 } else { (text_width(small, fill_label) + 28).max(80) };
    let slot_w = if compact { ((room - fill_w - 3 * 8) / 3).clamp(40, (y2 - y1) * 3) } else { ((room - fill_w - 4 * 8) / 3).clamp(110, 200) };
    let mut x = right - fill_w;
    let fb = (x, y1, right, y2);
    // (1680x1050: rounded pills on what is behind them, as the flat buttons do)
    let bg_at = |px: i32, py: i32| pixel_at(px, py).unwrap_or([0x16, 0x1a, 0x20]);
    if compact {
        fill(fb.0, fb.1, fb.2, fb.3, [0x2b, 0x45, 0x66]);
    } else {
        fill(fb.0, fb.1, fb.2, fb.3, bg_at(fb.0 - 2, (y1 + y2) / 2));
        rounded(fb.0, fb.1, fb.2, fb.3, [0x2b, 0x47, 0x66], 6);
    }
    text_at((fb.0 + fb.2) / 2, y1, y2, small, white, fill_label, 1);
    RECTS.push((fb, Action::FillSubs));
    for k in (0..3).rev() {
        x -= slot_w + 8;
        let r = (x, y1, x + slot_w, y2);
        let s = &saved[k];
        let active = LOADED.is_some_and(|l| l.0 == k);
        // two buttons a slot (test note 1: a floppy and a second number were unclear): "Load XI n"
        // with its shape under it (grey "XI n empty" when nothing is kept), and "Save" at the
        // right ("Save over?" in red after a first click on a full slot)
        let armed = ARMED == Some(k);
        let save_label: &[u8] = if armed { b"Sure?" } else { b"Save" };
        let save_w = if tight { (y2 - y1).min(slot_w / 2) } else if compact { (y2 - y1) * 3 / 2 } else { text_width(small, b"Save") + 14 };
        let save = (r.2 - save_w, y1, r.2, y2);
        let load = (r.0, y1, if compact { save.0 - 4 } else { save.0 }, y2);
        let blue = [0x2b, 0x45, 0x66];
        if compact {
            fill(load.0, load.1, load.2, load.3, if s.is_some() { blue } else { [0x22, 0x27, 0x2f] });
            if active {
                fill(load.0, load.1, load.2, load.1 + 2, GOLD);
                fill(load.0, load.3 - 2, load.2, load.3, GOLD);
            }
        } else {
            // one segmented pill: Load part + Save part touching, a 1 px divider between
            const R: i32 = 6;
            fill(r.0, r.1, r.2, r.3, bg_at(r.0 - 2, (y1 + y2) / 2));
            let edge = [0x3a, 0x40, 0x4a];
            let save_face = if armed { [0xa8, 0x3a, 0x3a] } else { [0x22, 0x33, 0x48] };
            if s.is_some() {
                rounded(load.0, load.1, load.2, load.3, [0x2b, 0x47, 0x66], R);
                fill(load.2 - R, load.1, load.2, load.3, [0x2b, 0x47, 0x66]);
            } else {
                rounded(load.0, load.1, load.2, load.3, edge, R);
                fill(load.2 - R, load.1, load.2, load.3, edge);
                let face = [0x1e, 0x23, 0x2a];
                rounded(load.0 + 1, load.1 + 1, load.2, load.3 - 1, face, R - 1);
                fill(load.2 - R, load.1 + 1, load.2, load.3 - 1, face);
            }
            rounded(save.0, save.1, save.2, save.3, save_face, R);
            fill(save.0, save.1, save.0 + R, save.3, save_face);
            fill(save.0, save.1, save.0 + 1, save.3, [0x16, 0x1e, 0x2a]);
            if active {
                fill(load.0 + 5, load.3 - 3, save.2 - 5, load.3 - 1, GOLD);
            }
        }
        let mid = (y1 + y2) / 2;
        let cx = (load.0 + load.2) / 2;
        // (a narrow button: its number only)
        let short = if text_width(small, b"XI 1") + 6 > load.2 - load.0 { format!("{}", k + 1) } else { format!("XI {}", k + 1) };
        match s {
            Some(s) if compact => text_at(cx, y1, y2, small, white, short.as_bytes(), 1),
            Some(s) => {
                text_at(cx, y1, mid + 2, small, white, format!("Load XI {}", k + 1).as_bytes(), 1);
                // (inside the button: the shape, or the changes where both don't fit; night 7 shot
                // "4-1-2-2-1 · 2 changed" ran over the view's title)
                let changed = LOADED.filter(|l| l.0 == k && l.1 > 0).map(|l| l.1);
                let room = load.2 - load.0 - 8;
                let both = changed.map(|n| format!("{s} \u{b7} {n} changed"));
                let under = match (both, changed) {
                    (Some(b), _) if text_width(small, &to_latin1(&b)) <= room => b,
                    (_, Some(n)) => format!("{n} changed"),
                    _ => s.clone(),
                };
                let line = clip_text(small, &to_latin1(&under), room);
                text_at(cx, mid - 2, y2, small, if changed.is_some() { GOLD } else { [0xb8, 0xc4, 0xd4] }, &line, 1);
            }
            None if compact => text_at(cx, y1, y2, small, muted, short.as_bytes(), 1),
            None => {
                text_at(cx, y1, mid + 2, small, muted, format!("XI {}", k + 1).as_bytes(), 1);
                text_at(cx, mid - 2, y2, small, muted, b"empty", 1);
            }
        }
        if compact {
            fill(save.0, save.1, save.2, save.3, if armed { [0xa8, 0x3a, 0x3a] } else { [0x3a, 0x5a, 0x3a] });
        }
        text_at((save.0 + save.2) / 2, y1, y2, small, white, if compact { if armed { b"?".as_slice() } else { b"S".as_slice() } } else { save_label }, 1);
        RECTS.push((save, Action::Save(k)));
        if s.is_some() {
            RECTS.push((load, Action::Load(k)));
        }
    }
}

/// A new screen: the strip's spots go (drawn again on the squad screen).
#[allow(static_mut_refs)]
pub(crate) unsafe fn forget() {
    RECTS.clear();
}

/// The mouse on the squad screen: a click on the strip. A right click on a slot clears it.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    // (only on the squad screen: the strip's spots of an earlier screen took clicks on any
    // screen there, night 8: the Short List's page arrow ran Fill subs)
    assistant::strip_team()?;
    let hit = RECTS.iter().find(|r| (r.0 .0..=r.0 .2).contains(&mx) && (r.0 .1..=r.0 .3).contains(&my)).map(|r| r.1)?;
    // (this process's own mouse state: `rd` reads the host's memory on a network client)
    if *(assistant::RIGHT_CLICK as *const u32) != 0 {
        *(assistant::RIGHT_CLICK as *mut u32) = 0;
        if let Action::Load(k) | Action::Save(k) = hit {
            // (a client's goes to the host like its other actions; it was lost there, night 8 audit)
            if remote::CLIENT {
                if LOADED.is_some_and(|l| l.0 == k) {
                    LOADED = None;
                }
                assistant::client_lineup(code(Action::Clear(k)));
            } else {
                PENDING = Some(Action::Clear(k));
            }
            return Some(true);
        }
    }
    if !released {
        return Some(false);
    }
    *(LEFT_CLICK as *mut u32) = 0;
    let action = match hit {
        // (a full slot: the first click arms, the second saves over it)
        Action::Save(k) if assistant::strip_team().and_then(|c| slot_shapes(c)).is_some_and(|s| s[k].is_some()) && ARMED != Some(k) => {
            ARMED = Some(k);
            return Some(true);
        }
        a => a,
    };
    if !matches!(action, Action::Save(_)) {
        ARMED = None;
    }
    if remote::CLIENT {
        // (the host carries it out on its copy of this club, the note comes back by 'J'; what
        // is loaded is shown here at once)
        match action {
            Action::Save(_) => ARMED = None,
            Action::Load(k) => LOADED = Some((k, 0)),
            Action::FillSubs => LOADED = None,
            Action::Clear(k) if LOADED.is_some_and(|l| l.0 == k) => LOADED = None,
            _ => {}
        }
        assistant::client_lineup(code(action));
    } else {
        PENDING = Some(action);
    }
    Some(true)
}
