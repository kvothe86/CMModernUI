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

#[cfg(test)]
#[test]
fn ordinals() {
    assert_eq!([ordinal(1), ordinal(2), ordinal(3), ordinal(11), ordinal(22), ordinal(113)], ["1st", "2nd", "3rd", "11th", "22nd", "113th"]);
}

/// One side of the card, centred on cx from `top`: crest, name, place, last five.
unsafe fn side(club_index: i32, cx: i32, top: i32, crest: i32, room: i32) {
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
    let mut last = form::results(club_index, 5);
    last.reverse();
    let (w, h) = ((line * 13 / 10).max(18), (line * 9 / 10).max(14));
    let x0 = cx - (last.len() as i32 * (w + 4)) / 2;
    for (k, r) in last.iter().enumerate() {
        let x = x0 + k as i32 * (w + 4);
        pill(x, y, x + w, y + h, form::colour(r.letter()));
        text_at(x + w / 2, y, y + h, small, [0xf4, 0xf6, 0xf8], &[r.letter()], 1);
    }
}

/// The flip pass: the card, when the question was drawn this frame and the human's club plays today.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    let Some(((px1, py1, px2, _), ty)) = CONFIRM.take() else { return };
    let Some(club) = human_club_ptr() else { return };
    let index = ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32;
    let Some((home, away)) = form::today_fixture(index) else { return };
    let (x1, y1, x2, y2) = (px1 + 40, py1 + 24, px2 - 40, ty - 24);
    if y2 - y1 < 180 || x2 - x1 < 500 {
        return;
    }
    let crest = ((y2 - y1) * 40 / 100).min(120);
    let w = x2 - x1;
    text_at(x1, y1 - 18, y1 - 2, SMALL_FONT.unwrap_or(0xffff0002), GOLD, b"MATCH DAY", 0);
    side(home, x1 + w / 4, y1 + 8, crest, w / 2 - 40);
    side(away, x1 + w * 3 / 4, y1 + 8, crest, w / 2 - 40);
    // the middle: v, then the last meetings this season (the home side's results against the other)
    let mid = x1 + w / 2;
    text_at(mid, y1 + crest / 3, y1 + crest * 2 / 3 + 10, 0xffff0005, [0x9a, 0xa4, 0xb4], b"v", 1);
    let small = SMALL_FONT.unwrap_or(0xffff0002);
    let met: Vec<form::Result_> = form::results(home, 10).into_iter().filter(|r| r.opponent == away).take(3).collect();
    let mut y = y1 + crest + 20;
    let line = (crest / 5).max(16);
    text_at(mid, y, y + line, small, GOLD, if met.is_empty() { b"No meeting this season".as_slice() } else { b"Last meetings".as_slice() }, 1);
    y += line + 4;
    for r in met {
        // (the home side of today first)
        let text = format!("{}-{} {}", r.goals_for, r.against, if r.home { "(home)" } else { "(away)" });
        text_at(mid, y, y + line, small, form::colour(r.letter()), text.as_bytes(), 1);
        y += line + 2;
    }
}
