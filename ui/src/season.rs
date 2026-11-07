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
#[derive(Default, Clone)]
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

static mut SNAP: Option<Snap> = None;
static mut LOADED: bool = false;
static mut DAY: i32 = -1;
static mut OPEN: bool = false;
static mut HOVER: bool = false;

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

/// The human manager of this game (channel) and his club.
unsafe fn human() -> Option<(usize, usize)> {
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let staff = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    let club = if readable(staff, 0x3d) { rd::<usize>(staff + 0x39) } else { 0 };
    (club != 0 && readable(club, 0x1b3 + 28)).then_some((staff, club))
}

/// The day's snapshot of the running season (only while the league has games played).
#[allow(static_mut_refs)]
unsafe fn take(day: i32, year: i32) {
    let Some((staff, club)) = human() else { return };
    let Some((row, league)) = challenge::league_row(club) else { return };
    if row[0] == 0 {
        return; // (the new season's empty table: the last season's snapshot stands)
    }
    let season = season_of(day, year);
    let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
    let old = SNAP.clone().unwrap_or_default();
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
    SNAP = Some(Snap {
        season,
        club: index,
        place,
        teams,
        league,
        row,
        scorer: best(2, 1.0, 0.0),
        assists: best(5, 1.0, 0.0),
        rating: best(0x11, 100.0, 10.0),
        manager: staff_name(staff),
        last_place,
        shown: old.shown,
    });
    save();
}

#[allow(static_mut_refs)]
unsafe fn save() {
    if let Some(s) = SNAP.as_ref() {
        let _ = std::fs::write(FILE, s.line());
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
    if !LOADED {
        LOADED = true;
        SNAP = std::fs::read(FILE).ok().and_then(|l| Snap::read(l.split(|&c| c == b'\n').next().unwrap_or(&[])));
    }
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    if !(1900..2200).contains(&year) || remote::CLIENT {
        return;
    }
    let today = year * 366 + day;
    if today != DAY {
        DAY = today;
        let season = season_of(day, year);
        if let Some(s) = SNAP.as_mut().filter(|s| s.season == season - 1 && s.shown < s.season) {
            s.shown = s.season;
            OPEN = true;
            save();
        }
        take(day, year);
    }
    if OPEN {
        draw();
    }
}

/// The card, over the whole screen.
#[allow(static_mut_refs)]
unsafe fn draw() {
    let Some(s) = SNAP.clone() else { return };
    let (sw, sh) = screen();
    fill(0, 0, sw - 1, sh - 1, BACKGROUND);
    let (cw, ch) = ((sw * 70 / 100).max(600), (sh * 78 / 100).max(420));
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
    // (the season's matches read once a card: a whole season of fixtures each frame would cost)
    static mut ALL: Option<(i32, Vec<form::Result_>)> = None;
    if ALL.as_ref().is_none_or(|a| a.0 != s.season) {
        ALL = Some((s.season, form::played(s.club, (181, s.season), (180, s.season + 1), 120)));
    }
    let all = ALL.as_ref().map(|a| a.1.clone()).unwrap_or_default();
    let biggest = all.iter().filter(|r| r.goals_for > r.against).max_by_key(|r| (r.goals_for - r.against, r.goals_for));
    let opponent = |r: &form::Result_| {
        let c = rd::<usize>(CLUB_TABLE) + r.opponent.max(0) as usize * CLUB_SIZE;
        if readable(c, 0x40) { latin1(&game_text(c + 4, 51)).into_owned() } else { String::new() }
    };
    let tiles: [(&str, String, String); 4] = [
        ("RECORD", format!("{}-{}-{}", s.row[1], s.row[2], s.row[3]), format!("{} league games", s.row[0])),
        ("GOALS", format!("{}-{}", s.row[4], s.row[5]), format!("{:+} difference", s.row[4] - s.row[5])),
        ("BIGGEST WIN", biggest.map_or("-".into(), |r| format!("{}-{}", r.goals_for, r.against)), biggest.map_or(String::new(), |r| format!("v {}", opponent(r)))),
        ("POINTS", s.row[6].to_string(), if s.teams > 0 { format!("of {} teams", s.teams) } else { String::new() }),
    ];
    for (k, (label, big, small)) in tiles.iter().enumerate() {
        let tx = x1 + m + k as i32 * (tw + 12);
        fill(tx, top, tx + tw, top + th, [0x24, 0x28, 0x30]);
        text_at(tx + 14, top + 6, top + 6 + th / 5, f1, muted, label.as_bytes(), 0);
        text_at(tx + 14, top + th / 5 + 4, top + th * 7 / 10, f5, white, big.as_bytes(), 0);
        text_at(tx + 14, top + th * 7 / 10, top + th - 4, f1, muted, &to_latin1(small), 0);
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
        text_at(x1 + m + colw * 4 / 10, y, y + line, f3, white, &to_latin1(if who.is_empty() { "-" } else { who }), 0);
        text_at(x1 + m + colw - 10, y, y + line, f3, GOLD, value.as_bytes(), 2);
    }
    let sx = x1 + m + colw + 20;
    text_at(sx, ctop, ctop + line, f2, GOLD, b"SIGNINGS", 0);
    let (ins, outs, spent, got) = transfers(s.club, s.season);
    let rows: [(&str, Vec<u8>); 4] = [
        ("In / out", format!("{ins} / {outs}").into_bytes()),
        ("Spent", money(spent)),
        ("Received", money(got)),
        ("Net", [if got >= spent { b"+".as_slice() } else { b"-".as_slice() }, &money((got - spent).abs())].concat()),
    ];
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = ctop + line + 8 + k as i32 * (line + 6);
        text_at(sx, y, y + line, f2, muted, label.as_bytes(), 0);
        text_at(sx + colw - 10, y, y + line, f3, white, value, 2);
    }
    // the last six results at the bottom
    let mut last: Vec<form::Result_> = all.iter().take(6).copied().collect();
    last.reverse();
    let by = y2 - m - line;
    text_at(x1 + m, by - line - 4, by - 4, f2, GOLD, b"LAST SIX", 0);
    for (k, r) in last.iter().enumerate() {
        let px = x1 + m + k as i32 * (line * 3 + 8);
        pill(px, by, px + line * 3, by + line, form::colour(r.letter()));
        text_at(px + line * 3 / 2, by, by + line, f1, white, format!("{}-{}", r.goals_for, r.against).as_bytes(), 1);
    }
    // Back
    let b = back_rect();
    fill(b.0, b.1, b.2, b.3, if HOVER { [0x3a, 0x5a, 0x86] } else { [0x2c, 0x46, 0x6a] });
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, white, b"Back", 1);
}

fn back_rect() -> (i32, i32, i32, i32) {
    let (sw, sh) = screen();
    let m = (sh / 88).max(6);
    (sw - m - sw / 8, sh - m - sh / 20, sw - m, sh - m)
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
    let redraw = std::mem::replace(&mut HOVER, over) != over;
    if released && over {
        OPEN = false;
        return Some(true);
    }
    Some(redraw)
}
