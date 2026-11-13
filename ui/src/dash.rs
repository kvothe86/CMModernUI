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
    let mut staff_of = Vec::new(); // (the squad's staff, in the order of the JSON's ids)
    for s in challenge::squad_of(club) {
        let player = rd::<usize>(s + 0x61);
        if !readable(player, 70) {
            continue;
        }
        staff_of.push(s);
        let age = age_of(s, day, year);
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
    j += &format!("\"transfers\":[{}]", moves.join(","));
    j += &extras(club, season, day, year, &staff_of, &shown);
    j += "}";
    Some(TEMPLATE.replace("/*DATA*/", &j))
}

/// A staff member's age on the day (staff +0x10 the birth date, +0x18 the year when only that is
/// known).
unsafe fn age_of(s: usize, day: i32, year: i32) -> i32 {
    let (bd, bm, by) = day_month(s + 0x10);
    let born = if by == 1900 { rd::<u16>(s + 0x18) as i32 } else { by };
    let born_day = if by == 1900 { 0 } else { [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][bm] + bd - 1 };
    year - born - (day < born_day) as i32
}

/// A team on the pitch: (slot label, name, half stars or -1) by slot, placed as the team report
/// places them.
fn team_json(shape: &str, says: &str, eleven: &[(Vec<u8>, Vec<u8>, i32)], bench: &[Vec<u8>]) -> String {
    let labels: Vec<&[u8]> = eleven.iter().map(|e| e.0.as_slice()).collect();
    let at = assistant::places(&labels);
    let xi: Vec<String> = eleven.iter().zip(at).map(|(e, (x, y))| format!("[{},{:.3},{:.3},{},{}]", js(&txt(&e.0)), x, y, js(&txt(&e.1)), e.2)).collect();
    let bench: Vec<String> = bench.iter().map(|b| js(&txt(b))).collect();
    format!("{{\"shape\":{},\"says\":{},\"xi\":[{}],\"bench\":[{}]}}", js(shape), js(says), xi.join(","), bench.join(","))
}

/// The assistant's report as two teams (role 1 the best eleven, 2 the backup) and what he says.
unsafe fn report_teams(club: usize, slots: &[Vec<u8>], unavailable: Vec<usize>) -> Vec<String> {
    let text = assistant::report_for(club, slots.to_vec(), unavailable);
    let mut lines = text.split(|&c| c == b'\n');
    let head: Vec<&[u8]> = lines.next().unwrap_or(&[]).split(|&c| c == b'\t').collect();
    if head.len() < 4 {
        return Vec::new();
    }
    let rows: Vec<Vec<&[u8]>> = lines.map(|l| l.split(|&c| c == b'\t').collect::<Vec<_>>()).filter(|f| f.len() >= 7).collect();
    let says = txt(head[3]).replace('*', "");
    [b'1', b'2']
        .iter()
        .map(|role| {
            let mut eleven: Vec<(usize, (Vec<u8>, Vec<u8>, i32))> = rows
                .iter()
                .filter(|f| f[3] == [*role])
                .map(|f| (txt(f[4]).parse().unwrap_or(99), (f[5].to_vec(), f[0].to_vec(), txt(f[6]).parse().unwrap_or(-1))))
                .collect();
            eleven.sort_by_key(|e| e.0);
            let eleven: Vec<(Vec<u8>, Vec<u8>, i32)> = eleven.into_iter().map(|e| e.1).collect();
            team_json(&txt(head[2]), if *role == b'1' { &says } else { "" }, &eleven, &[])
        })
        .collect()
}

/// The panels after the first build (2026-11-13, asked for from the dashboard session): injuries
/// and bans, the career (CM Challenge seasons, transfers by season), line-ups (the selection, the
/// assistant's best / backup / fittest eleven, the saved XIs), the Short List with the scout's
/// stars, a national team's pool. A panel whose switch is off or that has nothing is left empty
/// (the page hides it).
unsafe fn extras(club: usize, season: i32, day: i32, year: i32, squad: &[usize], shown: &dyn Fn(i64) -> i64) -> String {
    let mut j = String::new();
    let slot = CHANNEL_MANAGER + 0x300 * channel();
    let manager = if readable(slot, 4) { rd::<usize>(slot) } else { 0 };
    let manager = if readable(manager, 0x61) { manager } else { 0 };

    // ---- injuries and bans of the squad (the History tab's lists) ----
    let mut log_rows = Vec::new();
    if on(INJURY_HISTORY) {
        for (id, &s) in squad.iter().enumerate() {
            for line in history::rows(s).split(|&c| c == b'\n') {
                let f: Vec<&[u8]> = line.split(|&c| c == b'\t').collect();
                if f.len() >= 4 && (f[0] == b"I" || f[0] == b"B") {
                    log_rows.push(format!("[{id},{},{},{},{}]", js(&txt(f[0])), js(&txt(f[1])), js(&txt(f[2])), js(&txt(f[3]))));
                }
            }
        }
    }
    j += &format!(",\"injuries\":[{}]", log_rows.join(","));

    // ---- career: CM Challenge by season, the club's transfers by season ----
    let mut seasons = Vec::new();
    let (mut total, mut rank) = (0, String::new());
    if manager != 0 {
        for y in (season - 25..=season).rev() {
            let Some(s) = challenge::summary(manager, y) else { break };
            total = s.total;
            rank = s.rank.clone();
            if s.items.is_empty() && s.rads.is_empty() && s.goals.is_empty() {
                continue;
            }
            let list = |l: &[(String, i32)]| l.iter().map(|(t, p)| format!("[{},{p}]", js(t))).collect::<Vec<_>>().join(",");
            seasons.push(format!("[{y},[{}],[{}],[{}],{}]", list(&s.items), list(&s.rads), list(&s.goals), s.season));
        }
    }
    let mut by_season: Vec<(i32, [i64; 4])> = Vec::new();
    let tm = rd::<usize>(TRANSFER_MANAGER);
    if readable(tm, 0x86e) {
        let (rows, count) = (rd::<usize>(tm + 0x862), rd::<i32>(tm + 0x86a));
        let me = index_of(club);
        if count > 0 && readable(rows, count as usize * 0x25) {
            for t in (0..count as usize).map(|k| rows + k * 0x25) {
                let (joined, left, fee, kind, d, y) = (rd::<i32>(t + 0x10), rd::<i32>(t + 0x14), rd::<i32>(t + 0x18).max(0) as i64, rd::<u8>(t + 0x1c), rd::<i16>(t + 0x1d) as i32, rd::<i16>(t + 0x1f) as i32);
                if kind == 2 || (joined != me && left != me) || !(1900..2200).contains(&y) {
                    continue;
                }
                let s = if d >= 181 { y } else { y - 1 };
                let k = match by_season.iter().position(|b| b.0 == s) {
                    Some(k) => k,
                    None => {
                        by_season.push((s, [0; 4]));
                        by_season.len() - 1
                    }
                };
                let v = &mut by_season[k].1;
                if joined == me {
                    v[0] += 1;
                    v[2] += shown(fee);
                } else {
                    v[1] += 1;
                    v[3] += shown(fee);
                }
            }
        }
    }
    by_season.sort_by_key(|b| -b.0);
    let money: Vec<String> = by_season.iter().map(|(s, v)| format!("[{s},{},{},{},{}]", v[0], v[1], v[2], v[3])).collect();
    j += &format!(",\"career\":{{\"total\":{total},\"rank\":{},\"seasons\":[{}],\"money\":[{}]}}", js(&rank), seasons.join(","), money.join(","));

    // ---- line-ups: the selection, the assistant's elevens, the saved XIs ----
    let mut teams = Vec::new(); // [title, team]
    let humans = rd::<usize>(assistant::HUMAN_MANAGER);
    let formation = if manager != 0 && mapped(humans, 4) { game::<extern "thiscall" fn(usize, usize) -> usize>(assistant::ACTIVE_TACTIC)(humans, manager) } else { 0 };
    if mapped(formation, 0x200) {
        let slots = assistant::formation_slots(formation);
        if slots.len() >= 11 {
            if let Some((shape, eleven, bench, _)) = assistant::lineup(club, club, Some(manager)) {
                let eleven: Vec<(Vec<u8>, Vec<u8>, i32)> = eleven.into_iter().map(|(l, s)| (l, staff_name(s), -1)).collect();
                let bench: Vec<Vec<u8>> = bench.into_iter().map(|s| staff_name(s)).collect();
                if !eleven.is_empty() {
                    teams.push(format!("[\"Selected\",{}]", team_json(&shape, "", &eleven, &bench)));
                }
            }
            if on(TEAM_REPORT) {
                let injured: Vec<usize> = squad.iter().copied().filter(|&s| assistant::injured(s)).collect();
                let tired: Vec<usize> = squad.iter().copied().filter(|&s| assistant::injured(s) || condition(s).is_some_and(|c| c < 85)).collect();
                let best = report_teams(club, &slots, injured);
                let fit = report_teams(club, &slots, tired);
                for (title, team) in [("Best XI", best.first()), ("Backup XI", best.get(1)), ("Fittest XI", fit.first())] {
                    if let Some(t) = team {
                        teams.push(format!("[{},{t}]", js(title)));
                    }
                }
            }
        }
    }
    let mut saved = Vec::new();
    if on(SAVED_LINEUPS) {
        let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
        for (k, shape, eleven) in lineups::saved(club) {
            // (a player no longer at the club: found by his id, which is his row in the staff
            // table, and named with where he is now)
            let gone = |id: i32| {
                let s = base + id.max(0) as usize * 0x6e;
                if !(0..count).contains(&id) || !readable(s, 0x6e) || rd::<i32>(s) != id {
                    return "\"(unknown player)\"".to_string();
                }
                let now = rd::<usize>(s + 0x39);
                let at = if now != 0 && readable(now, 0x40) { format!("now {}", txt(&game_text(now + 4, 51))) } else { "no club now".into() };
                if trace() {
                    log(&format!("dashboard: saved XI {} id {id} = {} ({at})", k + 1, txt(&staff_name(s))));
                }
                js(&format!("{} ({at})", txt(&staff_name(s))))
            };
            let names: Vec<String> = eleven.iter().map(|&(s, id)| if s != 0 { js(&txt(&staff_name(s))) } else { gone(id) }).collect();
            saved.push(format!("[{},{},[{}]]", k + 1, js(&shape), names.join(",")));
        }
    }
    j += &format!(",\"teams\":[{}],\"saved\":[{}]", teams.join(","), saved.join(","));

    // ---- the Short List, as the club's best judge sees each player ----
    let mut targets = Vec::new();
    let judge = assistant::best_judge(club).or_else(|| assistant::assistant_of(club)).unwrap_or(0);
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    for s in (0..count.max(0) as usize).map(|k| base + k * 0x6e) {
        if targets.len() >= 200 || !readable(((s + 0x61) as *const usize).read_unaligned(), 70) || !natpool::shortlisted(s) {
            continue;
        }
        let other = rd::<usize>(s + 0x39);
        let other = if other != 0 && readable(other, 0x40) { txt(&game_text(other + 4, 51)) } else { "Free agent".into() };
        let stars = if judge != 0 { scout::seen_score(s, judge, None).map_or(-1, |v| scout::halves(v) as i32) } else { -1 };
        targets.push(format!(
            "[{},{},{},{},{},{}]",
            js(&txt(&staff_name(s))),
            js(&txt(&positions_text(s))),
            age_of(s, day, year),
            js(&other),
            shown(((s + 0x52) as *const i32).read_unaligned() as i64),
            stars
        ));
    }
    j += &format!(",\"targets\":[{}]", targets.join(","));

    // ---- a national team's pool ----
    let mut pool = Vec::new();
    if on(NATIONAL_POOL) && national_team(club) {
        for (n, p, a, c, caps, goals, score, in_squad) in natpool::pool_rows(club) {
            pool.push(format!("[{},{},{a},{},{caps},{goals},{score},{}]", js(&txt(&n)), js(&txt(&p)), js(&txt(&c)), in_squad as u8));
        }
    }
    j += &format!(",\"pool\":[{}]", pool.join(","));
    j
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
