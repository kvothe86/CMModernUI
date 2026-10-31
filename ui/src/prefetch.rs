//! Host: the answers a network client's cmui will ask for on a screen, sent with the screen
//! (`docs\network.md`, "Prefetch with the screen"). The host notes, per channel and screen kind
//! (the input handler of the channel's screen node, +8), what that client asked for there: keys
//! about a staff member on the screen as templates for every staff member on the next screen of
//! the kind, the rest as they were. Right after the game's screen (`screen_sent`) the answers for
//! those keys, and for the live match engine's rows on in-match tactics, go to the client in an
//! 'a' packet, the same as asked answers, so its first frame needs no round trip. The templates
//! that hold in any session are kept in `cmui.prefetch` and seed every new channel and session.
use crate::*;
use std::collections::{HashMap, HashSet};

/// The most a prefetch carries: four or five 'a' packets of `remote::PACKET_MAX`. The CM Network
/// wsock32.dll zstd-packs the wire (~10x), so on the line it costs about a tenth of that.
const MAX: usize = 160_000;
/// Exact keys kept per screen kind (the ones asked longest ago go first).
const EXACT_MAX: usize = 800;
/// Other templates kept per screen kind (the fewest hits go), and the longest `rest` of a Staff one.
const TEMPLATE_MAX: usize = 1200;
const REST_MAX: usize = 16;
/// A staff record's memory, and the chunks after the one a field points to that count as his.
const STAFF_SIZE: usize = 0x6e;
const AFTER: usize = 3;

#[derive(Clone, PartialEq, Eq, Hash)]
enum T {
    /// the key as asked
    Exact(Vec<u8>),
    /// a staff key (F, O, S field, A, W, V): its letter and the arguments after the staff
    Staff(u8, Vec<u8>),
    /// 'G' with the screen staff member's row name k (`row_names`)
    Name(u8),
    /// the memory chunks of the staff member's record
    Record,
    /// the chunk d after the one his field at +o points to (names, club, player record...)
    Ptr(u8, u8),
}

/// What a channel was asked on a kind of screen: template -> (screen count when last asked, hits).
#[derive(Default)]
struct Kind {
    screens: u32,
    seen: HashMap<T, (u32, u32)>,
}

/// The channel's last screen as sent: its kind, its staff, their row names, and the chunk each of
/// their fields points to (chunk -> (o, d)).
struct Sent {
    kind: usize,
    staff: Vec<usize>,
    set: HashSet<usize>,
    names: HashMap<Vec<u8>, u8>,
    ptrs: HashMap<usize, Vec<(u8, u8)>>,
    p0: usize,
}

/// Where an address lies, for the trace: the exe image (the same in any session), a record of the
/// club or staff table (index, offset of the address in it; the screen's own club marked), or not.
unsafe fn region(a: usize, p0: usize) -> String {
    if EXE.contains(&a) {
        return "exe".into();
    }
    for (name, table, count, size) in [("club", CLUB_TABLE, CLUB_COUNT, CLUB_SIZE), ("staff", STAFF_TABLE, STAFF_COUNT, STAFF_SIZE)] {
        let (base, n) = (rd::<usize>(table), rd::<i32>(count).max(0) as usize);
        if a >= base && a < base + n * size {
            let at = base + (a - base) / size * size;
            return format!("{name} {} +{:#x}{}", (a - base) / size, a - at, if at == p0 { " (screen's)" } else { "" });
        }
    }
    "other".into()
}

static mut KINDS: Option<HashMap<(i16, usize), Kind>> = None;
static mut SENT: Option<HashMap<i16, Sent>> = None;

/// The preset, in the game folder: per screen kind (the input handler, fixed in the exe) the
/// portable templates and their hits, read at the first prefetch, written every SAVE_EVERY.
const PRESET_FILE: &str = "cmui.prefetch";
const SAVE_EVERY: u32 = 20;
static mut PRESET: Option<HashMap<usize, HashMap<T, u32>>> = None;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// The exe image: memory there (the channels' displays, the screen manager) is at the same
/// address in every session.
const EXE: std::ops::Range<usize> = 0x400000..0xe00000;

/// An exact key that holds in another session: a screen parameter, a row name ('G'), a chunk of
/// the exe image (night3 D: 70-87% of a first visit's questions). Other chunks are this
/// session's heap.
fn portable(k: &[u8]) -> bool {
    matches!(k.first(), Some(b'P' | b'G')) || (k.first() == Some(&b'R') && k.len() == 5 && EXE.contains(&arg0(k)))
}

/// A template as a preset line, `kind \t type \t fields hex \t hits`; None when it does not hold
/// in another session (`portable`).
fn line(kind: usize, t: &T, hits: u32) -> Option<String> {
    let (ty, b) = match t {
        T::Exact(k) if portable(k) => ("E", k.clone()),
        T::Exact(_) => return None,
        T::Staff(letter, rest) => ("S", [&[*letter][..], rest].concat()),
        T::Name(k) => ("G", vec![*k]),
        T::Record => ("R", Vec::new()),
        T::Ptr(o, d) => ("P", vec![*o, *d]),
    };
    Some(format!("{kind:x}\t{ty}\t{}\t{hits}", hex(&b)))
}

/// A preset line back (None for a bad one).
fn parse(line: &str) -> Option<(usize, T, u32)> {
    let mut f = line.split('\t');
    let kind = usize::from_str_radix(f.next()?, 16).ok()?;
    let ty = f.next()?;
    let b = unhex(f.next()?)?;
    let hits = f.next()?.trim().parse().ok()?;
    let t = match (ty, &b[..]) {
        ("E", _) if portable(&b) => T::Exact(b.to_vec()),
        ("S", [letter, rest @ ..]) if b"FSWVAO".contains(letter) && rest.len() <= REST_MAX => T::Staff(*letter, rest.to_vec()),
        ("G", &[k]) => T::Name(k),
        ("R", []) => T::Record,
        ("P", &[o, d]) => T::Ptr(o, d),
        _ => return None,
    };
    Some((kind, t, hits))
}

/// The preset, read once (missing file or bad lines: skipped).
#[allow(static_mut_refs)]
unsafe fn preset() -> &'static HashMap<usize, HashMap<T, u32>> {
    PRESET.get_or_insert_with(|| {
        let mut p: HashMap<usize, HashMap<T, u32>> = HashMap::new();
        for (kind, t, hits) in std::fs::read_to_string(PRESET_FILE).unwrap_or_default().lines().filter_map(parse) {
            let h = p.entry(kind).or_default().entry(t).or_default();
            *h = (*h).max(hits);
        }
        log(&format!("prefetch preset: {} templates for {} screen kinds", p.values().map(|k| k.len()).sum::<usize>(), p.len()));
        p
    })
}

/// The preset and what every channel learned, written back: portable templates, pointer ones
/// with 2 hits or more (spurious matches stay low), the most hits first, TEMPLATE_MAX a kind. (An
/// exact key keeps 1 hit: once prefetched it is not asked again, night3 E.)
#[allow(static_mut_refs)]
unsafe fn save() {
    let mut all = preset().clone();
    for (&(_, kind), k) in KINDS.iter().flatten() {
        for (t, &(_, hits)) in &k.seen {
            let h = all.entry(kind).or_default().entry(t.clone()).or_default();
            *h = (*h).max(hits);
        }
    }
    let mut kinds: Vec<_> = all.into_iter().collect();
    kinds.sort_by_key(|k| k.0);
    let (mut text, mut n) = (String::new(), 0);
    for (kind, ts) in kinds {
        let mut ts: Vec<(T, u32)> = ts.into_iter().filter(|t| t.1 >= 2 || !matches!(t.0, T::Ptr(..))).collect();
        ts.sort_by(|a, b| b.1.cmp(&a.1));
        for l in ts.iter().filter_map(|(t, h)| line(kind, t, *h)).take(TEMPLATE_MAX) {
            text.push_str(&l);
            text.push('\n');
            n += 1;
        }
    }
    // (a temp file first: a game that stops mid-write leaves the old preset whole)
    let tmp = format!("{PRESET_FILE}.tmp");
    if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, PRESET_FILE).is_ok() && trace() {
        log(&format!("prefetch preset saved: {n} templates"));
    }
}

/// Keys a prefetch may answer: lookups and memory, never an action (X, Q, H, K) nor a news one.
fn kept(letter: u8) -> bool {
    b"RPCNFSWVAOMG".contains(&letter)
}

fn arg0(key: &[u8]) -> usize {
    key.get(1..5).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
}

/// A client's question, noted for its channel's screen kind (called for each key of a 'q' packet).
#[allow(static_mut_refs)]
pub unsafe fn asked(channel: i16, key: &[u8]) {
    let Some(&letter) = key.first().filter(|&&l| kept(l)) else { return };
    let Some(sent) = SENT.as_ref().and_then(|s| s.get(&channel)) else { return };
    let kind = KINDS.get_or_insert_with(HashMap::new).entry((channel, sent.kind)).or_default();
    let screens = kind.screens;
    let mut found = false;
    let mut mark = |t: T| {
        let e = kind.seen.entry(t).or_insert((screens, 0));
        *e = (screens, e.1 + 1);
    };
    let staff = arg0(key);
    match letter {
        b'R' => {
            if sent.staff.iter().any(|&s| staff < s.saturating_add(STAFF_SIZE) && staff.saturating_add(remote::CHUNK) > s) {
                mark(T::Record);
                found = true;
            }
            // (the nearest only: the chunk a name points into, not also the one after another's)
            let ptrs = sent.ptrs.get(&staff).map_or(&[][..], |v| &v[..]);
            let nearest = ptrs.iter().map(|p| p.1).min();
            for &(o, d) in ptrs.iter().filter(|p| Some(p.1) == nearest) {
                mark(T::Ptr(o, d));
                found = true;
            }
        }
        b'F' | b'S' | b'W' | b'V' | b'A' | b'O' if sent.set.contains(&staff) && key.len() >= 5 && key.len() - 5 <= REST_MAX => {
            mark(T::Staff(letter, key[5..].to_vec()));
            found = true;
        }
        b'G' => {
            if let Some(&k) = sent.names.get(&key[1..]) {
                mark(T::Name(k));
                found = true;
            }
        }
        _ => {}
    }
    if !found {
        mark(T::Exact(key.to_vec()));
    }
    // (each question and what it was filed as: exact keys with where they point, as only those
    // stay out of the preset)
    if trace() {
        let what = if found { "template".into() } else if letter == b'P' { "exact, screen parameter".into() } else { format!("exact, {}", region(staff, sent.p0)) };
        log(&format!("prefetch asked on kind {:#x}: {} {} ({what})", sent.kind, letter as char, hex(&key[1..])));
    }
    // (only looked at when the map is big at all: no recount for every key)
    if kind.seen.len() > EXACT_MAX {
        prune(&mut kind.seen, |t| matches!(t, T::Exact(_)), EXACT_MAX);
        prune(&mut kind.seen, |t| !matches!(t, T::Exact(_)), TEMPLATE_MAX);
    }
}

/// Keep `max` of the entries `pick` selects: the newest asked, then the most hits (ties cut too).
fn prune(seen: &mut HashMap<T, (u32, u32)>, pick: impl Fn(&T) -> bool, max: usize) {
    let mut all: Vec<(u32, u32, T)> = seen.iter().filter(|(t, _)| pick(t)).map(|(t, v)| (v.0, v.1, t.clone())).collect();
    if all.len() <= max {
        return;
    }
    all.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    for (_, _, t) in all.drain(max..) {
        seen.remove(&t);
    }
}

/// The staff the screen being sent is about: staff objects of its items, screen parameter 0 when
/// it is a staff member, or a club's squad (club +0xd7, 50 staff) when it is a club, the players
/// of the live match engine.
unsafe fn screen_staff(display: usize, engine: Option<usize>) -> Vec<usize> {
    let mut out = Vec::new();
    let count = if mapped(display + 0x12e9a0, 2) { rd::<i16>(display + 0x12e9a0).clamp(0, 1200) as usize } else { 0 };
    for k in 0..count {
        let w = display + 0xba95e + k * 0x18c;
        if !mapped(w, 0x18c) {
            break;
        }
        out.push(rd::<usize>(w + 0x48));
    }
    let p0 = screen_param(0);
    out.push(p0);
    if valid_club(p0) {
        out.extend((0..50).map(|k| rd::<usize>(p0 + 0xd7 + 4 * k)));
    }
    if let Some(e) = engine {
        out.extend((0..40).map(|k| rd::<usize>(e + 0x4796 + k * 0x1be + 0x69)));
    }
    let mut seen = HashSet::new();
    out.retain(|&s| valid_staff(s) && seen.insert(s));
    out.truncate(160);
    out
}

/// A club record of the club table.
pub unsafe fn valid_club(club: usize) -> bool {
    let (base, count) = (rd::<usize>(CLUB_TABLE), rd::<i32>(CLUB_COUNT));
    count > 0 && club >= base && club < base + count as usize * CLUB_SIZE && (club - base) % CLUB_SIZE == 0 && mapped(club, CLUB_SIZE)
}

/// In-match tactics (`live_engine`): the engine, and the chunks of its player rows and of the
/// screen's own copy of them.
unsafe fn live_chunks() -> (Option<usize>, Vec<usize>) {
    let t = screen_param(0);
    if screen_param(10) != 6 || !mapped(t, 0x2735 + 20 * 0x1be) {
        return (None, Vec::new());
    }
    let e = rd::<usize>(t + 4);
    if !mapped(e + 0x4796, 40 * 0x1be) || rd::<usize>(e + 0x4796 + 0x19e) != e {
        return (None, Vec::new());
    }
    let span = |from: usize, len: usize| (from / remote::CHUNK..=(from + len - 1) / remote::CHUNK).map(|c| c * remote::CHUNK);
    let mut chunks: Vec<usize> = span(t, 8).chain(span(e + 0x4796, 40 * 0x1be)).chain(span(t + 0x2735, 20 * 0x1be)).chain(span(e + 0x8eb3, 0x11)).collect();
    chunks.dedup();
    (Some(e), chunks)
}

/// Right after the game sent the channel its screen (in `screen_sent`, while the screen manager
/// serves that channel): the answers its cmui will want there, to its seat `slot`.
#[allow(static_mut_refs)]
pub unsafe fn send(channel: i16, display: usize, slot: i32) {
    // (only for the channel being served, with a screen: as host_answer's 'P')
    // (a screen not sent below must not leave the old one's kind to file the new questions under)
    let Some(node) = served_node(channel).filter(|&n| mapped(n + 8, 4)) else {
        SENT.as_mut().map(|s| s.remove(&channel));
        return;
    };
    let kind_id = rd::<usize>(node + 8);
    // (the same screen of the same kind again within a second: not sent again)
    static mut LAST: Option<HashMap<i16, (usize, usize, std::time::Instant)>> = None;
    let same = (kind_id, screen_param(0));
    let last = LAST.get_or_insert_with(HashMap::new);
    if last.get(&channel).is_some_and(|l| (l.0, l.1) == same && l.2.elapsed() < std::time::Duration::from_secs(1)) {
        return;
    }
    last.insert(channel, (same.0, same.1, std::time::Instant::now()));
    let (engine, live) = if kind_id == TACTICS_INPUT { live_chunks() } else { (None, Vec::new()) };
    let staff = screen_staff(display, engine);
    let kinds = KINDS.get_or_insert_with(HashMap::new);
    // (a kind new on this channel starts from the preset: screens 0, never asked, the saved hits)
    let kind = kinds.entry((channel, kind_id)).or_insert_with(|| Kind {
        screens: 0,
        seen: preset().get(&kind_id).map_or_else(HashMap::new, |p| p.iter().map(|(t, &hits)| (t.clone(), (0, hits))).collect()),
    });
    kind.screens += 1;
    // what the staff's fields point to (every byte offset: the record is packed), for `asked`;
    // worked out when the kind has no pointer template yet and now and then to learn new ones
    // (a kind's first screen, screens 1, learns: so a seeded one still files new pointers)
    let learn = kind.screens % 10 == 1 || !kind.seen.keys().any(|t| matches!(t, T::Ptr(..)));
    let mut ptrs: HashMap<usize, Vec<(u8, u8)>> = HashMap::new();
    let mut names = HashMap::new();
    let rows: Vec<Vec<Vec<u8>>> = staff.iter().map(|&s| row_names(s)).collect();
    for (i, &s) in staff.iter().enumerate() {
        for o in 0..=STAFF_SIZE - 4 {
            if !learn {
                break;
            }
            let p = rd::<usize>(s + o);
            if p > 0x10000 {
                for d in 0..AFTER {
                    let c = (p - p % remote::CHUNK).saturating_add(d * remote::CHUNK);
                    let v = ptrs.entry(c).or_default();
                    if !v.contains(&(o as u8, d as u8)) {
                        v.push((o as u8, d as u8));
                    }
                }
            }
        }
        for (k, n) in rows[i].iter().enumerate() {
            names.entry(n.clone()).or_insert(k as u8);
        }
    }
    // the keys, most wanted first: screen parameters, lookups, then memory (the live engine, the
    // staff records, what they point to by how often it was asked, the rest newest first, and
    // last the chunks after the one a field points to: a name that runs over now and then, but
    // sent for every staff member they crowded the asked chunks out at the cap, night3 D)
    let mut keys: Vec<(u8, u32, Vec<u8>)> = live.iter().map(|&c| (2, u32::MAX, key(b'R', &[c as u32]))).collect();
    for (t, &(at, hits)) in &kind.seen {
        match t {
            T::Exact(k) => keys.push((if k[0] == b'P' { 0 } else if k[0] == b'R' { 4 } else { 1 }, at, k.clone())),
            T::Staff(letter, rest) => keys.extend(staff.iter().map(|&s| (1, hits, [&key(*letter, &[s as u32])[..], rest].concat()))),
            T::Name(n) => keys.extend(rows.iter().filter_map(|r| r.get(*n as usize).cloned()).map(|name| (1, hits, [&b"G"[..], &name].concat()))),
            T::Record => {
                for &s in &staff {
                    let (first, last) = (s / remote::CHUNK, (s + STAFF_SIZE - 1) / remote::CHUNK);
                    keys.extend((first..=last).map(|c| (2, hits, key(b'R', &[(c * remote::CHUNK) as u32]))));
                }
            }
            T::Ptr(o, d) => keys.extend(staff.iter().filter_map(|&s| {
                let p = rd::<usize>(s + *o as usize);
                (p > 0x10000).then(|| (if *d == 0 { 3 } else { 5 }, hits, key(b'R', &[(p - p % remote::CHUNK).wrapping_add(*d as usize * remote::CHUNK) as u32])))
            })),
        }
    }
    keys.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut done = HashSet::new();
    let (mut entries, mut bytes) = (Vec::new(), 0);
    // (under the trace the keys past the cap are counted too, by class: what the cap cuts)
    let (mut full, mut sent, mut cut) = (false, [(0, 0); 6], [(0, 0); 6]);
    for (class, _, k) in keys {
        if full && !trace() {
            break;
        }
        if !done.insert(k.clone()) {
            continue;
        }
        // (what the host has nothing for is left to the client to ask: not cached as "none"; but
        // a row name is looked up in a fixed index, and the client tries every cell ("23",
        // "£4.1M"): its "none" is sent, night3 E asked 112-129 of them on every search visit)
        let v = host_answer(channel, &k);
        if v.is_none() && k[0] != b'G' {
            continue;
        }
        let e = remote::answer_entry(&k, v);
        full |= bytes + e.len() > MAX;
        let c = if full { &mut cut[class as usize] } else { &mut sent[class as usize] };
        *c = (c.0 + 1, c.1 + e.len());
        if !full {
            bytes += e.len();
            entries.push(e);
        }
    }
    if trace() {
        let p0 = screen_param(0);
        log(&format!("prefetch by class (P, lookups, records + live, pointers, exact R, pointers d >= 1) as keys/bytes: sent {sent:?}, cut {cut:?} (parameter 0 {p0:#x}, {})", region(p0, p0)));
    }
    // (logged without the trace too, the first 500 screens: the night tests count them)
    static mut LOGGED: u32 = 0;
    LOGGED += 1;
    if LOGGED <= 500 || trace() {
        log(&format!("prefetch to channel {channel}: {} keys, {bytes} bytes (screen kind {kind_id:#x}, {} staff, {} templates)", entries.len(), staff.len(), kind.seen.len()));
    }
    if !entries.is_empty() {
        remote::send_answers(channel, slot, entries);
    }
    SENT.get_or_insert_with(HashMap::new).insert(channel, Sent { kind: kind_id, set: staff.iter().copied().collect(), staff, names, ptrs, p0: screen_param(0) });
    static mut SENDS: u32 = 0;
    SENDS += 1;
    if SENDS % SAVE_EVERY == 0 {
        save();
    }
}

#[cfg(test)]
#[test]
fn preset_lines() {
    let all = [T::Exact(b"P\x03\0\0\0".to_vec()), T::Staff(b'S', vec![0x2a, 0]), T::Name(2), T::Record, T::Ptr(0x39, 1)];
    for t in all {
        let l = line(0x89f710, &t, 7).unwrap();
        assert!(parse(&l) == Some((0x89f710, t, 7)), "{l}");
    }
    assert!(line(1, &T::Exact(b"R\0\x10\0\0".to_vec()), 9).is_none());
    for k in [&b"R\0\x43\xd6\0"[..], b"GVega"] {
        let l = line(1, &T::Exact(k.to_vec()), 2).unwrap();
        assert!(parse(&l) == Some((1, T::Exact(k.to_vec()), 2)), "{l}");
    }
    assert!(parse("1\tE\t00dfc942\t2").is_none() && parse("1\tE\t5200dfc942\t2").is_none());
    assert!(parse("89f710\tS\t5a\t3").is_none() && parse("zz\tR\t\t3").is_none() && parse("89f710\tP\t3").is_none());
}
