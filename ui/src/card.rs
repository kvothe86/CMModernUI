//! The match-day card (switch `match_card`, the user's wish of 2026-11-07): on the "Please Confirm
//! / Proceed to the match ?" screen before a match of the human manager, whose big panel is empty
//! but for that line, a card above it: both clubs (crest, name, league place, last five results)
//! and their recent meetings in the middle.

use crate::*;

/// This frame's question line: the panel it stands in, the line's top.
static mut CONFIRM: Option<((i32, i32, i32, i32), i32)> = None;

/// A text the game draws: the question that comes before the human's match.
pub unsafe fn note(panel: (i32, i32, i32, i32), y1: i32, t: &[u8]) {
    if t.trim_ascii().starts_with(b"Proceed to the match") && panel.3 > y1 {
        CONFIRM = Some((panel, y1));
    }
}

/// "1st", "2nd", "23rd".
pub(crate) fn ordinal(n: i32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
#[test]
fn ordinals() {
    assert_eq!([ordinal(1), ordinal(2), ordinal(3), ordinal(11), ordinal(22), ordinal(113)], ["1st", "2nd", "3rd", "11th", "22nd", "113th"]);
}

/// One side of the card, centred on cx from `top`: crest, name, place, last five.
unsafe fn side(club_index: i32, last: &[u8], cx: i32, top: i32, crest: i32, room: i32) {
    let club = rd::<usize>(CLUB_TABLE) + club_index.max(0) as usize * CLUB_SIZE;
    if !readable(club, 0x5b) {
        return;
    }
    let name = game_text(club + 4, 51);
    draw_crest(&name, cx - crest / 2, top, crest);
    let (big, small) = (0xffff0003u32, SMALL_FONT.unwrap_or(0xffff0002));
    let line = crest / 4 + 6;
    let mut y = top + crest + 8;
    text_at(cx, y, y + line, big, [0xf4, 0xf6, 0xf8], &clip_text(big, &name, room), 1);
    y += line + 2;
    let place = challenge::league_place(club).map_or(String::new(), |(p, n)| format!("{} of {n} in the league", ordinal(p)));
    if !place.is_empty() {
        text_at(cx, y, y + line * 8 / 10, small, [0x9a, 0xa4, 0xb4], place.as_bytes(), 1);
        y += line * 8 / 10 + 6;
    }
    // the last five, oldest at the left
    let (w, h) = ((line * 13 / 10).max(18), (line * 9 / 10).max(14));
    let x0 = cx - (last.len() as i32 * (w + 4)) / 2;
    for (k, &r) in last.iter().enumerate() {
        let x = x0 + k as i32 * (w + 4);
        pill(x, y, x + w, y + h, form::colour(r));
        text_at(x + w / 2, y, y + h, small, [0xf4, 0xf6, 0xf8], &[r], 1);
    }
}

/// A side's formation, eleven (slot label, staff), bench, and whether the eleven is a guess.
type Lineup = (String, Vec<(Vec<u8>, usize)>, Vec<usize>, bool);

/// A club of the club table by index.
unsafe fn club_at(index: i32) -> usize {
    rd::<usize>(CLUB_TABLE) + index.max(0) as usize * CLUB_SIZE
}

/// What one side of the card needs from the game: its line-up (None: no line-up), the injured
/// (staff, up to 6) and its last five results as letters, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct SideData {
    pub lineup: Option<Lineup>,
    pub injured: Vec<usize>,
    pub last: Vec<u8>,
}

/// Everything the card draws that only the game can tell (a network client asks the host for it):
/// the human's club and manager (host pointers), today's two clubs (indices), both sides, and the
/// last meetings (goals for, against of the home side, whether it was at home).
#[derive(Clone, Debug, PartialEq)]
pub struct CardData {
    pub own: usize,
    pub manager: usize,
    pub home: i32,
    pub away: i32,
    pub sides: [SideData; 2],
    pub met: Vec<(i32, i32, bool)>,
}

fn letter_of(gf: i32, ag: i32) -> u8 {
    match gf.cmp(&ag) {
        std::cmp::Ordering::Greater => b'W',
        std::cmp::Ordering::Equal => b'D',
        std::cmp::Ordering::Less => b'L',
    }
}

impl CardData {
    /// Bytes for the network: u32 own, manager, home, away; per side a line-up flag (0 none, else
    /// 1 + guess), when set: formation (u8 len, text), eleven (u8 n; each u8 len, label, u32
    /// staff), bench (u8 n, u32s); then injured (u8 n, u32s), last (u8 n, letters); then the
    /// meetings (u8 n; each i16 for, i16 against, u8 home). Little endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [self.own, self.manager, self.home as usize, self.away as usize] {
            b.extend((v as u32).to_le_bytes());
        }
        for s in &self.sides {
            match &s.lineup {
                None => b.push(0),
                Some((formation, eleven, bench, guess)) => {
                    b.push(1 + *guess as u8);
                    b.push(formation.len().min(255) as u8);
                    b.extend(&formation.as_bytes()[..formation.len().min(255)]);
                    b.push(eleven.len().min(255) as u8);
                    for (label, staff) in eleven.iter().take(255) {
                        b.push(label.len().min(255) as u8);
                        b.extend(&label[..label.len().min(255)]);
                        b.extend((*staff as u32).to_le_bytes());
                    }
                    b.push(bench.len().min(255) as u8);
                    for staff in bench.iter().take(255) {
                        b.extend((*staff as u32).to_le_bytes());
                    }
                }
            }
            b.push(s.injured.len().min(255) as u8);
            for staff in s.injured.iter().take(255) {
                b.extend((*staff as u32).to_le_bytes());
            }
            b.push(s.last.len().min(255) as u8);
            b.extend(&s.last[..s.last.len().min(255)]);
        }
        b.push(self.met.len().min(255) as u8);
        for (gf, ag, home) in self.met.iter().take(255) {
            b.extend((*gf as i16).to_le_bytes());
            b.extend((*ag as i16).to_le_bytes());
            b.push(*home as u8);
        }
        b
    }

    /// None for an empty or cut-off answer.
    pub fn decode(v: &[u8]) -> Option<CardData> {
        let mut at = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = v.get(at..at + n)?;
            at += n;
            Some(s)
        };
        let u8_ = |s: &[u8]| s[0] as usize;
        let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().unwrap()) as usize;
        let i16_ = |s: &[u8]| i16::from_le_bytes(s.try_into().unwrap()) as i32;
        let (own, manager) = (u32_(take(4)?), u32_(take(4)?));
        let (home, away) = (u32_(take(4)?) as i32, u32_(take(4)?) as i32);
        let mut read_side = || -> Option<SideData> {
            let flag = u8_(take(1)?);
            let lineup = if flag == 0 {
                None
            } else {
                let n = u8_(take(1)?);
                let formation = String::from_utf8_lossy(take(n)?).into_owned();
                let mut eleven = Vec::new();
                for _ in 0..u8_(take(1)?) {
                    let n = u8_(take(1)?);
                    let label = take(n)?.to_vec();
                    eleven.push((label, u32_(take(4)?)));
                }
                let mut bench = Vec::new();
                for _ in 0..u8_(take(1)?) {
                    bench.push(u32_(take(4)?));
                }
                Some((formation, eleven, bench, flag == 2))
            };
            let mut injured = Vec::new();
            for _ in 0..u8_(take(1)?) {
                injured.push(u32_(take(4)?));
            }
            let n = u8_(take(1)?);
            let last = take(n)?.to_vec();
            Some(SideData { lineup, injured, last })
        };
        let sides = [read_side()?, read_side()?];
        let mut met = Vec::new();
        for _ in 0..u8_(take(1)?) {
            let (gf, ag) = (i16_(take(2)?), i16_(take(2)?));
            met.push((gf, ag, u8_(take(1)?) != 0));
        }
        Some(CardData { own, manager, home, away, sides, met })
    }
}

#[cfg(test)]
#[test]
fn card_data_roundtrip() {
    let side = |k: usize| SideData {
        lineup: if k == 0 { Some(("4-4-2".into(), vec![(b"GK".to_vec(), 0x1234), (b"DC".to_vec(), 0xabcdef)], vec![7, 8], k == 1)) } else { None },
        injured: vec![1, 2, 3],
        last: b"WDLWW".to_vec(),
    };
    let d = CardData { own: 0x5000, manager: 0x6000, home: 12, away: 345, sides: [side(0), side(1)], met: vec![(2, 1, true), (0, 3, false)] };
    assert_eq!(CardData::decode(&d.encode()), Some(d.clone()));
    let mut g = d.clone();
    g.sides[0].lineup.as_mut().unwrap().3 = true;
    assert_eq!(CardData::decode(&g.encode()), Some(g));
    assert_eq!(CardData::decode(&[]), None);
    assert_eq!(CardData::decode(&d.encode()[..20]), None);
}

/// The game's side of the card for the human's club (`own`, his `manager`): None without a match
/// today. Host and single player only (it calls the game).
#[allow(static_mut_refs)]
pub unsafe fn card_data(own: usize, manager: usize) -> Option<CardData> {
    let index = (own.checked_sub(rd::<usize>(CLUB_TABLE))? / CLUB_SIZE) as i32;
    let (home, away) = form::today_fixture(index)?;
    // (the other side's line-up once a fixture: its guess runs the assistant's pick; the human's
    // own each frame, it is only read)
    static mut OTHER: Option<((i32, i32), Option<Lineup>)> = None;
    let side = |i: i32| {
        let club = club_at(i);
        let lineup = if club == own {
            assistant::lineup(club, own, Some(manager))
        } else {
            if OTHER.as_ref().is_none_or(|o| o.0 != (home, away)) {
                OTHER = Some(((home, away), assistant::lineup(club, own, None)));
            }
            OTHER.as_ref().and_then(|o| o.1.clone())
        };
        let injured = (0..50).map(|k| rd::<usize>(club + 0xd7 + 4 * k)).filter(|&s| readable(s, 0x6e) && plays_for(s, club) && assistant::injured(s)).take(6).collect();
        let mut last = form::results(i, 5);
        last.reverse();
        SideData { lineup, injured, last: last.iter().map(|r| r.letter()).collect() }
    };
    let sides = [side(home), side(away)];
    let met = form::results(home, 10).into_iter().filter(|r| r.opponent == away).take(3).map(|r| (r.goals_for, r.against, r.home)).collect();
    Some(CardData { own, manager, home, away, sides, met })
}

/// The host's answer to a client's 'm' question: the card of that channel's manager, empty when
/// his club does not play today.
pub unsafe fn remote(channel: i16) -> Vec<u8> {
    let manager = rd::<usize>(CHANNEL_MANAGER + 0x300 * channel as usize);
    if !readable(manager, 0x3d) {
        return Vec::new();
    }
    let club = rd::<usize>(manager + 0x39);
    if club == 0 || !readable(club, 0x1b3 + 28) {
        return Vec::new();
    }
    card_data(club, manager).map_or(Vec::new(), |d| d.encode())
}

/// v2 (user's design, 2026-11-07): a slim row of both clubs on top, the two line-ups filling the
/// panel (formation above, manager under), the injured in the middle, the question at the bottom.
/// False: no room or no line-up; the first card is drawn instead.
unsafe fn lineups((px1, py1, px2, py2): (i32, i32, i32, i32), ty: i32, d: &CardData) -> bool {
    let (home, away, own, manager) = (d.home, d.away, d.own, d.manager);
    let (Some(h), Some(a)) = (&d.sides[0].lineup, &d.sides[1].lineup) else { return false };
    let (x1, x2) = (px1 + 24, px2 - 24);
    let (y1, y2) = (py1 + 16, py2 - 12);
    let row_h = (y2 - y1) / 22;
    if row_h < 14 || x2 - x1 < 560 {
        return false;
    }
    // the game's question goes: the whole panel is the card's
    fill(px1 + 2, py1 + 2, px2 - 2, py2 - 2, AREA);
    let (big, small, f3) = (0xffff0005u32, SMALL_FONT.unwrap_or(0xffff0002), 0xffff0003u32);
    let (white, muted) = ([0xf4, 0xf6, 0xf8], [0x9a, 0xa4, 0xb4]);
    let w = x2 - x1;
    // top row: crest, name, place and form a side; v and the meetings in the middle
    let top = row_h * 4;
    let crest = top - 8;
    for (index, left) in [(home, true), (away, false)] {
        let club = club_at(index);
        let last = &d.sides[!left as usize].last;
        if !readable(club, 0x5b) {
            continue;
        }
        let name = game_text(club + 4, 51);
        let cx = if left { x1 } else { x2 - crest };
        draw_crest(&name, cx, y1, crest);
        let tx = if left { x1 + crest + 12 } else { x2 - crest - 12 };
        let align = if left { 0 } else { 2 };
        text_at(tx, y1, y1 + top / 2, f3, white, &clip_text(f3, &name, w / 3), align);
        let place = challenge::league_place(club).map_or(String::new(), |(p, n)| format!("{} of {n}", ordinal(p)));
        text_at(tx, y1 + top / 2, y1 + top * 3 / 4, small, muted, place.as_bytes(), align);
        let (pw, ph) = (row_h * 5 / 4, row_h * 3 / 4);
        let n = last.len() as i32;
        let x0 = if left { tx } else { tx - n * (pw + 3) };
        for (k, &r) in last.iter().enumerate() {
            let x = x0 + k as i32 * (pw + 3);
            pill(x, y1 + top * 3 / 4, x + pw, y1 + top * 3 / 4 + ph, form::colour(r));
            text_at(x + pw / 2, y1 + top * 3 / 4, y1 + top * 3 / 4 + ph, small, white, &[r], 1);
        }
    }
    let mid = x1 + w / 2;
    text_at(mid, y1, y1 + top / 2, big, muted, b"v", 1);
    let mut my = y1 + top / 2;
    for &(gf, ag, at_home) in d.met.iter().take(2) {
        let text = format!("{gf}-{ag} {}", if at_home { "(home)" } else { "(away)" });
        text_at(mid, my, my + row_h, small, form::colour(letter_of(gf, ag)), text.as_bytes(), 1);
        my += row_h;
    }
    fill(x1, y1 + top + 4, x2, y1 + top + 5, [0x3a, 0x41, 0x4b]);
    // the two line-ups, the middle column between them
    let cols = y1 + top + 14;
    let midw = w / 6;
    let colw = (w - midw - 40) / 2;
    let bottom = y2 - row_h * 2 - 10;
    for ((formation, eleven, bench, guess), cx, index) in [(h, x1, home), (a, x2 - colw, away)] {
        let club = club_at(index);
        let mut y = cols;
        text_at(cx, y, y + row_h * 3 / 2, big, GOLD, formation.as_bytes(), 0);
        let tag: &[u8] = if *guess { b"Expected XI" } else if club == own { b"Your XI" } else { b"Their XI" };
        text_at(cx + colw, y, y + row_h * 3 / 2, small, muted, tag, 2);
        y += if small_screen() { row_h + 4 } else { row_h * 3 / 2 + 6 }; // (tighter on a small screen: eleven rows and the manager)
        // (a small screen has no subs line under the eleven: the room is the eleventh man's, he
        // was left out at 800x600, night 8)
        let limit = if small_screen() { bottom - row_h - 6 } else { bottom - row_h * 2 };
        for (label, staff) in eleven.iter() {
            if y + row_h > limit {
                break;
            }
            text_at(cx, y, y + row_h, small, muted, label, 0);
            text_at(cx + colw / 6, y, y + row_h, small, white, &clip_text(small, &assistant::short_name(*staff), colw * 3 / 5), 0);
            if let Some(c) = condition(*staff) {
                let colour = if c >= 90 { [0x4c, 0xc0, 0x7a] } else if c >= 75 { GOLD } else { [0xe0, 0x58, 0x50] };
                text_at(cx + colw, y, y + row_h, small, colour, format!("{c}%").as_bytes(), 2);
            }
            fill(cx, y + row_h, cx + colw, y + row_h + 1, [0x2e, 0x34, 0x3d]);
            y += row_h + if small_screen() { 0 } else { 2 };
        }
        // the bench on one line (not on a small screen), then the manager
        if !bench.is_empty() && !small_screen() {
            let names: Vec<Vec<u8>> = bench.iter().map(|&s| assistant::short_name(s)).collect();
            let line = [b"Subs: ".as_slice(), &names.join(&b", "[..])].concat();
            text_at(cx, y + 4, y + 4 + row_h, small, muted, &clip_text(small, &line, colw), 0);
        }
        let boss = if club == own { manager } else { [0xcf, 0xd3].into_iter().map(|at| rd::<usize>(club + at)).find(|&b| readable(b, 0x6e)).unwrap_or(0) };
        if readable(boss, 0x6e) {
            fill(cx, bottom - row_h - 6, cx + colw, bottom - row_h - 5, [0x3a, 0x41, 0x4b]);
            text_at(cx, bottom - row_h, bottom, small, muted, b"Manager", 0);
            text_at(cx + colw, bottom - row_h, bottom, small, white, &staff_name(boss), 2);
        }
    }
    // the middle: who is injured on either side
    let mx = x1 + colw + 20;
    let mut y = cols;
    text_at(mx, y, y + row_h, small, GOLD, b"Injured", 0);
    y += row_h + 4;
    for (k, index) in [home, away].into_iter().enumerate() {
        let club = club_at(index);
        let hurt: Vec<Vec<u8>> = d.sides[k].injured.iter().filter(|&&s| readable(s, 0x6e)).map(|&s| assistant::short_name(s)).collect();
        let name = game_text(club + 4, 51);
        text_at(mx, y, y + row_h, small, white, &clip_text(small, &name, midw), 0);
        y += row_h;
        if hurt.is_empty() {
            text_at(mx, y, y + row_h, small, muted, b"none", 0);
            y += row_h;
        }
        for n in hurt {
            text_at(mx, y, y + row_h, small, muted, &clip_text(small, &n, midw), 0);
            y += row_h;
        }
        y += 6;
    }
    // the question at the bottom, between the game's No and Yes
    let _ = ty;
    text_at(mid, y2 - row_h * 2, y2, f3, white, b"Proceed to the match ?", 1);
    true
}

/// The flip pass: the card, when the question was drawn this frame and the human's club plays today.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    let Some((panel, ty)) = CONFIRM.take() else { return };
    let (px1, py1, px2, _) = panel;
    let data = if remote::CLIENT {
        // (the host works it out; the screen number makes it a fresh question per screen)
        ask_host(&key(b'm', &[CLIENT_SCREEN as u32])).and_then(|v| CardData::decode(&v))
    } else {
        let Some(club) = human_club_ptr() else { return };
        card_data(club, rd::<usize>(CHANNEL_MANAGER + 0x300 * channel()))
    };
    let Some(d) = data else { return };
    if lineups(panel, ty, &d) {
        return;
    }
    let (home, away) = (d.home, d.away);
    let (x1, y1, x2, y2) = (px1 + 40, py1 + 24, px2 - 40, ty - 24);
    if y2 - y1 < 180 || x2 - x1 < 500 {
        return;
    }
    let crest = ((y2 - y1) * 40 / 100).min(120);
    let w = x2 - x1;
    text_at(x1, y1 - 18, y1 - 2, SMALL_FONT.unwrap_or(0xffff0002), GOLD, b"MATCH DAY", 0);
    side(home, &d.sides[0].last, x1 + w / 4, y1 + 8, crest, w / 2 - 40);
    side(away, &d.sides[1].last, x1 + w * 3 / 4, y1 + 8, crest, w / 2 - 40);
    // the middle: v, then the last meetings this season (the home side's results against the other)
    let mid = x1 + w / 2;
    text_at(mid, y1 + crest / 3, y1 + crest * 2 / 3 + 10, 0xffff0005, [0x9a, 0xa4, 0xb4], b"v", 1);
    let small = SMALL_FONT.unwrap_or(0xffff0002);
    let mut y = y1 + crest + 20;
    let line = (crest / 5).max(16);
    text_at(mid, y, y + line, small, GOLD, if d.met.is_empty() { b"No meeting this season".as_slice() } else { b"Last meetings".as_slice() }, 1);
    y += line + 4;
    for &(gf, ag, at_home) in &d.met {
        // (the home side of today first)
        let text = format!("{gf}-{ag} {}", if at_home { "(home)" } else { "(away)" });
        text_at(mid, y, y + line, small, form::colour(letter_of(gf, ag)), text.as_bytes(), 1);
        y += line + 2;
    }
}
