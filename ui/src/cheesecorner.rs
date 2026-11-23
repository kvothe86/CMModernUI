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
    if let (Some(n), true) = (ours(item), out != 0) {
        load();
        let text = if full == 0 { TITLE.to_string() } else { REPORTS.iter().find(|r| r.0 == n).map_or_else(|| "The report is lost.".to_string(), |r| r.1.clone()) };
        let mut bytes = to_latin1(&text);
        bytes.push(0);
        game::<extern "thiscall" fn(usize) -> i32>(TEXT_CLEAR)(out);
        game::<extern "thiscall" fn(usize, *const u8) -> i32>(TEXT_SET)(out, bytes.as_ptr());
        return 1;
    }
    game::<extern "thiscall" fn(usize, usize, usize, i32, i32) -> i32>(WRITE_ORIG)(item, manager, out, full, flag)
}

/// { 0x782b70 } get_action_button: his report has nobody to reply to.
unsafe extern "thiscall" fn action(item: usize, manager: usize, label: *mut u8, size: i32, screen: *mut usize) -> u8 {
    if ours(item).is_some() {
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

/// His words on the month behind (`month` is the new one): the league's top and bottom, its top
/// scorer, the side in form, and the cups' results of the last five weeks.
unsafe fn review(club: usize, year: i32, month: i32) -> Option<String> {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let (comp, table) = challenge::league_table(club)?;
    let league = latin1(&game_text(rd::<usize>(club + 0x57) + 4, 51)).trim().to_string();
    let past = MONTHS[((month + 10) % 12) as usize];
    let pick = |options: &[&'static str]| options[(year * 12 + month) as usize % options.len()];
    let mut s = String::new();
    s += &pick(&[
        "Good evening. Another month has gone by in the {league}, and as ever I have watched every minute of it so you don't have to.",
        "Here we are again: my look back at {month} in the {league}. Pour yourself something, it was quite a month.",
        "{month} in the {league}, through my eyes. I warn you in advance: not everybody will like what I have to say.",
    ])
    .replace("{league}", &league)
    .replace("{month}", past);
    let base = rd::<usize>(CLUB_TABLE);
    let name = |c: usize| short(c);
    // (before the first round every side has 0 points: no "top" or "bottom" then, night 11:
    // "Accrington and Barnet both have 0 points" on 1 July)
    if !table.is_empty() && table.iter().all(|r| r.1[0] == 0) {
        s += &format!(
            "\n\nNo league football yet. The fixtures are out, {} clubs start from nothing, and until the first whistle it is all friendlies and talk. I trust neither. Ask me again in a month and I will tell you who is for real.",
            table.len()
        );
    } else if table.len() >= 4 {
        let (lead, second, third) = (&table[0], &table[1], &table[2]);
        let gap = lead.1[6] - second.1[6];
        s += "\n\n";
        s += &if gap > 0 {
            format!("At the top, {} lead the way with {} points from {} games, {} clear of {}. {} complete the top three.", name(lead.0), lead.1[6], lead.1[0], gap, name(second.0), name(third.0))
        } else {
            format!("At the top it could not be tighter: {} and {} both have {} points, with {} lurking behind them.", name(lead.0), name(second.0), lead.1[6], name(third.0))
        };
        let n = table.len();
        let (last, prev) = (&table[n - 1], &table[n - 2]);
        s += &format!(" At the other end {} ({} points) and {} ({}) are looking over their shoulders, and frankly I have seen little to make me think that will change soon.", name(last.0), last.1[6], name(prev.0), prev.1[6]);
    }
    // the top scorer of the league
    let mut best: Option<(i32, Vec<u8>, usize)> = None;
    for (c, _) in &table {
        for p in challenge::squad_of(*c) {
            let goals = challenge::season_stat(p, 2, 2) as i32;
            if goals > best.as_ref().map_or(0, |b| b.0) {
                best = Some((goals, staff_name(p), *c));
            }
        }
    }
    if let Some((goals, who, c)) = best {
        s += &format!("\n\n{} of {} leads the scoring charts with {} goals. A player like that wins you points on days you don't deserve them.", latin1(&who), name(c), goals);
    }
    // the side in form: points from its last five league matches; the cups of the last five weeks
    let (day, today_year) = (rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32);
    let start = if day >= 70 { (day - 70, today_year) } else { (day + 295, today_year - 1) };
    let index = |c: usize| ((c - base) / CLUB_SIZE) as i32;
    let mut in_form: Option<(i32, i32, usize, usize)> = None;
    let mut cups: Vec<(i32, i32, i32, i32, i32, i32)> = Vec::new(); // day, comp, home, away, home goals, away goals
    for (c, _) in &table {
        let results = form::played(index(*c), start, (day, today_year), 12);
        let league_games: Vec<&form::Result_> = results.iter().filter(|r| !r.friendly && r.comp == comp).take(5).collect();
        if league_games.len() >= 3 {
            let points: i32 = league_games.iter().map(|r| match r.letter() { b'W' => 3, b'D' => 1, _ => 0 }).sum();
            let wins = league_games.iter().filter(|r| r.letter() == b'W').count() as i32;
            if in_form.is_none_or(|f| points > f.0) {
                in_form = Some((points, wins, *c, league_games.len()));
            }
        }
        for r in results.iter().filter(|r| !r.friendly && r.comp != comp && r.day >= today_year * 366 + day - 35) {
            let g = if r.home { (r.day, r.comp, index(*c), r.opponent, r.goals_for, r.against) } else { (r.day, r.comp, r.opponent, index(*c), r.against, r.goals_for) };
            if !cups.contains(&g) {
                cups.push(g);
            }
        }
    }
    if let Some((points, wins, c, played)) = in_form {
        // (after three rounds "from their last five" read wrong, night 11)
        let games = match played {
            5.. => "their last five".to_string(),
            n => format!("their first {}", ["", "one", "two", "three", "four"][n.min(4)]),
        };
        s += &format!("\n\nThe side in form is {}: {} wins and {} points from {}. Keep an eye on them.", name(c), wins, points, games);
    }
    if !cups.is_empty() {
        s += "\n\nIn the cups:";
        cups.sort();
        for (_, cup, h, a, hg, ag) in cups.iter().rev().take(6) {
            let cup_name = latin1(&form::comp_name(*cup)).trim().to_string();
            s += &format!("\n- {} {}-{} {} ({})", name(base + *h as usize * CLUB_SIZE), hg, ag, name(base + *a as usize * CLUB_SIZE), cup_name);
        }
    }
    s += "\n\n";
    s += pick(&["Until next month. Pete Cheesecorner", "That was the month. See you in four weeks. Pete Cheesecorner", "Make of it what you will. Pete Cheesecorner"]);
    Some(s)
}
