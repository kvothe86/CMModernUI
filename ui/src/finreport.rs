//! The P&L report (switch `pl_report`): a button right of the View dropdown on a club's Finances
//! & Info tab opens a full profit and loss account on a sheet of paper, signed by the made-up firm
//! "Accountants & Co" (a fake logo drawn here), with notes written from how the club is doing
//! (the wording is `accountants_notes.tsv`, drafted ahead, picked by the figures and the club).
//!
//! The figures are the game's own: a club's finance record (359 bytes, `re\docs\finance.md`)
//! holds four blocks of 0x50 bytes, last month +0x20, this month +0x70, last year +0xc0, this
//! year +0x110 (what Finance::get_last_month 0x4671b0 and its three twins return). Inside a block:
//! +0 players bought and +8 players sold (64 bits), +0x10 other costs, +0x14 interest paid, +0x18
//! league fines, +0x1c ground, +0x20 wages, +0x24 dividends, +0x28 bonuses, +0x2c signing-on fees,
//! +0x30 gate receipts, +0x34 other income, +0x38 prize money, +0x3c season tickets, +0x40 interest
//! earned, +0x44 investments, +0x48 TV, +0x4c merchandising (read off Finance::pay_* / add_* 0x5987c0..
//! 0x5999e0 and the info tab's row code, 0x460590). The record's +0 is the balance (64 bits). A
//! network client asks the host for the figures (key `$`, `remote`) and builds the page itself.

use crate::*;

const FINANCE_TABLE: usize = 0xae2a38; // FinanceTable*
const FINANCE_FOR_CLUB: usize = 0x5a0590; // thiscall(table, Club*) -> record*
const TRANSFER_BUDGET: usize = 0x598800; // thiscall(record, player, add_squad, out*, fog, deduct) -> i64
const TODAY: usize = 0xae2c90; // Date: i16 day of the year, i16 year
const RECORD: usize = 359;
/// This month, last month, this year, last year: the columns of the game's screen.
const BLOCKS: [usize; 4] = [0x70, 0x20, 0x110, 0xc0];
const NOTES: &str = include_str!("../accountants_notes.tsv");

/// One column of the account.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Col {
    pub gate: i64,
    pub tickets: i64,
    pub tv: i64,
    pub merch: i64,
    pub sold: i64,
    pub other_in: i64, // prize money, investments, interest earned, other income
    pub wages: i64,
    pub bonuses: i64, // bonuses and signing-on fees
    pub bought: i64,
    pub ground: i64,
    pub other_out: i64, // interest paid, league fines, dividends, other costs
}

impl Col {
    pub fn turnover(&self) -> i64 {
        self.gate + self.tickets + self.tv + self.merch
    }
    pub fn income(&self) -> i64 {
        self.turnover() + self.sold + self.other_in
    }
    pub fn spend(&self) -> i64 {
        self.wages + self.bonuses + self.bought + self.ground + self.other_out
    }
    pub fn result(&self) -> i64 {
        self.income() - self.spend()
    }
}

fn i32_at(rec: &[u8], at: usize) -> i64 {
    i32::from_le_bytes(rec[at..at + 4].try_into().unwrap()) as i64
}

fn i64_at(rec: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(rec[at..at + 8].try_into().unwrap())
}

/// The account's column from the block at `base` of a finance record.
pub fn column(rec: &[u8], base: usize) -> Col {
    let n = |o: usize| i32_at(rec, base + o);
    Col {
        gate: n(0x30),
        tickets: n(0x3c),
        tv: n(0x48),
        merch: n(0x4c),
        sold: i64_at(rec, base + 8),
        other_in: n(0x38) + n(0x44) + n(0x40) + n(0x34),
        wages: n(0x20),
        bonuses: n(0x28) + n(0x2c),
        bought: i64_at(rec, base),
        ground: n(0x1c),
        other_out: n(0x14) + n(0x18) + n(0x24) + n(0x10),
    }
}

/// Everything the page shows.
#[derive(Clone, Default)]
pub struct Report {
    pub club: String,
    pub date: String,
    pub reference: String,
    pub cols: [Col; 4],
    pub balance: i64,
    pub funds: i64,
    pub money: Money,
    pub verdict: String,
    pub notes: Vec<String>,
    pub basis: String,
    pub closing: String,
    pub partner: (String, String),
    pub footer: String,
    /// month-end balances, oldest first (`diary`): year, month, balance
    pub history: Vec<(i32, i32, i64)>,
    /// the balance at the season's end: its date, the figure, the average month it goes by
    pub forecast: Option<(String, i64, i64)>,
    /// a second forecast in the partner's own words (the local build's)
    pub second: Option<String>,
}

/// The balance on 30 June (the season's end; next year's from July): the average monthly change of
/// the last six months recorded, else this year's result spread over the months since July;
/// `month` 1..12 today's. (end date, balance then, average month)
pub fn forecast(history: &[(i32, i32, i64)], balance: i64, year_result: i64, month: i32, year: i32) -> (String, i64, i64) {
    let recent = &history[history.len().saturating_sub(7)..];
    let per_month = if recent.len() >= 2 {
        (recent[recent.len() - 1].2 - recent[0].2) / (recent.len() as i64 - 1)
    } else {
        year_result / (if month >= 7 { month - 6 } else { month + 6 }) as i64
    };
    let (left, end_year) = if month <= 6 { (6 - month, year) } else { (18 - month, year + 1) };
    (format!("30 June {end_year}"), balance + per_month * left as i64, per_month)
}

/// How the game shows money: its currency's symbol, and what 1 of the game's units comes to in it
/// (read off the game's own formatter once, `game_money`; the tests use euro at 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Money {
    pub symbol: String,
    pub rate: f64,
}

impl Default for Money {
    fn default() -> Money {
        Money { symbol: "E".into(), rate: 1.0 }
    }
}

impl Money {
    /// "E1,129,140"; a negative one "(E1,129,140)".
    pub fn full(&self, v: i64) -> String {
        // (the game cuts the conversion, it does not round it: 8,741,367 at 2.5 shows 21,853,417)
        let shown = (v as f64 * self.rate).trunc() as i64;
        let digits = shown.unsigned_abs().to_string();
        let mut out = String::new();
        for (k, c) in digits.chars().enumerate() {
            if k > 0 && (digits.len() - k) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        if shown < 0 { format!("({}{out})", self.symbol) } else { format!("{}{out}", self.symbol) }
    }

    /// "E10.5m"; a negative one in brackets.
    pub fn millions(&self, v: i64) -> String {
        let shown = v as f64 * self.rate;
        let m = format!("{}{:.1}m", self.symbol, shown.abs() / 1e6);
        if shown < 0.0 { format!("({m})") } else { m }
    }
}

/// "28 January 2004" from the game's day of the year (0-based, 29 February counted in a leap year).
pub fn date_text(day: i32, year: i32) -> String {
    const MONTHS: [(&str, i32); 12] = [("January", 31), ("February", 28), ("March", 31), ("April", 30), ("May", 31), ("June", 30), ("July", 31), ("August", 31), ("September", 30), ("October", 31), ("November", 30), ("December", 31)];
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut left = day.max(0);
    for (k, (name, days)) in MONTHS.iter().enumerate() {
        let days = days + (leap && k == 1) as i32;
        if left < days {
            return format!("{} {} {}", left + 1, name, year);
        }
        left -= days;
    }
    format!("31 December {year}")
}

/// A text of the library: the variants of a key.
fn variants(key: &str) -> Vec<&'static str> {
    NOTES.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once('\t')).filter(|(k, _)| *k == key).map(|(_, t)| t).collect()
}

/// One variant of a key, by the club (a club keeps its wording, another club reads differently).
fn pick(key: &str, seed: usize) -> Option<&'static str> {
    let v = variants(key);
    (!v.is_empty()).then(|| v[(seed + key.len() * 7) % v.len()])
}

fn substitute(text: &str, values: &[(&str, String)]) -> String {
    let mut out = text.to_string();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

fn pct(part: i64, whole: i64) -> i64 {
    if whole > 0 { (part * 100 + whole / 2) / whole } else { 0 }
}

/// The report of a club from its figures. `seed` (the club's index) picks the wording.
pub fn build(club: &str, seed: usize, date: &str, cols: [Col; 4], balance: i64, funds: i64, money: &Money) -> Report {
    let (euro, euro_m) = (|v: i64| money.full(v), |v: i64| money.millions(v));
    let (m, lm, y, ly) = (cols[0], cols[1], cols[2], cols[3]);
    let (income, spend, result) = (y.income(), y.spend(), y.result());
    let wages_pct = pct(y.wages, income);
    let wages_pct_last = pct(ly.wages, ly.income());
    let sales_pct = pct(y.sold, income);
    let ex_sales = result - y.sold;
    let values: Vec<(&str, String)> = vec![
        ("club", club.to_string()),
        ("date", date.to_string()),
        ("income", euro(income)),
        ("expenditure", euro(spend)),
        ("result", euro(result.abs())),
        ("balance", euro(balance)),
        ("funds", euro(funds)),
        ("sales", euro(y.sold)),
        ("bought", euro(y.bought)),
        ("wages", euro(y.wages)),
        ("profit_ex_sales", euro(ex_sales)),
        ("income_m", euro_m(income)),
        ("result_m", euro_m(result.abs())),
        ("balance_m", euro_m(balance)),
        ("sales_m", euro_m(y.sold)),
        ("wages_m", euro_m(y.wages)),
        ("profit_ex_sales_m", euro_m(ex_sales)),
        ("bought_m", euro_m(y.bought)),
        ("wages_pct", wages_pct.to_string()),
        ("wages_pct_last", wages_pct_last.to_string()),
        ("sales_pct", sales_pct.to_string()),
        ("tickets_pct", pct(y.tickets, income).to_string()),
        ("bought_pct", pct(y.bought, spend).to_string()),
        ("month_result", euro(m.result())),
        ("month_result_last", euro(lm.result())),
    ];
    let say = |key: &str| pick(key, seed).map(|t| substitute(t, &values));
    // the headline: the first rule that fits
    let verdict_key = if result > 0 {
        if balance < 0 {
            "VERDICT_PROFIT_OVERDRAWN"
        } else if result * 10 >= income {
            "VERDICT_STRONG"
        } else {
            "VERDICT_PROFIT"
        }
    } else if balance < 0 {
        if -result * 4 > income { "VERDICT_DEEP" } else { "VERDICT_LOSS_OVERDRAWN" }
    } else {
        "VERDICT_LOSS_FUNDED"
    };
    // the notes, in the order of weight; at most five
    let mut keys: Vec<&str> = Vec::new();
    if wages_pct >= 70 {
        keys.push("NOTE_WAGES_HIGH");
    } else if income > 0 && wages_pct < 45 {
        keys.push("NOTE_WAGES_MODEST");
    }
    if ly.income() > 0 && income > 0 {
        if wages_pct + 3 <= wages_pct_last {
            keys.push("NOTE_WAGES_FELL");
        } else if wages_pct >= wages_pct_last + 3 {
            keys.push("NOTE_WAGES_ROSE");
        }
    }
    if sales_pct >= 40 {
        keys.push("NOTE_SALES_RELIANT");
    } else if y.sold == 0 && income > 0 {
        keys.push("NOTE_NO_SALES");
    }
    if pct(y.bought, spend) >= 25 {
        keys.push("NOTE_BOUGHT_HEAVY");
    }
    if pct(y.tickets, income) >= 30 {
        keys.push("NOTE_TICKETS_STRONG");
    }
    if (m.result() - lm.result()).abs() >= 10_000 {
        keys.push(if m.result() > lm.result() { "NOTE_MONTH_BETTER" } else { "NOTE_MONTH_WORSE" });
    }
    if funds > 0 {
        keys.push("NOTE_FUNDS_AVAILABLE");
    } else {
        keys.push("NOTE_FUNDS_NONE");
    }
    if balance < 0 {
        keys.push("NOTE_OVERDRAWN_ADVICE");
    } else if balance >= 5_000_000 {
        keys.push("NOTE_RESERVES");
    }
    // the partner: the local build's own accountants (their names stay out of the public code),
    // else the library's; one with advice of his own adds it as his last note (user 2026-11-22)
    #[cfg(cmui_local)]
    let own: &[(&str, &str, &[&str])] = &crate::local::PARTNERS;
    #[cfg(not(cmui_local))]
    let own: &[(&str, &str, &[&str])] = &[];
    let (partner, advice) = match own.get(seed % own.len().max(1)) {
        Some(&(full, short, advice)) => ((full.to_string(), short.to_string()), advice.get(seed / own.len() % advice.len().max(1)).map(|t| substitute(t, &values))),
        None => (pick("PARTNER", seed).and_then(|p| p.split_once('|')).map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or(("Marcus Hartwell FCA".into(), "M. Hartwell".into())), None),
    };
    keys.truncate(if advice.is_some() { 4 } else { 5 });
    let notes: Vec<String> = keys.iter().filter_map(|k| say(k)).chain(advice).collect();
    let initials: String = club.split_whitespace().filter_map(|w| w.chars().next()).filter(|c| c.is_alphabetic()).take(3).collect::<String>().to_uppercase();
    Report {
        club: club.to_string(),
        date: date.to_string(),
        reference: format!("{}/PL/{:04}", initials, (seed * 37 + 1104) % 10000),
        cols,
        balance,
        funds,
        money: money.clone(),
        verdict: say(verdict_key).unwrap_or_default(),
        notes,
        basis: say("BASIS").unwrap_or_default(),
        closing: say("CLOSING").unwrap_or_default(),
        partner,
        footer: say("FOOTER").unwrap_or_default(),
        ..Default::default()
    }
}

// ---- the game's memory ----

static mut OPEN: bool = false;
static mut REPORT: Option<Report> = None;
/// The button (right of the View dropdown) while the Finances tab shows, and what this frame saw.
static mut BUTTON: Option<(i32, i32, i32, i32, u32)> = None;
static mut VIEW_SEEN: Option<(i32, i32, i32, i32, u32)> = None;
static mut FIN_SEEN: bool = false;
static mut BTN_HOVER: bool = false;
static mut HOVER: bool = false;
static mut SAVE_HOVER: bool = false;
static mut SAVE: bool = false;
static mut SAVED: Option<(String, std::time::Instant)> = None;
static mut OPENED_AT: Option<std::time::Instant> = None;

pub unsafe fn forget() {
    BUTTON = None;
    VIEW_SEEN = None;
    FIN_SEEN = false;
    OPEN = false;
}

/// Every text of the screen: the View dropdown's box and the column heads of the money views.
pub unsafe fn note_text(x1: i32, y1: i32, x2: i32, y2: i32, font: u32, t: &[u8]) {
    let (_, sh) = screen();
    if t == b"View" && y1 < sh / 3 && x2 - x1 > 60 {
        VIEW_SEEN = Some((x1, y1, x2, y2, font));
    } else if t == b"This Month" || t == b"Last Month" {
        FIN_SEEN = true;
    }
}

/// A club record's address when it is one.
unsafe fn club_ptr(p: usize) -> Option<usize> {
    let base = rd::<usize>(CLUB_TABLE);
    (p >= base && (p - base) % CLUB_SIZE == 0 && (p - base) / CLUB_SIZE < 100_000 && readable(p, 0x5b)).then_some(p)
}

const CASH_SET: usize = 0x4671a0; // thiscall(Cash*, amount)
const CASH_CONVERT: usize = 0x43f460; // thiscall(Cash*, flags) -> i64 (ret 4; the screen's second "6" is left on the stack): in the shown currency, rounded as the flags say (0x20: not rounded)
const CASH_TEXT: usize = 0x450aa0; // thiscall(Cash*, out, size, flags): 0x22 the whole number, 0x21 shortened

/// The game's currency: what its own formatter makes of 1,000,000 (the finance screens' call).
unsafe fn game_money() -> Money {
    let mut cash = [0u32; 4];
    let mut out = [0u8; 0x7d0];
    game::<extern "thiscall" fn(*mut u32, i32)>(CASH_SET)(cash.as_mut_ptr(), 1_000_000);
    game::<extern "thiscall" fn(*mut u32, *mut u8, i32, i32)>(CASH_TEXT)(cash.as_mut_ptr(), out.as_mut_ptr(), 0x7d0, 0x22);
    let shown = latin1(out.split(|&c| c == 0).next().unwrap_or(&[])).into_owned();
    let symbol: String = shown.chars().take_while(|c| !c.is_ascii_digit()).filter(|c| !"()-".contains(*c)).collect();
    let digits: String = shown.chars().filter(char::is_ascii_digit).collect();
    let rate = digits.parse::<f64>().map_or(1.0, |d| d / 1e6);
    log(&format!("P&L report: the game shows 1,000,000 as {shown:?}"));
    Money { symbol: if symbol.is_empty() { "E".into() } else { symbol }, rate: if rate > 0.0 { rate } else { 1.0 } }
}

/// The report of the club the screen shows, from the game's memory; a network client's from the
/// host (key `$` club: its figures as text, the page built here; user 2026-11-22).
unsafe fn read() -> Option<Report> {
    let club = [screen_param(0), SQUAD_CLUB, human_club_ptr().unwrap_or(0)].into_iter().find_map(|p| club_ptr(p))?;
    let text = if remote::CLIENT { ask_host(&key(b'$', &[club as u32]))? } else { remote(club)? };
    let (name, seed, date, cols, balance, funds, money) = decode(std::str::from_utf8(&text).ok()?)?;
    // (a test: cmui.ini `pl_accountant = 1..3` has that one of the local accountants sign every
    // report, 0 or none by the club; user 2026-11-22, to see each one's own notes)
    #[cfg(cmui_local)]
    let seed = {
        let n = crate::local::PARTNERS.len();
        match setting("pl_accountant").and_then(|v| v.trim().parse::<usize>().ok()) {
            Some(k) if (1..=n).contains(&k) => seed - seed % n + (k - 1),
            _ => seed,
        }
    };
    let mut r = build(&name, seed, &date, cols, balance, funds, &money);
    // the balance's months (`diary`, the host's) and where they lead by the season's end
    r.history = if remote::CLIENT {
        ask_host(&key(b'%', &[club as u32])).map_or_else(Vec::new, |t| diary::parse_remote(&latin1(&t)))
    } else {
        seed_of(club).map_or_else(Vec::new, |s| diary::balances(s))
    };
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let (end, value, per_month) = forecast(&r.history, balance, cols[2].result(), diary::month_of(day, year), year);
    r.forecast = Some((end.clone(), value, per_month));
    // (an accountant with a forecast of his own: his figure takes the forecast's place, with his reason under the chart)
    #[cfg(cmui_local)]
    if let Some((text, figure)) = crate::local::second_forecast(seed, balance, per_month, &end) {
        r.second = Some(text.replace("{forecast}", &money.full(figure)).replace("{date}", &end));
        r.forecast = Some((end.clone(), figure, per_month));
    }
    Some(r)
}

/// The contract and wage planner (the Finances tab's second button, user 2026-11-22): the first
/// team's contracts, which run out when, and the wage budget. Wages are the contracts' weekly ones
/// (contract +0xc; the finance record's +0xc wage bill and +0x14 what the club can pay a week,
/// `re\docs\finance.md`), shown a year (x 52).
#[derive(Clone, Default)]
pub struct Plan {
    pub club: String,
    pub date: String,
    pub season_end: i32, // the year of the 30 June that ends this season
    pub bill: i64,       // a week
    pub can_pay: i64,    // a week: the most the club can offer one new man (finance +0x14)
    pub money: Money,
    pub rows: Vec<PlanRow>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct PlanRow {
    pub name: String,
    pub pos: String,
    pub age: i32,
    pub wage: i64, // a week
    pub wage_text: String, // as the game writes it (its period from Game Settings)
    pub end_month: i32,    // 0..11
    pub end_year: i32,
    pub value: i64,
}

impl Plan {
    /// Ends by 30 June of `year`.
    pub fn ends_by(row: &PlanRow, year: i32) -> bool {
        (row.end_year, row.end_month) <= (year, 5)
    }
    /// What the contracts that end this season pay a year.
    pub fn freed(&self) -> i64 {
        self.rows.iter().filter(|r| Plan::ends_by(r, self.season_end)).map(|r| r.wage).sum::<i64>() * 52
    }
}

fn encode_plan(p: &Plan) -> String {
    let mut out = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", p.club, p.date, p.season_end, p.bill, p.can_pay, p.money.symbol, p.money.rate);
    for r in &p.rows {
        out += &format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.name, r.pos, r.age, r.wage, r.wage_text, r.end_month, r.end_year, r.value);
    }
    out
}

fn decode_plan(text: &str) -> Option<Plan> {
    let mut lines = text.lines();
    let h: Vec<&str> = lines.next()?.split('\t').collect();
    if h.len() != 7 {
        return None;
    }
    let mut p = Plan { club: h[0].into(), date: h[1].into(), season_end: h[2].parse().ok()?, bill: h[3].parse().ok()?, can_pay: h[4].parse().ok()?, money: Money { symbol: h[5].into(), rate: h[6].parse().ok()? }, rows: Vec::new() };
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 8 {
            p.rows.push(PlanRow { name: f[0].into(), pos: f[1].into(), age: f[2].parse().ok()?, wage: f[3].parse().ok()?, wage_text: f[4].into(), end_month: f[5].parse().ok()?, end_year: f[6].parse().ok()?, value: f[7].parse().ok()? });
        }
    }
    Some(p)
}

/// Host: a club's planner as text (also the host's own page goes this way; a client's key `&`).
pub unsafe fn plan_remote(club: usize) -> Option<Vec<u8>> {
    let club = club_ptr(club)?;
    let table = rd::<usize>(FINANCE_TABLE);
    let record = if table == 0 { 0 } else { game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club) };
    if !readable(record, RECORD) {
        return None;
    }
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let mut p = Plan {
        club: latin1(&game_text(club + 4, 51)).into_owned(),
        date: date_text(day, year),
        season_end: if diary::month_of(day, year) >= 7 { year + 1 } else { year },
        bill: rd::<i32>(record + 0xc) as i64,
        can_pay: rd::<i32>(record + 0x14) as i64,
        money: game_money(),
        rows: Vec::new(),
    };
    for s in challenge::squad_of(club) {
        if !readable(rd::<usize>(s + 0x61), 70) || !plays_for(s, club) {
            continue;
        }
        let Some(c) = main_contract(s) else { continue };
        let (_, m, y) = day_month(c + 0x2d);
        p.rows.push(PlanRow {
            name: latin1(&staff_name(s)).replace('\t', " "),
            pos: latin1(&positions_text(s)).into_owned(),
            age: dash::age_of(s, day, year),
            wage: rd::<i32>(c + 0xc) as i64,
            wage_text: money_texts(s).0,
            end_month: m as i32,
            end_year: y,
            value: ((s + 0x52) as *const i32).read_unaligned() as i64,
        });
    }
    // (soonest out first, the dearest first among them)
    p.rows.sort_by_key(|r| (r.end_year, r.end_month, -r.wage));
    Some(encode_plan(&p).into_bytes())
}

unsafe fn read_plan() -> Option<Plan> {
    let club = [screen_param(0), SQUAD_CLUB, human_club_ptr().unwrap_or(0)].into_iter().find_map(|p| club_ptr(p))?;
    let text = if remote::CLIENT { ask_host(&key(b'&', &[club as u32]))? } else { plan_remote(club)? };
    // (the text is UTF-8, as `encode_plan` wrote it: read as Latin-1 the euro came out "Ã€",
    // João "JoÃ£o", user 2026-11-22)
    decode_plan(&String::from_utf8_lossy(&text))
}

/// The physio's medical report (the squad screen's Assistant menu, user 2026-11-22): who is out,
/// and who the game's own daily injury draw is most likely to hit (`re\docs\injuries.md`,
/// 0x62b7b0: on a weekday `random(160) <= 2 x ((injury proneness - 3) x 5 + jadedness)`, never
/// below jadedness 20). The injury record (manager [0xb63d28] +0, 31 bytes a person): +8
/// condition, +0xc match load, +0xe the injury's slot (48 bytes, [manager +4]: +0x2c days out,
/// +0x2e days to training), +0x1e jadedness. Player record +45 injury proneness, +52 natural
/// fitness. The club's three physios at +0x1cf; their Physiotherapy at the non-player record +0x1e.
#[derive(Clone, Default)]
pub struct Medical {
    pub club: String,
    pub date: String,
    pub physio: String,
    pub skill: i32,
    pub rows: Vec<MedRow>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct MedRow {
    pub name: String,
    pub pos: String,
    pub age: i32,
    pub condition: i32, // %
    pub load: i32,
    pub jaded: i32,
    pub prone: i32,
    pub natural: i32,
    pub out: i32,    // days out, 0 fit
    pub trains: i32, // days until he trains again
}

impl MedRow {
    /// His risk against what is normal, 0..100: the game's daily draw weighs injury proneness x 5
    /// and jadedness, and jadedness stands at 50 for nearly everyone (a whole squad came out "very
    /// high", user 2026-11-22), so only what is above that counts, twice.
    pub fn risk(&self) -> i32 {
        if self.out > 0 { 0 } else { ((self.prone - 3) * 5 + (self.jaded - 50) * 2).clamp(0, 100) }
    }
    pub fn risk_word(&self) -> &'static str {
        if self.out > 0 {
            return "-";
        }
        match self.risk() {
            0..=19 => "Low",
            20..=44 => "Normal",
            45..=69 => "High",
            _ => "Very high",
        }
    }
    /// The physio's word on him.
    pub fn advice(&self) -> String {
        if self.out > 0 {
            return format!("Out {} days, training in {}", self.out, self.trains.max(0));
        }
        if self.condition < 80 {
            return format!("Rest him, {}% fit", self.condition);
        }
        // (jadedness is the tiredness of many matches, not today's condition: at 100% "needs a
        // breather" read as a mistake, user 2026-11-23)
        if self.jaded >= 65 {
            return if self.condition >= 95 { "Fit today, but jaded: rotate".into() } else { "Needs a breather".into() };
        }
        if self.prone >= 15 {
            return "Fragile: manage his minutes".into();
        }
        if self.load > 1000 {
            return "Heavy load of matches".into();
        }
        "Fit".into()
    }
    /// On the watch list: a high risk, low condition, tired or a heavy load.
    pub fn to_watch(&self) -> bool {
        self.out == 0 && (self.risk() >= 45 || self.condition < 85 || self.jaded >= 65 || self.load > 1000)
    }
}

/// A non-player's skill as the game shows it (1..20): the eight intrinsic ones (Physiotherapy,
/// Judging Potential, Coaching...) go through the game's conversion (0x544410, always in role;
/// `rust::converted`) with his current ability (non-player +4); the raw value read 125.
pub fn shown_skill(intrinsic: i8, current_ability: i16) -> i32 {
    let t = (current_ability as f64 + 2.0 * intrinsic as f64) * 0.05 + 10.0;
    ((t * t * 0.1 + t) * (1.0 / 3.0) + 0.5).clamp(1.0, 20.0) as i32
}

fn encode_medical(m: &Medical) -> String {
    let mut out = format!("{}\t{}\t{}\t{}\n", m.club, m.date, m.physio, m.skill);
    for r in &m.rows {
        out += &format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.name, r.pos, r.age, r.condition, r.load, r.jaded, r.prone, r.natural, r.out, r.trains);
    }
    out
}

fn decode_medical(text: &str) -> Option<Medical> {
    let mut lines = text.lines();
    let h: Vec<&str> = lines.next()?.split('\t').collect();
    if h.len() != 4 {
        return None;
    }
    let mut m = Medical { club: h[0].into(), date: h[1].into(), physio: h[2].into(), skill: h[3].parse().ok()?, rows: Vec::new() };
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 10 {
            let n = |k: usize| f[k].parse::<i32>().unwrap_or(0);
            m.rows.push(MedRow { name: f[0].into(), pos: f[1].into(), age: n(2), condition: n(3), load: n(4), jaded: n(5), prone: n(6), natural: n(7), out: n(8), trains: n(9) });
        }
    }
    Some(m)
}

/// Host: a club's medical report as text (the host's own page too; a client's key `)`).
pub unsafe fn medical_remote(club: usize) -> Option<Vec<u8>> {
    let club = club_ptr(club)?;
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    // the best of the three physios
    let mut physio = (String::from("the club doctor"), 0);
    for k in 0..3 {
        let s = rd::<usize>(club + 0x1cf + 4 * k);
        let np = if readable(s, 0x6e) { rd::<usize>(s + 0x69) } else { 0 };
        let skill = if readable(np, 0x23) { shown_skill(rd::<i8>(np + 0x1e), rd::<i16>(np + 4)) } else { 0 };
        if skill > physio.1 {
            physio = (latin1(&staff_name(s)).into_owned(), skill);
        }
    }
    let mut m = Medical { club: latin1(&game_text(club + 4, 51)).into_owned(), date: date_text(day, year), physio: physio.0, skill: physio.1, rows: Vec::new() };
    let im = rd::<usize>(0xb63d28);
    let (people, slots) = if readable(im, 8) { (rd::<usize>(im), rd::<usize>(im + 4)) } else { (0, 0) };
    for s in challenge::squad_of(club) {
        let player = rd::<usize>(s + 0x61);
        if !readable(player, 70) || !plays_for(s, club) {
            continue;
        }
        let rec = people + rd::<i32>(s).max(0) as usize * 31;
        if people == 0 || !readable(rec, 31) {
            continue;
        }
        let slot = rd::<i16>(rec + 0xe);
        let inj = if slot >= 0 { slots + slot as usize * 48 } else { 0 };
        let (out, trains) = if inj != 0 && readable(inj, 48) { (rd::<i16>(inj + 0x2c) as i32, rd::<i16>(inj + 0x2e) as i32) } else { (0, 0) };
        m.rows.push(MedRow {
            name: latin1(&staff_name(s)).replace('\t', " "),
            pos: latin1(&positions_text(s)).into_owned(),
            age: dash::age_of(s, day, year),
            condition: (rd::<i16>(rec + 8) as i32 / 100).clamp(0, 100),
            load: rd::<i16>(rec + 0xc) as i32,
            jaded: rd::<u8>(rec + 0x1e) as i32,
            prone: rd::<i8>(player + 45) as i32,
            natural: rd::<i8>(player + 52) as i32,
            out: out.max(0),
            trains,
        });
    }
    Some(encode_medical(&m).into_bytes())
}

/// Opens the physio's report of a club (the Assistant menu's item), the host's or asked of it.
pub unsafe fn open_medical(club: usize) {
    let text = if remote::CLIENT { ask_host(&key(b')', &[club as u32])) } else { medical_remote(club) };
    MEDICAL = text.and_then(|t| decode_medical(&String::from_utf8_lossy(&t)));
    if MEDICAL.is_some() {
        (PAGE, OPEN, OPENED_AT) = (PAGE_MEDICAL, true, Some(std::time::Instant::now()));
    }
}

/// The physio's page (or only measures it: `dry`). Returns its last line's end.
#[allow(clippy::too_many_arguments)]
unsafe fn paint_medical(md: &Medical, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32) -> i32 {
    let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
    let m = pw / 20;
    let (l, rt) = (x0 + m, x0 + pw - m);
    let text = |x: i32, y: i32, f: u32, c: [u8; 3], s: &str, align: i32| {
        if !dry {
            text_at(x, y, y + tall(f) + 2, f, c, &to_latin1(s), align);
        }
    };
    let rule = |y: i32, h: i32, c: [u8; 3]| {
        if !dry {
            fill(l, y, rt, y + h, c);
        }
    };
    let red = [0xe8, 0x6a, 0x5a];
    let mut y = top + m / 2;
    // (in the physio's own words, nothing about the game's workings: user 2026-11-22)
    let said = "Who is out, and who I would keep an eye on this week: the tired ones and the ones whose bodies give way easily. The rest can train and play as normal.";
    for line_text in wrap(fs, &said, rt - l) {
        text(l, y, fs, LIGHT, &latin1(&line_text), 0);
        y += tall(fs) + 1;
    }
    y += 8;
    let col = |pct: i32| l + (rt - l) * pct / 100;
    let injured: Vec<&MedRow> = md.rows.iter().filter(|r| r.out > 0).collect();
    text(l, y, fs, red, &format!("Injured ({})", injured.len()), 0);
    y += tall(fs) + 3;
    if injured.is_empty() {
        text(l, y, ft, GREY, "Nobody: the whole squad trains.", 0);
        y += tall(ft) + 8;
    } else {
        for r in &injured {
            text(l, y, ft, LIGHT, &r.name, 0);
            text(col(40), y, ft, LIGHT, &r.pos, 0);
            text(rt, y, ft, red, &r.advice(), 2);
            y += tall(ft) + 2;
        }
        y += 8;
    }
    let mut watch: Vec<&MedRow> = md.rows.iter().filter(|r| r.to_watch()).collect();
    watch.sort_by_key(|r| (-r.risk(), r.condition));
    text(l, y, fs, GOLD, &format!("To watch ({})", watch.len()), 0);
    y += tall(fs) + 3;
    for (x, head, align) in [(l, "Player", 0), (col(34), "Pos", 0), (col(48), "Age", 2), (col(58), "Cond", 2), (col(72), "Risk", 2), (rt, "Physio's word", 2)] {
        text(x, y, ft, GREY, head, align);
    }
    y += tall(ft) + 2;
    rule(y, 1, [0x4a, 0x50, 0x5a]);
    y += 3;
    for r in watch.iter().take(16) {
        let risky = r.risk() >= 50;
        text(l, y, ft, LIGHT, &r.name, 0);
        text(col(34), y, ft, LIGHT, &r.pos, 0);
        text(col(48), y, ft, LIGHT, &r.age.to_string(), 2);
        text(col(58), y, ft, if r.condition < 80 { red } else { LIGHT }, &format!("{}%", r.condition), 2);
        text(col(72), y, ft, if risky { red } else { LIGHT }, r.risk_word(), 2);
        text(rt, y, ft, LIGHT, &r.advice(), 2);
        y += tall(ft) + 2;
    }
    let fit = md.rows.iter().filter(|r| r.out == 0).count() - watch.len();
    y += 6;
    text(l, y, fs, GREY, &format!("The other {fit} are fit and fresh."), 0);
    y += tall(fs) + 4;
    y
}

/// The youth coach's report (the squad screen's Assistant menu, user 2026-11-22): the club's
/// players of 19 and under, their level now and their potential as the coach sees it. The coach is
/// the one of the club's five (+0x19f) with the most Judging Potential (non-player +0x18, intrinsic)
/// and Working with Youngsters (+0x22) together; his eye is off by up to (20 - judging) x 4 points
/// of potential, the same for a player each time. "New": joined in the last month (staff +0x3e).
#[derive(Clone, Default)]
pub struct Youth {
    pub club: String,
    pub date: String,
    pub coach: String,
    pub judging: i32,
    pub youngsters: i32,
    pub rows: Vec<YouthRow>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct YouthRow {
    pub name: String,
    pub pos: String,
    pub age: i32,
    pub now: i32,  // CA, 0..200
    pub seen: i32, // the potential as the coach sees it, 0..200
    pub new: bool,
}

impl YouthRow {
    pub fn word(&self) -> &'static str {
        match self.seen {
            170.. => "A future star. Keep him close",
            140..=169 => "First-team player in the making",
            110..=139 => "Squad player one day",
            80..=109 => "Needs a loan to grow",
            _ => "I would let him go",
        }
    }
}

fn encode_youth(y: &Youth) -> String {
    let mut out = format!("{}\t{}\t{}\t{}\t{}\n", y.club, y.date, y.coach, y.judging, y.youngsters);
    for r in &y.rows {
        out += &format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.name, r.pos, r.age, r.now, r.seen, r.new as i32);
    }
    out
}

fn decode_youth(text: &str) -> Option<Youth> {
    let mut lines = text.lines();
    let h: Vec<&str> = lines.next()?.split('\t').collect();
    if h.len() != 5 {
        return None;
    }
    let mut y = Youth { club: h[0].into(), date: h[1].into(), coach: h[2].into(), judging: h[3].parse().ok()?, youngsters: h[4].parse().ok()?, rows: Vec::new() };
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 6 {
            let n = |k: usize| f[k].parse::<i32>().unwrap_or(0);
            y.rows.push(YouthRow { name: f[0].into(), pos: f[1].into(), age: n(2), now: n(3), seen: n(4), new: n(5) == 1 });
        }
    }
    Some(y)
}

/// The coach's error on a player: -1..1, the same each time (his id and the player's).
fn eye(coach: i32, player: i32) -> f64 {
    let mut h = (coach as u32).wrapping_mul(0x9e37_79b9) ^ (player as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h % 2001) as f64 / 1000.0 - 1.0
}

/// Host: a club's youth report as text (the host's own page too; a client's key `+`).
pub unsafe fn youth_remote(club: usize) -> Option<Vec<u8>> {
    let club = club_ptr(club)?;
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let month_now = year * 12 + diary::month_of(day, year) - 1;
    // the coach with the best eye and hand for youngsters
    let mut coach = (String::from("the youth coach"), 0, 0, 0);
    for k in 0..5 {
        let s = rd::<usize>(club + 0x19f + 4 * k);
        let np = if readable(s, 0x6e) { rd::<usize>(s + 0x69) } else { 0 };
        if !readable(np, 0x23) {
            continue;
        }
        let (judging, youngsters) = (shown_skill(rd::<i8>(np + 0x18), rd::<i16>(np + 4)), (rd::<i8>(np + 0x22) as i32).clamp(1, 20));
        if judging + youngsters > coach.1 + coach.2 {
            coach = (latin1(&staff_name(s)).into_owned(), judging, youngsters, rd::<i32>(s));
        }
    }
    let mut y = Youth { club: latin1(&game_text(club + 4, 51)).into_owned(), date: date_text(day, year), coach: coach.0, judging: coach.1, youngsters: coach.2, rows: Vec::new() };
    for s in challenge::squad_of(club) {
        let player = rd::<usize>(s + 0x61);
        let age = dash::age_of(s, day, year);
        let Some((ca, pa)) = ca_pa(player).filter(|_| age <= 19 && plays_for(s, club)) else { continue };
        let off = (eye(coach.3, rd::<i32>(s)) * (20 - y.judging.clamp(1, 20)) as f64 * 4.0) as i32;
        let (_, m, jy) = day_month(s + 0x3e);
        y.rows.push(YouthRow {
            name: latin1(&staff_name(s)).replace('\t', " "),
            pos: latin1(&positions_text(s)).into_owned(),
            age,
            now: ca,
            seen: (pa + off).clamp(ca, 200),
            new: jy * 12 + m as i32 >= month_now - 1,
        });
    }
    y.rows.sort_by_key(|r| (!r.new, -r.seen));
    Some(encode_youth(&y).into_bytes())
}

static mut YOUTH: Option<Youth> = None;

/// The chief scout's monthly assignments (`diary::assign`): this month's three and the two months
/// before. Rows: month, line, name, position, age, club, value text, half stars as he sees him.
#[derive(Clone, Default)]
pub struct Scouting {
    pub club: String,
    pub date: String,
    pub scout: String,
    pub rows: Vec<(String, String, String, String, i32, String, String, i32)>,
}

/// Host: a club's scouting report as text (the host's own page too; a client's key `-`).
pub unsafe fn scouting_remote(club: usize) -> Option<Vec<u8>> {
    let club = club_ptr(club)?;
    let seed = seed_of(club)?;
    let (day, year) = (rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    let staff = |id: i32| (0..count).contains(&id).then(|| base + id as usize * 0x6e).filter(|&s| readable(s, 0x6e));
    let money = game_money();
    let all = diary::scouted(seed);
    let scout = all.first().and_then(|a| staff(a.3));
    let mut out = format!("{}\t{}\t{}\n", latin1(&game_text(club + 4, 51)), date_text(day, year), scout.map_or("the chief scout".to_string(), |s| latin1(&staff_name(s)).into_owned()));
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    for a in all.iter().take(3) {
        let judge = staff(a.3).unwrap_or(0);
        for &id in &a.4 {
            let Some(s) = staff(id) else { continue };
            let here = rd::<usize>(s + 0x39);
            let club_name = if readable(here, 0x40) { latin1(&game_text(here + 4, 51)).into_owned() } else { "Free agent".into() };
            let halves = scout::seen_score(s, judge, None).map_or(2, scout::halves) as i32;
            let value = money.full(((s + 0x52) as *const i32).read_unaligned() as i64);
            out += &format!(
                "{} {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                MONTHS[(a.1 as usize).clamp(1, 12) - 1],
                a.0,
                diary::KEY_ATTRIBUTES[a.2.min(5)].0,
                latin1(&staff_name(s)).replace('\t', " "),
                latin1(&positions_text(s)),
                dash::age_of(s, day, year),
                club_name.replace('\t', " "),
                value,
                halves
            );
        }
    }
    Some(out.into_bytes())
}

fn decode_scouting(text: &str) -> Option<Scouting> {
    let mut lines = text.lines();
    let h: Vec<&str> = lines.next()?.split('\t').collect();
    if h.len() != 3 {
        return None;
    }
    let mut sc = Scouting { club: h[0].into(), date: h[1].into(), scout: h[2].into(), rows: Vec::new() };
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 8 {
            sc.rows.push((f[0].into(), f[1].into(), f[2].into(), f[3].into(), f[4].parse().unwrap_or(0), f[5].into(), f[6].into(), f[7].parse().unwrap_or(2)));
        }
    }
    Some(sc)
}

static mut SCOUTING: Option<Scouting> = None;

/// Opens the chief scout's report of a club (the Assistant menu's item).
pub unsafe fn open_scouting(club: usize) {
    let text = if remote::CLIENT { ask_host(&key(b'-', &[club as u32])) } else { scouting_remote(club) };
    SCOUTING = text.and_then(|t| decode_scouting(&String::from_utf8_lossy(&t)));
    if SCOUTING.is_some() {
        (PAGE, OPEN, OPENED_AT) = (PAGE_SCOUTING, true, Some(std::time::Instant::now()));
    }
}

/// The chief scout's page (or only measures it: `dry`). Returns its last line's end.
#[allow(clippy::too_many_arguments)]
unsafe fn paint_scouting(sc: &Scouting, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32) -> i32 {
    let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
    let m = pw / 20;
    let (l, rt) = (x0 + m, x0 + pw - m);
    let text = |x: i32, y: i32, f: u32, c: [u8; 3], s: &str, align: i32| {
        if !dry {
            text_at(x, y, y + tall(f) + 2, f, c, &to_latin1(s), align);
        }
    };
    let rule = |y: i32, h: i32, c: [u8; 3]| {
        if !dry {
            fill(l, y, rt, y + h, c);
        }
    };
    let mut y = top + m / 2;
    if sc.rows.is_empty() {
        for line_text in wrap(fs, "No assignment yet: I report at the start of every month with three names for where we need it most.", rt - l) {
            text(l, y, fs, LIGHT, &latin1(&line_text), 0);
            y += tall(fs) + 1;
        }
        return y;
    }
    let col = |pct: i32| l + (rt - l) * pct / 100;
    let star = tall(ft) - 2;
    let mut month = String::new();
    for (k, r) in sc.rows.iter().enumerate() {
        if r.0 != month {
            month = r.0.clone();
            y += if k > 0 { 8 } else { 0 };
            let head = if k == 0 { format!("{}: a {} better than ours", r.0, r.1.to_lowercase()) } else { format!("{}: {}", r.0, r.1.to_lowercase()) };
            text(l, y, fs, GOLD, &head, 0);
            y += tall(fs) + 3;
            for (x, h, align) in [(l, "Player", 0), (col(30), "Pos", 0), (col(42), "Age", 2), (col(45), "Club", 0), (col(80), "Value", 2), (col(82), "Rating", 0)] {
                text(x, y, ft, GREY, h, align);
            }
            y += tall(ft) + 2;
            rule(y, 1, [0x4a, 0x50, 0x5a]);
            y += 3;
        }
        let f = if k < 3 { fs } else { ft };
        text(l, y, f, LIGHT, &r.2, 0);
        text(col(30), y, ft, LIGHT, &r.3, 0);
        text(col(42), y, ft, LIGHT, &r.4.to_string(), 2);
        text(col(45), y, ft, LIGHT, &r.5, 0);
        text(col(80), y, ft, LIGHT, &r.6, 2);
        if !dry {
            stars(col(82), y + 2, star, r.7 * 20, GOLD);
        }
        y += tall(f) + 3;
    }
    y + 4
}

/// Opens the youth coach's report of a club (the Assistant menu's item), the host's or asked of it.
pub unsafe fn open_youth(club: usize) {
    let text = if remote::CLIENT { ask_host(&key(b'+', &[club as u32])) } else { youth_remote(club) };
    YOUTH = text.and_then(|t| decode_youth(&String::from_utf8_lossy(&t)));
    if YOUTH.is_some() {
        (PAGE, OPEN, OPENED_AT) = (PAGE_YOUTH, true, Some(std::time::Instant::now()));
    }
}

/// The youth coach's page (or only measures it: `dry`). Returns its last line's end.
#[allow(clippy::too_many_arguments)]
unsafe fn paint_youth(yr: &Youth, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32) -> i32 {
    let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
    let m = pw / 20;
    let (l, rt) = (x0 + m, x0 + pw - m);
    let text = |x: i32, y: i32, f: u32, c: [u8; 3], s: &str, align: i32| {
        if !dry {
            text_at(x, y, y + tall(f) + 2, f, c, &to_latin1(s), align);
        }
    };
    let rule = |y: i32, h: i32, c: [u8; 3]| {
        if !dry {
            fill(l, y, rt, y + h, c);
        }
    };
    let green = [0x5a, 0xc8, 0x8c];
    let mut y = top + m / 2;
    let new = yr.rows.iter().filter(|r| r.new).count();
    let said = if new > 0 {
        format!("{new} new face{} this month, marked new. Below is how I rate every youngster here: where he stands now, and how far I think he can go.", if new == 1 { "" } else { "s" })
    } else {
        "How I rate every youngster here: where he stands now, and how far I think he can go.".to_string()
    };
    for line_text in wrap(fs, &said, rt - l) {
        text(l, y, fs, LIGHT, &latin1(&line_text), 0);
        y += tall(fs) + 1;
    }
    y += 8;
    let col = |pct: i32| l + (rt - l) * pct / 100;
    for (x, head, align) in [(l, "Player", 0), (col(30), "Pos", 0), (col(42), "Age", 2), (col(46), "Now", 0), (col(60), "Potential", 0), (rt, "Coach's word", 2)] {
        text(x, y, ft, GREY, head, align);
    }
    y += tall(ft) + 2;
    rule(y, 1, [0x4a, 0x50, 0x5a]);
    y += 3;
    if yr.rows.is_empty() {
        text(l, y, ft, GREY, "No players of 19 or under at the club.", 0);
        y += tall(ft) + 2;
    }
    let star = tall(ft) - 2;
    for r in yr.rows.iter().take(26) {
        text(l, y, ft, if r.new { green } else { LIGHT }, &if r.new { format!("{}  new", r.name) } else { r.name.clone() }, 0);
        text(col(30), y, ft, LIGHT, &r.pos, 0);
        text(col(42), y, ft, LIGHT, &r.age.to_string(), 2);
        if !dry {
            stars(col(46), y + 2, star, r.now, [0x9a, 0xa0, 0xab]);
            stars(col(60), y + 2, star, r.seen, GOLD);
        }
        text(rt, y, ft, LIGHT, r.word(), 2);
        y += tall(ft) + 3;
    }
    y + 4
}

/// Pays a sum from a club's bank as the game pays its ground works (`pay_out(f, sum, 0x8c,
/// 0x12c)`: this month's and this year's ground costs, off the balance); false without the money
/// (the stadium builder, user 2026-11-23).
/// `debt`: paid also when the bank cannot (a new stadium's monthly instalment).
pub unsafe fn pay_ground(club: usize, sum: i64, debt: bool) -> bool {
    let Some(club) = club_ptr(club) else { return false };
    let table = rd::<usize>(FINANCE_TABLE);
    let record = if table == 0 { 0 } else { game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club) };
    if !readable(record, RECORD) || sum < 0 || sum > i32::MAX as i64 {
        return false;
    }
    let balance = (record as *const i64).read_unaligned();
    if balance < sum && !debt {
        return false;
    }
    for at in [0x8c, 0x12c] {
        let p = (record + at) as *mut i32;
        p.write_unaligned(p.read_unaligned().wrapping_add(sum as i32));
    }
    (record as *mut i64).write_unaligned(balance - sum);
    true
}

/// A club's finance record.
unsafe fn record_of(club: usize) -> Option<usize> {
    let club = club_ptr(club)?;
    let table = rd::<usize>(FINANCE_TABLE);
    let record = if table == 0 { 0 } else { game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club) };
    readable(record, RECORD).then_some(record)
}

/// A club's merchandising this month (record +0xbc, as the game's Sunday pays it in, 0x59b7e0).
pub unsafe fn merch_month(club: usize) -> Option<i32> {
    record_of(club).map(|r| ((r + 0xbc) as *const i32).read_unaligned())
}

/// A club's merchandising this season (record +0x15c).
pub unsafe fn merch_year(club: usize) -> i64 {
    record_of(club).map_or(0, |r| ((r + 0x15c) as *const i32).read_unaligned() as i64)
}

/// More merchandising paid in (the club shop, `shop.rs`): this month's and this year's lines and
/// the bank, as the game's own Sunday does.
pub unsafe fn add_merch(club: usize, sum: i64) -> bool {
    let Some(record) = record_of(club) else { return false };
    for at in [0xbc, 0x15c] {
        let p = (record + at) as *mut i32;
        p.write_unaligned(p.read_unaligned().wrapping_add(sum as i32));
    }
    let balance = (record as *const i64).read_unaligned();
    (record as *mut i64).write_unaligned(balance + sum);
    true
}

/// A test script's money (`money n` in cmui.click): the bank balance only, no income line.
pub unsafe fn add_money(club: usize, sum: i64) -> bool {
    let Some(club) = club_ptr(club) else { return false };
    let table = rd::<usize>(FINANCE_TABLE);
    let record = if table == 0 { 0 } else { game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club) };
    if !readable(record, RECORD) {
        return false;
    }
    let balance = (record as *const i64).read_unaligned();
    (record as *mut i64).write_unaligned(balance + sum);
    true
}

/// How the game shows money now (its currency and rate).
pub unsafe fn money_now() -> Money {
    game_money()
}

/// A club's index in the club table (the report's seed, the diary's key).
pub unsafe fn seed_of(club: usize) -> Option<usize> {
    let club = club_ptr(club)?;
    Some((club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE)
}

/// A club's balance, this month's result and last month's, in the game's units (the diary's
/// daily reading: no currency asked, nothing logged).
pub unsafe fn balance_now(club: usize) -> Option<(i64, i64, i64)> {
    let club = club_ptr(club)?;
    let table = rd::<usize>(FINANCE_TABLE);
    if table == 0 {
        return None;
    }
    let record = game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club);
    if !readable(record, RECORD) {
        return None;
    }
    let rec = std::slice::from_raw_parts(record as *const u8, RECORD);
    Some((i64_at(rec, 0), column(rec, BLOCKS[0]).result(), column(rec, BLOCKS[1]).result()))
}

/// Host: what a club's report is made of, as text (also the host's own page goes this way).
pub unsafe fn remote(club: usize) -> Option<Vec<u8>> {
    let club = club_ptr(club)?;
    let (cols, balance, funds, money) = figures(club)?;
    let name = latin1(&game_text(club + 4, 51)).into_owned();
    let seed = (club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE;
    let date = date_text(rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    Some(encode(&name, seed, &date, &cols, balance, funds, &money).into_bytes())
}

/// The figures a field a tab: name, seed, date, balance, funds, currency symbol and rate, then
/// the four columns' eleven figures each.
fn encode(name: &str, seed: usize, date: &str, cols: &[Col; 4], balance: i64, funds: i64, money: &Money) -> String {
    let mut f: Vec<String> = vec![name.into(), seed.to_string(), date.into(), balance.to_string(), funds.to_string(), money.symbol.clone(), money.rate.to_string()];
    for c in cols {
        f.extend([c.gate, c.tickets, c.tv, c.merch, c.sold, c.other_in, c.wages, c.bonuses, c.bought, c.ground, c.other_out].map(|v| v.to_string()));
    }
    f.join("\t")
}

#[allow(clippy::type_complexity)]
fn decode(text: &str) -> Option<(String, usize, String, [Col; 4], i64, i64, Money)> {
    let f: Vec<&str> = text.split('\t').collect();
    if f.len() != 7 + 4 * 11 {
        return None;
    }
    let n = |k: usize| f[k].parse::<i64>().ok();
    let mut cols = [Col::default(); 4];
    for (k, c) in cols.iter_mut().enumerate() {
        let v = |i: usize| n(7 + 11 * k + i);
        *c = Col { gate: v(0)?, tickets: v(1)?, tv: v(2)?, merch: v(3)?, sold: v(4)?, other_in: v(5)?, wages: v(6)?, bonuses: v(7)?, bought: v(8)?, ground: v(9)?, other_out: v(10)? };
    }
    let money = Money { symbol: f[5].into(), rate: f[6].parse().ok()? };
    Some((f[0].into(), f[1].parse().ok()?, f[2].into(), cols, n(3)?, n(4)?, money))
}

/// A club's account (this month, last month, this year, last year), bank balance and transfer
/// funds in the game's units, and how the game shows money (the P&L report, the dashboard export).
pub unsafe fn figures(club: usize) -> Option<([Col; 4], i64, i64, Money)> {
    let table = rd::<usize>(FINANCE_TABLE);
    if table == 0 {
        return None;
    }
    let record = game::<extern "thiscall" fn(usize, usize) -> usize>(FINANCE_FOR_CLUB)(table, club);
    if !readable(record, RECORD) {
        return None;
    }
    let rec = std::slice::from_raw_parts(record as *const u8, RECORD);
    let cols = BLOCKS.map(|b| column(rec, b));
    // (the game's own transfer funds figure; below zero it shows nothing)
    let money = game_money();
    let raw = game::<extern "thiscall" fn(usize, usize, i32, usize, i32, i32) -> i64>(TRANSFER_BUDGET)(record, 0, 0, 0, 1, 0).max(0);
    // (the screen then has the game's Cash::convert 0x43f460 turn it into the shown currency, rounded
    // by flags 6; taken back into the game's units so the page's own conversion gives the same figure)
    let mut cash = [raw as u32, 0, 0, 0];
    let shown = game::<extern "thiscall" fn(*mut u32, i32) -> i64>(CASH_CONVERT)(cash.as_mut_ptr(), 6).max(0);
    let funds = (shown as f64 / money.rate).round() as i64;
    Some((cols, i64_at(rec, 0), funds, money))
}

unsafe fn button_rect() -> Option<(i32, i32, i32, i32, u32)> {
    let (_, y1, x2, y2, font) = BUTTON?;
    let w = text_width(font, b"P&L report") + 56;
    Some((x2 + 14, y1, x2 + 14 + w, y2, font))
}

/// One of cmui's own pages is over the screen: the Finances tab is still the game's under it, and
/// the button stood on CM Challenge's page (user 2026-11-22).
pub(crate) fn covered() -> bool {
    challenge::page_open() || coach::page_open() || season::page_open() || natpool::page_open() || teletext::page_open() || stadium::page_open()
}

/// The flip pass: the button while the money views show, the report while it is open.
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    let fin = std::mem::take(&mut FIN_SEEN);
    let view = VIEW_SEEN.take();
    if fin {
        if let Some(v) = view {
            BUTTON = Some(v);
        }
    } else if full_frame {
        // (another tab of the club screen is the same screen: the report stayed over it, user
        // 2026-11-22; a whole frame without the money views closes it)
        BUTTON = None;
        // (the physio's and the youth coach's reports are the squad screen's, not the money views')
        if PAGE == PAGE_PL || PAGE == PAGE_PLAN {
            OPEN = false;
        }
    }
    if OPEN {
        draw();
        return;
    }
    if covered() {
        return;
    }
    let Some(b) = button_rect() else { return };
    let (blue, hover) = ([0x2c, 0x46, 0x6a], [0x3a, 0x5a, 0x86]);
    let mut buttons = vec![(b, BTN_HOVER, "P&L report"), (plan_rect(b), PLAN_HOVER, "Contracts")];
    if on(STADIUM) {
        buttons.push((stadium_rect(b), STADIUM_HOVER, "Stadium"));
    }
    for (b, lit, label) in buttons {
        fill(b.0, b.1, b.2, b.3, if lit { hover } else { blue });
        fill(b.0, b.1, b.2, b.1 + 1, [0x4a, 0x6a, 0x96]);
        put(b.0, b.1, b.2, b.3, 0xc, b.4, WHITE, label);
    }
}

/// The Stadium button (switch `stadium`), right of Contracts.
unsafe fn stadium_rect(b: (i32, i32, i32, i32, u32)) -> (i32, i32, i32, i32, u32) {
    let p = plan_rect(b);
    let w = text_width(b.4, b"Stadium") + 56;
    (p.2 + 10, b.1, p.2 + 10 + w, b.3, b.4)
}
static mut STADIUM_HOVER: bool = false;

/// The Contracts button, right of the P&L report's.
unsafe fn plan_rect(b: (i32, i32, i32, i32, u32)) -> (i32, i32, i32, i32, u32) {
    let w = text_width(b.4, b"Contracts") + 56;
    (b.2 + 10, b.1, b.2 + 10 + w, b.3, b.4)
}

/// Which page is open: the P&L report (false) or the planner; the planner's figures.
static mut PAGE: u8 = PAGE_PL;
/// The balance chart's height in lines of text (the page's fit picks it, `draw`).
static mut CHART_ROWS: i32 = 5;
const PAGE_PL: u8 = 0;
const PAGE_PLAN: u8 = 1;
const PAGE_MEDICAL: u8 = 2;
const PAGE_YOUTH: u8 = 3;
const PAGE_SCOUTING: u8 = 4;
static mut MEDICAL: Option<Medical> = None;
static mut PLAN: Option<Plan> = None;
static mut PLAN_HOVER: bool = false;

/// The mouse: the button opens the report; while it is open it takes every click.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    let inside = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if !OPEN {
        if covered() {
            return None;
        }
        let b = button_rect()?;
        let p = plan_rect(b);
        let (over, over_plan) = (inside((b.0, b.1, b.2, b.3)), inside((p.0, p.1, p.2, p.3)));
        let st = stadium_rect(b);
        let over_stadium = on(STADIUM) && inside((st.0, st.1, st.2, st.3));
        let changed = (std::mem::replace(&mut BTN_HOVER, over) != over) | (std::mem::replace(&mut PLAN_HOVER, over_plan) != over_plan) | (std::mem::replace(&mut STADIUM_HOVER, over_stadium) != over_stadium);
        // the ground, its own page (`stadium.rs`), of the club the screen shows
        if released && over_stadium {
            *(LEFT_CLICK as *mut u32) = 0;
            // (a test setting: `stadium_test_club=<name>` opens that club's ground instead, for
            // pictures of a big club's; computer clubs have no Finances screen of their own)
            let test = setting("stadium_test_club").and_then(|n| club_named(n.trim().as_bytes()));
            if let Some(club) = test.into_iter().chain([screen_param(0), SQUAD_CLUB, human_club_ptr().unwrap_or(0)].into_iter().filter_map(|p| club_ptr(p))).next() {
                stadium::open(club);
            }
            return Some(true);
        }
        if released && (over || over_plan) {
            *(LEFT_CLICK as *mut u32) = 0;
            PAGE = if over_plan { PAGE_PLAN } else { PAGE_PL };
            if over_plan {
                PLAN = read_plan();
                OPEN = PLAN.is_some();
            } else {
                REPORT = read();
                OPEN = REPORT.is_some();
            }
            OPENED_AT = Some(std::time::Instant::now());
            return Some(true);
        }
        return changed.then_some(true);
    }
    // the game's menu bar stays the game's: a click there leaves the report (as CM Challenge's page)
    if mx < bar_end(screen().0) {
        if released {
            OPEN = false;
            REDRAW = true;
        }
        return None;
    }
    *(LEFT_CLICK as *mut u32) = 0;
    *(assistant::RIGHT_CLICK as *mut u32) = 0;
    let back = back_rect();
    let over = inside(back);
    let mut redraw = std::mem::replace(&mut HOVER, over) != over;
    // (a release counts only after a press seen while the report is up, as the season card)
    static mut PRESSED_AT: Option<std::time::Instant> = None;
    static mut WAS_HELD: bool = true;
    let held = *(BUTTON_HELD as *const u32) != 0;
    let was = std::mem::replace(&mut WAS_HELD, held);
    if held && !was {
        PRESSED_AT = Some(std::time::Instant::now());
    }
    let settled = PRESSED_AT.is_some_and(|p| OPENED_AT.is_some_and(|t| p > t));
    if released && over && settled {
        OPEN = false;
        return Some(true);
    }
    let sv = save_rect();
    let on_save = inside(sv);
    redraw |= std::mem::replace(&mut SAVE_HOVER, on_save) != on_save;
    if released && on_save && settled {
        SAVE = true;
        return Some(true);
    }
    Some(redraw)
}

fn back_rect() -> (i32, i32, i32, i32) {
    let (sw, sh) = screen();
    let m = if small_screen() { (sh / 88).max(6) } else { 20 }; // (user 2026-11-13 polish)
    (sw - m - sw / 8, sh - m - sh / 20, sw - m, sh - m)
}

fn save_rect() -> (i32, i32, i32, i32) {
    let b = back_rect();
    let w = (b.2 - b.0) * 5 / 4;
    (b.0 - 12 - w, b.1, b.0 - 12, b.3)
}

// ---- the page ----

const PAPER: [u8; 3] = [0xfb, 0xfa, 0xf6];
const INK: [u8; 3] = [0x1d, 0x23, 0x30];
const MUTED: [u8; 3] = [0x5b, 0x62, 0x70];
const NAVY: [u8; 3] = [0x1f, 0x3a, 0x5f];
const BRASS: [u8; 3] = [0xc9, 0xa5, 0x4a];
const LOSS: [u8; 3] = [0xa3, 0x2d, 0x2d];
const BOX: [u8; 3] = [0xee, 0xe9, 0xdb];

/// A text in lines of at most `width` pixels.
unsafe fn wrap(font: u32, text: &str, width: i32) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut line: Vec<u8> = Vec::new();
    for word in to_latin1(text).split(|&c| c == b' ') {
        let mut next = line.clone();
        if !next.is_empty() {
            next.push(b' ');
        }
        next.extend_from_slice(word);
        if !line.is_empty() && text_width(font, &next) > width {
            lines.push(std::mem::replace(&mut line, word.to_vec()));
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The page, over the whole screen. `paint` runs first only measuring with the roomier fonts, and
/// if that does not fit the paper, it draws with the smaller ones.
#[allow(static_mut_refs)]
unsafe fn draw() {
    let r = REPORT.clone().unwrap_or_default();
    let plan = if PAGE == PAGE_PLAN { PLAN.clone() } else { None };
    let medical = if PAGE == PAGE_MEDICAL { MEDICAL.clone() } else { None };
    let youth = if PAGE == PAGE_YOUTH { YOUTH.clone() } else { None };
    let scouting = if PAGE == PAGE_SCOUTING { SCOUTING.clone() } else { None };
    if (PAGE == PAGE_PL && REPORT.is_none()) || (PAGE == PAGE_PLAN && plan.is_none()) || (PAGE == PAGE_MEDICAL && medical.is_none()) || (PAGE == PAGE_YOUTH && youth.is_none()) || (PAGE == PAGE_SCOUTING && scouting.is_none()) {
        return;
    }
    // (the page's painter: the report, the planner, the physio's, the youth coach's or the chief
    // scout's report, on the same paper)
    let paint = |r: &Report, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32| {
        if let Some(p) = &plan {
            paint_plan(p, dry, x0, top, pw, fb, fm, fs, ft)
        } else if let Some(m) = &medical {
            paint_medical(m, dry, x0, top, pw, fb, fm, fs, ft)
        } else if let Some(y) = &youth {
            paint_youth(y, dry, x0, top, pw, fb, fm, fs, ft)
        } else if let Some(s) = &scouting {
            paint_scouting(s, dry, x0, top, pw, fb, fm, fs, ft)
        } else {
            paint(r, dry, x0, top, pw, fb, fm, fs, ft)
        }
    };
    let (sw, sh) = screen();
    fill(0, 0, sw - 1, sh - 1, BACKGROUND);
    let (f1, f2, f3, f5) = (0xffff0001u32, 0xffff0002u32, 0xffff0003u32, 0xffff0005u32);
    // the staff's reports in the Team report's style (user 2026-11-22: "meer in de team report
    // stijl"): a dark bar with the title and who wrote it, the page on a dark panel; the money
    // pages stay on paper
    let staff_page = match (&medical, &youth, &scouting) {
        (Some(m), ..) => Some((&m.club, "Physio's report", format!("{}  -  {}, head physio ({}/20)", m.date, m.physio, m.skill))),
        (_, Some(y), _) => Some((&y.club, "Youth report", format!("{}  -  {}, youth coach (judging potential {}/20, working with youngsters {}/20)", y.date, y.coach, y.judging, y.youngsters))),
        (.., Some(s)) => Some((&s.club, "Scouting report", format!("{}  -  {}, chief scout", s.date, s.scout))),
        _ => None,
    };
    let (top, ph, pw) = if let Some((club, title, by)) = &staff_page {
        let bar = sh * 8 / 100;
        fill(0, 0, sw - 1, bar, [0x22, 0x26, 0x2c]);
        fill(0, bar - 3, sw - 1, bar, GOLD);
        let m = (sh / 88).max(6);
        text_at(m * 3, 4, bar * 58 / 100, 0xffff0006, WHITE, &to_latin1(&format!("{club} - {title}")), 0);
        text_at(m * 3, bar * 54 / 100, bar - 6, f3, LIGHT, &to_latin1(by), 0);
        let top = bar + m * 2;
        (top, sh - sh / 10 - top, (sw * 72 / 100).min(sw - 40))
    } else {
        let top = sh / 60;
        let ph = sh - sh / 10 - top;
        (top, ph, (ph * 82 / 100).min(sw - 40))
    };
    let x0 = (sw - pw) / 2;
    let bottom = top + ph;
    // (title, table, text, fine print): the table in the small body font so the columns clear the labels
    // (the balance chart made the report a few lines longer than the smallest tier: its signature
    // and fine print hung under the paper, user 2026-11-23. A smaller tier, then a lower chart)
    let tiers = [(f5, f2, f2, f1, 5), (f3, f2, f1, f1, 5), (f3, f1, f1, f1, 4), (f2, f1, f1, f1, 3)];
    let tier = *tiers
        .iter()
        .find(|t| {
            CHART_ROWS = t.4;
            paint(&r, true, x0, top, pw, t.0, t.1, t.2, t.3) <= bottom - 8
        })
        .unwrap_or(&tiers[3]);
    CHART_ROWS = tier.4;
    if staff_page.is_some() {
        rounded(x0, top, x0 + pw, bottom, AREA, 6);
    } else {
        fill(x0 - 1, top - 1, x0 + pw + 1, bottom + 1, [0xc9, 0xc3, 0xb3]);
        fill(x0, top, x0 + pw, bottom, PAPER);
    }
    paint(&r, false, x0, top, pw, tier.0, tier.1, tier.2, tier.3);
    if std::mem::take(&mut SAVE) {
        let (kind, club, date) = match (&plan, &medical, &youth, &scouting) {
            (Some(p), ..) => ("contracts", &p.club, &p.date),
            (_, Some(m), ..) => ("medical", &m.club, &m.date),
            (_, _, Some(y), _) => ("youth", &y.club, &y.date),
            (.., Some(s)) => ("scouting", &s.club, &s.date),
            _ => ("pl", &r.club, &r.date),
        };
        let club: String = club.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let name = format!("{kind}_{}_{}", club, date.replace(' ', "_"));
        SAVED = Some((save_png(x0, top, x0 + pw + 1, bottom + 1, &name).map_or("Could not save the picture".to_string(), |p| format!("Saved: {p}")), std::time::Instant::now()));
    }
    let b = back_rect();
    // (user 2026-11-13 polish: at 1680 rounded, in from the corner)
    let (bc, sc) = (if HOVER { [0x3a, 0x5a, 0x86] } else { [0x2c, 0x46, 0x6a] }, if SAVE_HOVER { [0x3a, 0x5a, 0x86] } else { [0x24, 0x28, 0x30] });
    if small_screen() {
        fill(b.0, b.1, b.2, b.3, bc);
    } else {
        rounded(b.0, b.1, b.2, b.3, bc, 6);
    }
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, WHITE, b"Back", 1);
    let sv = save_rect();
    if small_screen() {
        fill(sv.0, sv.1, sv.2, sv.3, sc);
    } else {
        rounded(sv.0, sv.1, sv.2, sv.3, sc, 6);
    }
    text_at((sv.0 + sv.2) / 2, sv.1, sv.3, f3, WHITE, b"Save image", 1);
    if let Some((note, _)) = SAVED.as_ref().filter(|s| s.1.elapsed().as_secs() < 6) {
        text_at(sv.0 - 12, sv.1, sv.3, f2, GOLD, &to_latin1(note), 2);
    }
}

/// The planner's page (or only measures it: `dry`): the wage budget a year, then the contracts
/// that end this season and next, a line each; the later ones counted. Returns its last line's end.
#[allow(clippy::too_many_arguments)]
unsafe fn paint_plan(p: &Plan, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32) -> i32 {
    let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
    let m = pw / 20;
    let (l, rt) = (x0 + m, x0 + pw - m);
    let text = |x: i32, y: i32, f: u32, c: [u8; 3], s: &str, align: i32| {
        if !dry {
            text_at(x, y, y + tall(f) + 2, f, c, &to_latin1(s), align);
        }
    };
    let rule = |y: i32, h: i32, c: [u8; 3]| {
        if !dry {
            fill(l, y, rt, y + h, c);
        }
    };
    let money = &p.money;
    let mut y = top + m / 2;
    text(l, y, fb, NAVY, "Accountants & Co", 0);
    text(rt, y + 2, fs, MUTED, "Private and confidential", 2);
    y += tall(fb) + 6;
    rule(y, 2, NAVY);
    y += 12;
    text((l + rt) / 2, y, fb, INK, &p.club, 1);
    y += tall(fb) + 2;
    text((l + rt) / 2, y, fm, INK, "Contract and wage planner", 1);
    y += tall(fm) + 2;
    text((l + rt) / 2, y, fs, MUTED, &format!("As at {} - wages a year (the club pays them weekly)", p.date), 1);
    y += tall(fs) + 10;
    // the wage budget
    let bw = (rt - l - 20) / 3;
    let bh = tall(fs) + tall(fm) + 12;
    // (the finance record's +0x14 is the most the club can offer one new man, 0x5968d0 asked for
    // nobody, not a budget for the whole squad: "room" from it went millions negative, user
    // 2026-11-22)
    let top = p.rows.iter().map(|r| r.wage).max().unwrap_or(0) * 52;
    let boxes = [
        ("Wage bill", money.full(p.bill * 52), INK),
        ("Top wage in the squad", money.full(top), INK),
        ("Most it can offer a newcomer", money.full(p.can_pay * 52), [0x1f, 0x6b, 0x3a]),
    ];
    for (k, (label, value, colour)) in boxes.iter().enumerate() {
        let bx = l + k as i32 * (bw + 10);
        if !dry {
            fill(bx, y, bx + bw, y + bh, BOX);
        }
        text(bx + 8, y + 4, fs, MUTED, label, 0);
        text(bx + 8, y + 6 + tall(fs), fm, *colour, value, 0);
    }
    y += bh + 10;
    let ending = p.rows.iter().filter(|r| Plan::ends_by(r, p.season_end)).count();
    let said = if ending == 0 {
        format!("No contract runs out by 30 June {}.", p.season_end)
    } else {
        format!("{ending} contract{} run{} out by 30 June {}: {} a year comes free when they leave.", if ending == 1 { "" } else { "s" }, if ending == 1 { "s" } else { "" }, p.season_end, money.full(p.freed()))
    };
    for line_text in wrap(fs, &said, rt - l) {
        text(l, y, fs, INK, &latin1(&line_text), 0);
        y += tall(fs) + 1;
    }
    y += 8;
    // the columns
    let col = |pct: i32| l + (rt - l) * pct / 100;
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    for (title, from, to) in [(format!("Running out by 30 June {}", p.season_end), i32::MIN, p.season_end), (format!("Running out by 30 June {}", p.season_end + 1), p.season_end, p.season_end + 1)] {
        let rows: Vec<&PlanRow> = p.rows.iter().filter(|r| Plan::ends_by(r, to) && !Plan::ends_by(r, from)).collect();
        text(l, y, fs, NAVY, &title, 0);
        y += tall(fs) + 3;
        if rows.is_empty() {
            text(l, y, ft, MUTED, "None", 0);
            y += tall(ft) + 8;
            continue;
        }
        for (x, head, align) in [(l, "Player", 0), (col(40), "Pos", 0), (col(56), "Age", 2), (col(78), "Wage", 2), (col(88), "Ends", 2), (rt, "Value", 2)] {
            text(x, y, ft, MUTED, head, align);
        }
        y += tall(ft) + 2;
        rule(y, 1, INK);
        y += 3;
        for r in rows {
            text(l, y, ft, INK, &r.name, 0);
            text(col(40), y, ft, INK, &r.pos, 0);
            text(col(56), y, ft, INK, &r.age.to_string(), 2);
            text(col(78), y, ft, INK, &money.full(r.wage * 52), 2);
            text(col(88), y, ft, INK, &format!("{} {}", MONTHS[r.end_month.clamp(0, 11) as usize], r.end_year), 2);
            text(rt, y, ft, INK, &money.full(r.value), 2);
            y += tall(ft) + 2;
        }
        y += 8;
    }
    // the rest, counted
    let later: Vec<&PlanRow> = p.rows.iter().filter(|r| !Plan::ends_by(r, p.season_end + 1)).collect();
    if !later.is_empty() {
        let wages: i64 = later.iter().map(|r| r.wage).sum::<i64>() * 52;
        text(l, y, fs, MUTED, &format!("{} more under contract beyond June {}, together {} a year.", later.len(), p.season_end + 1, money.full(wages)), 0);
        y += tall(fs) + 4;
    }
    y
}

/// Draws the page (or only measures it: `dry`); returns where its last line ends.
#[allow(clippy::too_many_arguments)]
unsafe fn paint(r: &Report, dry: bool, x0: i32, top: i32, pw: i32, fb: u32, fm: u32, fs: u32, ft: u32) -> i32 {
    let tall = |f: u32| game::<extern "C" fn(i32) -> i32>(FONT_HEIGHT)((f & 0xffff) as i32);
    let m = pw / 20;
    let (l, rt) = (x0 + m, x0 + pw - m);
    let text = |x: i32, y: i32, f: u32, c: [u8; 3], s: &[u8], align: i32| {
        if !dry {
            text_at(x, y, y + tall(f) + 2, f, c, s, align);
        }
    };
    let rule = |y: i32, h: i32, c: [u8; 3]| {
        if !dry {
            fill(l, y, rt, y + h, c);
        }
    };
    let mut y = top + m / 2;
    // letterhead: the logo, the firm, the reference
    let d = tall(fb) * 2;
    if !dry {
        let (cx, cy, rad) = (l + d / 2, y + d / 2, d / 2);
        dot(cx, cy, rad, BRASS);
        dot(cx, cy, rad - 3, NAVY);
        circle(cx, cy, rad - 6, 1.5, BRASS);
        text_at(cx, cy - tall(ft) / 2 - 2, cy + tall(ft), ft, [0xf2, 0xe6, 0xc2], b"A&C", 1);
        fill(cx - rad / 3, cy + tall(ft) / 2 + 2, cx + rad / 3, cy + tall(ft) / 2 + 3, BRASS);
    }
    text(l + d + 12, y + 2, fb, NAVY, b"Accountants & Co", 0);
    text(l + d + 12, y + 4 + tall(fb), fs, MUTED, b"Chartered accountants and business advisers - est. 1891", 0);
    text(rt, y + 2, fs, MUTED, b"Private and confidential", 2);
    text(rt, y + 4 + tall(fs), fs, MUTED, to_latin1(&format!("Ref. {}", r.reference)).as_slice(), 2);
    y += d + 8;
    rule(y, 2, NAVY);
    y += 12;
    // the title
    text((l + rt) / 2, y, fb, INK, to_latin1(&r.club).as_slice(), 1);
    y += tall(fb) + 2;
    text((l + rt) / 2, y, fm, INK, b"Profit and loss account", 1);
    y += tall(fm) + 2;
    text((l + rt) / 2, y, fs, MUTED, to_latin1(&format!("Management accounts to {} - unaudited", r.date)).as_slice(), 1);
    y += tall(fs) + 10;
    // the table
    let cw = (pw - 2 * m) * 16 / 100;
    let edge = |k: usize| rt - (3 - k as i32) * cw;
    for (k, head) in ["This month", "Last month", "This year", "Last year"].iter().enumerate() {
        text(edge(k), y, fs, MUTED, head.as_bytes(), 2);
    }
    y += tall(fs) + 3;
    rule(y, 1, INK);
    y += 4;
    let row_h = tall(fm) + tall(fm) / 3 + 3;
    let section = |y: &mut i32, name: &str| {
        *y += row_h / 3;
        text(l, *y, fs, NAVY, name.as_bytes(), 0);
        *y += tall(fs) + 4;
    };
    let line = |y: &mut i32, name: &str, v: [i64; 4], kind: u8| {
        match kind {
            1 => rule(*y, 1, [0x9a, 0xa0, 0xab]),
            2 => rule(*y, 1, INK),
            _ => {}
        }
        if kind > 0 {
            *y += 2;
        }
        text(l + if kind == 0 { m / 2 } else { 0 }, *y, fm, INK, name.as_bytes(), 0);
        for (k, &x) in v.iter().enumerate() {
            text(edge(k), *y, fm, if x < 0 { LOSS } else { INK }, to_latin1(&r.money.full(x)).as_slice(), 2);
        }
        *y += row_h;
        if kind == 2 {
            rule(*y - 1, 1, INK);
            rule(*y + 1, 1, INK);
            *y += 6;
        }
    };
    let c = &r.cols;
    let each = |f: fn(&Col) -> i64| [f(&c[0]), f(&c[1]), f(&c[2]), f(&c[3])];
    section(&mut y, "Turnover");
    line(&mut y, "Gate receipts", each(|c| c.gate), 0);
    line(&mut y, "Season tickets", each(|c| c.tickets), 0);
    line(&mut y, "TV revenue", each(|c| c.tv), 0);
    line(&mut y, "Merchandising", each(|c| c.merch), 0);
    line(&mut y, "Turnover", each(|c| c.turnover()), 1);
    section(&mut y, "Other income");
    line(&mut y, "Players sold", each(|c| c.sold), 0);
    line(&mut y, "Prize money and other", each(|c| c.other_in), 0);
    line(&mut y, "Total income", each(|c| c.income()), 2);
    section(&mut y, "Expenditure");
    line(&mut y, "Wages", each(|c| c.wages), 0);
    line(&mut y, "Bonuses and signing-on", each(|c| c.bonuses), 0);
    line(&mut y, "Players bought", each(|c| c.bought), 0);
    line(&mut y, "Ground maintenance", each(|c| c.ground), 0);
    line(&mut y, "Interest and other costs", each(|c| c.other_out), 0);
    line(&mut y, "Total expenditure", each(|c| c.spend()), 2);
    line(&mut y, "Profit / (loss)", each(|c| c.result()), 2);
    // the balance and the transfer funds
    y += 6;
    let bw = (rt - l - 10) / 2;
    let bh = tall(fs) + tall(fm) + 12;
    if !dry {
        fill(l, y, l + bw, y + bh, BOX);
        fill(l + bw + 10, y, rt, y + bh, BOX);
    }
    text(l + 8, y + 4, fs, MUTED, b"Closing bank balance", 0);
    text(l + 8, y + 6 + tall(fs), fm, if r.balance < 0 { LOSS } else { INK }, to_latin1(&r.money.full(r.balance)).as_slice(), 0);
    text(l + bw + 18, y + 4, fs, MUTED, b"Transfer funds available", 0);
    text(l + bw + 18, y + 6 + tall(fs), fm, [0x1f, 0x6b, 0x3a], to_latin1(&r.money.full(r.funds)).as_slice(), 0);
    y += bh + 10;
    // the balance month by month and where it is heading (`diary`, user 2026-11-22)
    if r.history.len() >= 2 {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        text(l, y, fs, NAVY, b"Bank balance by month", 0);
        if let Some((end, value, _)) = &r.forecast {
            text(rt, y, fs, MUTED, to_latin1(&format!("Forecast {end}: {}", r.money.full(*value))).as_slice(), 2);
        }
        y += tall(fs) + 4;
        let ch = tall(fs) * CHART_ROWS;
        let vals: Vec<i64> = r.history.iter().map(|h| h.2).collect();
        let fc = r.forecast.as_ref().map(|f| f.1);
        let (lo, hi) = vals.iter().copied().chain(fc).chain([0]).fold((i64::MAX, i64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
        let span = (hi - lo).max(1) as f64;
        let n = vals.len() + fc.is_some() as usize;
        let (px1, px2, py1, py2) = (l + 30, rt - 30, y + 8, y + ch - tall(ft) - 6);
        let at = |k: usize, v: i64| (px1 + (px2 - px1) * k as i32 / (n as i32 - 1).max(1), py2 - ((v - lo) as f64 / span * (py2 - py1) as f64) as i32);
        if !dry {
            fill(l, y, rt, y + ch, BOX);
            let line = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_LINE);
            let zero = at(0, 0).1;
            fill(px1, zero, px2, zero + 1, [0x9a, 0xa0, 0xab]);
            for k in 1..vals.len() {
                let (a, b) = (at(k - 1, vals[k - 1]), at(k, vals[k]));
                line(a.0, a.1, b.0, b.1, 2, rgb(NAVY) as u32);
            }
            // (the forecast dashed, from the last month)
            if let Some(f) = fc {
                let (a, b) = (at(vals.len() - 1, vals[vals.len() - 1]), at(n - 1, f));
                for s in (0..10).step_by(2) {
                    let p = |t: i32| (a.0 + (b.0 - a.0) * t / 10, a.1 + (b.1 - a.1) * t / 10);
                    let (p1, p2) = (p(s), p(s + 1));
                    line(p1.0, p1.1, p2.0, p2.1, 2, rgb(MUTED) as u32);
                }
                dot(b.0, b.1, 3, if f < 0 { LOSS } else { MUTED });
            }
            for (k, &v) in vals.iter().enumerate() {
                let p = at(k, v);
                dot(p.0, p.1, 3, if v < 0 { LOSS } else { NAVY });
            }
        }
        for (k, h) in r.history.iter().enumerate() {
            text(at(k, 0).0, y + ch - tall(ft) - 4, ft, MUTED, MONTHS[(h.1 as usize).clamp(1, 12) - 1].as_bytes(), 1);
        }
        if fc.is_some() {
            text(at(n - 1, 0).0, y + ch - tall(ft) - 4, ft, MUTED, b"Jun", 1);
        }
        y += ch + 6;
        if let Some(s) = &r.second {
            for line_text in wrap(ft, s, rt - l) {
                text(l, y, ft, MUTED, &line_text, 0);
                y += tall(ft) + 1;
            }
            y += 6;
        }
    }
    // the verdict and the notes
    for line_text in wrap(fs, &r.verdict, rt - l) {
        text(l, y, fs, INK, &line_text, 0);
        y += tall(fs) + 1;
    }
    y += 4;
    text(l, y, fs, NAVY, b"Notes to the accounts", 0);
    y += tall(fs) + 2;
    for (k, note) in r.notes.iter().enumerate() {
        let lead = format!("{}. ", k + 1);
        let lw = text_width(fs, lead.as_bytes()) + 8;
        text(l, y, fs, MUTED, lead.as_bytes(), 0);
        for line_text in wrap(fs, note, rt - l - lw) {
            text(l + lw, y, fs, INK, &line_text, 0);
            y += tall(fs) + 1;
        }
        y += 2;
    }
    let basis = if r.basis.starts_with("Basis of preparation") { r.basis.clone() } else { format!("Basis of preparation: {}", r.basis) };
    for line_text in wrap(ft, &basis, rt - l) {
        text(l, y, ft, MUTED, &line_text, 0);
        y += tall(ft) + 1;
    }
    y += 4;
    for line_text in wrap(fs, &r.closing, rt - l) {
        text(l, y, fs, INK, &line_text, 0);
        y += tall(fs) + 1;
    }
    // the signature
    y += 6;
    rule(y, 1, [0x9a, 0xa0, 0xab]);
    y += 6;
    text(l, y, fb, NAVY, to_latin1(&r.partner.1).as_slice(), 0);
    text(rt, y + 2, ft, MUTED, to_latin1(&format!("Signed {}", r.date)).as_slice(), 2);
    y += tall(fb) + 2;
    text(l, y, ft, INK, to_latin1(&r.partner.0).as_slice(), 0);
    text(rt, y, ft, MUTED, b"Page 1 of 1", 2);
    y += tall(ft) + 1;
    text(l, y, ft, MUTED, b"for and on behalf of Accountants & Co", 0);
    y += tall(ft) + 6;
    for line_text in wrap(ft, &r.footer, rt - l) {
        text((l + rt) / 2, y, ft, MUTED, &line_text, 1);
        y += tall(ft) + 1;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FC Twente on 28 January 2004, the figures of the user's three screens.
    fn twente() -> [Col; 4] {
        let mk = |gate, tickets, tv, merch, sold, other_in, wages, bonuses, bought, ground, other_out| Col { gate, tickets, tv, merch, sold, other_in, wages, bonuses, bought, ground, other_out };
        [
            mk(0, 0, 0, 174_736, 73_876, 0, 1_129_140, 199_640, 0, 0, 0),
            mk(119_112, 0, 29_062, 173_706, 245_690, 15_005, 1_135_043, 196_164, 0, 102_031, 92_151),
            mk(1_410_489, 1_922_892, 1_622_656, 1_402_948, 10_486_786, 104_136, 9_152_784, 898_995, 0, 651_500, 561_294),
            mk(1_508_379, 5_355_772, 1_643_000, 2_385_656, 898_931, 962_219, 19_269_987, 593_328, 9_430, 1_107_719, 1_325_940),
        ]
    }

    #[test]
    fn the_screens_totals() {
        let c = twente();
        // the game's Finances view (its Income and Expenditure views differ by a few euro: floats)
        assert_eq!((c[2].income(), c[2].spend(), c[2].result()), (16_949_907, 11_264_573, 5_685_334));
        assert_eq!(c[0].result(), -1_080_168);
        assert_eq!(c[3].result(), -9_552_447);
    }

    #[test]
    fn a_club_in_the_red_with_sales() {
        let r = build("FC Twente", 1808, "28 January 2004", twente(), -5_974_292, 210_000, &Money::default());
        assert!(!r.verdict.is_empty(), "no verdict");
        assert!(r.notes.len() >= 3 && r.notes.len() <= 5, "{:?}", r.notes);
        assert!(r.notes.iter().any(|n| n.contains("62") || n.contains("10.5m")), "player sales note: {:?}", r.notes);
        for t in r.notes.iter().chain([&r.verdict, &r.basis, &r.closing, &r.footer]) {
            assert!(!t.contains('{') && !t.contains('}'), "unfilled placeholder in {t}");
            assert!(t.is_ascii());
        }
    }

    #[test]
    fn every_verdict_has_its_words() {
        let make = |balance, margin: i64| {
            let mut c = [Col::default(); 4];
            c[2] = Col { gate: 10_000_000, wages: 10_000_000 - margin, ..Default::default() };
            c[3] = c[2];
            build("Test FC", 7, "1 July 2004", c, balance, 0, &Money::default())
        };
        for (balance, margin) in [(9_000_000, 3_000_000), (9_000_000, 100_000), (-1, 500_000), (9_000_000, -500_000), (-1, -500_000), (-1, -4_000_000)] {
            let r = make(balance, margin);
            assert!(!r.verdict.is_empty() && !r.verdict.contains('{'), "{balance} {margin}: {:?}", r.verdict);
        }
    }

    #[test]
    fn the_library_is_whole() {
        let want: [(&str, usize); 24] = [
            ("VERDICT_STRONG", 4), ("VERDICT_PROFIT", 4), ("VERDICT_PROFIT_OVERDRAWN", 4), ("VERDICT_LOSS_FUNDED", 4), ("VERDICT_LOSS_OVERDRAWN", 4), ("VERDICT_DEEP", 4),
            ("NOTE_WAGES_HIGH", 4), ("NOTE_WAGES_MODEST", 3), ("NOTE_WAGES_FELL", 3), ("NOTE_WAGES_ROSE", 3), ("NOTE_SALES_RELIANT", 4), ("NOTE_NO_SALES", 2),
            ("NOTE_BOUGHT_HEAVY", 3), ("NOTE_TICKETS_STRONG", 3), ("NOTE_MONTH_BETTER", 3), ("NOTE_MONTH_WORSE", 3), ("NOTE_FUNDS_NONE", 3), ("NOTE_FUNDS_AVAILABLE", 3),
            ("NOTE_OVERDRAWN_ADVICE", 4), ("NOTE_RESERVES", 3), ("BASIS", 4), ("CLOSING", 4), ("PARTNER", 6), ("FOOTER", 3),
        ];
        for (key, n) in want {
            assert_eq!(variants(key).len(), n, "{key}");
        }
        // only placeholders the code fills
        let known = ["club", "date", "income", "expenditure", "result", "balance", "funds", "sales", "bought", "wages", "profit_ex_sales", "income_m", "result_m", "balance_m", "sales_m", "wages_m", "profit_ex_sales_m", "bought_m", "wages_pct", "wages_pct_last", "sales_pct", "tickets_pct", "bought_pct", "month_result", "month_result_last"];
        for t in NOTES.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once('\t')).map(|(_, t)| t) {
            let mut rest = t;
            while let Some(i) = rest.find('{') {
                let j = rest[i..].find('}').expect("open brace") + i;
                assert!(known.contains(&&rest[i + 1..j]), "unknown placeholder {{{}}} in {t}", &rest[i + 1..j]);
                rest = &rest[j + 1..];
            }
            assert!(t.len() <= 260, "long text: {t}");
        }
    }

    #[test]
    fn money_and_dates() {
        let e = Money::default();
        assert_eq!(e.full(5_974_292), "E5,974,292");
        assert_eq!(e.full(-1_080_167), "(E1,080,167)");
        assert_eq!(e.full(0), "E0");
        assert_eq!(e.millions(10_486_786), "E10.5m");
        // another currency: the game's symbol and its rate
        let pounds = Money { symbol: "\u{a3}".into(), rate: 0.5 };
        assert_eq!(pounds.full(1_000_000), "\u{a3}500,000");
        assert_eq!(pounds.millions(-3_000_000), "(\u{a3}1.5m)");
        assert_eq!(date_text(27, 2004), "28 January 2004");
        assert_eq!(date_text(181, 2003), "1 July 2003");
        assert_eq!(date_text(182, 2004), "1 July 2004"); // a leap year counts 29 February
        assert_eq!(date_text(364, 2003), "31 December 2003");
    }

    #[test]
    fn the_youth_coach_rates() {
        let r = |seen: i32, new: bool| YouthRow { name: "Kid".into(), pos: "S C".into(), age: 16, now: 40, seen, new };
        assert_eq!((r(175, false).word(), r(120, false).word(), r(60, false).word()), ("A future star. Keep him close", "Squad player one day", "I would let him go"));
        let y = Youth { club: "Vitesse".into(), date: "1 July 2027".into(), coach: "Henk".into(), judging: 16, youngsters: 18, rows: vec![r(150, true), r(90, false)] };
        let back = decode_youth(&encode_youth(&y)).unwrap();
        assert_eq!((back.coach.as_str(), back.judging, back.youngsters, back.rows.clone()), ("Henk", 16, 18, y.rows));
        // his eye: -1..1, the same for a player each time
        assert!((0..500).all(|p| (-1.0..=1.0).contains(&eye(7, p))) && eye(7, 42) == eye(7, 42));
    }

    #[test]
    fn the_physio_reads_the_draw() {
        let row = |jaded: i32, prone: i32, condition: i32, out: i32| MedRow { name: "X".into(), pos: "F C".into(), age: 25, condition, load: 500, jaded, prone, natural: 12, out, trains: 3 };
        // against normal: jadedness 50 is nearly everyone's, an average body (proneness 8) is Normal
        assert_eq!((row(50, 8, 100, 0).risk_word(), row(50, 3, 100, 0).risk_word(), row(50, 13, 100, 0).risk_word(), row(70, 14, 100, 0).risk_word(), row(70, 14, 100, 5).risk_word()), ("Normal", "Low", "High", "Very high", "-"));
        assert!(!row(50, 8, 100, 0).to_watch() && row(66, 8, 100, 0).to_watch() && row(50, 8, 82, 0).to_watch());
        assert_eq!((row(50, 8, 75, 0).advice(), row(66, 8, 100, 0).advice(), row(50, 16, 100, 0).advice(), row(50, 8, 100, 0).advice()), ("Rest him, 75% fit".to_string(), "Fit today, but jaded: rotate".to_string(), "Fragile: manage his minutes".to_string(), "Fit".to_string()));
        assert_eq!(row(66, 8, 90, 0).advice(), "Needs a breather");
        assert_eq!(row(30, 3, 100, 12).advice(), "Out 12 days, training in 3");
        // a skill shown 1..20 from the stored value: never 125
        assert!((1..=20).contains(&shown_skill(125, 150)) && (1..=20).contains(&shown_skill(-40, 20)));
        let m = Medical { club: "Vitesse".into(), date: "12 July 2026".into(), physio: "Jan Smit".into(), skill: 14, rows: vec![row(50, 3, 75, 0), row(30, 3, 100, 12)] };
        let back = decode_medical(&encode_medical(&m)).unwrap();
        assert_eq!((back.physio.as_str(), back.skill, back.rows.clone()), ("Jan Smit", 14, m.rows));
    }

    #[test]
    fn the_planner_travels_and_sorts() {
        let row = |name: &str, wage: i64, m: i32, y: i32| PlanRow { name: name.into(), pos: "D C".into(), age: 24, wage, wage_text: format!("E{wage} p/w"), end_month: m, end_year: y, value: 1_000 };
        let p = Plan { club: "Vitesse".into(), date: "12 July 2026".into(), season_end: 2027, bill: 50_000, can_pay: 60_000, money: Money::default(), rows: vec![row("A", 1_000, 5, 2027), row("B", 2_000, 6, 2027), row("C", 500, 11, 2026)] };
        let back = decode_plan(&encode_plan(&p)).unwrap();
        assert_eq!((back.club.as_str(), back.season_end, back.bill, back.can_pay), ("Vitesse", 2027, 50_000, 60_000));
        assert_eq!(back.rows, p.rows);
        // ending by 30 June 2027: A (June) and C (December 2026), not B (July 2027)
        assert_eq!(back.freed(), (1_000 + 500) * 52);
    }

    #[test]
    fn the_balance_by_the_end_of_june() {
        // three months recorded, 100 down a month: in October, eight months to go
        let h = [(2026, 8, 1_000), (2026, 9, 900), (2026, 10, 800)];
        assert_eq!(forecast(&h, 800, 0, 10, 2026), ("30 June 2027".to_string(), 0, -100));
        // in March: three months to go, the same June
        assert_eq!(forecast(&[(2027, 2, 500), (2027, 3, 800)], 800, 0, 3, 2027), ("30 June 2027".to_string(), 1_700, 300));
        // nothing recorded: this year's result spread over the months since July
        assert_eq!(forecast(&[], 1_000, -600, 9, 2026).1, 1_000 - 200 * 9);
    }

    #[test]
    fn a_clients_figures_come_back_whole() {
        let money = Money { symbol: "€".into(), rate: 2.5 };
        let text = encode("Vitesse Arnhem", 17, "29 June 2026", &twente(), -4_763_142, 0, &money);
        let (name, seed, date, cols, balance, funds, back) = decode(&text).unwrap();
        assert_eq!((name.as_str(), seed, date.as_str(), balance, funds), ("Vitesse Arnhem", 17, "29 June 2026", -4_763_142, 0));
        assert_eq!((cols, back), (twente(), money));
        assert!(decode("Vitesse\t1").is_none());
    }

    #[test]
    fn the_blocks_of_a_record() {
        let mut rec = [0u8; RECORD];
        // this month (+0x70): wages at block +0x20 (0x90), players sold i64 at +8 (0x78), TV at +0x48 (0xb8)
        rec[0x90..0x94].copy_from_slice(&1_129_140i32.to_le_bytes());
        rec[0x78..0x80].copy_from_slice(&73_876i64.to_le_bytes());
        rec[0xb8..0xbc].copy_from_slice(&29_062i32.to_le_bytes());
        // this year (+0x110): wages at 0x130
        rec[0x130..0x134].copy_from_slice(&9_152_784i32.to_le_bytes());
        let (m, y) = (column(&rec, BLOCKS[0]), column(&rec, BLOCKS[2]));
        assert_eq!((m.wages, m.sold, m.tv, y.wages), (1_129_140, 73_876, 29_062, 9_152_784));
    }
}
