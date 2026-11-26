//! The Pete Cheesecorner report (user 2026-11-23): a real news item in each human manager's inbox at
//! the start of every month, his look back at the month in that manager's league and cups (in
//! general, not about the manager's own team), with his portrait as the item's badge.
//!
//! The item is the game's manager-to-manager message (type 0x1388), as Rust Tweaks' intake message:
//! `NewsItem::NewsItem` 0x77bbe0, slots at +5 (0 the manager, 1 private, 2 and 20 our marks, 3..
//! the short text the game shows on its own, 21 the report's number), `News::send_to_club_manager`
//! 0x7861e0 (humans only). Its words come from our hook on `NewsItem::write_text` 0x77bc80, which
//! passes every other item on (to Rust Tweaks' hook, then the game: its own check only looks past
//! the entry, and with intake_news = 2 its message stays on). The texts are kept per game in
//! `cmui_cheesecorner_<game>.txt`, so a report reads the same each time it is opened.
use crate::*;

const NEWS_NEW: usize = 0x77bbe0; // NewsItem::NewsItem(this, type, important), thiscall
const SEND: usize = 0x7861e0; // News::send_to_club_manager(news, item, club), thiscall
const NEWS: usize = 0xb7431c; // the news object
const WRITE_TEXT: usize = 0x77bc80;
const ACTION: usize = 0x782b70;
const TEXT_CLEAR: usize = 0x4afa40;
const TEXT_SET: usize = 0x4ae660;
const MARK: u32 = 0x5341_414b; // (kept from the first saves that carry the item)
const MARK2: u32 = 0x4b45_4f48; //
pub const TITLE: &str = "Pete Cheesecorner report";

static mut WRITE_ORIG: usize = 0;
static mut ACTION_ORIG: usize = 0;
/// The reports: number, text; the month last posted (year x 12 + month); the file.
static mut REPORTS: Vec<(u32, String)> = Vec::new();
static mut LAST: i32 = 0;
static mut FILE: String = String::new();

/// His portrait, 64 x 64 RGB (a generated face, cut to a square).
const PHOTO: &[u8; 64 * 64 * 3] = include_bytes!("cheesecorner.rgb");

/// The portrait in a box of `size` (the news list's badge, the story's crest place).
pub unsafe fn photo(x: i32, y: i32, size: i32) {
    let size = size.max(8);
    blend(x, y, size, size, |col, row| {
        let (c, r) = ((col * 64 / size).clamp(0, 63) as usize, (row * 64 / size).clamp(0, 63) as usize);
        let p = &PHOTO[(r * 64 + c) * 3..(r * 64 + c) * 3 + 3];
        (rgb([p[0], p[1], p[2]]), 255)
    });
}

/// A news headline that is his.
pub fn is_his(t: &[u8]) -> bool {
    t.starts_with(TITLE.as_bytes())
}

pub unsafe fn install() {
    WRITE_ORIG = hook_entry(WRITE_TEXT, &[0x6a, 0xff, 0x64, 0xa1, 0, 0, 0, 0], write_text as usize);
    ACTION_ORIG = hook_entry(ACTION, &[0x64, 0xa1, 0, 0, 0, 0], action as usize);
    log(&format!("cheesecorner: news writer {}, action {}", WRITE_ORIG != 0, ACTION_ORIG != 0));
}

unsafe fn ours(item: usize) -> Option<u32> {
    let slot = |k: usize| ((item + 5 + 4 * k) as *const u32).read_unaligned();
    (item != 0 && mapped(item, 0xe0) && rd::<i32>(item) == 0x1388 && slot(2) == MARK && slot(20) == MARK2).then(|| slot(21))
}

/// { 0x77bc80 } write_text(item, manager, out, full, flag): ours gets its title or its report.
unsafe extern "thiscall" fn write_text(item: usize, manager: usize, out: usize, full: i32, flag: i32) -> i32 {
    #[allow(static_mut_refs)]
    // (and the staff's monthly reports, `staff.rs`, the same kind of item)
    let mine = ours(item).map(|n| {
        load();
        if full == 0 { TITLE.to_string() } else { REPORTS.iter().find(|r| r.0 == n).map_or_else(|| "The report is lost.".to_string(), |r| r.1.clone()) }
    });
    if let (Some(text), true) = (mine.or_else(|| crate::staff::text(item, full != 0)), out != 0) {
        let mut bytes = to_latin1(&text);
        bytes.push(0);
        game::<extern "thiscall" fn(usize) -> i32>(TEXT_CLEAR)(out);
        game::<extern "thiscall" fn(usize, *const u8) -> i32>(TEXT_SET)(out, bytes.as_ptr());
        if full != 0 {
            crate::staff::add_links(item, out); // (a staff report's player names, clickable)
        }
        return 1;
    }
    game::<extern "thiscall" fn(usize, usize, usize, i32, i32) -> i32>(WRITE_ORIG)(item, manager, out, full, flag)
}

/// { 0x782b70 } get_action_button: his report has nobody to reply to.
unsafe extern "thiscall" fn action(item: usize, manager: usize, label: *mut u8, size: i32, screen: *mut usize) -> u8 {
    // (a staff report: its Full Report button, as the next opposition's scout report)
    if let Some(view) = crate::staff::button(item, manager, label, size, screen) {
        return view;
    }
    if ours(item).is_some() || crate::staff::ours(item).is_some() {
        if !label.is_null() {
            *label = 0;
        }
        if !screen.is_null() {
            *screen = 0;
        }
        return 0;
    }
    game::<extern "thiscall" fn(usize, usize, *mut u8, i32, *mut usize) -> u8>(ACTION_ORIG)(item, manager, label, size, screen)
}

#[allow(static_mut_refs)]
unsafe fn load() {
    let Some(game) = challenge::game_number() else { return };
    let file = format!("cmui_cheesecorner_{game:08x}.txt");
    if file == FILE {
        return;
    }
    REPORTS.clear();
    LAST = 0;
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    for l in text.lines() {
        if let Some(v) = l.strip_prefix("LAST\t") {
            LAST = v.parse().unwrap_or(0);
        } else if let Some((n, t)) = l.split_once('\t') {
            if let Ok(n) = n.parse() {
                REPORTS.push((n, t.replace('|', "\n")));
            }
        }
    }
    FILE = file;
}

#[allow(static_mut_refs)]
unsafe fn save() {
    let mut out = format!("LAST\t{LAST}\n");
    for (n, t) in &REPORTS {
        out += &format!("{n}\t{}\n", t.replace('\n', "|"));
    }
    let _ = std::fs::write(&FILE, out);
}

/// The diary's daily tick (host): on the first day seen of a new month, a report to every human
/// manager on his league and cups. The game's first month gets none (no month behind it yet).
#[allow(static_mut_refs)]
pub unsafe fn tick(year: i32, month: i32) {
    load();
    if FILE.is_empty() {
        return;
    }
    let now = year * 12 + month;
    if LAST == 0 {
        LAST = now;
        save();
        return;
    }
    if now <= LAST {
        return;
    }
    LAST = now;
    let news = rd::<usize>(NEWS);
    for (staff, _) in challenge::human_list() {
        let club = rd::<usize>(staff + 0x39);
        if news == 0 || !readable(club, 0x5b) {
            continue;
        }
        let Some(text) = review(club, year, month) else { continue };
        let n = REPORTS.iter().map(|r| r.0).max().unwrap_or(0) + 1;
        REPORTS.push((n, text));
        let mut item = [0u8; 0xe0];
        let it = item.as_mut_ptr() as usize;
        game::<extern "thiscall" fn(usize, i32, i32) -> usize>(NEWS_NEW)(it, 0x1388, 1);
        let slot = |k: usize, v: u32| ((it + 5 + 4 * k) as *mut u32).write_unaligned(v);
        slot(0, rd::<i32>(staff) as u32);
        slot(1, 1);
        slot(2, MARK);
        let short = to_latin1("Pete Cheesecorner's monthly review");
        std::ptr::copy_nonoverlapping(short.as_ptr(), (it + 5 + 12) as *mut u8, short.len().min(67));
        slot(20, MARK2);
        slot(21, n);
        game::<extern "thiscall" fn(usize, usize, usize) -> i32>(SEND)(news, it, club);
        log(&format!("cheesecorner: report {n} to {}", latin1(&game_text(club + 4, 51))));
    }
    save();
}

/// A club's short name, else its name.
unsafe fn short(club: usize) -> String {
    let s = latin1(&game_text(club + 0x38, 26)).trim().to_string();
    if s.is_empty() { latin1(&game_text(club + 4, 51)).trim().to_string() } else { s }
}

/// What the month in a league came to, for his words (`words`): read from the game in `review`.
#[derive(Default)]
struct Facts {
    league: String,
    month: &'static str,
    /// the table, top first: name, reputation, played, won, drawn, lost, for, against, points
    rows: Vec<(String, i32, [i32; 7])>,
    /// the reader's own club's place in it
    own: Option<usize>,
    /// the two top scorers: goals, player, club
    scorers: Vec<(i32, String, String)>,
    /// the best and worst run of league games (up to five): club, points, wins, games
    best_form: Option<(String, i32, i32, usize)>,
    worst_form: Option<(String, i32, i32, usize)>,
    /// the cups of the last five weeks: home, home goals, away goals, away, cup, home and away reputation
    cups: Vec<(String, i32, i32, String, String, i32, i32)>,
    /// which words this month (the month and the league)
    seed: u32,
}

/// One of `options`, the same for a month and a part (`salt`), another the next month.
pub fn pick<'a>(seed: u32, salt: u32, options: &[&'a str]) -> &'a str {
    let mut h = (seed ^ salt.wrapping_mul(0x9e37_79b9)).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    options[h as usize % options.len()]
}

/// His report from the facts: an opening, the top (a runaway leader, a tight race, a surprise, a
/// big name in trouble), the bottom (no win yet, cut adrift), the scorers (a race, a goal a game),
/// the form (perfect, in free fall), the cups (a giant killed), now and then a word on the
/// reader's own club, and a sign-off.
fn words(f: &Facts) -> String {
    let r = |salt: u32, options: &[&'static str]| pick(f.seed, salt, options).to_string();
    let fill = |s: String, pairs: &[(&str, String)]| pairs.iter().fold(s, |s, (k, v)| s.replace(k, v));
    let mut s = fill(
        r(1, &[
            "Good evening. Another month has gone by in the {league}, and as ever I have watched every minute of it so you don't have to.",
            "Here we are again: my look back at {month} in the {league}. Pour yourself something, it was quite a month.",
            "{month} in the {league}, through my eyes. I warn you in advance: not everybody will like what I have to say.",
            "Another {month} in the books. I have been to the grounds, eaten the pies and read the programmes, so here is the {league} as I saw it.",
            "Settle in. {month} in the {league} gave us plenty to talk about, and I intend to talk about all of it.",
            "My notebook is full again. {month} in the {league}, the good, the bad and the frankly baffling.",
        ]),
        &[("{league}", f.league.clone()), ("{month}", f.month.to_string())],
    );
    let n = f.rows.len();
    let rounds = f.rows.iter().map(|x| x.2[0]).max().unwrap_or(0);
    let name = |i: usize| f.rows[i].0.clone();
    // the clubs the table talk names: the form lines skip them (night 12: NAC led on 10 points,
    // then "Watch NAC: 10 points"; Helmond had 0 points, then "Helmond have taken 0 points")
    let told: Vec<String> = if n >= 4 && rounds > 0 { [0, n - 2, n - 1].map(name).to_vec() } else { Vec::new() };
    if n > 0 && rounds == 0 {
        s += "\n\n";
        s += &r(2, &[
            "No league football yet. The fixtures are out, {n} clubs start from nothing, and until the first whistle it is all friendlies and talk. I trust neither. Ask me again in a month and I will tell you who is for real.",
            "Not a ball kicked in anger yet. {n} clubs, {n} chairmen promising the earth, and every one of them level on nothing. Enjoy it while it lasts.",
            "Pre-season, the time of year when every signing is a bargain and every manager has a plan. {n} clubs are about to find out whose plan survives the first month.",
        ])
        .replace("{n}", &n.to_string());
    } else if n >= 4 {
        let (lead, second) = (&f.rows[0], &f.rows[1]);
        let gap = lead.2[6] - second.2[6];
        let pairs = [("{1}", name(0)), ("{2}", name(1)), ("{3}", name(2)), ("{p}", lead.2[6].to_string()), ("{g}", lead.2[0].to_string()), ("{gap}", gap.to_string())];
        let top = match gap {
            7.. if rounds >= 6 => r(3, &[
                "{1} are running away with it: {p} points from {g} games and {gap} clear of {2}. Unless somebody finds a second wind, the rest are playing for second.",
                "It is getting lonely at the top for {1}. {gap} points clear of {2} after {g} games, and they show no sign of slowing down.",
                "{1} ({p} points) are out of sight already, {gap} ahead of {2}. {3} complete the top three, for what that is worth.",
            ]),
            3.. => r(4, &[
                "At the top, {1} lead the way with {p} points from {g} games, {gap} clear of {2}. {3} complete the top three.",
                "{1} sit on top with {p} points, a cushion of {gap} over {2}. Comfortable, but not safe: {3} are still in touch.",
                "{1} are the team to catch: {p} points, {gap} more than {2}, with {3} third.",
            ]),
            1.. => r(5, &[
                "{1} lead, but only just: {p} points, {gap} ahead of {2}, who are breathing down their necks. {3} are watching both.",
                "A narrow lead for {1} ({p} points). {2} are {gap} behind, {3} close enough to matter.",
                "{1} top the table by {gap}, from {2}. One bad afternoon and that is gone.",
            ]),
            _ => r(6, &[
                "At the top it could not be tighter: {1} and {2} both have {p} points, with {3} lurking behind them.",
                "Level at the top: {1} and {2} on {p} points each, separated only by the goals. {3} would love them both to slip.",
                "You could not put a cigarette paper between {1} and {2}, both on {p} points. {3} are next in line.",
            ]),
        };
        s += "\n\n";
        s += &fill(top, &pairs);
        // a surprise leader: not among the league's five biggest names
        let bigger = f.rows.iter().filter(|x| x.1 > lead.1).count();
        if bigger >= 5 && rounds >= 4 {
            s += " ";
            s += &fill(
                r(7, &[
                    "And let us be honest: nobody had {1} up there in August. I certainly did not.",
                    "{1} were nobody's favourites. They are making a lot of people look foolish, me included.",
                    "The bigger clubs will be asking how {1} got there. The answer, I suspect, is hard work and no fear.",
                ]),
                &pairs,
            );
        }
        // a big name in trouble: the best known club in the bottom half, one of the league's three biggest
        if let Some(i) = (n / 2..n).max_by_key(|&i| f.rows[i].1).filter(|&i| rounds >= 4 && f.rows.iter().filter(|x| x.1 > f.rows[i].1).count() < 3) {
            s += "\n\n";
            s += &fill(
                r(8, &[
                    "Spare a thought for {c}, down in {pos} place. A club of that size has no business being there.",
                    "{c} in {pos}? I had to read the table twice. The board will not be reading it at all, if they have any sense.",
                    "Whatever is going on at {c}, it is not working. {pos} is not where anyone expected them.",
                ]),
                &[("{c}", name(i)), ("{pos}", ordinal(i + 1))],
            );
        }
        // the bottom
        let (last, prev) = (&f.rows[n - 1], &f.rows[n - 2]);
        let adrift = f.rows.get(n.saturating_sub(4)).map_or(0, |x| x.2[6]) - last.2[6];
        let pairs = [("{l}", name(n - 1)), ("{lp}", last.2[6].to_string()), ("{p2}", name(n - 2)), ("{pp}", prev.2[6].to_string()), ("{g}", last.2[0].to_string()), ("{adrift}", adrift.to_string())];
        let bottom = if last.2[1] == 0 && last.2[0] >= 4 {
            r(9, &[
                "At the other end {l} are still waiting for a first win after {g} games. {lp} points. It is starting to look like a long winter.",
                "{l} have not won a league game yet: {g} played, {lp} points. Somebody at that club needs to find an answer, and quickly.",
                "No wins in {g} for {l}. I have seen this before, and it rarely ends well for the man in the dugout.",
            ])
        } else if adrift >= 6 && last.2[0] >= 6 {
            r(10, &[
                "At the bottom {l} are cut adrift, {adrift} points from safety with {lp}. It will take something special now.",
                "{l} are {adrift} points off the clubs above them. I would like to tell you there is hope. I am not sure there is.",
                "Bottom of the pile and drifting: {l} ({lp} points), {adrift} behind the pack. {p2} are not much happier.",
            ])
        } else {
            r(11, &[
                "At the other end {l} ({lp} points) and {p2} ({pp}) are looking over their shoulders, and frankly I have seen little to make me think that will change soon.",
                "Down at the bottom {l} have {lp} points and {p2} {pp}. Plenty of football left, but not plenty of time.",
                "{l} prop up the table with {lp} points, {p2} just above them on {pp}. Both need a run, and soon.",
                "Trouble at the foot of the table for {l} ({lp}) and {p2} ({pp}). Neither looks like a side about to turn it round.",
            ])
        };
        s += " ";
        s += &fill(bottom, &pairs);
    }
    // the scorers
    if let Some((goals, who, club)) = f.scorers.first() {
        let close = f.scorers.get(1).filter(|x| goals - x.0 <= 1 && x.0 > 0);
        let pairs = [("{who}", who.clone()), ("{c}", club.clone()), ("{n}", goals.to_string())];
        let line = if let Some((g2, w2, c2)) = close {
            fill(
                r(12, &[
                    "The scoring race is on: {who} of {c} has {n}, {w2} of {c2} {n2} right behind him. My money is on neither, which usually means one of them wins it.",
                    "{who} ({c}) leads the scorers with {n}, but {w2} of {c2} is only on {n2}. That one will run and run.",
                    "Two men fighting for the golden boot: {who} of {c} on {n}, {w2} of {c2} on {n2}.",
                ]),
                &[("{w2}", w2.clone()), ("{c2}", c2.clone()), ("{n2}", g2.to_string())],
            )
        } else if *goals >= rounds && rounds >= 5 {
            r(13, &[
                "{who} of {c} has {n} goals already, better than one a game. Defenders in this league are having nightmares about him.",
                "A goal a game and then some: {who} ({c}), {n} so far. Somebody bigger will come calling, mark my words.",
                "{n} goals for {who} of {c}. At this rate he will need a bigger trophy cabinet.",
            ])
        } else {
            r(14, &[
                "{who} of {c} leads the scoring charts with {n} goals. A player like that wins you points on days you don't deserve them.",
                "Top of the scorers: {who} of {c}, with {n}. Not always pretty, always there.",
                "{who} ({c}) is the man in form in front of goal: {n} so far.",
                "The league's top scorer is {who} of {c}, {n} goals. Keep an eye on him in the transfer window.",
            ])
        };
        s += "\n\n";
        s += &fill(line, &pairs);
    }
    // the form
    if let Some((c, points, wins, games)) = f.best_form.as_ref().filter(|b| !told.contains(&b.0)) {
        let of = if *games >= 5 { "their last five".to_string() } else { format!("their first {}", ["", "one", "two", "three", "four"][(*games).min(4)]) };
        let pairs = [("{c}", c.clone()), ("{w}", wins.to_string()), ("{p}", points.to_string()), ("{of}", of)];
        let line = if *wins as usize == *games && *games >= 4 {
            r(15, &[
                "Nobody is hotter than {c}: {w} wins out of {w}. Perfect. I would not want to play them right now.",
                "{c} have won every one of {of}. Form like that is not luck.",
                "Played {w}, won {w}. {c} are flying.",
            ])
        } else {
            r(16, &[
                "The side in form is {c}: {w} wins and {p} points from {of}. Keep an eye on them.",
                "{c} are the form team, {p} points from {of}. Quietly, they are building something.",
                "Watch {c}: {w} wins, {p} points from {of}. Momentum is a wonderful thing.",
                "If I had to back one side right now, it would be {c}. {p} points from {of}.",
            ])
        };
        s += "\n\n";
        s += &fill(line, &pairs);
    }
    if let Some((c, points, _, games)) = f.worst_form.as_ref().filter(|w| w.1 <= 1 && w.3 >= 4 && !told.contains(&w.0)) {
        let pairs = [("{c}", c.clone()), ("{p}", points.to_string()), ("{g}", games.to_string())];
        s += " ";
        s += &fill(
            r(17, &[
                "At the other end of the form book, {c} have {p} points from their last {g}. Free fall.",
                "{c}, meanwhile, have taken {p} points from {g}. Somebody switch the lights back on.",
                "Spare a thought for {c}: {p} points from their last {g}. The supporters deserve better.",
            ]),
            &pairs,
        );
    }
    // the cups
    if !f.cups.is_empty() {
        s += "\n\n";
        s += &r(18, &["In the cups:", "Cup football, the bit everybody secretly loves:", "And in the cups:", "The cup results:"]);
        for (h, hg, ag, a, cup, _, _) in f.cups.iter().take(6) {
            s += &format!("\n- {h} {hg}-{ag} {a} ({cup})");
        }
        // a giant killed: the winner far less known than the loser
        let shock = f.cups.iter().take(6).filter_map(|(h, hg, ag, a, _, hr, ar)| match hg.cmp(ag) {
            std::cmp::Ordering::Greater if ar - hr >= 1500 => Some((h.clone(), a.clone(), ar - hr)),
            std::cmp::Ordering::Less if hr - ar >= 1500 => Some((a.clone(), h.clone(), hr - ar)),
            _ => None,
        }).max_by_key(|x| x.2);
        if let Some((winner, loser, _)) = shock {
            s += "\n\n";
            s += &fill(
                r(19, &[
                    "The shock of the month: {w} knocking out {l}. That is what the cups are for.",
                    "Hats off to {w}. Beating {l} will be talked about in that town for years.",
                    "{l} beaten by {w}. Somebody at {l} has some explaining to do.",
                ]),
                &[("{w}", winner), ("{l}", loser)],
            );
        }
    }
    // now and then a word on the reader's own club (every other month)
    if let Some(i) = f.own.filter(|_| rounds >= 3 && f.seed % 2 == 0 && n >= 6) {
        let pairs = [("{c}", name(i)), ("{pos}", ordinal(i + 1))];
        let line = if i < 3 {
            r(20, &[
                "And a word for {c}: {pos}, and nobody laughing at them any more.",
                "I notice {c} up in {pos}. I am told the manager reads this column. Do not get carried away.",
                "{c} in {pos}. Steady, steady.",
            ])
        } else if i + 3 >= n {
            r(21, &[
                "{c} in {pos} will be worrying a few people at that club. It is not too late, but the clock is ticking.",
                "A word for {c}, down in {pos}: I have seen sides escape from worse. I have also seen plenty that did not.",
                "{c} need points, and they need them now. {pos} is no place to spend the winter.",
            ])
        } else {
            r(22, &[
                "{c} sit in {pos}, neither here nor there. A good run either way would tell us a lot.",
                "Mid-table for {c}, {pos}. Safe, solid, and I suspect a little restless.",
                "{c} in {pos}: quiet, steady. Sometimes that is exactly what a club needs.",
            ])
        };
        s += "\n\n";
        s += &fill(line, &pairs);
    }
    s += "\n\n";
    s += &r(23, &[
        "Until next month. Pete Cheesecorner",
        "That was the month. See you in four weeks. Pete Cheesecorner",
        "Make of it what you will. Pete Cheesecorner",
        "I will be back when the calendar turns. Pete Cheesecorner",
        "Write in, by all means. I will not read it. Pete Cheesecorner",
        "Same time next month. Pete Cheesecorner",
    ]);
    // ("1 points", "1 goals": the game, 2026-11-23)
    s.replace(" 1 points", " 1 point").replace("(1 points)", "(1 point)").replace(" 1 goals", " 1 goal").replace(" 1 wins", " 1 win")
}

/// 1st, 2nd, 3rd, 4th ... 21st.
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// His words on the month behind (`month` is the new one): the facts read from the game, then
/// `words`.
unsafe fn review(club: usize, year: i32, month: i32) -> Option<String> {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let (comp, table) = challenge::league_table(club)?;
    let base = rd::<usize>(CLUB_TABLE);
    let rep = |c: usize| rd::<i16>(c + 0x80) as i32;
    let mut f = Facts {
        league: latin1(&game_text(rd::<usize>(club + 0x57) + 4, 51)).trim().to_string(),
        month: MONTHS[((month + 10) % 12) as usize],
        rows: table.iter().map(|(c, v)| (short(*c), rep(*c), *v)).collect(),
        own: table.iter().position(|(c, _)| *c == club),
        seed: (year * 12 + month) as u32 ^ (comp as u32).wrapping_mul(7919),
        ..Default::default()
    };
    // the two top scorers of the league
    let mut scorers: Vec<(i32, String, String)> = Vec::new();
    for (c, _) in &table {
        for p in challenge::squad_of(*c) {
            let goals = challenge::season_stat(p, 2, 2) as i32;
            if goals > 0 {
                scorers.push((goals, latin1(&staff_name(p)).into_owned(), short(*c)));
            }
        }
    }
    scorers.sort_by(|a, b| b.0.cmp(&a.0));
    scorers.truncate(2);
    f.scorers = scorers;
    // the form (up to five league games) and the cups of the last five weeks
    let (day, today_year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 70 { (day - 70, today_year) } else { (day + 295, today_year - 1) };
    let index = |c: usize| ((c - base) / CLUB_SIZE) as i32;
    let mut cups: Vec<(i32, i32, i32, i32, i32, i32)> = Vec::new(); // day, comp, home, away, home goals, away goals
    for (c, _) in &table {
        let results = form::played(index(*c), start, (day, today_year), 12);
        let league_games: Vec<&form::Result_> = results.iter().filter(|r| !r.friendly && r.comp == comp).take(5).collect();
        if league_games.len() >= 3 {
            let points: i32 = league_games.iter().map(|r| match r.letter() { b'W' => 3, b'D' => 1, _ => 0 }).sum();
            let wins = league_games.iter().filter(|r| r.letter() == b'W').count() as i32;
            let run = (short(*c), points, wins, league_games.len());
            if f.best_form.as_ref().is_none_or(|b| points > b.1) {
                f.best_form = Some(run.clone());
            }
            if f.worst_form.as_ref().is_none_or(|w| points < w.1) {
                f.worst_form = Some(run);
            }
        }
        for r in results.iter().filter(|r| !r.friendly && r.comp != comp && r.day >= today_year * 366 + day - 35) {
            let g = if r.home { (r.day, r.comp, index(*c), r.opponent, r.goals_for, r.against) } else { (r.day, r.comp, r.opponent, index(*c), r.against, r.goals_for) };
            if !cups.contains(&g) {
                cups.push(g);
            }
        }
    }
    cups.sort();
    let club_at = |i: i32| base + i as usize * CLUB_SIZE;
    f.cups = cups
        .iter()
        .rev()
        .map(|(_, cup, h, a, hg, ag)| (short(club_at(*h)), *hg, *ag, short(club_at(*a)), latin1(&form::comp_name(*cup)).trim().to_string(), rep(club_at(*h)), rep(club_at(*a))))
        .collect();
    Some(words(&f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, rep: i32, p: i32, w: i32, d: i32, pts: i32) -> (String, i32, [i32; 7]) {
        (name.into(), rep, [p, w, d, p - w - d, 0, 0, pts])
    }

    #[test]
    fn his_words_vary_and_follow_the_table() {
        let league = |seed| Facts {
            league: "Test League".into(),
            month: "October",
            rows: vec![
                row("Minnows", 1000, 10, 9, 1, 28),
                row("Middling", 3000, 10, 5, 2, 17),
                row("Third", 4000, 10, 5, 1, 16),
                row("Big Club", 8000, 10, 4, 1, 13),
                row("Giants", 9000, 10, 3, 3, 12),
                row("Fallen", 9500, 10, 2, 2, 8),
                row("Bigger", 7000, 10, 2, 2, 8),
                row("Rock Bottom", 500, 10, 0, 1, 1),
            ],
            own: Some(7),
            scorers: vec![(11, "A Striker".into(), "Minnows".into()), (6, "B Striker".into(), "Giants".into())],
            best_form: Some(("Minnows".into(), 15, 5, 5)),
            worst_form: Some(("Rock Bottom".into(), 0, 0, 5)),
            cups: vec![("Minnows".into(), 2, 1, "Giants".into(), "The Cup".into(), 1000, 9000)],
            seed,
        };
        let s = words(&league(4));
        // the leader, the bottom without a win, the scorer, the giant killed, the reader's club
        // (an even seed), the big name in trouble, signed
        for part in ["Minnows", "Rock Bottom", "A Striker", "Giants", "8th", "Fallen", "Pete Cheesecorner"] {
            assert!(s.contains(part), "{part} missing in:\n{s}");
        }
        assert!(!s.contains('{'), "a hole left in:\n{s}");
        // the form lines skip the leader and the bottom club, named already (night 12)
        for k in 0..24 {
            let s = words(&league(k));
            for said in ["hotter than", "won every one", "are flying", "Free fall", "lights back on", "deserve better"] {
                assert!(!s.contains(said), "{said} in:\n{s}");
            }
        }
        // the openings change month by month
        let openings: std::collections::HashSet<String> = (0..24).map(|k| words(&league(k)).lines().next().unwrap().to_string()).collect();
        assert!(openings.len() >= 5, "{openings:?}");
        // before the first round: no table talk
        let pre = Facts { rows: vec![row("A", 1, 0, 0, 0, 0), row("B", 1, 0, 0, 0, 0)], best_form: None, worst_form: None, scorers: Vec::new(), cups: Vec::new(), ..league(1) };
        assert!(!words(&pre).contains("points from"));
        assert_eq!((ordinal(1), ordinal(2), ordinal(13), ordinal(22)), ("1st".into(), "2nd".into(), "13th".into(), "22nd".into()));
    }
}
