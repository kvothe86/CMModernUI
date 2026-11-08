//! The season review card (switch `season_review`, the user's wish of 2026-11-07, look approved):
//! on 1 July a card on the season just over, over the screen until Back. The game resets its
//! tables at the turn of the season, so the card is a snapshot taken every day while the season
//! runs (league row and place, form, the squad's best), kept in `cmui_season.txt` in the game
//! folder (a restart keeps it), with the season's transfers and biggest win read on 1 July.

use crate::*;

const FILE: &str = "cmui_season.txt";
const TRANSFER_MANAGER: usize = 0xde1f54; // past transfers: +0x862 rows of 37 bytes, +0x86a count
const TODAY: usize = 0xae2c90;

/// The snapshot: one line of tab-separated fields (Latin-1).
#[derive(Default, Clone, Debug, PartialEq)]
struct Snap {
    season: i32,       // the season's first year
    club: i32,         // club index
    place: i32,        // league place, 0 unknown
    teams: i32,
    league: Vec<u8>,
    row: [i32; 7],     // played, won, drawn, lost, for, against, points
    scorer: (Vec<u8>, i32),
    assists: (Vec<u8>, i32),
    rating: (Vec<u8>, i32), // average rating x100
    manager: Vec<u8>,
    last_place: i32,   // the season before's place, 0 unknown
    shown: i32,        // the season whose card was shown
}

/// What the card shows, worked out from a snapshot where the game can be called (single player,
/// host) and sent to a network client as bytes; `draw` only draws from it.
#[derive(Default, Clone, Debug, PartialEq)]
struct Card {
    snap: Snap,
    biggest: (Vec<u8>, Vec<u8>),   // the big text ("4-0") and the opponent line
    signings: [Vec<u8>; 4],        // in / out, spent, received, net
    chal: Option<Chal>,            // CM Challenge: only with the challenge on
    last: Vec<(i32, i32, u8)>,     // the last six results, oldest first: for, against, letter
}

#[derive(Default, Clone, Debug, PartialEq)]
struct Chal {
    season: i32,
    total: i32,
    rank: Vec<u8>,
    lists: [Vec<(Vec<u8>, i32)>; 3], // items, RAD challenges, career goals: text, points
}

/// Every human club's latest snapshot (one line a club in the file).
static mut SNAPS: Vec<Snap> = Vec::new();
static mut CARD: Option<Card> = None;
static mut LOADED: bool = false;
static mut DAY: i32 = -1;
static mut OPEN: bool = false;
/// When the card came up: a release in its first moments belongs to the click that loaded the
/// game (it loads on the press) and lands on Back, closing the card unseen (night 8).
static mut OPENED_AT: Option<std::time::Instant> = None;
static mut HOVER: bool = false;
/// The test trigger (file `cmui.seasoncard`): 0 not looked at, 1 waiting, 2 done.
static mut TRIGGER: u8 = 0;
/// The times the card was asked for (the client's key changes with it, not the cached answer).
static mut ASKED_NOW: u32 = 0;

/// The card of the season so far (or the last one), any day: the Assistant menu's "Season
/// review" (night 8). The test trigger's way: the latest snapshot of the human's club.
pub unsafe fn open_now() {
    ASKED_NOW += 1;
    TRIGGER = 1;
}
/// A network client's own record (in its own folder): the season whose card it showed, and the
/// season it has had the host's answer for.
const CLIENT_FILE: &str = "cmui_season_client.txt";
static mut CLIENT_SHOWN: i32 = -1;
static mut CLIENT_ASKED: i32 = 0;

fn season_of(day: i32, year: i32) -> i32 {
    if day >= 181 { year } else { year - 1 }
}

impl Snap {
    fn line(&self) -> Vec<u8> {
        let n = |v: i32| v.to_string().into_bytes();
        let mut f: Vec<Vec<u8>> = vec![n(self.season), n(self.club), n(self.place), n(self.teams), self.league.clone()];
        f.extend(self.row.iter().map(|&v| n(v)));
        f.extend([self.scorer.0.clone(), n(self.scorer.1), self.assists.0.clone(), n(self.assists.1), self.rating.0.clone(), n(self.rating.1)]);
        f.extend([self.manager.clone(), n(self.last_place), n(self.shown)]);
        f.join(&b'\t')
    }

    fn read(line: &[u8]) -> Option<Snap> {
        let f: Vec<&[u8]> = line.split(|&c| c == b'\t').collect();
        if f.len() < 21 {
            return None;
        }
        let n = |k: usize| std::str::from_utf8(f[k]).ok().and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
        let mut row = [0; 7];
        for (k, r) in row.iter_mut().enumerate() {
            *r = n(5 + k);
        }
        Some(Snap { season: n(0), club: n(1), place: n(2), teams: n(3), league: f[4].to_vec(), row, scorer: (f[12].to_vec(), n(13)), assists: (f[14].to_vec(), n(15)), rating: (f[16].to_vec(), n(17)), manager: f[18].to_vec(), last_place: n(19), shown: n(20) })
    }
}

#[cfg(test)]
#[test]
fn snapshot_line() {
    let s = Snap { season: 2028, club: 1808, place: 5, teams: 24, league: b"English Football League One".to_vec(), row: [46, 24, 10, 12, 70, 50, 82], scorer: (b"Wright".to_vec(), 21), assists: (b"Wellens".to_vec(), 9), rating: (b"Ball".to_vec(), 712), manager: b"Test Manager".to_vec(), last_place: 3, shown: 2027 };
    let back = Snap::read(&s.line()).unwrap();
    assert_eq!((back.season, back.place, back.row, back.rating.1, back.shown), (2028, 5, [46, 24, 10, 12, 70, 50, 82], 712, 2027));
    assert_eq!(season_of(181, 2029), 2029);
    assert_eq!(season_of(180, 2029), 2028);
}

impl Card {
    /// Lines: the snapshot, then B biggest win, T signings, C challenge head, i / r / g its pills,
    /// L the last six (tab-separated, Latin-1; the game's texts hold no tab or newline).
    fn bytes(&self) -> Vec<u8> {
        let mut o = self.snap.line();
        let mut line = |f: Vec<&[u8]>| {
            o.push(b'\n');
            o.extend(f.join(&b'\t'));
        };
        line(vec![&b"B"[..], &self.biggest.0, &self.biggest.1]);
        line([b"T".as_slice()].into_iter().chain(self.signings.iter().map(|s| s.as_slice())).collect());
        if let Some(c) = &self.chal {
            line(vec![&b"C"[..], c.season.to_string().as_bytes(), c.total.to_string().as_bytes(), &c.rank]);
            for (list, tag) in c.lists.iter().zip([b'i', b'r', b'g']) {
                for (name, points) in list {
                    line(vec![&[tag][..], name, points.to_string().as_bytes()]);
                }
            }
        }
        let last: Vec<Vec<u8>> = self.last.iter().map(|r| format!("{}-{}-{}", r.0, r.1, r.2 as char).into_bytes()).collect();
        line([b"L".as_slice()].into_iter().chain(last.iter().map(|s| s.as_slice())).collect());
        o
    }

    fn read(bytes: &[u8]) -> Option<Card> {
        let mut lines = bytes.split(|&c| c == b'\n');
        let mut card = Card { snap: Snap::read(lines.next()?)?, ..Default::default() };
        let num = |f: &[u8]| std::str::from_utf8(f).ok().and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
        for l in lines {
            let f: Vec<&[u8]> = l.split(|&c| c == b'\t').collect();
            match (f[0], f.len()) {
                (b"B", 3) => card.biggest = (f[1].to_vec(), f[2].to_vec()),
                (b"T", 5) => card.signings = [f[1].to_vec(), f[2].to_vec(), f[3].to_vec(), f[4].to_vec()],
                (b"C", 4) => card.chal = Some(Chal { season: num(f[1]), total: num(f[2]), rank: f[3].to_vec(), ..Default::default() }),
                (b"i" | b"r" | b"g", 3) => {
                    let k = [b'i', b'r', b'g'].iter().position(|&t| t == f[0][0]).unwrap_or(0);
                    if let Some(c) = card.chal.as_mut() {
                        c.lists[k].push((f[1].to_vec(), num(f[2])));
                    }
                }
                (b"L", _) => {
                    card.last = f[1..].iter().filter_map(|r| {
                        let p: Vec<&[u8]> = r.split(|&c| c == b'-').collect();
                        (p.len() == 3).then(|| (num(p[0]), num(p[1]), *p[2].first().unwrap_or(&b'D')))
                    }).collect()
                }
                _ => {}
            }
        }
        Some(card)
    }
}

#[cfg(test)]
#[test]
fn card_bytes() {
    let snap = Snap { season: 2028, club: 1808, place: 5, teams: 24, league: b"League One".to_vec(), row: [46, 24, 10, 12, 70, 50, 82], scorer: (b"Wright".to_vec(), 21), assists: (b"Wellens".to_vec(), 9), rating: (b"Ball".to_vec(), 712), manager: b"Test Manager".to_vec(), last_place: 3, shown: 2027 };
    let mut c = Card { snap, biggest: (b"5-0".to_vec(), b"v Bury".to_vec()), signings: [b"3 / 2".to_vec(), b"1.5M".to_vec(), b"2M".to_vec(), b"+500K".to_vec()], chal: None, last: vec![(2, 1, b'W'), (0, 0, b'D')] };
    assert_eq!(Card::read(&c.bytes()), Some(c.clone()));
    c.chal = Some(Chal { season: 2028, total: 90, rank: b"Pro".to_vec(), lists: [vec![(b"Title".to_vec(), 20)], vec![], vec![(b"Cup \xe9".to_vec(), -5)]] });
    assert_eq!(Card::read(&c.bytes()), Some(c));
    assert_eq!(Card::read(b""), None);
}

/// The human manager of a channel and his club.
unsafe fn human_of(c: usize) -> Option<(usize, usize)> {
    let slot = CHANNEL_MANAGER + 0x300 * c;
    let staff = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    let club = if readable(staff, 0x3d) { rd::<usize>(staff + 0x39) } else { 0 };
    (club != 0 && readable(club, 0x1b3 + 28)).then_some((staff, club))
}

/// The human manager of this game (channel) and his club.
unsafe fn human() -> Option<(usize, usize)> {
    human_of(channel())
}

unsafe fn club_index(club: usize) -> i32 {
    ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32
}

/// The day's snapshot of every human manager's club (a network game has several) of the running
/// season (only while the league has games played).
unsafe fn take(day: i32, year: i32) {
    for c in 0..16 {
        if let Some((staff, club)) = human_of(c) {
            take_one(staff, club, day, year, false);
        }
    }
}

/// `now`: asked from the menu, so also a season with no league match yet (an empty card).
#[allow(static_mut_refs)]
unsafe fn take_one(staff: usize, club: usize, day: i32, year: i32, now: bool) {
    let Some((row, league)) = challenge::league_row(club) else { return };
    if row[0] == 0 && !now {
        return; // (the new season's empty table: the last season's snapshot stands)
    }
    let season = season_of(day, year);
    let index = club_index(club);
    let old = SNAPS.iter().find(|s| s.club == index).cloned().unwrap_or_default();
    let (place, teams) = challenge::league_place(club).unwrap_or((0, 0));
    let best = |field: i32, scale: f32, min_apps: f32| {
        challenge::squad_of(club)
            .into_iter()
            .filter(|&s| min_apps == 0.0 || challenge::season_stat(s, 6, 1) >= min_apps)
            .map(|s| (s, challenge::season_stat(s, 6, field)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|b| b.1 > 0.0)
            .map(|(s, v)| (staff_name(s), (v * scale) as i32))
            .unwrap_or_default()
    };
    let last_place = if old.season == season - 1 && old.club == index { old.place } else if old.season == season { old.last_place } else { 0 };
    // the game clears the players' season stats after the last match day, before 1 July (seen
    // 2026-11-07: the card's stars empty): an empty star of the same season keeps the last one
    let same = old.season == season && old.club == index;
    let keep = |new: (Vec<u8>, i32), was: &(Vec<u8>, i32)| if new.1 == 0 && same { was.clone() } else { new };
    let snap = Snap {
        season,
        club: index,
        place,
        teams,
        league,
        row,
        scorer: keep(best(2, 1.0, 0.0), &old.scorer),
        assists: keep(best(5, 1.0, 0.0), &old.assists),
        rating: keep(best(0x11, 100.0, 10.0), &old.rating),
        manager: staff_name(staff),
        last_place,
        shown: old.shown,
    };
    match SNAPS.iter_mut().find(|s| s.club == index) {
        Some(s) => *s = snap,
        None => SNAPS.push(snap),
    }
    save();
}

/// All clubs' lines, one a club (the file of the first version held one line).
#[allow(static_mut_refs)]
unsafe fn save() {
    let lines: Vec<Vec<u8>> = SNAPS.iter().map(|s| s.line()).collect();
    let _ = std::fs::write(FILE, lines.join(&b'\n'));
}

/// What the card shows for `snap` of the manager `staff` (the game is called: not in a client).
unsafe fn card(staff: usize, snap: &Snap) -> Card {
    let all = form::played(snap.club, (181, snap.season), (180, snap.season + 1), 120);
    let biggest = all.iter().filter(|r| r.goals_for > r.against).max_by_key(|r| (r.goals_for - r.against, r.goals_for));
    let opponent = |r: &form::Result_| {
        let c = rd::<usize>(CLUB_TABLE) + r.opponent.max(0) as usize * CLUB_SIZE;
        if readable(c, 0x40) { game_text(c + 4, 51) } else { Vec::new() }
    };
    let biggest = biggest.map_or((b"-".to_vec(), Vec::new()), |r| (format!("{}-{}", r.goals_for, r.against).into_bytes(), [b"v ".as_slice(), &opponent(r)].concat()));
    let (ins, outs, spent, got) = transfers(snap.club, snap.season);
    let signings = [
        format!("{ins} / {outs}").into_bytes(),
        money(spent),
        money(got),
        [if got >= spent { b"+".as_slice() } else { b"-".as_slice() }, &money((got - spent).abs())].concat(),
    ];
    // CM CHALLENGE: what the season gave (read from the game and the league's file)
    let chal = challenge::summary(staff, snap.season).map(|c| {
        let list = |l: &[(String, i32)]| l.iter().map(|(n, p)| (to_latin1(n), *p)).collect();
        Chal { season: c.season, total: c.total, rank: to_latin1(&c.rank), lists: [list(&c.items), list(&c.rads), list(&c.goals)] }
    });
    let mut last: Vec<(i32, i32, u8)> = all.iter().take(6).map(|r| (r.goals_for, r.against, r.letter())).collect();
    last.reverse();
    Card { snap: snap.clone(), biggest, signings, chal, last }
}

/// The card of the latest snapshot of a channel's manager's club for a network client; `season`
/// 0 any, else only that season's. Empty: none. None: the channel has no manager yet (a client
/// asks on Manager Status before it takes one, night 8: "none" then stopped its asking), asked
/// again.
#[allow(static_mut_refs)]
pub unsafe fn remote(channel: i16, season: i32) -> Option<Vec<u8>> {
    load();
    let (staff, club) = human_of(channel.max(0) as usize)?;
    let index = club_index(club);
    if season == 0 && !SNAPS.iter().any(|s| s.club == index) {
        take_one(staff, club, rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32, true);
    }
    Some(match SNAPS.iter().find(|s| s.club == index && (season == 0 || s.season == season)) {
        Some(s) => card(staff, s).bytes(),
        None => Vec::new(),
    })
}

#[allow(static_mut_refs)]
unsafe fn load() {
    if !LOADED {
        LOADED = true;
        SNAPS = std::fs::read(FILE).map(|b| b.split(|&c| c == b'\n').filter_map(Snap::read).collect()).unwrap_or_default();
    }
}

/// The season's transfers of the club (index): in, out, money spent and received (the game's
/// amounts), from the transfer manager's rows (+0x10 the club joined, +0x14 the club left, +0x18
/// the fee, +0x1d the day, +0x1f the year).
unsafe fn transfers(club: i32, season: i32) -> (i32, i32, i64, i64) {
    let tm = rd::<usize>(TRANSFER_MANAGER);
    if !readable(tm, 0x86e) {
        return (0, 0, 0, 0);
    }
    let (rows, count) = (rd::<usize>(tm + 0x862), rd::<i32>(tm + 0x86a));
    if count <= 0 || !readable(rows, count as usize * 0x25) {
        return (0, 0, 0, 0);
    }
    let (mut ins, mut outs, mut spent, mut got) = (0, 0, 0i64, 0i64);
    for t in (0..count as usize).map(|k| rows + k * 0x25) {
        let (joined, left, fee, kind) = (rd::<i32>(t + 0x10), rd::<i32>(t + 0x14), rd::<i32>(t + 0x18) as i64, rd::<u8>(t + 0x1c));
        if kind == 2 || season_of(rd::<i16>(t + 0x1d) as i32, rd::<i16>(t + 0x1f) as i32) != season {
            continue; // (loans, and other seasons)
        }
        if joined == club {
            ins += 1;
            spent += fee.max(0);
        } else if left == club {
            outs += 1;
            got += fee.max(0);
        }
    }
    (ins, outs, spent, got)
}

/// A game amount as the game writes money (Cash::get_display_amount 0x43f460 flags 6,
/// cash_format_money 0x441c10, as the profile's value).
unsafe fn money(amount: i64) -> Vec<u8> {
    let cash = [amount.clamp(i32::MIN as i64, i32::MAX as i64) as i32, 0, 0, 0];
    let shown = game::<extern "thiscall" fn(*const i32, u32) -> i64>(0x43f460)(cash.as_ptr(), 6);
    let mut buf = [0u8; 64];
    game::<extern "C" fn(*mut u8, i32, i64, u32) -> i32>(0x441c10)(buf.as_mut_ptr(), 60, shown, 6);
    buf.split(|&c| c == 0).next().unwrap_or(&[]).to_vec()
}

/// Every flip: a new day takes the snapshot; on 1 July (the first day of a new season with the
/// last one's snapshot not shown yet) the card opens; then it is drawn while open.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    load();
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    if !(1900..2200).contains(&year) {
        return;
    }
    if TRIGGER == 0 {
        TRIGGER = if std::path::Path::new("cmui.seasoncard").exists() { 1 } else { 2 };
    }
    let season = season_of(day, year);
    if remote::CLIENT {
        // (no game world here: the host works the card out; the date is read from the host)
        if CLIENT_SHOWN < 0 {
            CLIENT_SHOWN = std::fs::read_to_string(CLIENT_FILE).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        }
        let (want, ask) = if TRIGGER == 1 { (0, key(b's', &[0, ASKED_NOW])) } else if CLIENT_SHOWN < season - 1 && CLIENT_ASKED < season - 1 { (season - 1, key(b's', &[(season - 1) as u32])) } else { (-1, Vec::new()) };
        if want >= 0 {
            if let Some(v) = ask_host(&ask) {
                // (an empty answer: no card for that season; asked once)
                match Card::read(&v) {
                    Some(c) if want == 0 || c.snap.season == want => {
                        if want > 0 {
                            CLIENT_SHOWN = want;
                            let _ = std::fs::write(CLIENT_FILE, want.to_string());
                        }
                        CARD = Some(c);
                        OPEN = true;
                    }
                    _ => {}
                }
                if want == 0 {
                    TRIGGER = 2;
                } else {
                    CLIENT_ASKED = want;
                }
            }
        }
    } else if let Some((staff, club)) = human() {
        let today = year * 366 + day;
        let index = club_index(club);
        if today != DAY {
            DAY = today;
            if let Some(s) = SNAPS.iter_mut().find(|s| s.club == index && s.season == season - 1 && s.shown < s.season) {
                s.shown = s.season;
                let s = s.clone();
                CARD = Some(card(staff, &s));
                OPEN = true;
                save();
            }
            take(day, year);
        }
        if TRIGGER == 1 {
            TRIGGER = 2;
            // (the menu's Season review on a day with no snapshot of this club yet, a save of
            // another game or before its league started: did nothing, user 2026-11-08)
            if !SNAPS.iter().any(|s| s.club == index && s.season >= season - 1) {
                take_one(staff, club, day, year, true);
            }
            match SNAPS.iter().find(|s| s.club == index).cloned() {
                Some(s) => {
                    CARD = Some(card(staff, &s));
                    OPEN = true;
                }
                None => log(&format!("season card: no snapshot of club {index} (no league row)")),
            }
        }
    }
    // (logged: a test's click on the bottom right closes it unseen, night 8)
    static mut LOGGED: bool = false;
    if OPEN && !LOGGED {
        log(&format!("season card open: season {} (client {})", CARD.as_ref().map_or(0, |c| c.snap.season), remote::CLIENT));
        OPENED_AT = Some(std::time::Instant::now());
    }
    LOGGED = OPEN;
    if OPEN {
        draw();
    }
}

/// The card, over the whole screen.
#[allow(static_mut_refs)]
unsafe fn draw() {
    let Some(card) = CARD.clone() else { return };
    let s = &card.snap;
    let (sw, sh) = screen();
    fill(0, 0, sw - 1, sh - 1, BACKGROUND);
    // (a small screen: more of its height, the CM Challenge line and Last six touched at 800x600)
    let (cw, ch) = ((sw * 70 / 100).max(600), (sh * if small_screen() { 86 } else { 78 } / 100).max(420));
    let (x1, y1) = ((sw - cw) / 2, (sh - ch) / 2 - sh / 30);
    let (x2, y2) = (x1 + cw, y1 + ch);
    fill(x1, y1, x2, y2, AREA);
    let m = cw / 30;
    let (f1, f2, f3, f5) = (0xffff0001u32, 0xffff0002u32, 0xffff0003u32, 0xffff0005u32);
    let white = [0xf4, 0xf6, 0xf8];
    let muted = [0x8a, 0x94, 0xa4];
    // header: crest, name, season and manager; the place big in gold at the right
    let club = rd::<usize>(CLUB_TABLE) + s.club.max(0) as usize * CLUB_SIZE;
    let head = ch / 6;
    let crest = head - 10;
    if readable(club, 0x40) {
        draw_crest(&game_text(club + 4, 51), x1 + m, y1 + m, crest);
        text_at(x1 + m + crest + 16, y1 + m, y1 + m + crest / 2, f5, white, &game_text(club + 4, 51), 0);
    }
    let sub = format!("Season {}/{:02} \u{b7} {}", s.season, (s.season + 1) % 100, latin1(&s.manager));
    text_at(x1 + m + crest + 16, y1 + m + crest / 2, y1 + m + crest, f2, muted, &to_latin1(&sub), 0);
    if s.place > 0 {
        let place = format!("{}", ordinal(s.place));
        text_at(x2 - m, y1 + m, y1 + m + crest * 6 / 10, f5 + 2, GOLD, place.as_bytes(), 2);
        let under = if s.last_place > 0 { format!("{} \u{b7} last season {}", latin1(&s.league), ordinal(s.last_place)) } else { latin1(&s.league).into_owned() };
        text_at(x2 - m, y1 + m + crest * 6 / 10, y1 + m + crest, f2, muted, &to_latin1(&under), 2);
    }
    fill(x1 + m, y1 + m + head, x2 - m, y1 + m + head + 2, GOLD);
    // four tiles: record, goals, biggest win, points
    let top = y1 + m + head + 16;
    let (tw, th) = ((cw - 2 * m - 3 * 12) / 4, ch / 7);
    let tiles: [(&str, Vec<u8>, Vec<u8>); 4] = [
        ("RECORD", format!("{}-{}-{}", s.row[1], s.row[2], s.row[3]).into_bytes(), format!("{} league games", s.row[0]).into_bytes()),
        ("GOALS", format!("{}-{}", s.row[4], s.row[5]).into_bytes(), format!("{:+} difference", s.row[4] - s.row[5]).into_bytes()),
        ("BIGGEST WIN", card.biggest.0.clone(), card.biggest.1.clone()),
        ("POINTS", s.row[6].to_string().into_bytes(), if s.teams > 0 && s.place > 0 { format!("{} out of {}", ordinal(s.place), s.teams) } else if s.teams > 0 { format!("of {} teams", s.teams) } else { String::new() }.into_bytes()),
    ];
    for (k, (label, big, small)) in tiles.iter().enumerate() {
        let tx = x1 + m + k as i32 * (tw + 12);
        fill(tx, top, tx + tw, top + th, [0x24, 0x28, 0x30]);
        text_at(tx + 14, top + 6, top + 6 + th / 5, f1, muted, label.as_bytes(), 0);
        text_at(tx + 14, top + th / 5 + 4, top + th * 7 / 10, f5, white, big, 0);
        text_at(tx + 14, top + th * 7 / 10, top + th - 4, f1, muted, small, 0);
    }
    // two columns: STARS, SIGNINGS
    let ctop = top + th + 20;
    let colw = (cw - 2 * m - 20) / 2;
    let line = (ch / 18).max(18);
    text_at(x1 + m, ctop, ctop + line, f2, GOLD, b"STARS", 0);
    let stars = [
        ("Top scorer", latin1(&s.scorer.0).into_owned(), format!("{} goals", s.scorer.1)),
        ("Most assists", latin1(&s.assists.0).into_owned(), format!("{}", s.assists.1)),
        ("Best average", latin1(&s.rating.0).into_owned(), format!("{:.2}", s.rating.1 as f32 / 100.0)),
    ];
    for (k, (label, who, value)) in stars.iter().enumerate() {
        let y = ctop + line + 8 + k as i32 * (line + 6);
        text_at(x1 + m, y, y + line, f2, muted, label.as_bytes(), 0);
        // (the name clipped short of its value: at 800x600 "Nikos Machlas" ran over "28 goals")
        let fs = if small_screen() { f2 } else { f3 }; // (a size that fits a small screen)
        let room = colw - 10 - text_width(fs, value.as_bytes()) - colw * 4 / 10 - 12;
        // (too long: without the first name, "van der Vaart", before cutting it)
        let full = to_latin1(if who.is_empty() { "-" } else { who });
        let name = if text_width(fs, &full) <= room { full } else { who.split_once(' ').map_or(full.clone(), |(_, rest)| to_latin1(rest)) };
        text_at(x1 + m + colw * 4 / 10, y, y + line, fs, white, &clip_text(fs, &name, room.max(40)), 0);
        // (no one yet, early in a season: no "0.00" beside the dash)
        if !who.is_empty() {
            text_at(x1 + m + colw - 10, y, y + line, fs, GOLD, value.as_bytes(), 2);
        }
    }
    let sx = x1 + m + colw + 20;
    text_at(sx, ctop, ctop + line, f2, GOLD, b"SIGNINGS", 0);
    let labels = ["In / out", "Spent", "Received", "Net"];
    for (k, (label, value)) in labels.iter().zip(card.signings.iter()).enumerate() {
        let y = ctop + line + 8 + k as i32 * (line + 6);
        text_at(sx, y, y + line, f2, muted, label.as_bytes(), 0);
        text_at(sx + colw - 10, y, y + line, f3, white, value, 2);
    }
    let by = y2 - m - line;
    // CM CHALLENGE: what the season gave (items green, RAD challenges blue, career goals gold),
    // the season's points, the career total and rank; only with the challenge on (read once a
    // card: the league's seasons are worked out from the game and its file)
    if let Some(c) = card.chal.as_ref() {
        let mut y = ctop + line + 8 + 4 * (line + 6) + 14;
        text_at(x1 + m, y, y + line, f2, GOLD, b"CM CHALLENGE", 0);
        let head = format!("+{} this season \u{b7} {} total \u{b7} {}", c.season, c.total, latin1(&c.rank));
        text_at(x2 - m, y, y + line, f2, white, &to_latin1(&head), 2);
        y += line + 8;
        let (mut x, ph) = (x1 + m, line);
        let kinds = [(&c.lists[0], [0x2e, 0x8a, 0x58]), (&c.lists[1], [0x2c, 0x5a, 0x96]), (&c.lists[2], [0xa8, 0x82, 0x2c])];
        // (no row left above Last six, a small screen: the header line says it all; at 800x600
        // "Nothing gained" and the pills ran into Last six, night 8)
        let room = y + ph <= by - line - 10;
        if room && c.lists.iter().all(|l| l.is_empty()) {
            text_at(x, y, y + ph, f2, muted, b"Nothing gained this season", 0);
        }
        for (list, colour) in kinds.into_iter().filter(|_| room) {
            for (name, points) in list.iter() {
                let label = to_latin1(&format!("{}  {points:+}", latin1(name)));
                let w = text_width(f1, &label) + 24;
                if x + w > x2 - m {
                    (x, y) = (x1 + m, y + ph + 6);
                }
                if y + ph > by - line - 10 {
                    break; // (no room left above Last six)
                }
                pill(x, y, x + w, y + ph, colour);
                text_at(x + w / 2, y, y + ph, f1, white, &label, 1);
                x += w + 8;
            }
        }
    }
    // the last six results at the bottom
    text_at(x1 + m, by - line - 4, by - 4, f2, GOLD, b"LAST SIX", 0);
    for (k, r) in card.last.iter().enumerate() {
        let px = x1 + m + k as i32 * (line * 3 + 8);
        pill(px, by, px + line * 3, by + line, form::colour(r.2));
        text_at(px + line * 3 / 2, by, by + line, f1, white, format!("{}-{}", r.0, r.1).as_bytes(), 1);
    }
    // the card as a picture to share (friends' league): taken now, before the buttons and the
    // note, the card's own area only
    if std::mem::take(&mut SAVE) {
        let club_name: String = card_club_name(s).chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let name = format!("season_{}_{}-{:02}", club_name, s.season, (s.season + 1) % 100);
        SAVED = Some((save_png(x1, y1, x2 + 1, y2 + 1, &name).map_or("Could not save the picture".to_string(), |p| format!("Saved: {p}")), std::time::Instant::now()));
    }
    // Back, and Save image left of it
    let b = back_rect();
    fill(b.0, b.1, b.2, b.3, if HOVER { [0x3a, 0x5a, 0x86] } else { [0x2c, 0x46, 0x6a] });
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, white, b"Back", 1);
    let sv = save_rect();
    fill(sv.0, sv.1, sv.2, sv.3, if SAVE_HOVER { [0x3a, 0x5a, 0x86] } else { [0x24, 0x28, 0x30] });
    text_at((sv.0 + sv.2) / 2, sv.1, sv.3, f3, white, b"Save image", 1);
    if let Some((note, at)) = SAVED.as_ref().filter(|s| s.1.elapsed().as_secs() < 6) {
        let _ = at;
        text_at(sv.0 - 12, sv.1, sv.3, f2, GOLD, &to_latin1(note), 2);
    }
}

/// The club's name of a card, for the picture's file name.
unsafe fn card_club_name(s: &Snap) -> String {
    let club = rd::<usize>(CLUB_TABLE) + s.club.max(0) as usize * CLUB_SIZE;
    if readable(club, 0x40) { latin1(&game_text(club + 4, 51)).into_owned() } else { format!("club{}", s.club) }
}

/// Save image was clicked (the next draw takes the picture), the note of the last one.
static mut SAVE: bool = false;
static mut SAVE_HOVER: bool = false;
static mut SAVED: Option<(String, std::time::Instant)> = None;

fn back_rect() -> (i32, i32, i32, i32) {
    let (sw, sh) = screen();
    let m = (sh / 88).max(6);
    (sw - m - sw / 8, sh - m - sh / 20, sw - m, sh - m)
}

fn save_rect() -> (i32, i32, i32, i32) {
    let b = back_rect();
    let w = (b.2 - b.0) * 5 / 4;
    (b.0 - 12 - w, b.1, b.0 - 12, b.3)
}

/// "1st", "2nd", "23rd".
fn ordinal(n: i32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The mouse: while the card is open it takes every click; Back closes it. Some(redraw) when it
/// took the mouse.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if !OPEN {
        return None;
    }
    // (the game's poll does not run: a click would still be waiting for it after Back)
    *(LEFT_CLICK as *mut u32) = 0;
    *(assistant::RIGHT_CLICK as *mut u32) = 0;
    let b = back_rect();
    let over = (b.0..=b.2).contains(&mx) && (b.1..=b.3).contains(&my);
    // (a client: drawn again when the host's memory comes, the crest and name read from it;
    // this poll returns before lib's own redraw, night 8: no crest on the client's card)
    let redraw = (std::mem::replace(&mut HOVER, over) != over) || (remote::CLIENT && (std::mem::take(&mut remote::ARRIVED) | remote::hold_over()));
    // (a release counts only after a press seen while the card is up: the load's own click
    // came up on Back and shut it unseen, even after 600 ms at 1920x1080, night 8)
    // (a new press: up at the last poll, down now; the load's click is still down at first)
    static mut PRESSED_AT: Option<std::time::Instant> = None;
    static mut WAS_HELD: bool = true;
    // (this process's own mouse state: `rd` reads the host's memory on a network client)
    let held = *(BUTTON_HELD as *const u32) != 0;
    let was = std::mem::replace(&mut WAS_HELD, held);
    if held && !was && OPENED_AT.is_some() {
        PRESSED_AT = Some(std::time::Instant::now());
    }
    let settled = PRESSED_AT.is_some_and(|p| OPENED_AT.is_some_and(|t| p > t));
    if released && over && settled {
        OPEN = false;
        return Some(true);
    }
    let sv = save_rect();
    let on_save = (sv.0..=sv.2).contains(&mx) && (sv.1..=sv.3).contains(&my);
    let redraw = redraw | (std::mem::replace(&mut SAVE_HOVER, on_save) != on_save);
    if released && on_save && settled {
        SAVE = true;
        return Some(true);
    }
    // (the "Saved" note goes after a while)
    static mut NOTE_SHOWN: bool = false;
    let note = SAVED.as_ref().is_some_and(|s| s.1.elapsed().as_secs() < 6);
    let redraw = redraw | (std::mem::replace(&mut NOTE_SHOWN, note) != note);
    Some(redraw)
}
