//! cmui in a network game. A client is a terminal: the host builds its screens and holds all the
//! game data, so what cmui reads from game memory while drawing comes from the host. The two
//! cmuis talk through the game's own connection, in packets with an opcode the game does not use
//! (0x7c): the client asks for keys (a memory chunk, a screen parameter, one of cmui's lookups),
//! the host answers from its memory on its game thread, the client keeps the answers and redraws.
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub const OPCODE: u8 = 0x7c;
/// Memory goes in chunks of this size (a page holds 16, so a chunk is readable as a whole).
pub const CHUNK: usize = 256;
/// Answers older than this are asked for again (still used meanwhile).
const FRESH: Duration = Duration::from_millis(2000);
/// On a screen older than SETTLED: this instead. A live page (Latest Scores) draws all the time
/// and asked ~300 chunks every 2 s for as long as it stood.
const SETTLED: Duration = Duration::from_secs(10);
const REFRESH: Duration = Duration::from_secs(10);
/// Questions without an answer are repeated after this. Every 2 s (until 2026-10-31) they piled up
/// while the host was busy: it came back to 97 s of questions, each answered at 80 kB on its game
/// thread, and hung ("Not responding", a friendly took 15 minutes).
const UNANSWERED: Duration = Duration::from_secs(15);
/// The most a packet carries (the game's buffers hold 50,000 bytes).
const PACKET_MAX: usize = 40_000;

const SCREEN_MANAGER: usize = 0xb74450;
const NETWORK: usize = SCREEN_MANAGER + 0x3840; // Network* of the session
const SEND_PACKET: usize = 0x77ac30; // Network::send_packet(this, data, length, slot), thiscall

/// This process is a network client (in ScreenManager::run_network_client).
pub static mut CLIENT: bool = false;

/// A network game, host or client: the session has a Network object. Single-player-only
/// features (saved line-ups, the form strip: no unfair advantage, user 2026-11-07) check this.
/// ponytail: the pointer is taken as 0 in single player; the log line on each change shows it.
pub unsafe fn network_game() -> bool {
    static mut WAS: Option<bool> = None;
    let now = CLIENT || *(NETWORK as *const usize) != 0;
    if WAS != Some(now) {
        WAS = Some(now);
        crate::log(&format!("network game: {now}"));
    }
    now
}
/// The client's channel on the host (its seat in the host's screen manager), from the answers.
pub static mut CHANNEL: i16 = 0;
/// Something new arrived: the screen is drawn again.
pub static mut ARRIVED: bool = false;

struct Entry {
    value: Option<Vec<u8>>, // None: the host has nothing for it (unreadable, not found)
    at: Instant,
    used: Instant, // last stored or read (the cache drops what no screen has used for a while)
}

/// Answers nobody read for this long are dropped at a new screen, and the cache keeps this many at most.
const KEEP: Duration = Duration::from_secs(180);
const CACHE_MAX: usize = 20_000;
/// Keys waiting / asked beyond this are dropped at a new screen (they are asked again when drawn).
const PENDING_MAX: usize = 5_000;

static mut CACHE: Option<HashMap<Vec<u8>, Entry>> = None;
static mut WANTED: Option<HashSet<Vec<u8>>> = None;
static mut ASKED: Option<HashMap<Vec<u8>, Instant>> = None;

#[allow(static_mut_refs)]
unsafe fn state() -> (&'static mut HashMap<Vec<u8>, Entry>, &'static mut HashSet<Vec<u8>>, &'static mut HashMap<Vec<u8>, Instant>) {
    (CACHE.get_or_insert_with(HashMap::new), WANTED.get_or_insert_with(HashSet::new), ASKED.get_or_insert_with(HashMap::new))
}

/// How old an answer may be on the current screen.
unsafe fn fresh() -> Duration {
    if SCREEN.1.is_some_and(|t| t.elapsed() > SETTLED) {
        REFRESH
    } else {
        FRESH
    }
}

/// The host's answer for a key: None while it is not known yet (asked for), Some(None) when the
/// host has nothing for it.
pub unsafe fn get(key: &[u8]) -> Option<Option<Vec<u8>>> {
    let (cache, wanted, asked) = state();
    let entry = cache.get_mut(key);
    if entry.as_ref().is_none_or(|e| e.at.elapsed() > fresh()) && !asked.contains_key(key) {
        wanted.insert(key.to_vec());
    }
    entry.map(|e| {
        e.used = Instant::now();
        e.value.clone()
    })
}

/// One memory chunk of the host (CHUNK bytes from an aligned address), None when unknown or
/// unreadable there.
pub unsafe fn chunk(base: usize) -> Option<Vec<u8>> {
    let mut key = vec![b'R'];
    key.extend((base as u32).to_le_bytes());
    get(&key).flatten()
}

/// Everything known is old now (a new screen): used until the fresh answers come.
#[allow(static_mut_refs)]
pub unsafe fn new_screen() {
    // (logged without the trace too, the first 500 screens: the night tests count them)
    if SCREEN.0 > 0 && (SCREEN.0 <= 500 || crate::trace()) {
        log_screen();
    }
    SCREEN = (SCREEN.0 + 1, Some(Instant::now()), 0, 0, 0);
    CLICKED = None;
    // (a host that sends blocks sends its prefetch before the block: questions wait for it)
    HOLD = HOST_BLOCKS.then(Instant::now);
    let old = Instant::now() - FRESH - Duration::from_millis(1);
    let (cache, wanted, asked) = state();
    for e in cache.values_mut() {
        e.at = old;
    }
    // (what no screen read for KEEP goes; over CACHE_MAX the least recently read go first)
    cache.retain(|_, e| e.used.elapsed() < KEEP);
    if cache.len() > CACHE_MAX {
        let mut used: Vec<Instant> = cache.values().map(|e| e.used).collect();
        used.sort_unstable();
        let cut = used[cache.len() - CACHE_MAX];
        cache.retain(|_, e| e.used >= cut);
    }
    if wanted.len() > PENDING_MAX {
        wanted.clear();
    }
    if asked.len() > PENDING_MAX {
        asked.clear();
    }
}

/// The client's questions on its current screen, for the trace: screen count, its start, keys
/// asked, of those in its first FRESH, answers that came unasked (the host's prefetch).
static mut SCREEN: (usize, Option<Instant>, usize, usize, usize) = (0, None, 0, 0, 0);
/// The host sent a cmui block once: it sends one (and its prefetch first) with every screen.
static mut HOST_BLOCKS: bool = false;
/// A new screen whose block has not come yet: questions wait (HOLD_MAX at most). 300 ms ran out
/// behind every prefetch of ~150 kB or more (night3 D: 11 of 12 screens; the block was in by
/// 0.5 s) and the client asked again for what was on its way, so a second.
static mut HOLD: Option<Instant> = None;
const HOLD_MAX: Duration = Duration::from_millis(1000);

unsafe fn log_screen() {
    let (n, _, asked, first, unasked) = SCREEN;
    crate::log(&format!("client screen {n}: {asked} keys asked ({first} in its first 2 s), {unasked} answers came unasked (prefetch)"));
}

/// The wait for a new screen's block ran out (it never came): true once, the screen is drawn
/// again and asks.
pub unsafe fn hold_over() -> bool {
    if HOLD.is_some_and(|t| t.elapsed() >= HOLD_MAX) {
        HOLD = None;
        if crate::trace() {
            crate::log("client: the wait for the screen's block ran out (1 s, no block came)");
        }
        return true;
    }
    false
}

/// Leaving the network game: nothing kept.
#[allow(static_mut_refs)]
pub unsafe fn reset() {
    CACHE = None;
    WANTED = None;
    ASKED = None;
    CHANNEL = 0;
    #[allow(static_mut_refs)]
    BLOCK.clear();
    SCREEN = (0, None, 0, 0, 0);
    HOST_BLOCKS = false;
    HOLD = None;
    CLICKED = None;
}

/// A network client: the cmui block of its current screen (`NETWORK_PLAN.md`), as the host sent
/// it right after the screen; cleared when a new screen begins.
pub static mut BLOCK: Vec<u8> = Vec::new();

pub unsafe fn send(data: &[u8], slot: i32) {
    let net = *(NETWORK as *const usize);
    if net != 0 {
        crate::game::<extern "thiscall" fn(usize, *const u8, i32, i32) -> i32>(SEND_PACKET)(net, data.as_ptr(), data.len() as i32, slot);
    }
}

/// Packets of entries (u16 count, then the entries as given), each under PACKET_MAX.
fn packets(head: &[u8], entries: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut packet: Option<(Vec<u8>, u16)> = None;
    for e in entries {
        if packet.as_ref().is_some_and(|p| p.0.len() + e.len() > PACKET_MAX) {
            out.push(packet.take().unwrap());
        }
        let p = packet.get_or_insert_with(|| (head.to_vec(), 0));
        p.0.extend(&e);
        p.1 += 1;
    }
    out.extend(packet);
    out.into_iter()
        .map(|(mut p, n)| {
            p.splice(head.len()..head.len(), n.to_le_bytes());
            p
        })
        .collect()
}

/// The client asks for what the last draw missed or found old (and what went unanswered).
pub unsafe fn ask() {
    // ponytail: test switch (a file cmui.noremote in the game folder), read once
    static mut SILENT: Option<bool> = None;
    #[allow(static_mut_refs)]
    if *SILENT.get_or_insert_with(|| std::path::Path::new("cmui.noremote").exists()) {
        return;
    }
    if HOLD.is_some_and(|t| t.elapsed() < HOLD_MAX) {
        return; // (asked once the screen's block, and with it the prefetch, is here)
    }
    let (cache, wanted, asked) = state();
    asked.retain(|key, at| {
        let late = at.elapsed() > UNANSWERED;
        if late {
            wanted.insert(key.clone());
        }
        !late
    });
    // (what came fresh meanwhile, with the screen, is not asked for)
    let fresh = fresh();
    wanted.retain(|key| cache.get(key).is_none_or(|e| e.at.elapsed() > fresh));
    if wanted.is_empty() {
        return;
    }
    SCREEN.2 += wanted.len();
    if SCREEN.1.is_some_and(|t| t.elapsed() <= FRESH) {
        SCREEN.3 += wanted.len();
    }
    let now = Instant::now();
    let entries: Vec<Vec<u8>> = wanted
        .drain()
        .map(|key| {
            asked.insert(key.clone(), now);
            let mut e = (key.len() as u16).to_le_bytes().to_vec();
            e.extend(key);
            e
        })
        .collect();
    for p in packets(&[OPCODE, b'q'], entries) {
        send(&p, 0);
    }
}

/// One question sent now (for an action on the host, ahead of the click that follows it).
pub unsafe fn send_now(key: &[u8]) {
    let mut e = (key.len() as u16).to_le_bytes().to_vec();
    e.extend(key);
    for p in packets(&[OPCODE, b'q'], vec![e]) {
        send(&p, 0);
    }
}

/// A click went to the host and nothing has come from it since. Its game thread is elsewhere (a
/// dialog on the host's own screen held links dead for two minutes, night 5 b0): the click waits
/// in the connection and is taken when the host is back.
static mut CLICKED: Option<Instant> = None;
static mut SILENT_SHOWN: bool = false;
const SILENT: Duration = Duration::from_millis(1500);

/// A click was sent: a question with it, whose answer says the host is there.
pub unsafe fn clicked() {
    send_now(&crate::key(b'p', &[crate::screen().1 as u32]));
    CLICKED = Some(Instant::now());
}

pub unsafe fn host_silent() -> bool {
    // ponytail: test switch, a file cmui.silent in a client's folder at its start: the wait box
    // stays on (the real stall could not be made on demand; waiting for a screen is the game's
    // own "Contacting Server...")
    static mut SHOW_ALWAYS: Option<bool> = None;
    #[allow(static_mut_refs)]
    let always = *SHOW_ALWAYS.get_or_insert_with(|| std::path::Path::new("cmui.silent").exists());
    always || CLICKED.is_some_and(|t| t.elapsed() > SILENT)
}

/// The host fell silent or came back since the last look: the screen is drawn again.
pub unsafe fn silence_changed() -> bool {
    let now = host_silent();
    now != std::mem::replace(&mut *(&raw mut SILENT_SHOWN), now)
}

/// One answer as an 'a' packet carries it: key length u16, key, found u8, value length u32, value.
pub fn answer_entry(key: &[u8], value: Option<Vec<u8>>) -> Vec<u8> {
    let mut e = (key.len() as u16).to_le_bytes().to_vec();
    e.extend(key);
    e.push(value.is_some() as u8);
    let v = value.unwrap_or_default();
    e.extend((v.len() as u32).to_le_bytes());
    e.extend(v);
    e
}

/// Host: answers (`answer_entry`) to the client of `channel` on its seat `slot`, asked or not.
pub unsafe fn send_answers(channel: i16, slot: i32, entries: Vec<Vec<u8>>) {
    for p in packets(&[OPCODE, b'a', channel as u8, (channel >> 8) as u8], entries) {
        send(&p, slot);
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A cmui packet (opcode OPCODE) the game passed to Display::process_command_packet: a question
/// on the host (answered with `answer`, for the channel that sent it), answers on the client.
pub unsafe fn received(data: &[u8], slot: i32, answer: &dyn Fn(i16, &[u8]) -> Option<Vec<u8>>) {
    CLICKED = None;
    match data.get(1) {
        Some(b'q') => {
            // the channel whose seat is this slot: record +0x7d4 (0x300 a channel)
            let Some(channel) = (1..16).find(|&c| *((SCREEN_MANAGER + 0x300 * c + 0x7d4) as *const i16) as i32 == slot) else { return };
            let channel = channel as i16;
            let n = u16_at(data, 2).unwrap_or(0) as usize;
            let mut at = 4;
            let mut entries = Vec::new();
            for _ in 0..n {
                let Some(len) = u16_at(data, at) else { break };
                let Some(key) = data.get(at + 2..at + 2 + len as usize) else { break };
                at += 2 + len as usize;
                entries.push(answer_entry(key, answer(channel, key)));
            }
            send_answers(channel, slot, entries);
        }
        Some(b'b') if CLIENT => {
            BLOCK = data[2..].to_vec();
            ARRIVED = true;
            HOST_BLOCKS = true;
            HOLD = None;
        }
        Some(b'a') if CLIENT => {
            CHANNEL = u16_at(data, 2).unwrap_or(0) as i16;
            let (cache, _, asked) = state();
            let n = u16_at(data, 4).unwrap_or(0) as usize;
            let mut at = 6;
            let now = Instant::now();
            for _ in 0..n {
                let Some(len) = u16_at(data, at) else { break };
                let Some(key) = data.get(at + 2..at + 2 + len as usize) else { break };
                at += 2 + len as usize;
                let (Some(&found), Some(vlen)) = (data.get(at), u32_at(data, at + 1)) else { break };
                let Some(v) = data.get(at + 5..at + 5 + vlen as usize) else { break };
                at += 5 + vlen as usize;
                let value = (found != 0).then(|| v.to_vec());
                if asked.remove(key).is_none() {
                    SCREEN.4 += 1;
                }
                if cache.get(key).is_none_or(|e| e.value != value) {
                    ARRIVED = true;
                }
                cache.insert(key.to_vec(), Entry { value, at: now, used: now });
            }
        }
        _ => {}
    }
}
