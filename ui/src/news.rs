//! The news screen, modern (part of the `news` switch): list rows with an unread dot, a type icon
//! and a crest; short dates; flat tabs; a Mark all read button; the story with a category label,
//! its text laid out again across the panel with the links in colour, a card for a transfer,
//! a record, a media story or a player's mood, and a row of chips for what else it links to; flat
//! scrollbars for every list. Addresses from the community map
//! (`re\ext\cm0102-function-map\map\news.cpp.md`, `news_screens.cpp.md`).

use crate::*;

const NEWS_STORE: usize = 0xb7431c; // News* g_news: +0 the items, +8 the inboxes (12 bytes: first item or -1, -, unread)
const MARK_READ: usize = 0x787140; // News::mark_item_read(this, manager, item_id, sync_others), thiscall
const ACTION_BUTTON: usize = 0x782b70; // NewsItem::get_action_button(this, manager, label, size, handler) -> 2: needs a reply
const FIRST_HUMAN: usize = 0xae2364; // a manager's inbox: his id less this, + 0x10
const ITEM: usize = 0xdf; // a stored news item: +0 type, +0xd6 id, +0xda read
const UNREAD_COLOUR: usize = 0xae3184; // palette: an unread headline's colour (0x7881e0), a read one's
const READ_COLOUR: usize = 0xae3154;
const LINK: [u8; 3] = [0x7c, 0xc0, 0xff];
const TEXT: [u8; 3] = [0xe6, 0xe8, 0xec];
const CHIP: [u8; 3] = [0x2a, 0x2e, 0x35];
const DIM: [u8; 3] = [0xa4, 0xaa, 0xb2];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Transfer,
    /// A finished transfer (its list icon is a briefcase); everywhere else it is a `Transfer`.
    Signed,
    Sacked,
    Injury,
    Ban,
    /// A fine (the FA's for a manager's outburst, a club's for a player): a banknote (user,
    /// 2026-11-07: "like a euro bill").
    Fine,
    Scout,
    Contract,
    Record,
    MoodUp,
    MoodDown,
    Media,
    Award,
    Match,
    Club,
    Other,
}

/// The game's headline for a completed signing, "Sunderland get Vega": "<club> get <Name>", the
/// name one capitalised last word ("PSV get easy draw" is no signing).
fn gets_player(head: &[u8]) -> bool {
    let h = head.trim_ascii();
    h.windows(5).rposition(|w| w == b" get ").is_some_and(|at| {
        let name = &h[at + 5..];
        !name.is_empty() && !name.contains(&b' ') && name[0].is_ascii_uppercase() | (name[0] >= 0xc0) && at > 0
    })
}

/// A transfer that has happened ("X sign Y", "X get Y", "Y moves to X", "Y joins X"), not one
/// still going on (a bid, talks, a rumour, "to sign"). The game gives a client only the headline,
/// so host and client both go by its words.
fn finished_transfer(head: &[u8]) -> bool {
    let h = format!(" {} ", latin1(head).to_lowercase());
    let going = [" bid", "talks", "rumour", "target", "linked", "interest", "offer", "could ", " may ", " to sign", " to join", " to move", " want", " eye", " chase", " plan", "?"];
    if going.iter().any(|w| h.contains(w)) {
        return false;
    }
    gets_player(head) || [" sign ", " signs ", " signed ", " moves to ", " joins ", " completes ", " seal "].iter().any(|w| h.contains(w))
}

#[cfg(test)]
#[test]
fn signing_headlines() {
    assert!(gets_player(b"Sunderland get Vega"));
    assert!(gets_player(b"Newcastle get Soul\xe9"));
    assert!(!gets_player(b"PSV get easy draw"));
    assert!(!gets_player(b"Ajax get Champions League place"));
}

#[cfg(test)]
#[test]
fn kinds_of_headlines() {
    // a finished transfer: the briefcase, in three phrasings
    for h in [&b"Dunmere sign Kolvar"[..], b"Brightwick get Tamsen", b"Orsolo moves to Dunmere", b"Dunmere sign Orsolo from Brightwick"] {
        assert!(kind(h) == Kind::Signed, "{}", String::from_utf8_lossy(h));
    }
    // still going on: the two arrows
    for h in [&b"Dunmere bid for Kolvar"[..], b"Brightwick in transfer talks for Tamsen", b"Dunmere to sign Orsolo", b"Rumour: Dunmere target Kolvar"] {
        assert!(kind(h) == Kind::Transfer, "{}", String::from_utf8_lossy(h));
    }
    // a Discipline Player report is no transfer; a sacking has its own kind
    assert!(kind(b"Kolvar accepts warning") == Kind::Ban);
    assert!(kind(b"Orsolo accepts reprimand") == Kind::Ban);
    assert!(kind(b"Dunmere sack Tamsen") == Kind::Sacked);
    assert!(kind(b"Brightwick manager sacked") == Kind::Sacked);
    // the user's two from 2026-11-06
    assert!(kind(b"Fitz-Jim unhappy with role") == Kind::MoodDown);
    assert!(kind(b"Kolvar happy at Dunmere") == Kind::MoodUp);
    assert!(kind(b"Training facility upgraded") == Kind::Club);
    assert!(kind(b"Orsolo back in training") == Kind::Injury);
    // the user's of 2026-11-07: a fine is money, a complaint discipline
    assert!(kind(b"FA fine Dorking over Manager's outburst") == Kind::Fine);
    assert!(kind(b"Kolvar fined two weeks' wages") == Kind::Fine);
    assert!(kind(b"FA reaction to complaint about referee") == Kind::Ban);
    assert!(kind(b"Finland call up Kolvar") != Kind::Fine);
    assert!(kind(b"Dunmere reach cup final") != Kind::Fine);
    assert!(kind(b"Kolvar in fine form") != Kind::Fine);
    assert_eq!(short_date(b"31.12.28"), b"31 Dec");
    assert_eq!(short_date(b"Mon 1 Jan AM"), b"Mon 1 Jan");
}

/// What a story is about, by its headline's words.
fn kind(head: &[u8]) -> Kind {
    let h = latin1(head).to_lowercase();
    let any = |ws: &[&str]| ws.iter().any(|w| h.contains(w));
    // (whole words: "fine" is no "Finland", "finest" or "final")
    let word = |ws: &[&str]| h.split(|c: char| !c.is_alphanumeric()).any(|x| ws.contains(&x));
    // ("fine" also an adjective: "Kolvar in fine form"; a fine has the FA, "over", wages with it)
    if word(&["fined"]) || (word(&["fine", "fines"]) && word(&["fa", "over", "wages", "for", "club"])) {
        Kind::Fine
    } else if any(&["banned", "suspen", "sent off", "disciplin", "warning", "reprimand", "outburst", "complaint", "referee", "touchline"]) {
        Kind::Ban
    } else if any(&["sacked", "sacks", " sack ", "dismiss", "fired", "relieved of"]) {
        Kind::Sacked
    } else if any(&["facilit"]) {
        Kind::Club // (the board's training facilities: no injury)
    } else if any(&["injur", "back in training", "fitness", "out for", "resumes", "surgery", "knock", "returns to"]) {
        Kind::Injury
    } else if any(&["scout"]) {
        Kind::Scout
    } else if any(&["record"]) {
        Kind::Record
    } else if any(&["disheartened", "unhappy", "angry", "upset", "frustrated", "disappointed", "criticis", "furious", "unsettled"]) {
        Kind::MoodDown // (before the happy words: "unhappy" holds "happy")
    } else if any(&["praised", "delighted", "happy", "pleased", "impressed", "thrilled"]) {
        Kind::MoodUp
    } else if any(&["contract", "renew", "extension", "new deal", "retire"]) {
        Kind::Contract
    } else if finished_transfer(head) {
        Kind::Signed
    } else if any(&["transfer", "bid", "sign", "offer", "loan", "fee", "joins", "accepts", "move", "swap", "release", "free agent", "target"]) || gets_player(head) {
        Kind::Transfer
    } else if any(&["rumour", "linked", "mark on", "media", "speculat", "claims", "press", "tipped", "backs", "warns", "insists", "says"]) {
        Kind::Media
    } else if any(&["award", "player of", "manager of", "team of", "golden", "of the month", "of the year"]) {
        Kind::Award
    } else if any(&[" v ", " win", " draw", "beat", "defeat", "result", "match", "victory", "thrash", "cup", "league", "fixture", "goal", "title", "promot", "relegat"]) {
        Kind::Match
    } else if any(&["board", "chairman", "manager", "job", "sacked", "appointed", "resign", "takeover", "financ", "stadium", "message", "squad"]) {
        Kind::Club
    } else {
        Kind::Other
    }
}

/// The category label and colour of a kind.
fn label(k: Kind) -> (&'static [u8], [u8; 3]) {
    match k {
        Kind::Transfer | Kind::Signed => (b"TRANSFER", [0x4c, 0x9c, 0xe8]),
        Kind::Sacked => (b"CLUB", [0xe0, 0x50, 0x50]),
        Kind::Injury => (b"INJURY", [0xe0, 0x60, 0x58]),
        Kind::Ban => (b"DISCIPLINE", [0xd8, 0x30, 0x30]),
        Kind::Fine => (b"FINE", [0x4c, 0xb0, 0x6a]),
        Kind::Scout => (b"SCOUTING", [0x3e, 0xb0, 0xa8]),
        Kind::Contract => (b"CONTRACT", [0x9a, 0x7c, 0xe0]),
        Kind::Record => (b"RECORD", [0xd0, 0xa0, 0x40]),
        Kind::MoodUp => (b"PLAYER", [0x4c, 0xc0, 0x7a]),
        Kind::MoodDown => (b"PLAYER", [0xe0, 0x60, 0x58]),
        Kind::Media => (b"MEDIA", [0xe8, 0x8a, 0x3a]),
        Kind::Award => (b"AWARD", GOLD),
        Kind::Match => (b"MATCH", [0x4c, 0xc0, 0x7a]),
        Kind::Club => (b"CLUB", [0x8a, 0x98, 0xb0]),
        Kind::Other => (b"NEWS", [0x6a, 0x70, 0x78]),
    }
}

fn darker(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (v as u32 * 30 / 100) as u8)
}

/// An arrow up (green) or down (red), centred on (cx, cy), r its half size.
unsafe fn arrow_mark(cx: i32, cy: i32, r: i32, up: bool) {
    let (c, dir) = if up { ([0x4c, 0xc0, 0x7a], -1) } else { ([0xe0, 0x60, 0x58], 1) };
    let tip = cy + dir * r;
    for i in 0..=r * 7 / 10 {
        let y = tip - dir * i;
        fill(cx - i, y, cx + i + 1, y + 1, c);
    }
    let a = (r / 4).max(1);
    let (y1, y2) = if up { (cy - r / 4, cy + r + 1) } else { (cy - r, cy + r / 4 + 1) };
    fill(cx - a, y1, cx + a + 1, y2, c);
}

/// A small picture of a kind, centred on (cx, cy), r its half size.
unsafe fn icon(k: Kind, cx: i32, cy: i32, r: i32) {
    let c = label(k).1;
    match k {
        Kind::Match => ball(cx, cy, r),
        Kind::Injury => {
            let a = (r / 3).max(1);
            fill(cx - r, cy - a, cx + r + 1, cy + a + 1, c);
            fill(cx - a, cy - r, cx + a + 1, cy + r + 1, c);
        }
        Kind::Ban => fill(cx - r * 7 / 10, cy - r, cx + r * 7 / 10, cy + r + 1, c),
        Kind::Fine => {
            // a banknote: a green note, a dark inner frame, a ring in the middle
            let (w, h) = (r + r / 3, r * 3 / 4);
            fill(cx - w, cy - h, cx + w + 1, cy + h + 1, c);
            let f = (r / 5).max(1);
            fill(cx - w + f, cy - h + f, cx + w + 1 - f, cy - h + 2 * f, darker(c));
            fill(cx - w + f, cy + h + 1 - 2 * f, cx + w + 1 - f, cy + h + 1 - f, darker(c));
            circle(cx, cy, (h * 6 / 10).max(2), 1.5, darker(c));
        }
        Kind::Transfer => {
            // two arrows: right over left
            let h = (r / 2).max(2);
            for (y, dir) in [(cy - r / 2, 1), (cy + r / 2, -1)] {
                fill(cx - r, y - 1, cx + r + 1, y + 1, c);
                let tip = cx + dir * r;
                for i in 0..=h {
                    fill(tip - dir * i, y - i, tip - dir * i + 1, y + i + 1, c);
                }
            }
        }
        Kind::Signed => {
            // a briefcase: handle over a body with a clasp line
            let t = (r / 4).max(1);
            let hw = r / 2;
            fill(cx - hw, cy - r, cx - hw + t, cy - r / 4, c);
            fill(cx + hw - t + 1, cy - r, cx + hw + 1, cy - r / 4, c);
            fill(cx - hw, cy - r, cx + hw + 1, cy - r + t, c);
            fill(cx - r, cy - r / 4, cx + r + 1, cy + r + 1, c);
            fill(cx - r, cy + r / 4, cx + r + 1, cy + r / 4 + 1, AREA);
        }
        Kind::Sacked => {
            // a cross
            for i in -r..=r {
                fill(cx + i, cy + i, cx + i + 2, cy + i + 1, c);
                fill(cx + i, cy - i, cx + i + 2, cy - i + 1, c);
            }
        }
        Kind::Scout => {
            circle(cx - r / 4, cy - r / 4, r * 6 / 10, 2.0, c);
            for i in 0..r / 2 + 1 {
                fill(cx + r / 4 + i, cy + r / 4 + i, cx + r / 4 + i + 2, cy + r / 4 + i + 2, c);
            }
        }
        Kind::Contract => {
            let w = r * 7 / 10;
            fill(cx - w, cy - r, cx + w + 1, cy + r + 1, c);
            for k in 0..3 {
                let y = cy - r / 2 + k * (r / 2);
                fill(cx - w + 2, y, cx + w - 1, y + 1, AREA);
            }
        }
        Kind::Record => {
            // three bars, rising
            let w = (r * 2 / 3).max(2);
            for (k, h) in [r * 2 / 3, r * 4 / 3, 2 * r].into_iter().enumerate() {
                let x = cx - r + k as i32 * (w + 1);
                fill(x, cy + r - h, x + w, cy + r + 1, c);
            }
        }
        Kind::Media => {
            // a speech bubble
            pill(cx - r, cy - r * 7 / 10, cx + r + 1, cy + r / 2, c);
            for i in 0..r / 2 {
                fill(cx - r / 2, cy + r / 2 + i, cx - r / 2 + r / 2 - i, cy + r / 2 + i + 1, c);
            }
        }
        Kind::MoodUp | Kind::MoodDown => arrow_mark(cx, cy, r, k == Kind::MoodUp),
        Kind::Award => {
            fill(cx - r / 2, cy - r, cx - r / 6, cy - r / 4, [0x4c, 0x9c, 0xe8]);
            fill(cx + r / 6, cy - r, cx + r / 2, cy - r / 4, [0xd8, 0x30, 0x30]);
            dot(cx, cy + r / 4, r * 6 / 10, c);
        }
        Kind::Club => dot(cx, cy, r * 6 / 10, c),
        Kind::Other => dot(cx, cy, r * 4 / 10, c),
    }
}

/// "Fri 17th Oct EVE" -> "Fri 17 Oct".
pub fn short_date(t: &[u8]) -> Vec<u8> {
    // (an earlier year's item: "31.12.28" -> "31 Dec")
    let parts: Vec<&[u8]> = t.trim_ascii().split(|&c| c == b'.').collect();
    if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.iter().all(u8::is_ascii_digit)) {
        const MONTHS: [&[u8]; 12] = [b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec"];
        let month = std::str::from_utf8(parts[1]).ok().and_then(|m| m.parse::<usize>().ok()).filter(|m| (1..=12).contains(m));
        if let Some(m) = month {
            return [parts[0], b" ", MONTHS[m - 1]].concat();
        }
    }
    let words: Vec<Vec<u8>> = t.split(|&c| c == b' ').filter(|w| !w.is_empty()).take(3).map(|w| {
        let digits = w.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 { w[..digits].to_vec() } else { w.to_vec() }
    }).collect();
    words.join(&b' ')
}

/// The crest a list row's headline names (its story's links are not known for every row).
static mut ROW_CRESTS: Vec<(Vec<u8>, Option<Vec<u8>>)> = Vec::new();

#[allow(static_mut_refs)]
unsafe fn row_crest(head: &[u8]) -> Option<Vec<u8>> {
    if let Some((_, k)) = ROW_CRESTS.iter().find(|r| r.0 == head) {
        return k.clone();
    }
    let k = crests_named(head).into_iter().next();
    if ROW_CRESTS.len() > 300 {
        ROW_CRESTS.clear();
    }
    ROW_CRESTS.push((head.to_vec(), k.clone()));
    k
}

/// A news list row's headline (on the row of the date cell just drawn, right of it): a gold dot
/// when unread (the game writes it in another palette colour), the kind's icon, the crest, the
/// headline white when unread. True: drawn here.
#[allow(static_mut_refs)]
pub unsafe fn row(x1: i32, y1: i32, _x2: i32, y2: i32, font: u32, colour: u32, t: &[u8]) -> bool {
    let Some(d) = NEWS_DATE.as_ref() else { return false };
    if d.1 != y1 || x1 <= d.2 || t.is_empty() || y2 - y1 > 40 {
        return false;
    }
    // (this process's own palette slots, not the host's: night 8 audit)
    let (unread_c, read_c) = ((UNREAD_COLOUR as *const u16).read_unaligned(), (READ_COLOUR as *const u16).read_unaligned());
    let unread = unread_c != read_c && colour as u16 == unread_c;
    let (h, cy) = (y2 - y1, (y1 + y2) / 2);
    let mut x = x1 + 10;
    if unread {
        dot(x + 3, cy, 4, GOLD);
    }
    x += 14;
    icon(kind(t), x + h / 2, cy, h * 30 / 100);
    x += h + 4;
    if let Some(key) = row_crest(t) {
        draw_crest(&key, x, y1 + 2, h - 4);
    }
    x += h + 4;
    text_at(x, y1, y2, font, if unread { WHITE } else { DIM }, t, 0);
    true
}

/// The Next Unread button and the filter's label as drawn this frame; the tabs and buttons of
/// the screen (box, active, label, font); the Mark all read button (for poll_input) and a click
/// on it for the news input of a channel.
static mut NEXT_UNREAD: Option<((i32, i32, i32, i32), u32)> = None;
static mut FILTER_LEFT: i32 = 0;
static mut NEXT_ACTIVE: bool = true;
static mut TABS: Vec<((i32, i32, i32, i32), bool, Vec<u8>, u32)> = Vec::new();
pub static mut MARK_BUTTON: Option<(i32, i32, i32, i32)> = None;
pub static mut PENDING_MARK: Option<i16> = None;

/// The screen's texts cmui places things by.
#[allow(static_mut_refs)]
pub unsafe fn note_text(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, t: &[u8]) {
    if t == b"Next Unread" {
        NEXT_UNREAD = Some(((x1, y1, x2, y2), font));
        NEXT_ACTIVE = flags & 0x20 == 0;
    } else if t == b"Filter :" {
        FILTER_LEFT = x1;
    }
    // (matched to the tabs in the flip pass: the active tab's label comes before its button)
    if !t.is_empty() && TEXTS.len() < 400 {
        TEXTS.push(((x1 + x2) / 2, (y1 + y2) / 2, t.to_vec(), font));
    }
}

/// The texts drawn this frame: middle, text, font.
static mut TEXTS: Vec<(i32, i32, Vec<u8>, u32)> = Vec::new();

/// Whether a channel's manager has an unread item that needs no reply (what Mark all read marks).
unsafe fn markable(channel: usize) -> bool {
    let Some((news, manager, first)) = inbox(channel) else { return false };
    let items = rd::<usize>(news);
    let ask = game::<extern "thiscall" fn(usize, usize, *mut u8, i32, *mut usize) -> i32>(ACTION_BUTTON);
    let (mut text, mut handler) = ([0u8; 0x80], 0usize);
    (0..100).any(|i| {
        let item = items + (first + i) * ITEM;
        rd::<u32>(item + 0xda) == 0 && rd::<i32>(item + 0xd6) > 0 && ask(item, manager, text.as_mut_ptr(), 0x7d, &mut handler) & 0xff != 2
    })
}

/// In the flip pass, on the news screen (its Next Unread drawn this frame): the tabs flat, and
/// Mark all read, as wide as Next Unread, left of the filter box (grey with nothing to mark).
#[allow(static_mut_refs)]
pub unsafe fn mark_button(full_frame: bool) {
    let mut tabs = std::mem::take(&mut TABS);
    let texts = std::mem::take(&mut TEXTS);
    for tab in &mut tabs {
        if let Some(t) = texts.iter().rev().find(|t| (tab.0 .0..=tab.0 .2).contains(&t.0) && (tab.0 .1..=tab.0 .3).contains(&t.1)) {
            // (the size the game's own tabs come out at: `bigger_text`)
            let label = std::ffi::CString::new(t.2.clone()).unwrap_or_default();
            (tab.2, tab.3) = (t.2.clone(), roomier_font(tab.0 .0, tab.0 .1, tab.0 .2, tab.0 .3, 0xc, t.3, label.as_ptr(), -1));
        }
    }
    let Some(((x1, y1, x2, y2), font)) = NEXT_UNREAD.take() else {
        if full_frame {
            MARK_BUTTON = None;
        }
        return;
    };
    flat_tabs(&tabs);
    // Next Unread flat like Mark all read (dim when the game disables it: flags bit 0x20)
    fill(x1, y1, x2, y2, if NEXT_ACTIVE { CHIP } else { [0x22, 0x25, 0x2a] });
    for (a, b, c, d) in [(x1, y1, x2, y1 + 1), (x1, y2 - 1, x2, y2), (x1, y1, x1 + 1, y2), (x2 - 1, y1, x2, y2)] {
        fill(a, b, c, d, if NEXT_ACTIVE { [0x8a, 0x70, 0x34] } else { [0x34, 0x38, 0x40] });
    }
    text_at((x1 + x2) / 2, y1, y2, font, if NEXT_ACTIVE { LIGHT } else { [0x6a, 0x70, 0x78] }, b"Next Unread", 1);
    let right = if FILTER_LEFT > 0 { FILTER_LEFT - 12 } else { x1 - 8 };
    let r = (right - (x2 - x1), y1, right, y2);
    let active = remote::CLIENT || markable(channel());
    fill(r.0, r.1, r.2, r.3, if active { CHIP } else { [0x22, 0x25, 0x2a] });
    let edge = if active { [0x8a, 0x70, 0x34] } else { [0x34, 0x38, 0x40] };
    for (a, b, c, d) in [(r.0, r.1, r.2, r.1 + 1), (r.0, r.3 - 1, r.2, r.3), (r.0, r.1, r.0 + 1, r.3), (r.2 - 1, r.1, r.2, r.3)] {
        fill(a, b, c, d, edge);
    }
    // a tick before the words
    let w = text_width(font, b"Mark all read");
    let (cx, cy) = ((r.0 + r.2) / 2 - w / 2 - 12, (y1 + y2) / 2);
    let tick = if active { GOLD } else { [0x5a, 0x60, 0x68] };
    for i in 0..4 {
        fill(cx + i, cy + i - 1, cx + i + 2, cy + i + 1, tick);
    }
    for i in 0..7 {
        fill(cx + 3 + i, cy + 2 - i, cx + 5 + i, cy + 4 - i, tick);
    }
    text_at((r.0 + r.2) / 2 + 8, y1, y2, font, if active { LIGHT } else { [0x6a, 0x70, 0x78] }, b"Mark all read", 1);
    MARK_BUTTON = active.then_some(r);
}

/// A row of three or more tabs or buttons (the categories above and below the news) as one flat
/// bar: grey labels with thin dividers, the active one gold over a gold line.
unsafe fn flat_tabs(tabs: &[((i32, i32, i32, i32), bool, Vec<u8>, u32)]) {
    let mut rows: Vec<i32> = tabs.iter().map(|t| t.0 .1).collect();
    rows.sort();
    rows.dedup();
    for y in rows {
        let row: Vec<_> = tabs.iter().filter(|t| t.0 .1 == y).collect();
        if row.iter().filter(|t| !t.2.is_empty()).count() < 3 {
            continue;
        }
        let y2 = row[0].0 .3;
        for (k, t) in row.iter().enumerate() {
            let (a, b) = (t.0 .0, t.0 .2);
            // (a tab without a label of its own is a disabled one: left as the game greys it)
            if t.2.is_empty() {
                continue;
            }
            fill(a, y, b + 2, y2 + 1, [0x1e, 0x21, 0x26]);
            fill(a, y2, b + 2, y2 + 1, [0x34, 0x38, 0x40]);
            if t.1 {
                fill(a, y, b + 1, y2 + 1, [0x26, 0x2a, 0x31]);
                fill(a, y2 - 2, b + 1, y2 + 1, GOLD);
            }
            if k > 0 {
                let h = y2 - y;
                fill(a - 1, y + h / 4, a, y2 - h / 4, [0x34, 0x38, 0x40]);
            }
            text_at((a + b) / 2, y, y2, t.3, if t.1 { GOLD } else { DIM }, &t.2, 1);
        }
    }
}

/// A channel's inbox: the news store, its manager, his first item.
unsafe fn inbox(channel: usize) -> Option<(usize, usize, usize)> {
    let news = rd::<usize>(NEWS_STORE);
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel);
    if !mapped(news, 12) || !mapped(manager, 4) {
        return None;
    }
    let k = rd::<i32>(manager).wrapping_sub(rd::<i32>(FIRST_HUMAN)).wrapping_add(0x10);
    if !(0..16).contains(&k) {
        return None;
    }
    let first = rd::<i32>(rd::<usize>(news + 8) + 12 * k as usize);
    (first >= 0 && mapped(rd::<usize>(news) + first as usize * ITEM, 100 * ITEM)).then_some((news, manager, first as usize))
}

/// Marks every item of a channel's manager read that needs no reply, as the game does when an
/// item is opened (News::mark_item_read leaves those that do), and gives the news screen's own
/// copies of its list (screen parameters 2 and 3) the new flags.
pub unsafe fn mark_all_read(channel: usize) {
    let Some((news, manager, first)) = inbox(channel) else { return };
    let items = rd::<usize>(news);
    let item = |i: usize| items + (first + i) * ITEM;
    let mark = game::<extern "thiscall" fn(usize, usize, i32, i32) -> i32>(MARK_READ);
    for i in 0..100 {
        // (empty slots have no id)
        if rd::<u32>(item(i) + 0xda) == 0 && rd::<i32>(item(i) + 0xd6) > 0 {
            mark(news, manager, rd::<i32>(item(i) + 0xd6), 1);
        }
    }
    let (list, n) = (screen_param(2), screen_param(3).min(100));
    for c in 0..n {
        let copy = list + c * ITEM;
        if !mapped(copy, ITEM) {
            break;
        }
        if let Some(i) = (0..100).find(|&i| rd::<i32>(item(i) + 0xd6) == rd::<i32>(copy + 0xd6)) {
            *((copy + 0xda) as *mut u32) = rd::<u32>(item(i) + 0xda);
        }
    }
}

/// The story's lines as the game draws them (box, font, text), not drawn by it: laid out again in
/// the flip pass; the tops of its blank lines (paragraph breaks).
static mut LINES: Vec<((i32, i32, i32, i32), u32, Vec<u8>)> = Vec::new();
static mut BLANKS: Vec<i32> = Vec::new();

/// A text of the story's body from the text hook: kept, never drawn by the game.
#[allow(static_mut_refs)]
pub unsafe fn story_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, s: &[u8], panel_width: i32) {
    if s.trim_ascii().is_empty() {
        // a link's box over its words (the links come from the widgets) or a blank line
        if x2 - x1 > panel_width * 3 / 4 {
            BLANKS.push(y1);
        }
    } else if LINES.len() < 80 {
        LINES.push(((x1, y1, x2, y2), font, s.to_vec()));
    }
}

/// A link of the story (a display widget with a box): kind, box where the game has it, the
/// object, its name and crest key (a club's, a person's nation's flag); then its words in the
/// text (the first and last word, as the story writes it).
#[derive(Clone)]
struct Link {
    kind: i16,
    rect: (i32, i32, i32, i32),
    object: usize,
    name: Vec<u8>,
    key: Vec<u8>,
    words: Option<(usize, usize)>,
    text: Vec<u8>,
    /// A network client's link from the host's block: its item's cell on the host's display for
    /// this client, which a click asks the host to open ('X'); -1 otherwise.
    cell: i16,
}

impl Link {
    fn club(&self) -> bool {
        self.kind == 0x7d5 || self.kind == 0x7d6
    }
    fn person(&self) -> bool {
        (0x7d7..=0x7e0).contains(&self.kind) || self.kind == 0x7e7
    }
    /// The name as the story writes it, else the database's.
    fn shown(&self) -> &[u8] {
        if self.text.is_empty() { &self.name } else { &self.text }
    }
    fn target(&self) -> (i32, i32) {
        ((self.rect.0 + self.rect.2) / 2, (self.rect.1 + self.rect.3) / 2)
    }
}

/// The links of the story on a channel's display (where the display is: single player, the host).
pub unsafe fn link_list(channel: usize) -> Vec<u8> {
    let display = rd::<usize>(CHANNEL_DISPLAY + 0x300 * channel);
    let mut out = Vec::new();
    if !mapped(display + 0x12e9a0, 2) {
        return out;
    }
    let count = (*((display + 0x12e9a0) as *const i16)).clamp(0, 1200) as usize;
    if !mapped(display + 0xba95e, count * 0x18c) {
        return out;
    }
    for k in 0..count {
        let w = display + 0xba95e + k * 0x18c;
        let kind = *((w + 8) as *const i16);
        let r: Vec<i32> = (0..4).map(|i| *((w + 0x10 + 4 * i) as *const i32)).collect();
        let object = *((w + 0x48) as *const usize);
        if r[2] <= r[0] || r[3] <= r[1] {
            continue;
        }
        let Some((name, key)) = link_names(kind, object) else {
            if trace() && (2000..2100).contains(&kind) {
                log(&format!("story link of kind {kind:#x}, object {object:#x}: not shown"));
            }
            continue;
        };
        out.extend((kind as u16).to_le_bytes());
        for v in &r {
            out.extend(v.to_le_bytes());
        }
        out.extend((object as u32).to_le_bytes());
        out.extend(name_bytes(&name, &key));
    }
    out
}

/// A link's name and crest key (where its object is: single player, the host).
pub unsafe fn link_names(kind: i16, object: usize) -> Option<(Vec<u8>, Vec<u8>)> {
    Some(match kind {
        0x7d0 if mapped(object, 0x55) => (game_text(object + 4, 51), game_text(object + 4, 51)),
        // (an award: its staff competition, `menubar_handle_object_link` 0x7643d0 opens its screen)
        0x7d4 if mapped(object, 0x37) => (game_text(object + 4, 51), Vec::new()),
        0x7d5 | 0x7d6 if mapped(object, 0x5b) => {
            let (full, short) = (game_text(object + 4, 51), game_text(object + 0x38, 26));
            let key = if has_crest(&full) { full.clone() } else { short };
            (full, key)
        }
        0x7d7..=0x7e0 | 0x7e7 if mapped(object, 0x6e) => {
            let nation = rd::<usize>(object + 0x1a);
            let flag = if mapped(nation, 55) { [b"flag:".as_slice(), &game_text(nation + 4, 51)].concat() } else { Vec::new() };
            (staff_name(object), flag)
        }
        _ => return None,
    })
}

/// Host: the cmui block for a client's screen just built (`NETWORK_PLAN.md`): its link items,
/// each kind (u16), object (u32), and text, name, crest key as a length byte and the bytes. Read
/// while the display still has them; their boxes are empty (the host lays a client's text out
/// nowhere), so the client places them by their text.
pub unsafe fn screen_block(display: usize) -> Vec<u8> {
    // (the entry count first: the game's packet buffer runs on past the packet)
    let mut out = vec![0u8, 0];
    let count = if mapped(display + 0x12e9a0, 2) { rd::<i16>(display + 0x12e9a0).clamp(0, 1200) as usize } else { 0 };
    let mut n = 0;
    for k in 0..count {
        let w = display + 0xba95e + k * 0x18c;
        if !mapped(w, 0x18c) {
            break;
        }
        let (kind, object) = (rd::<i16>(w + 8), rd::<usize>(w + 0x48));
        let text = std::slice::from_raw_parts((w + 0x80) as *const u8, 64).split(|&c| c == 0).next().unwrap().trim_ascii().to_vec();
        // (a story's own links have no text of their own, their words are in the story's: sent
        // by the name of what they link, placed on the client by it)
        // (items with a text of their own are the menu bar's, on every screen: not sent, they
        // name competitions the story doesn't mean, the OFC "Champions League")
        if !text.is_empty() {
            continue;
        }
        let Some((name, key)) = link_names(kind, object) else { continue };
        let text = name.clone();
        if text.is_empty() {
            continue;
        }
        out.extend((kind as u16).to_le_bytes());
        out.extend((object as u32).to_le_bytes());
        out.extend((k as u16).to_le_bytes());
        out.extend(name_bytes(&text[..text.len().min(255)], b""));
        out.truncate(out.len() - 1); // (name_bytes wrote two: the text's, and an empty one)
        out.extend(name_bytes(&name, &key));
        n += 1;
        if out.len() > 36_000 {
            break;
        }
    }
    out[..2].copy_from_slice(&(n as u16).to_le_bytes());
    if trace() {
        log(&format!("cmui block: {n} links"));
    }
    out
}

/// Client: the block's links (kind, object, cell, text, name, key).
unsafe fn block_links() -> Vec<(i16, usize, i16, Vec<u8>, Vec<u8>, Vec<u8>)> {
    #[allow(static_mut_refs)]
    let b = &remote::BLOCK;
    let mut out = Vec::new();
    let count = if b.len() >= 2 { u16::from_le_bytes([b[0], b[1]]) as usize } else { 0 };
    let mut at = 2;
    let text = |at: &mut usize| -> Vec<u8> {
        let n = *b.get(*at).unwrap_or(&0) as usize;
        let t = b.get(*at + 1..*at + 1 + n).unwrap_or(&[]).to_vec();
        *at += 1 + n;
        t
    };
    while out.len() < count && at + 8 <= b.len() {
        let kind = i16::from_le_bytes([b[at], b[at + 1]]);
        let object = u32::from_le_bytes(b[at + 2..at + 6].try_into().unwrap()) as usize;
        let cell = i16::from_le_bytes([b[at + 6], b[at + 7]]);
        at += 8;
        let (t, name, key) = (text(&mut at), text(&mut at), text(&mut at));
        out.push((kind, object, cell, t, name, key));
    }
    BLOCK_TAIL = at;
    out
}

/// Where the block's links end (the next section starts), after `block_links`.
static mut BLOCK_TAIL: usize = 0;

/// Client: the block after its links (the history section, `comp::history_block`).
#[allow(static_mut_refs)]
pub unsafe fn block_tail() -> Vec<u8> {
    block_links();
    remote::BLOCK.get(BLOCK_TAIL..).unwrap_or(&[]).to_vec()
}

/// A word as a link's text would have it: without the punctuation after it and "'s".
fn bare(w: &[u8]) -> Vec<u8> {
    let t = w.strip_suffix(b".").or(w.strip_suffix(b",")).or(w.strip_suffix(b":")).or(w.strip_suffix(b";")).unwrap_or(w);
    t.strip_suffix(b"'s").or(t.strip_suffix(b"'")).unwrap_or(t).to_vec()
}

/// Client: the story's links from the host's block, each placed on the first run of the story's
/// words that reads as its text (and is no other link's yet). False when the block has none.
unsafe fn block_words(words: &mut [Word], links: &mut Vec<Link>) -> bool {
    let block = block_links();
    let mut placed = 0;
    // (the screen's own items last in its list: the story's links before the menu bar's, which
    // name competitions too, "Champions League" several times)
    for (kind, object, cell, text, name, key) in block.iter().rev() {
        // the story may name it shorter: a club by its short name (its crest key), a player by
        // his surname
        // ("UEFA Champions League" written "Champions League", "NEC Nijmegen" "NEC", a player
        // by his surname): the name, its crest key, then shorter from either end, longest first
        let mut tries: Vec<Vec<u8>> = vec![text.clone()];
        if (0x7d5..=0x7d6).contains(kind) && !key.is_empty() && key != text {
            tries.push(key.clone());
        }
        let parts: Vec<&[u8]> = text.split(|&c| c == b' ').filter(|p| !p.is_empty()).collect();
        for n in (1..parts.len()).rev() {
            for t in [parts[parts.len() - n..].join(&b' '), parts[..n].join(&b' ')] {
                if t.len() >= 3 && !tries.contains(&t) {
                    tries.push(t);
                }
            }
        }
        let found = tries.iter().find_map(|t| {
            let parts: Vec<&[u8]> = t.split(|&c| c == b' ').filter(|p| !p.is_empty()).collect();
            let n = parts.len();
            if n == 0 || n > words.len() {
                return None;
            }
            (0..=words.len() - n)
                .find(|&i| words[i..i + n].iter().all(|w| w.link.is_none()) && words[i..i + n].iter().zip(&parts).all(|(w, p)| bare(&w.text) == *p || w.text == *p))
                .map(|i| (i, n, t.clone()))
        });
        let Some((i, n, text)) = found else { continue };
        for w in &mut words[i..i + n] {
            w.link = Some(links.len());
        }
        links.push(Link { kind: *kind, rect: (0, 0, 0, 0), object: *object, name: name.clone(), key: key.clone(), words: Some((i, i + n - 1)), text: text.clone(), cell: *cell });
        placed += 1;
    }
    if trace() {
        log(&format!("cmui block on the client: {} links, {placed} placed: {:?}", block.len(), block.iter().map(|b| latin1(&b.3).into_owned()).collect::<Vec<_>>()));
    }
    // (none placed: the game links nothing in some stories, "NEC scouting completed"; the
    // words are looked up then)
    placed > 0
}

/// Name and key, each a length byte and the bytes (as link_list writes them).
pub fn name_bytes(name: &[u8], key: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for t in [name, key] {
        out.push(t.len().min(255) as u8);
        out.extend(&t[..t.len().min(255)]);
    }
    out
}

fn parse_links(b: &[u8]) -> Vec<Link> {
    let mut out = Vec::new();
    let mut at = 0;
    let int = |b: &[u8], at: usize| i32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    while at + 22 <= b.len() {
        let kind = i16::from_le_bytes([b[at], b[at + 1]]);
        let rect = (int(b, at + 2), int(b, at + 6), int(b, at + 10), int(b, at + 14));
        let object = int(b, at + 18) as u32 as usize;
        at += 22;
        let mut texts = Vec::new();
        for _ in 0..2 {
            let n = *b.get(at).unwrap_or(&0) as usize;
            texts.push(b.get(at + 1..at + 1 + n).unwrap_or(&[]).to_vec());
            at += 1 + n;
        }
        out.push(Link { kind, rect, object, name: texts[0].clone(), key: texts[1].clone(), words: None, text: Vec::new(), cell: -1 });
    }
    out
}

/// The story's links, a network client's from the host (asked again until there; kept per story).
static mut LINKS: (Vec<u8>, Vec<Link>) = (Vec::new(), Vec::new());

#[allow(static_mut_refs)]
unsafe fn story_links(head: &[u8]) -> Vec<Link> {
    // (a client: never kept here. The key is the same for every story, so the first answer seen
    // for a story can be the host's for the screen before; remote asks again and redraws when
    // the fresh one differs.)
    // ponytail: still empty on a client: the host keeps no laid-out boxes for a client's screen
    // and the client's own display has no link items (bug round 2026-10-29, 6); links from the
    // story's words looked up on the host would be the way
    if remote::CLIENT {
        return parse_links(&ask_host(b"L").unwrap_or_default());
    }
    if LINKS.0 == head && !LINKS.1.is_empty() {
        return LINKS.1.clone();
    }
    let raw = link_list(channel());
    LINKS = (head.to_vec(), parse_links(&raw));
    LINKS.1.clone()
}

/// Where the pointer goes for the game: over a link of cmui's layout (or a chip, a card's part)
/// the middle of the game's own box for it; elsewhere over the story a spot with nothing (the
/// game's boxes are where it laid the text out, not where it is now).
static mut REMAP: Vec<((i32, i32, i32, i32), (i32, i32))> = Vec::new();
static mut HIDE: Option<((i32, i32, i32, i32), (i32, i32))> = None;

/// A dropdown menu is open (its rows drawn since the last full frame without them): the pointer is
/// the menu's then, wherever it opened (the Competitions menu reaches over the story).
pub static mut MENU_OPEN: bool = false;
pub static mut MENU_SEEN: bool = false;

/// A menu counted open whose rows are no longer on the screen is closed (the flag stayed on when
/// the menu's rows were drawn into the first full frame of the screen its entry opened).
pub unsafe fn menu_check() {
    if MENU_OPEN && !crate::menu_still_shown() {
        MENU_OPEN = false;
        // (the game closes a menu bar dropdown without drawing what was under it: its dark frame
        // stayed over the screen, night 6 C-1. The screen drawn again; at most every 2 s, should
        // the rows' pixel test miss an open menu)
        redraw_after_menu();
    }
}

/// The screen drawn again a moment after a menu closed (the game puts back what was under the
/// menu after the next screen's first frame too: a menu entry left the old box over the new
/// screen, night 6 C-1 / C-3). At most every 2 s, should the rows' pixel test miss an open menu.
unsafe fn redraw_after_menu() {
    static mut LAST: Option<std::time::Instant> = None;
    if LAST.is_none_or(|t| t.elapsed().as_secs() >= 2) {
        let now = std::time::Instant::now();
        LAST = Some(now);
        crate::REDRAW_AT = Some(now + std::time::Duration::from_millis(250));
    }
}

/// The end of a frame: a full frame without menu rows closes the menus.
pub unsafe fn frame_end(full_frame: bool) {
    if full_frame {
        let was = MENU_OPEN;
        MENU_OPEN = std::mem::take(&mut MENU_SEEN);
        if was && !MENU_OPEN {
            redraw_after_menu();
        }
    }
}

#[allow(static_mut_refs)]
pub unsafe fn pointer(mx: i32, my: i32) -> Option<(i32, i32)> {
    menu_check();
    if trace() && crate::BUTTON_DOWN_NOW() {
        log(&format!("news pointer {mx},{my}: menu open {MENU_OPEN} seen {MENU_SEEN}, {} remaps", REMAP.len()));
    }
    if MENU_OPEN {
        return None;
    }
    let over = |r: &(i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    let to = REMAP.iter().find(|r| over(&r.0)).map(|r| r.1).or_else(|| HIDE.filter(|h| over(&h.0)).map(|h| h.1));
    if trace() && crate::BUTTON_DOWN_NOW() {
        log(&format!("news pointer {mx},{my} -> {to:?} (remaps {:?})", REMAP.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>()));
    }
    to
}

/// A network client: where cmui drew a link of the host's block (its cell there), or a club found
/// by the story's words (the club, opened by 'K' as the crest's chip is).
pub static mut CLICKS: Vec<((i32, i32, i32, i32), Click)> = Vec::new();

#[derive(Clone, Copy)]
pub enum Click {
    Cell(i16),
    Club(usize),
}

/// What a click on a drawn link does: the pointer onto the game's own box for it, or on a client
/// a request for the host to open it.
#[allow(static_mut_refs)]
unsafe fn click_at(l: &Link, r: (i32, i32, i32, i32)) {
    if l.rect.2 > l.rect.0 {
        REMAP.push((r, l.target()));
    } else if l.cell >= 0 {
        CLICKS.push((r, Click::Cell(l.cell)));
    } else if l.club() && l.object != 0 {
        CLICKS.push((r, Click::Club(l.object)));
    }
}

/// A network client's click (poll_input, the button let go at mx, my): the link drawn there, for
/// the host to open.
#[allow(static_mut_refs)]
pub unsafe fn clicked(mx: i32, my: i32) -> Option<Click> {
    CLICKS.iter().find(|c| (c.0 .0..=c.0 .2).contains(&mx) && (c.0 .1..=c.0 .3).contains(&my)).map(|c| c.1)
}

/// Host: a client's link to open, by its cell (channel, cell), done by the news screen's input.
pub static mut PENDING_CELL: Option<(i16, i16)> = None;

/// A screen without a story.
#[allow(static_mut_refs)]
pub unsafe fn clear() {
    REMAP.clear();
    CLICKS.clear();
    HIDE = None;
    LINES.clear();
    BLANKS.clear();
}

/// The category label after the story's date: the kind's colour on a dark pill.
pub unsafe fn category(x: i32, y1: i32, y2: i32, font: u32, head: &[u8]) {
    let (text, c) = label(kind(head));
    let font = font & !0xffff | 1;
    let w = text_width(font, text) + 16;
    pill(x, y1 + 2, x + w, y2 - 2, darker(c));
    text_at(x + w / 2, y1, y2, font, c, text, 1);
}

/// A story's word: its text, where the game draws it (left, right), its line, its link, whether a
/// paragraph starts with it.
struct Word {
    text: Vec<u8>,
    a: i32,
    b: i32,
    line: usize,
    link: Option<usize>,
    para: bool,
}

/// The words each link covers. The game's boxes are its own measure of the text, a few percent
/// wider than it draws (5 px off at a line's start, 16 at its end): one factor for the story,
/// the one its boxes fit best, then each box's first and last word by it.
fn link_words(words: &mut [Word], links: &mut [Link], lines: &[((i32, i32, i32, i32), u32, Vec<u8>)]) {
    let boxes: Vec<(usize, usize)> = links.iter().enumerate().filter_map(|(l, link)| lines.iter().position(|ln| ln.0 .1 == link.rect.1).map(|li| (l, li))).collect();
    let on_line = |li: usize| words.iter().enumerate().filter(move |w| w.1.line == li).map(|(i, _)| i);
    let fit = |k: f64, l: usize, li: usize| -> Option<(f64, usize, usize)> {
        let lx = lines[li].0 .0 as f64;
        let at = |x: i32| lx + k * (x as f64 - lx);
        let r = links[l].rect;
        let i = on_line(li).min_by(|&p, &q| (at(words[p].a) - r.0 as f64).abs().total_cmp(&(at(words[q].a) - r.0 as f64).abs()))?;
        let j = on_line(li).filter(|&q| q >= i).min_by(|&p, &q| (at(words[p].b) - r.2 as f64).abs().total_cmp(&(at(words[q].b) - r.2 as f64).abs()))?;
        Some(((at(words[i].a) - r.0 as f64).abs() + (at(words[j].b) - r.2 as f64).abs(), i, j))
    };
    // (0.84 to 1.08: run C of night 4, the original folder, has boxes narrower than cmui's
    // measure, which the old 1.0 to 1.08 could not follow: links one word early)
    let k = (0..=120).map(|s| 0.84 + s as f64 * 0.002).min_by(|&p, &q| {
        let cost = |k: f64| boxes.iter().filter_map(|&(l, li)| fit(k, l, li)).map(|f| f.0).sum::<f64>();
        cost(p).total_cmp(&cost(q))
    }).unwrap_or(1.0);
    let picks: Vec<(usize, usize, usize)> = boxes.iter().filter_map(|&(l, li)| fit(k, l, li).map(|f| (l, f.1, f.2))).collect();
    for (l, i, mut j) in picks {
        // (the fit can end a link a word early at a long line's end: "Newcastle's Josh Thomson."
        // gave the player "Josh", the card "Josh praised", night 6 D-8 / E-2. The next words
        // still in the link's own name go with it)
        let lower = |t: &[u8]| {
            let t = t.strip_suffix(b".").or(t.strip_suffix(b",")).or(t.strip_suffix(b":")).or(t.strip_suffix(b";")).unwrap_or(t);
            t.strip_suffix(b"'s").unwrap_or(t).to_ascii_lowercase()
        };
        let name: Vec<Vec<u8>> = links[l].name.split(|&c| c == b' ' || c == b',').filter(|p| !p.is_empty()).map(|p| p.to_ascii_lowercase()).collect();
        while j + 1 < words.len() && words[j + 1].link.is_none() && !words[j + 1].para && name.contains(&lower(&words[j + 1].text))
            && !words[i..=j].iter().any(|w| lower(&w.text) == lower(&words[j + 1].text))
        {
            j += 1;
        }
        for w in &mut words[i..=j] {
            w.link = Some(l);
        }
        let text = words[i..=j].iter().map(|w| w.text.clone()).collect::<Vec<_>>().join(&b' ');
        let mut t = text.as_slice();
        t = t.strip_suffix(b".").or(t.strip_suffix(b",")).or(t.strip_suffix(b":")).or(t.strip_suffix(b";")).unwrap_or(t);
        t = t.strip_suffix(b"'s").unwrap_or(t);
        links[l].words = Some((i, j));
        links[l].text = t.to_vec();
    }
}

/// A network client's links, which the game gives it none of (the host lays a client's text out
/// nowhere and the client's display has no link items): runs of capitalised words of the story
/// that name a club or a person, looked up on the host (a club by its name, 'C'; a person by
/// "Second, First", 'G'), longest first, then each one's name and crest key ('Z').
unsafe fn word_links(words: &mut [Word], links: &mut Vec<Link>) {
    let bare = |w: &[u8]| -> Vec<u8> {
        let t = w.strip_suffix(b".").or(w.strip_suffix(b",")).or(w.strip_suffix(b":")).or(w.strip_suffix(b";")).unwrap_or(w);
        t.strip_suffix(b"'s").or(t.strip_suffix(b"'")).unwrap_or(t).to_vec()
    };
    let capital = |w: &[u8]| w.first().is_some_and(|c| c.is_ascii_uppercase() || *c >= 0xc0);
    // (inside a name: "Jaden de Guzmán", "Rick van Bommel")
    let particle = |w: &[u8]| [&b"de"[..], b"van", b"der", b"den", b"da", b"dos", b"di", b"du", b"le", b"la", b"el", b"von", b"ten", b"ter"].contains(&w);
    let mut i = 0;
    while i < words.len() {
        let mut found = None;
        for n in (1..=5).rev() {
            let j = i + n - 1;
            // (a run ends where a word ends a phrase: "PSV's", "Twente,")
            if j >= words.len() || !words[i..=j].iter().all(|w| capital(&w.text) || particle(&w.text)) || !capital(&words[j].text) || words[i..j].iter().any(|w| bare(&w.text) != w.text) {
                continue;
            }
            let parts: Vec<Vec<u8>> = words[i..=j].iter().map(|w| bare(&w.text)).collect();
            let t = parts.join(&b' ');
            if t.len() < 3 {
                continue;
            }
            // a person by "Second, First" at each split ("de Guzmán, Jaden"), else by his common name
            let person = || {
                (1..parts.len()).find_map(|k| staff_by_row_name(&[parts[k..].join(&b' ').as_slice(), b", ", &parts[..k].join(&b' ')].concat()))
                    .or_else(|| staff_by_row_name(&t))
            };
            let club = if capital(&words[i].text) { club_named(&t) } else { None };
            let hit = club.map(|c| (0x7d5i16, c)).or_else(|| person().map(|s| (0x7d7, s)));
            if let Some((kind, object)) = hit {
                found = Some((j, kind, object, t));
                break;
            }
        }
        let Some((j, kind, object, t)) = found else {
            i += 1;
            continue;
        };
        let (name, key) = ask_host(&key(b'Z', &[kind as u16 as u32, object as u32])).map(|n| {
            let len = *n.first().unwrap_or(&0) as usize;
            let klen = *n.get(1 + len).unwrap_or(&0) as usize;
            (n.get(1..1 + len).unwrap_or(&[]).to_vec(), n.get(2 + len..2 + len + klen).unwrap_or(&[]).to_vec())
        }).unwrap_or_else(|| (t.clone(), if has_crest(&t) { t.clone() } else { Vec::new() }));
        for w in &mut words[i..=j] {
            w.link = Some(links.len());
        }
        links.push(Link { kind, rect: (0, 0, 0, 0), object, name, key, words: Some((i, j)), text: t, cell: -1 });
        i = j + 1;
    }
}

/// The place a qualifier finished: the game's own last-season position (club +0x5f, the table is
/// reset by then: everyone read "1st", user 2026-11-09) while the new season has no match played,
/// else the live table. ponytail: a story read after the next season's first match shows that
/// table, as before; unverified against the game that +0x5f is already set when the story posts.
unsafe fn finishing_place(club: usize) -> Option<i32> {
    let fresh = challenge::league_row(club).is_some_and(|(row, _)| row[0] == 0);
    if fresh && mapped(club, 0x60) {
        let last = rd::<u8>(club + 0x5f) as i8 as i32;
        if last > 0 {
            return Some(last);
        }
    }
    challenge::league_place(club).map(|p| p.0)
}

/// "2029/30 European qualifiers decided" (and other continents' "... qualifiers decided").
pub fn qualifiers(head: &[u8]) -> bool {
    head.ends_with(b"qualifiers decided")
}

/// A qualifiers story: its first paragraph as text, then a block a competition ("Champions
/// League: Arsenal, Man City."): the competition's logo and name at the left, a tile a club at
/// the right (crest, name, league place; the human's own club gold), each opening its link; no
/// chip row (it ran out of room, Everton cut off: user 2026-11-07).
#[allow(static_mut_refs)]
unsafe fn qualifier_blocks(words: &[Word], links: &[Link], lines: &[((i32, i32, i32, i32), u32, Vec<u8>)], (left, right, bottom): (i32, i32, i32), shift: i32, row: i32, lf: u32, small: u32) {
    let lh = lines[0].0 .3 - lines[0].0 .1;
    let pitch = lh + (lh / 10).max(2);
    let space = text_width(lf, b" ").max(4).max(lh / 4);
    // the intro: the words before the second paragraph
    let split = words.iter().skip(1).position(|w| w.para).map_or(words.len(), |p| p + 1);
    let (mut x, mut y) = (left, lines[0].0 .1 + shift);
    for w in &words[..split] {
        let ww = text_width(lf, &w.text);
        if x > left && x + ww > right {
            (x, y) = (left, y + pitch);
        }
        text_at(x, y, y + lh, lf, TEXT, &w.text, 0);
        x += ww + space;
    }
    y += 2 * pitch;
    // the list: "Comp: a, b. Comp: c."
    let list: Vec<u8> = words[split..].iter().map(|w| w.text.as_slice()).collect::<Vec<_>>().join(&b' ');
    let link_of = |name: &[u8]| links.iter().find(|l| l.shown() == name || l.name == name);
    let own = human_club_ptr();
    let size = row - 14;
    let lw = (right - left) / 4;
    for part in list.split(|&c| c == b'.').map(|p| p.trim_ascii()).filter(|p| !p.is_empty()) {
        let Some(colon) = part.iter().position(|&c| c == b':') else { continue };
        let (comp, clubs) = (part[..colon].trim_ascii(), &part[colon + 1..]);
        if y + row > bottom {
            break;
        }
        // the competition
        let mut cx = left;
        if draw_crest(link_of(comp).map_or(comp, |l| l.key.as_slice()), cx, y + (row - size) / 2, size) {
            cx += size + 10;
        }
        text_at(cx, y, y + row, lf, WHITE, comp, 0);
        if let Some(l) = link_of(comp) {
            click_at(l, (left, y, left + lw - 8, y + row));
        }
        // the clubs, wrapping under each other in the right part
        let (mut tx, mut ty) = (left + lw, y);
        for name in clubs.split(|&c| c == b',').map(|n| n.trim_ascii()).filter(|n| !n.is_empty()) {
            let l = link_of(name);
            let place = l.filter(|l| l.club()).and_then(|l| finishing_place(l.object)).filter(|&p| p > 0);
            let place_text = place.map(card::ordinal).unwrap_or_default();
            let w = 8 + size + 8 + text_width(small, name) + if place_text.is_empty() { 0 } else { 10 + text_width(small, place_text.as_bytes()) } + 12;
            if tx + w > right && tx > left + lw {
                (tx, ty) = (left + lw, ty + row + 6);
            }
            if ty + row > bottom {
                break;
            }
            pill(tx, ty + 2, tx + w, ty + row - 2, CHIP);
            if !draw_crest(l.map_or(name, |l| l.key.as_slice()), tx + 8, ty + (row - size) / 2, size) {
                dot(tx + 8 + size / 2, ty + row / 2, size / 3, GREY);
            }
            let mine = l.is_some_and(|l| Some(l.object) == own);
            text_at(tx + 16 + size, ty, ty + row, small, if mine { GOLD } else { LIGHT }, name, 0);
            if !place_text.is_empty() {
                text_at(tx + w - 12, ty, ty + row, small, GREY, place_text.as_bytes(), 2);
            }
            if let Some(l) = l {
                click_at(l, (tx, ty + 2, tx + w, ty + row - 2));
            }
            tx += w + 8;
        }
        y = ty + row + 10;
        fill(left, y - 6, right, y - 5, [0x2e, 0x34, 0x3d]);
    }
}

/// The story under the header: its text across the panel (links in colour), a card for its kind,
/// and a row of chips for its crest's club and the links the card does not show. `shift`: how
/// far the header moved the text down; `row`: the chips' height.
#[allow(static_mut_refs)]
pub unsafe fn story(head: &[u8], header: (i32, i32, i32, i32), body: (i32, i32, i32, i32), shift: i32, row: i32, font: u32, crest: Option<Vec<u8>>) {
    let lines = std::mem::take(&mut LINES);
    let blanks = std::mem::take(&mut BLANKS);
    if lines.is_empty() {
        return; // (the text was not drawn this frame)
    }
    let (bx1, by1, bx2, by2) = body;
    // (not the links above the story: the screen's title names the manager)
    let mut links: Vec<Link> = story_links(head).into_iter().filter(|l| l.rect.1 >= header.1).collect();
    REMAP.clear();
    CLICKS.clear();
    HIDE = Some(((header.0, header.1, header.2, by2), (bx1 + 2, by1 + 2)));
    let (left, right) = (bx1 + 22, bx2 - 22);
    let small = smaller(font, 2);
    // (the briefcase is a list icon only: the card and the label are a transfer's)
    let what = match kind(head) {
        Kind::Signed => Kind::Transfer,
        k => k,
    };

    // the words, with where a paragraph starts (a blank line, or a gap, before its line)
    let lf = lines[0].1;
    let lh = lines[0].0 .3 - lines[0].0 .1;
    let pitch = lh + (lh / 10).max(2);
    let mut words: Vec<Word> = Vec::new();
    let mut last = None;
    for (li, ((x1, y1, ..), f, s)) in lines.iter().enumerate() {
        let para = last.is_some_and(|ly: i32| y1 - ly > pitch * 3 / 2 || blanks.iter().any(|&b| b > ly && b < *y1));
        last = Some(*y1);
        let mut at = 0;
        let mut first = true;
        for w in s.split(|&c| c == b' ') {
            if !w.is_empty() {
                let (a, b) = (x1 + text_width(*f, &s[..at]), x1 + text_width(*f, &s[..at + w.len()]));
                words.push(Word { text: w.to_vec(), a, b, line: li, link: None, para: first && para });
                first = false;
            }
            at += w.len() + 1;
        }
    }
    link_words(&mut words, &mut links, &lines);
    // (a client: the host's block, else the story's words looked up one by one)
    if remote::CLIENT && links.is_empty() && !block_words(&mut words, &mut links) {
        word_links(&mut words, &mut links);
    }

    if qualifiers(head) {
        qualifier_blocks(&words, &links, &lines, (left, right, by2 - 10), shift, row, lf, smaller(font, 2));
        return;
    }

    // the chips' row at the bottom, the card over it
    let (r1, r2) = (by2 - 14 - row, by2 - 14);
    let (c1, c2) = (r1 - row - 6, r1 - 6);
    let card_kinds = [Kind::Transfer, Kind::Record, Kind::MoodUp, Kind::MoodDown];
    let text_bottom = if card_kinds.contains(&what) { c1 - 8 } else { r1 - 8 };

    // (the font's own space is 3 px: "RafaelLosada", "2-1Dutch", night 6 F-4; a quarter line)
    let space = text_width(lf, b" ").max(4).max(lh / 4);
    let (mut x, mut y) = (left, lines[0].0 .1 + shift);
    for (k, w) in words.iter().enumerate() {
        let ww = text_width(lf, &w.text);
        if w.para {
            (x, y) = (left, y + 2 * pitch);
        } else if x > left && x + ww > right {
            (x, y) = (left, y + pitch);
        }
        if y + lh > text_bottom {
            break; // ponytail: an overlong story is cut, never seen one
        }
        text_at(x, y, y + lh, lf, if w.link.is_some() { LINK } else { TEXT }, &w.text, 0);
        if let Some(l) = w.link {
            // (the space before a word of the same link is part of it; a client's links have no
            // box of the game's to point at: a click asks the host to open the block's cell)
            let from = if k > 0 && words[k - 1].link == Some(l) && x > left { x - space } else { x };
            click_at(&links[l], (from, y, x + ww, y + lh));
        }
        x += ww + space;
    }

    // the links in the order of the text, each object once
    let mut order: Vec<usize> = (0..links.len()).collect();
    order.sort_by_key(|&i| (links[i].words.map_or(usize::MAX, |w| w.0), links[i].rect.1, links[i].rect.0));
    let mut seen = Vec::new();
    let ordered: Vec<Link> = order.into_iter().filter(|&i| {
        let fresh = !seen.contains(&links[i].object);
        seen.push(links[i].object);
        fresh && !links[i].name.is_empty()
    }).map(|i| links[i].clone()).collect();
    let before = |l: &Link| l.words.and_then(|w| w.0.checked_sub(1)).map(|i| words[i].text.to_ascii_lowercase());
    let owner = |l: &Link| l.words.is_some_and(|w| words[w.1].text.ends_with(b"'s") || words[w.1].text.ends_with(b"'s."));

    // the card: what the story is about, each part opening its link
    let mut on_card: Vec<usize> = Vec::new();
    let mut part = |l: &Link, x: &mut i32, size: i32, name_font: u32, colour: [u8; 3]| {
        let x0 = *x;
        if draw_crest(&l.key, *x, (c1 + c2 - size) / 2, size) {
            *x += size + 8;
        }
        text_at(*x, c1, c2, name_font, colour, l.shown(), 0);
        *x += text_width(name_font, l.shown()) + 16;
        click_at(l, (x0, c1, *x - 8, c2));
        on_card.push(l.object);
    };
    let size = c2 - c1 - 12;
    let card = |c: [u8; 3]| {
        fill(left - 6, c1, right + 6, c2, CHIP);
        fill(left - 6, c1, left - 3, c2, c);
    };
    let fee = || words.iter().find(|w| w.text.first() == Some(&0xa3)).map(|w| {
        let t = w.text.as_slice();
        t.strip_suffix(b".").or(t.strip_suffix(b",")).unwrap_or(t).to_vec()
    });
    match what {
        Kind::Transfer => {
            // the club it leaves ("X's", "from X") -> the club it goes to ("to X", else the one not
            // selling); the player; the fee (buyer first read "Twente -> VVV" for a move the other
            // way, night 6 C-4)
            let clubs: Vec<&Link> = ordered.iter().filter(|l| l.club()).collect();
            let seller = clubs.iter().copied().find(|l| owner(l) || before(l).as_deref() == Some(&b"from"[..]));
            let buyer = clubs.iter().copied().find(|l| before(l).as_deref() == Some(&b"to"[..]) && seller.is_none_or(|s| s.object != l.object))
                .or_else(|| clubs.iter().copied().find(|l| seller.is_none_or(|s| s.object != l.object)));
            let player = ordered.iter().find(|l| l.person());
            if let Some(b) = buyer {
                card(label(what).1);
                let mut cx = left + 6;
                if let Some(s) = seller {
                    part(s, &mut cx, size, small, LIGHT);
                    let cy = (c1 + c2) / 2;
                    fill(cx, cy - 1, cx + 22, cy + 1, GREY);
                    for i in 0..6 {
                        fill(cx - i + 22, cy - i, cx - i + 23, cy + i + 1, GREY);
                    }
                    cx += 34;
                }
                part(b, &mut cx, size, small, LIGHT);
                if let Some(p) = player {
                    cx += 12;
                    part(p, &mut cx, size / 2, small, WHITE);
                }
                if let Some(f) = fee() {
                    text_at(right - 4, c1, c2, font, GOLD, &f, 2);
                }
            }
        }
        Kind::Record => {
            // the number large, the headline by it, the clubs it names
            let number = words.iter().map(|w| w.text.as_slice()).find(|t| t.iter().filter(|c| c.is_ascii_digit()).count() >= 3 && t.iter().all(|c| c.is_ascii_digit() || b",.".contains(c)));
            card(label(what).1);
            let mut cx = left + 6;
            if let Some(n) = number {
                let n = n.strip_suffix(b".").unwrap_or(n);
                let big = font & !0xffff | 5;
                text_at(cx, c1, c2, big, GOLD, n, 0);
                cx += text_width(big, n) + 20;
            }
            for l in ordered.iter().filter(|l| l.club()).take(2) {
                part(l, &mut cx, size, small, DIM);
            }
        }
        Kind::MoodUp | Kind::MoodDown => {
            // the player, up or down, and the word for it (the headline less his name); no card
            // without a player (a board's mood: only an arrow, user 2026-10-29)
            if let Some(p) = ordered.iter().find(|l| l.person()) {
                card(label(what).1);
                let mut cx = left + 6;
                arrow_mark(cx + size / 3, (c1 + c2) / 2, size / 3, what == Kind::MoodUp);
                cx += size * 2 / 3 + 14;
                part(p, &mut cx, size / 2, font, WHITE);
                let mood: Vec<&[u8]> = head.split(|&c| c == b' ').filter(|w| !p.name.split(|&c| c == b' ').any(|n| n == *w)).collect();
                text_at(cx + 4, c1, c2, font, label(what).1, &mood.join(&b' '), 0);
            }
        }
        _ => {}
    }

    // the chips: the story's crest's club first when no link shows it (it opens as before, by
    // NEWS_LINK), then the links the card does not show
    let mut x = left - 6;
    let size = r2 - r1 - 14;
    let chip = |key: &[u8], name: &[u8], x: &mut i32| -> Option<(i32, i32, i32, i32)> {
        let w = 8 + size + 8 + text_width(small, name) + 26;
        if *x + w > right + 6 {
            return None;
        }
        pill(*x, r1 + 4, *x + w, r2 - 4, CHIP);
        if !draw_crest(key, *x + 8, r1 + 7, size) {
            dot(*x + 8 + size / 2, (r1 + r2) / 2, size / 3, GREY);
        }
        text_at(*x + 16 + size, r1, r2, small, LIGHT, name, 0);
        text_at(*x + w - 10, r1, r2, small, GREY, b">", 2);
        let r = (*x, r1 + 4, *x + w, r2 - 4);
        *x += w + 8;
        Some(r)
    };
    NEWS_LINK = None;
    assistant::SCOUT_CHIP = None;
    // the scout's report on a club: its own screen (`assistant::scout_for`)
    if let Some(name) = head.strip_suffix(b" scouting completed").filter(|_| on(SCOUT_SCREEN)) {
        if let Some((club, r)) = club_named(name).and_then(|c| Some((c, chip(name, b"Full scout report", &mut x)?))) {
            assistant::SCOUT_CHIP = Some((r, club, small));
        }
    }
    if let Some(key) = crest.filter(|k| !k.starts_with(b"flag:")) {
        let club = club_named(&key).filter(|&c| readable(c + 0x53, 4) && rd::<usize>(c + 0x53) != 0);
        if let Some(c) = club.filter(|c| !ordered.iter().any(|l| l.object == *c)) {
            if let Some(r) = chip(&key, &key, &mut x) {
                NEWS_LINK = Some((r, c));
            }
        }
    }
    for l in ordered.iter().filter(|l| !on_card.contains(&l.object)) {
        match chip(&l.key, l.shown(), &mut x) {
            Some(r) => click_at(l, r),
            None => break,
        }
    }
}

/// The tabs (wide buttons of a tab's height): restyled in the flip pass. (Scroll bars stay the
/// game's own since 2026-11-06.)
#[allow(static_mut_refs)]
pub unsafe fn note_panel(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32) {
    if flags & 0xff == 0x30 && (40..=90).contains(&(y2 - y1)) && x2 - x1 > 150 && TABS.len() < 16 {
        TABS.push(((x1, y1, x2, y2), flags & 0x800 != 0, Vec::new(), 0));
    }
}

