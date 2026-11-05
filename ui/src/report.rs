//! Icons on the Match Report (switch `match_screen`): a ball for a goal, a yellow or red card, an
//! X for a missed penalty, a cross for an injury, arrows for a substitution, left of the line that
//! tells it. The lines are known by the game's own words: the report (R) lines of those events in
//! `Data\events_*.cfg`, `<p1>`, `<t>`, `<Rf>` and the like standing for any name, so every
//! language's file counts. The game draws the minute column after the text: the icons go on at the
//! flip.

use crate::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Goal,
    OwnGoal,
    PenaltyMissed,
    Yellow,
    Red,
    Injury,
    Substitution,
}

/// The events whose report lines get an icon.
const EVENTS: [(&str, Kind); 21] = [
    ("EVENT_GOAL_ONE", Kind::Goal),
    ("EVENT_GOAL_THREE", Kind::Goal),
    ("EVENT_GOLDEN_GOAL", Kind::Goal),
    ("EVENT_SCORED_PENALTY", Kind::Goal),
    ("EVENT_GOAL_TWO", Kind::OwnGoal),
    ("EVENT_MISSED_PENALTY", Kind::PenaltyMissed),
    ("EVENT_SAVES_PENALTY", Kind::PenaltyMissed),
    ("EVENT_PENALTY_HIT_WIDE", Kind::PenaltyMissed),
    ("EVENT_PENALTY_HIT_OVER", Kind::PenaltyMissed),
    ("EVENT_PENALTY_HITS_POST", Kind::PenaltyMissed),
    ("EVENT_PENALTY_HITS_BAR", Kind::PenaltyMissed),
    ("EVENT_YELLOW_CARD", Kind::Yellow),
    ("EVENT_SECOND_YELLOW_CARD", Kind::Red),
    ("EVENT_SENT_OFF", Kind::Red),
    ("EVENT_SENT_OFF_FOR_DELIBERATE_HANDBALL", Kind::Red),
    ("EVENT_SENT_OFF_FOR_PROFESSIONAL_FOUL", Kind::Red),
    ("EVENT_PLAYER_INJURED_FORCED_OFF", Kind::Injury),
    ("EVENT_PLAYER_INJURED_GOES_ON", Kind::Injury),
    ("EVENT_SUBSTITUTION", Kind::Substitution),
    ("EVENT_DOUBLE_SUBSTITUTION", Kind::Substitution),
    ("EVENT_REPLACING_INJURED_PLAYER", Kind::Substitution),
];

/// A report line's pattern: its words between the names, in order.
type Pattern = (Kind, Vec<Vec<u8>>);

/// The report lines of one events file (Latin-1): each `R` line under a `# EVENT_` heading of
/// `EVENTS`, its parts between `|` a line each.
fn patterns_of(text: &[u8]) -> Vec<Pattern> {
    let mut out = Vec::new();
    let mut kind = None;
    for line in text.split(|&c| c == b'\n') {
        let line = line.trim_ascii();
        if let Some(name) = line.strip_prefix(b"# ") {
            let name = name.split(|c| c.is_ascii_whitespace()).next().unwrap_or(b"");
            if name.starts_with(b"EVENT_") {
                kind = EVENTS.iter().find(|e| e.0.as_bytes() == name).map(|e| e.1);
            }
        } else if let (Some(k), Some(said)) = (kind, line.strip_prefix(b"R ")) {
            for part in said.split(|&c| c == b'|') {
                let pieces = pieces(part.trim_ascii());
                // (a line that is only a name says nothing)
                if pieces.iter().any(|p| p.iter().any(u8::is_ascii_alphabetic)) {
                    out.push((k, pieces));
                }
            }
        }
    }
    out
}

/// A line's words between its names (`<..>`, `{..}`), the first and last possibly empty.
fn pieces(said: &[u8]) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new()];
    let mut skip = None;
    for &c in said {
        match (skip, c) {
            (None, b'<') => skip = Some(b'>'),
            (None, b'{') => skip = Some(b'}'),
            (Some(end), c) if c == end => {
                skip = None;
                out.push(Vec::new());
            }
            (Some(_), _) => {}
            (None, c) => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// The line is the pattern's: it starts with its first words, ends with its last, has the others
/// in between in order.
fn fits(pieces: &[Vec<u8>], line: &[u8]) -> bool {
    let (first, last) = (&pieces[0], &pieces[pieces.len() - 1]);
    if pieces.len() == 1 {
        return line == first.as_slice();
    }
    if !line.starts_with(first) || !line.ends_with(last) || line.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &line[first.len()..line.len() - last.len()];
    for p in pieces[1..pieces.len() - 1].iter().filter(|p| !p.is_empty()) {
        match rest.windows(p.len()).position(|w| w == p.as_slice()) {
            Some(at) => rest = &rest[at + p.len()..],
            None => return false,
        }
    }
    true
}

/// Every events file's patterns, read once.
fn patterns() -> &'static [Pattern] {
    static ALL: OnceLock<Vec<Pattern>> = OnceLock::new();
    ALL.get_or_init(|| {
        let files = std::fs::read_dir("Data").map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default();
        let all: Vec<Pattern> = files
            .iter()
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("events_") && n.ends_with(".cfg")))
            .filter_map(|p| std::fs::read(p).ok())
            .flat_map(|t| patterns_of(&t))
            .collect();
        log(&format!("match report: {} event lines from Data\\events_*.cfg", all.len()));
        all
    })
}

/// What a report line tells, if it is one of `EVENTS`.
pub fn kind_of(line: &[u8]) -> Option<Kind> {
    let line = line.trim_ascii();
    patterns().iter().find(|p| fits(&p.1, line)).map(|p| p.0)
}

/// This frame's icons (centre, radius, kind).
static mut ICONS: Vec<(i32, i32, i32, Kind)> = Vec::new();

/// A text the game draws on a match screen: a report line (one space first; the match's own
/// event list has two) gets its icon left of it, in the minute column's right end.
#[allow(static_mut_refs)]
pub unsafe fn note_text(x1: i32, y1: i32, x2: i32, y2: i32, s: &[u8]) {
    if x2 - x1 < 600 || y2 - y1 > 40 || !s.starts_with(b" ") || s.starts_with(b"  ") || ICONS.len() >= 64 {
        return;
    }
    if let Some(kind) = kind_of(s) {
        let r = ((y2 - y1) * 36 / 100).max(4);
        ICONS.push((x1 - 8 - r, (y1 + y2) / 2, r, kind));
    }
}

/// The flip pass: the icons of this frame.
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    for (cx, cy, r, kind) in std::mem::take(&mut ICONS) {
        match kind {
            Kind::Goal | Kind::OwnGoal => _ = event_icon(cx, cy, r, b"x"),
            Kind::Red => _ = event_icon(cx, cy, r, b"x sent off"),
            Kind::PenaltyMissed => _ = event_icon(cx, cy, r, b"x missed"),
            Kind::Injury => _ = event_icon(cx, cy, r, b"x injured"),
            Kind::Yellow => {
                let hw = r * 7 / 10;
                fill(cx - hw, cy - r, cx + hw, cy + r + 1, [0xe8, 0xc4, 0x30]);
            }
            // (Segoe's "switch": two arrows)
            Kind::Substitution => draw_icon(0xE8AB, cx, cy - r, 2 * r, [0x9c, 0xc4, 0xe8]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_lines_known() {
        let cfg = b"# EVENT_GOAL_ONE\nG <p1> scores!!\nR <p1> scored !!\nR <p1> scored for <t> !!\n# EVENT_SENT_OFF\nR <p1> was sent off!!!\nR <Rf> showed <p1> the red card\n# EVENT_SECOND_YELLOW_CARD\nR <Rf> called <p1> over and showed him his second yellow card|<p1> took an early bath !\n# EVENT_YELLOW_CARD\nR <p1> was shown a yellow card\n# EVENT_SUBSTITUTION\nR <p1> came on for <p2>\n# EVENT_CORNER\nR Corner\n";
        let p = patterns_of(cfg);
        let kind = |l: &[u8]| p.iter().find(|x| fits(&x.1, l)).map(|x| x.0);
        assert_eq!(kind(b"Smeets scored for Rijnsburgse Boys !!"), Some(Kind::Goal));
        assert_eq!(kind(b"Kool scored !!"), Some(Kind::Goal));
        assert_eq!(kind(b"Linssen was sent off!!!"), Some(Kind::Red));
        assert_eq!(kind(b"The referee showed van Dijk the red card"), Some(Kind::Red));
        assert_eq!(kind(b"Snip took an early bath !"), Some(Kind::Red));
        assert_eq!(kind(b"Porte was shown a yellow card"), Some(Kind::Yellow));
        assert_eq!(kind(b"Graef came on for Keunen"), Some(Kind::Substitution));
        assert_eq!(kind(b"Corner"), None);
        assert_eq!(kind(b"Paul Paauwe had a go on goal"), None);
        assert_eq!(kind(b"Smeets was first to the rebound"), None);
    }

    /// The game's own English file against a real report (the trace of 2026-11-04): only the goal
    /// and the red card. `cargo test ... report -- --ignored` (needs E:\CM0102\Data).
    #[test]
    #[ignore]
    fn real_report() {
        let p = patterns_of(&std::fs::read(r"E:\CM0102\Data\events_eng.cfg").unwrap());
        let lines = ["A freekick was awarded", "Corner taken by Linssen", "Corner-kick to VVV", "de Nooijer pushed Werner Smits", "Driven in low by Linssen", "Gerrie Schaap had done enough to put him off", "Graef got above van den Herik to win the ball", "Header by Graef", "Jacobs pushed it away", "Kool took the freekick", "Linssen pushed Postma away !!", "Mark Adams stuck out his leg to bring down Porte", "Paul Paauwe got it all wrong sending the ball harmlessly over the bar", "Paul Paauwe had a go on goal", "Paul Paauwe struck the ball with fury", "Porte got up to head at goal", "Postma made a hash of the catch", "Postma turned the ball behind!", "Simons tackled Martens", "Smeets had the chance", "Smeets was first to the rebound", "The ball crossed the line for a throw-in", "The post denied Graef", "VVV Kicked Off", "Werner Smits won the ball in the air from de Graaf"];
        for l in lines {
            assert_eq!(p.iter().find(|x| fits(&x.1, l.as_bytes())).map(|x| x.0), None, "{l}");
        }
        let kind = |l: &str| p.iter().find(|x| fits(&x.1, l.as_bytes())).map(|x| x.0);
        assert_eq!(kind("Smeets scored for Rijnsburgse Boys !!"), Some(Kind::Goal));
        assert_eq!(kind("Linssen was sent off!!!"), Some(Kind::Red));
    }
}
