//! The club dashboard (switch `export_button`, the club screen's Export): a page in the look of
//! the CoreUI admin themes, read straight from the game's memory, not from the printout. The page
//! is `dash.html` (one file, CSS and script inline, charts drawn by its own script: it opens
//! offline); this module only writes its DATA block as JSON. Mockup and design:
//! `export_preview\dashboard_mockup.html`.
//!
//! What is read: the club's league table (`challenge::league_table`), every league club's
//! fixtures of the season (`form::fixtures`, the place after each matchday worked out from them),
//! the finance record (`finreport::figures`), the squad (staff record, player record, main
//! contract, season stats, attributes, condition), the transfer manager's rows of the season.

use crate::*;

const TEMPLATE: &str = include_str!("dash.html");
const TRANSFER_MANAGER: usize = 0xde1f54; // past transfers: +0x862 rows of 37 bytes, +0x86a count
const TODAY: usize = 0xae2c90;

/// A JSON string (the game's Latin-1 text; 0x80 is the euro sign in cp1252). "</" is cut so the
/// data cannot end the script block.
fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            '\u{80}' => o.push('\u{20ac}'),
            '<' => o += "\\u003c",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o + "\""
}

fn txt(b: &[u8]) -> String {
    latin1(b).into_owned()
}

/// "14 Feb" from a day of the year (0-based) and year.
fn short_date(day: i32, year: i32) -> String {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut left = day.max(0);
    for (m, len) in [31, 28 + leap as i32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31].into_iter().enumerate() {
        if left < len {
            return format!("{} {}", left + 1, MONTHS[m]);
        }
        left -= len;
    }
    format!("31 Dec")
}

unsafe fn club_at(index: i32) -> usize {
    let c = rd::<usize>(CLUB_TABLE) + index.max(0) as usize * CLUB_SIZE;
    if index >= 0 && readable(c, 0x5b) { c } else { 0 }
}

unsafe fn index_of(club: usize) -> i32 {
    ((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE) as i32
}

unsafe fn names(club: usize) -> (String, String) {
    if club == 0 {
        return ("?".into(), "?".into());
    }
    let (long, short) = (txt(&game_text(club + 4, 51)), txt(&game_text(club + 0x38, 26)));
    let short = if short.is_empty() { long.clone() } else { short };
    (long, short)
}

/// The club the club screen's print was asked for: its first argument when that is a club, else
/// the human's club.
pub unsafe fn club_of(p: usize) -> Option<usize> {
    let base = rd::<usize>(CLUB_TABLE);
    let is_club = |c: usize| c >= base && (c - base) % CLUB_SIZE == 0 && (c - base) / CLUB_SIZE < 100_000 && readable(c, 0x5b);
    [p, human_club_ptr().unwrap_or(0)].into_iter().find(|&c| is_club(c))
}

/// The page for the club, None when the game cannot be read (a network client).
pub unsafe fn page(club: usize) -> Option<String> {
    if remote::CLIENT || !readable(club, 0x1b3 + 28) {
        return None;
    }
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    if !(1900..2200).contains(&year) {
        return None;
    }
    let season = if day >= 181 { year } else { year - 1 };
    let (from, to) = ((181, season), (180, season + 1));
    let me = index_of(club);
    let (name, short) = names(club);
    let (cols, balance, funds, money) = finreport::figures(club).unwrap_or_default();
    let cur = money.symbol.replace('\u{80}', "\u{20ac}");
    let shown = |v: i64| (v as f64 * money.rate).round() as i64;
    let mut j = String::from("{");

    // ---- meta ----
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let manager = if readable(slot, 4) && readable(rd::<usize>(slot), 0x61) { txt(&staff_name(rd::<usize>(slot))) } else { String::new() };
    let league_comp = rd::<usize>(club + 0x57);
    let league_name = if readable(league_comp, 0x37) { txt(&game_text(league_comp + 4, 51)) } else { String::new() };
    j += &format!(
        "\"meta\":{{\"date\":{},\"season\":\"{}/{:02}\",\"manager\":{},\"currency\":{}}},",
        js(&finreport::date_text(day, year)),
        season,
        (season + 1) % 100,
        js(manager.trim()),
        js(&cur)
    );
    j += &format!("\"club\":{{\"name\":{},\"short\":{},\"league\":{}}},", js(&name), js(&short), js(&league_name));

    // ---- league table and the place after each of our league matches ----
    let table = challenge::league_table(club);
    let mut season_rows = Vec::new();
    let mut table_json = Vec::new();
    if let Some((comp_id, rows)) = &table {
        for (c, v) in rows {
            let (n, s) = names(*c);
            table_json.push(format!("[{},{},{},{},{},{},{},{},{},{}]", js(&n), js(&s), v[0], v[1], v[2], v[3], v[4], v[5], v[6], (*c == club) as u8));
        }
        // every club's played league matches: (date key, points, for, against)
        let all: Vec<(usize, Vec<(i32, i32, i32, i32)>)> = rows
            .iter()
            .map(|(c, _)| {
                let ci = index_of(*c);
                let games = form::fixtures(ci, from, to)
                    .into_iter()
                    .filter(|f| f.comp == *comp_id)
                    .filter_map(|f| {
                        let (h, a) = f.score?;
                        let (gf, ga) = if f.home == ci { (h, a) } else { (a, h) };
                        Some((f.date.1 * 366 + f.date.0, if gf > ga { 3 } else if gf == ga { 1 } else { 0 }, gf, ga))
                    })
                    .collect();
                (*c, games)
            })
            .collect();
        if let Some((_, mine)) = all.iter().find(|(c, _)| *c == club) {
            for (k, &(when, ..)) in mine.iter().enumerate() {
                let sums: Vec<(usize, i32, i32, i32)> = all
                    .iter()
                    .map(|(c, g)| {
                        let upto = g.iter().filter(|x| x.0 <= when);
                        let (p, f, a) = upto.fold((0, 0, 0), |s, x| (s.0 + x.1, s.1 + x.2, s.2 + x.3));
                        (*c, p, f, a)
                    })
                    .collect();
                let own = *sums.iter().find(|s| s.0 == club).unwrap();
                let key = |s: &(usize, i32, i32, i32)| (s.1, s.2 - s.3, s.2);
                let place = 1 + sums.iter().filter(|s| key(s) > key(&own)).count();
                season_rows.push(format!("[{},{},{},{},{}]", k + 1, place, own.1, own.2, own.3));
            }
        }
    }
    j += &format!("\"season\":[{}],\"table\":[{}],", season_rows.join(","), table_json.join(","));

    // ---- results and fixtures (all competitions, friendlies left out) ----
    // (the club's reserve team's matches come back too, under a competition named "Reserve...")
    let fx: Vec<form::Fixture> = form::fixtures(me, (day.max(0), year - 1), (day, year + 1))
        .into_iter()
        .filter(|f| !form::friendly(f.comp) && !form::comp_name(f.comp).starts_with(b"Reserve"))
        .collect();
    let today = year * 366 + day;
    let line = |f: &form::Fixture| {
        let home = f.home == me;
        let (n, s) = names(club_at(if home { f.away } else { f.home }));
        let comp = txt(&form::comp_name(f.comp));
        let score = f.score.map_or(String::new(), |(h, a)| if home { format!(",{h},{a}") } else { format!(",{a},{h}") });
        format!("[{},{},{},{},{}{}]", js(&short_date(f.date.0, f.date.1)), home as u8, js(&n), js(&s), js(&comp), score)
    };
    let played: Vec<String> = fx.iter().filter(|f| f.score.is_some() && f.date.1 * 366 + f.date.0 <= today).rev().take(8).map(line).collect();
    let coming: Vec<String> = fx.iter().filter(|f| f.score.is_none() && f.date.1 * 366 + f.date.0 >= today).take(6).map(line).collect();
    j += &format!("\"results\":[{}],\"fixtures\":[{}],", played.join(","), coming.join(","));

    // ---- finances (in the shown currency) ----
    let row = |label: &str, f: &dyn Fn(&finreport::Col) -> i64| format!("[{},{}]", js(label), cols.iter().map(|c| shown(f(c)).to_string()).collect::<Vec<_>>().join(","));
    let income = [
        row("Gate receipts", &|c| c.gate),
        row("Season tickets", &|c| c.tickets),
        row("TV", &|c| c.tv),
        row("Merchandising", &|c| c.merch),
        row("Players sold", &|c| c.sold),
        row("Other income", &|c| c.other_in),
    ];
    let costs = [
        row("Wages", &|c| c.wages),
        row("Bonuses", &|c| c.bonuses),
        row("Players bought", &|c| c.bought),
        row("Ground", &|c| c.ground),
        row("Other costs", &|c| c.other_out),
    ];
    j += &format!("\"fin\":{{\"balance\":{},\"funds\":{},\"income\":[{}],\"costs\":[{}]}},", shown(balance), shown(funds), income.join(","), costs.join(","));

    // ---- squad ----
    let mut men = Vec::new();
    for s in challenge::squad_of(club) {
        let player = rd::<usize>(s + 0x61);
        if !readable(player, 70) {
            continue;
        }
        let (bd, bm, by) = day_month(s + 0x10);
        let born = if by == 1900 { rd::<u16>(s + 0x18) as i32 } else { by };
        let born_day = if by == 1900 { 0 } else { [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][bm] + bd - 1 };
        let age = year - born - (day < born_day) as i32;
        let nation = rd::<usize>(s + 0x1a);
        let nat = if readable(nation, 55) { txt(&game_text(nation + 4, 51)) } else { String::new() };
        let (ca, pa) = ca_pa(player).unwrap_or((0, 0));
        let star = |v: i32| ((v * 10 + 100) / 200) as f32 / 2.0;
        let (wage, _) = money_texts(s);
        let wage_n: i64 = wage.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0);
        let ct = main_contract(s).map_or("-".to_string(), |c| {
            let (_, m, y) = day_month(c + 0x2d);
            format!("{} {y}", MONTHS[m])
        });
        let at = attributes(s);
        let keeper = rd::<i8>(player + 0xf) >= 15;
        men.push(format!(
            "{{\"id\":{},\"n\":{},\"p\":{},\"a\":{},\"nat\":{},\"ca\":{},\"pa\":{},\"c\":{},\"r\":{:.2},\"ap\":{},\"g\":{},\"as\":{},\"v\":{},\"w\":{},\"wn\":{},\"ct\":{},\"gk\":{},\"at\":[{}]}}",
            men.len(),
            js(&txt(&staff_name(s))),
            js(&txt(&positions_text(s))),
            age,
            js(&nat),
            star(ca),
            star(pa),
            condition(s).unwrap_or(-1),
            challenge::season_stat(s, 6, 0x11),
            challenge::season_stat(s, 6, 1) as i32,
            challenge::season_stat(s, 6, 2) as i32,
            challenge::season_stat(s, 6, 5) as i32,
            shown(((s + 0x52) as *const i32).read_unaligned() as i64),
            js(&wage),
            wage_n,
            js(&ct),
            keeper as u8,
            at.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
        ));
    }
    j += &format!("\"squad\":[{}],", men.join(","));

    // ---- this season's transfers in and out (loans left out) ----
    let mut moves = Vec::new();
    let tm = rd::<usize>(TRANSFER_MANAGER);
    if readable(tm, 0x86e) {
        let (rows, count) = (rd::<usize>(tm + 0x862), rd::<i32>(tm + 0x86a));
        if count > 0 && readable(rows, count as usize * 0x25) {
            let (staff_base, staff_count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
            for t in (0..count as usize).map(|k| rows + k * 0x25) {
                let (who, joined, left, fee, kind, d, y) = (rd::<i32>(t), rd::<i32>(t + 0x10), rd::<i32>(t + 0x14), rd::<i32>(t + 0x18), rd::<u8>(t + 0x1c), rd::<i16>(t + 0x1d) as i32, rd::<i16>(t + 0x1f) as i32);
                let in_season = (y == season && d >= 181) || (y == season + 1 && d < 181);
                if kind == 2 || !in_season || (joined != me && left != me) {
                    continue;
                }
                let staff = if (0..staff_count).contains(&who) { staff_base + who as usize * 0x6e } else { 0 };
                let person = if readable(staff, 0x10) { txt(&staff_name(staff)) } else { "?".into() };
                let incoming = joined == me;
                let other = names(club_at(if incoming { left } else { joined })).0;
                let other = if (if incoming { left } else { joined }) < 0 { String::new() } else { other }; // (released)
                moves.push(format!("[{},{},{},{},{}]", incoming as u8, js(&person), js(&other), js(&short_date(d, y)), shown(fee.max(0) as i64)));
            }
        }
    }
    j += &format!("\"transfers\":[{}]}}", moves.join(","));
    Some(TEMPLATE.replace("/*DATA*/", &j))
}

/// Write the club's page next to the printout and open it (single player); true when written.
pub unsafe fn write_and_open(club: usize, printout: &std::path::Path) -> bool {
    let Some(page) = page(club) else { return false };
    let stem = printout.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "club".into());
    let out = printout.with_file_name(format!("{stem}_dashboard.html"));
    if std::fs::write(&out, page.as_bytes()).is_err() {
        log(&format!("dashboard: {} not written", out.display()));
        return false;
    }
    log(&format!("dashboard: {} written", out.display()));
    export::open(&out);
    true
}

#[test]
fn json_text_is_safe() {
    assert_eq!(js("a\"b\\c</script>\u{80}"), "\"a\\\"b\\\\c\\u003c/script>\u{20ac}\"");
    assert_eq!(short_date(0, 2026), "1 Jan");
    assert_eq!(short_date(44, 2026), "14 Feb");
    assert!(TEMPLATE.contains("/*DATA*/"));
}
