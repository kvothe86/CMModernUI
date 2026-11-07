//! Weather icons (switch `weather_icons`, the user's wish of 2026-11-07): the game names a match's
//! weather as "<precipitation>, <wind>, <n>°C" (weather.cpp 0x9233a0, the wind left out when
//! there is none: "Dry, 8°C"); a drawn icon goes before that text wherever it is shown (the match
//! screen's WEATHER box, the pre-match screen).

use crate::*;

/// What the sky does, by the game's precipitation word.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Sky {
    Sun,
    Rain,
    Downpour,
    Snow,
}

/// The weather a text names, and whether it is windy: None for any other text.
fn read(t: &[u8]) -> Option<(Sky, bool)> {
    let t = t.trim_ascii();
    // (the pre-match screen writes "Weather - Gusty, Wet, 10°C": a label before " - ", the wind
    // first; traced on the user's game 2026-11-07)
    let t = t.windows(3).position(|w| w == b" - ").map_or(t, |at| &t[at + 3..]);
    let parts: Vec<&[u8]> = t.split(|&c| c == b',').map(|p| p.trim_ascii()).collect();
    // (the last part the temperature: digits, maybe a minus, then the degree sign and C)
    let temp = parts.last()?;
    if parts.is_empty() || parts.len() > 3 || !temp.ends_with(b"\xb0C") || temp.len() < 3 || !temp[..temp.len() - 2].iter().all(|c| c.is_ascii_digit() || *c == b'-') {
        return None;
    }
    let words = &parts[..parts.len() - 1];
    const WINDS: [&[u8]; 4] = [b"Gale", b"Strong Wind", b"Gusty", b"Breezy"];
    let sky = |w: &[u8]| match w {
        b"Dry" => Some(Sky::Sun),
        b"Wet" | b"Drizzle" | b"Shower" => Some(Sky::Rain),
        b"Downpour" => Some(Sky::Downpour),
        b"Hail" | b"Sleet" | b"Heavy Snow" | b"Light Snow" => Some(Sky::Snow),
        _ => None,
    };
    // (every word the game's own; no rain word is a dry day: "Breezy, 7°C", user's shot 2026-11-07)
    if !words.iter().all(|w| sky(w).is_some() || WINDS.contains(w)) {
        return None;
    }
    let windy = words.iter().any(|w| WINDS.contains(w));
    Some((words.iter().find_map(|w| sky(w)).unwrap_or(Sky::Sun), windy))
}

#[cfg(test)]
#[test]
fn weather_words() {
    assert_eq!(read(b"Dry, 8\xb0C"), Some((Sky::Sun, false)));
    assert_eq!(read(b"Downpour, Gale, -2\xb0C"), Some((Sky::Downpour, true)));
    assert_eq!(read(b"Light Snow, Breezy, 0\xb0C"), Some((Sky::Snow, true)));
    assert_eq!(read(b"Weather - Gusty, Wet, 10\xb0C"), Some((Sky::Rain, true)));
    assert_eq!(read(b"Breezy, 7\xb0C"), Some((Sky::Sun, true)));
    assert_eq!(read(b"7\xb0C"), Some((Sky::Sun, false)));
    assert_eq!(read(b"Holland, 7\xb0C"), None);
    assert_eq!(read(b"Dry, Holland"), None);
    assert_eq!(read(b"Feyenoord, Rotterdam"), None);
}

/// A cloud of three discs, its middle at (cx, cy), r its half width.
unsafe fn cloud(cx: i32, cy: i32, r: i32, c: [u8; 3]) {
    dot(cx - r / 2, cy + r / 6, r * 5 / 10, c);
    dot(cx + r / 8, cy - r / 6, r * 6 / 10, c);
    dot(cx + r * 6 / 10, cy + r / 5, r * 4 / 10, c);
    fill(cx - r / 2, cy + r / 6, cx + r * 6 / 10, cy + r * 6 / 10, c);
}

/// The icon, centred on (cx, cy), `s` its height.
unsafe fn icon(sky: Sky, windy: bool, cx: i32, cy: i32, s: i32) {
    let r = s / 2;
    let (sun, grey, blue, white) = ([0xf0, 0xc0, 0x40], [0xc8, 0xcc, 0xd4], [0x5c, 0xa8, 0xf0], [0xf2, 0xf4, 0xf8]);
    match sky {
        Sky::Sun => {
            dot(cx, cy, r * 5 / 10, sun);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::PI / 4.0;
                let (dx, dy) = (a.cos(), a.sin());
                let (x, y) = (cx + (dx * r as f32 * 0.8) as i32, cy + (dy * r as f32 * 0.8) as i32);
                fill(x - 1, y - 1, x + 1, y + 1, sun);
            }
        }
        Sky::Rain | Sky::Downpour | Sky::Snow => {
            cloud(cx, cy - r / 4, r * 8 / 10, grey);
            let drops = if sky == Sky::Downpour { 4 } else { 3 };
            for k in 0..drops {
                let x = cx - r / 2 + k * r / (drops - 1).max(1);
                let y = cy + r * 5 / 10;
                if sky == Sky::Snow {
                    dot(x, y + 1, (r / 7).max(1), white);
                } else {
                    fill(x, y, x + 1, y + r / 3 + 1, blue);
                }
            }
        }
    }
    if windy {
        // two short lines at the lower left
        for (k, len) in [(0, r), (1, r * 6 / 10)] {
            let y = cy + r - 2 - k * 3;
            fill(cx - r - len / 2, y, cx - r + len / 2, y + 1, grey);
        }
    }
}

/// The icon of a weather value ("Gusty, Wet, 10°C"), right-aligned text ending at `right` in
/// `font` on the band y1..y2: the match screen's WEATHER box (user, 2026-11-07). False: no weather.
pub unsafe fn before_right(value: &[u8], right: i32, y1: i32, y2: i32, font: u32) -> bool {
    let Some((sky, windy)) = read(value) else { return false };
    let s = ((y2 - y1) * 9 / 10).max(10);
    let w = text_width(font, value);
    icon(sky, windy, right - w - s / 3 - s / 2, (y1 + y2) / 2, s);
    true
}

/// A weather text: its icon before it, the text where the game puts it (alignment bits 1 left,
/// 2 right, 4 centre). True: drawn here.
pub unsafe fn text(x1: i32, y1: i32, x2: i32, y2: i32, flags: u32, font: u32, colour: u32, raw: &[u8]) -> bool {
    let Some((sky, windy)) = read(raw) else { return false };
    let h = y2 - y1;
    if h > 60 {
        return false;
    }
    let s = (h * 8 / 10).max(10);
    let w = text_width(font, raw.trim_ascii());
    let gap = s / 3;
    // where the text starts as the game would draw it
    let start = if flags & 2 != 0 { x2 - 2 - w } else if flags & 4 != 0 { (x1 + x2 - w) / 2 } else { x1 + 2 };
    let c = std::ffi::CString::new(raw).unwrap_or_default();
    let draw = game::<extern "C" fn(i32, i32, i32, i32, u32, u32, u32, *const i8, i32)>(TEXT_BOX_ORIG);
    if flags & 6 == 0 {
        // left aligned: the icon takes the start, the text moves right
        icon(sky, windy, x1 + 2 + s / 2, (y1 + y2) / 2, s);
        draw(x1 + s + gap, y1, x2, y2, flags, font, colour, c.as_ptr(), -1);
    } else {
        draw(x1, y1, x2, y2, flags, font, colour, c.as_ptr(), -1);
        icon(sky, windy, start - gap - s / 2, (y1 + y2) / 2, s);
    }
    true
}
