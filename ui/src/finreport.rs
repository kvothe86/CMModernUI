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
//! 0x5999e0 and the info tab's row code, 0x460590). The record's +0 is the balance (64 bits). Not on
//! a network client (its memory is the host's: the button is not drawn there).

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
    keys.truncate(5);
    let partner = pick("PARTNER", seed).and_then(|p| p.split_once('|')).map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or(("Marcus Hartwell FCA".into(), "M. Hartwell".into()));
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
        notes: keys.iter().filter_map(|k| say(k)).collect(),
        basis: say("BASIS").unwrap_or_default(),
        closing: say("CLOSING").unwrap_or_default(),
        partner,
        footer: say("FOOTER").unwrap_or_default(),
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

/// The report of the club the screen shows, from the game's memory.
unsafe fn read() -> Option<Report> {
    let club = [screen_param(0), SQUAD_CLUB, human_club_ptr().unwrap_or(0)].into_iter().find_map(|p| club_ptr(p))?;
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
    let name = latin1(&game_text(club + 4, 51)).into_owned();
    let seed = (club - rd::<usize>(CLUB_TABLE)) / CLUB_SIZE;
    let date = date_text(rd::<i16>(TODAY) as i32, rd::<i16>(TODAY + 2) as i32);
    Some(build(&name, seed, &date, cols, i64_at(rec, 0), funds, &money))
}

unsafe fn button_rect() -> Option<(i32, i32, i32, i32, u32)> {
    let (_, y1, x2, y2, font) = BUTTON?;
    let w = text_width(font, b"P&L report") + 56;
    Some((x2 + 14, y1, x2 + 14 + w, y2, font))
}

/// The flip pass: the button while the money views show, the report while it is open.
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    if remote::CLIENT {
        return;
    }
    let fin = std::mem::take(&mut FIN_SEEN);
    let view = VIEW_SEEN.take();
    if fin {
        if let Some(v) = view {
            BUTTON = Some(v);
        }
    } else if full_frame {
        BUTTON = None;
    }
    if OPEN {
        draw();
        return;
    }
    let Some(b) = button_rect() else { return };
    let (blue, hover) = ([0x2c, 0x46, 0x6a], [0x3a, 0x5a, 0x86]);
    fill(b.0, b.1, b.2, b.3, if BTN_HOVER { hover } else { blue });
    fill(b.0, b.1, b.2, b.1 + 1, [0x4a, 0x6a, 0x96]);
    put(b.0, b.1, b.2, b.3, 0xc, b.4, WHITE, "P&L report");
}

/// The mouse: the button opens the report; while it is open it takes every click.
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if remote::CLIENT {
        return None;
    }
    let inside = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    if !OPEN {
        let b = button_rect()?;
        let over = inside((b.0, b.1, b.2, b.3));
        let changed = std::mem::replace(&mut BTN_HOVER, over) != over;
        if released && over {
            *(LEFT_CLICK as *mut u32) = 0;
            REPORT = read();
            OPEN = REPORT.is_some();
            OPENED_AT = Some(std::time::Instant::now());
            return Some(true);
        }
        return changed.then_some(true);
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
    let m = (sh / 88).max(6);
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
    let Some(r) = REPORT.clone() else { return };
    let (sw, sh) = screen();
    fill(0, 0, sw - 1, sh - 1, BACKGROUND);
    let top = sh / 60;
    let ph = sh - sh / 10 - top;
    let pw = (ph * 82 / 100).min(sw - 40);
    let x0 = (sw - pw) / 2;
    let bottom = top + ph;
    let (f1, f2, f3, f5) = (0xffff0001u32, 0xffff0002u32, 0xffff0003u32, 0xffff0005u32);
    // (title, table, text, fine print): the table in the small body font so the columns clear the labels
    let tiers = [(f5, f2, f2, f1), (f3, f2, f1, f1)];
    let tier = *tiers.iter().find(|t| paint(&r, true, x0, top, pw, t.0, t.1, t.2, t.3) <= bottom - 8).unwrap_or(&tiers[1]);
    fill(x0 - 1, top - 1, x0 + pw + 1, bottom + 1, [0xc9, 0xc3, 0xb3]);
    fill(x0, top, x0 + pw, bottom, PAPER);
    paint(&r, false, x0, top, pw, tier.0, tier.1, tier.2, tier.3);
    if std::mem::take(&mut SAVE) {
        let club: String = r.club.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let name = format!("pl_{}_{}", club, r.date.replace(' ', "_"));
        SAVED = Some((save_png(x0, top, x0 + pw + 1, bottom + 1, &name).map_or("Could not save the picture".to_string(), |p| format!("Saved: {p}")), std::time::Instant::now()));
    }
    let b = back_rect();
    fill(b.0, b.1, b.2, b.3, if HOVER { [0x3a, 0x5a, 0x86] } else { [0x2c, 0x46, 0x6a] });
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, WHITE, b"Back", 1);
    let sv = save_rect();
    fill(sv.0, sv.1, sv.2, sv.3, if SAVE_HOVER { [0x3a, 0x5a, 0x86] } else { [0x24, 0x28, 0x30] });
    text_at((sv.0 + sv.2) / 2, sv.1, sv.3, f3, WHITE, b"Save image", 1);
    if let Some((note, _)) = SAVED.as_ref().filter(|s| s.1.elapsed().as_secs() < 6) {
        text_at(sv.0 - 12, sv.1, sv.3, f2, GOLD, &to_latin1(note), 2);
    }
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
