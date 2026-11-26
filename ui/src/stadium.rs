//! The stadium builder (switch `stadium`, user 2026-11-23: "like USM 2 had", in cmui's own look):
//! a Stadium button on the Finances tab opens the ground: a drawing of its four stands, its
//! figures, and works the manager orders himself instead of asking the board. The prices are the
//! game's own for the board's works (FinanceTable::season_end_club_update 0x59cfa0, rust
//! `finance_season.rs`); the money goes at once as the game pays them (ground costs, this month
//! and this year, off the balance); the stadium record changes when a work is finished (the
//! diary's daily tick, `complete`). The board still does its own works at the season's end.
//!
//! The page (night 11, after the user's Grok mock): the ground rendered in Blender
//! (`ui\stadium3d\build_stadium.py`, Data\cmui_stadium.bin) painted in the club's colours, the
//! figures and the works in cards; see `ui\docs\stadium.md`.
//!
//! Club +0x69 its stadium, +0x73 its average attendance; stadium +0x3c capacity, +0x40 seats,
//! +0x44 the most it can hold (expansion room), +0x4c roof, +0x4d under-soil heating. A network
//! client asks the host (key `!` club, order, amount).
use crate::*;

pub const SEATS: u8 = 1;
pub const STANDING: u8 = 2;
pub const CONVERT: u8 = 3;
pub const ROOF: u8 = 4;
pub const HEATING: u8 = 5;
/// A new stadium (user 2026-11-23: "a project of millions, only for very rich clubs", once the
/// ground is at the most it can hold): the size picked from `SIZES`, a name typed in.
pub const NEW: u8 = 6;
/// The club shop's next level (amount: the level), an item put on sale (amount: its index): `shop.rs`.
pub const SHOP: u8 = 7;
pub const ITEM: u8 = 8;
/// (from 3,000 since 2026-11-24: a level 7 club could only pick 15,000 for 62.5 million, some 600
/// seasons of its turnover in the cm25 Glory save)
const SIZES: [i32; 15] = [3_000, 5_000, 8_000, 12_000, 15_000, 20_000, 25_000, 30_000, 40_000, 50_000, 60_000, 75_000, 90_000, 110_000, 125_000];

/// The new stadiums a ground of `capacity` can move up to.
pub fn sizes_above(capacity: i32) -> Vec<i32> {
    SIZES.into_iter().filter(|&s| s > capacity + 1000).collect()
}

/// What `places` places cost a place, rising with the size of the ground they make (user
/// 2026-11-24, after the cm25 Glory save's turnovers: level 7 0.1 million, League Two 2.7,
/// Premier League 39, Man Utd 77; set a little above the plan for the European prize money
/// later on). A new stadium of 30,000: 1,935 a place, 58 million; of 3,000 0.74 million; of
/// 125,000 nearly a billion. Seats added cost half as much a place.
fn a_place(ground: i32) -> i64 {
    60 + ground as i64 / 16
}

/// What a new stadium asks at the order: 30% (the rest in monthly instalments, `instalments`).
pub fn deposit(cost: i64) -> i64 {
    cost * 3 / 10
}

/// Its months of instalments after the order's: one a month of its building time.
fn instalment_months(days: i32) -> i32 {
    ((days as f32 / 30.44).ceil() as i32 - 1).max(1)
}
/// The most a ground can hold (the game's own bound for its works was 125,000; 150,000 since the
/// new stadium's top level, `new_room`).
const MOST: i32 = 150_000;

/// A new stadium's room to grow, by the old ground's (the user 2026-11-24: a level 7 club with
/// 1,000 places and room for 1,000 had to build new stadiums five times over): a small ground's
/// club moves to a professional one (room to 30,000), a bigger one's to a big club's (70,000), a
/// ground of 50,000 and more to the very top (150,000); never less than a quarter over its places,
/// as before. The places themselves are the size picked, as before.
fn new_room(old_room: i32, places: i32) -> i32 {
    let level = match old_room {
        ..15_000 => 30_000,
        ..50_000 => 70_000,
        _ => MOST,
    };
    level.max(places * 5 / 4).min(MOST).max(places)
}

/// A work's price in the game's units and how many days it takes (`amount` places for 1..3).
pub fn price(kind: u8, amount: i32) -> (i64, i32) {
    let blocks = (amount / 1000 + 1) as i64;
    let n = amount as i64;
    // (build time: a fixed start and more for each 1,000 places; too short, user 2026-11-23:
    // seats 6 weeks + 3 a thousand, standing 4 + 2, converting 3 + 1.5, a roof half a year)
    let k = amount / 1000;
    match kind {
        // (the price by the ground they make: `Ground::cost`; here as if on an empty ground)
        SEATS => (n * a_place(amount) / 2, 42 + k * 21),
        STANDING => ((n + blocks * 1000) * 50, 28 + k * 14),
        CONVERT => (n * 75 + blocks * 50_000, 21 + k * 21 / 2),
        ROOF => (7_500_000, 182),
        // (a year and 10 days a thousand)
        NEW => (n * a_place(amount), 365 + k * 10),
        SHOP => shop::LEVELS.get(amount as usize).map_or((0, 0), |l| (l.2, l.3)),
        ITEM => shop::ITEMS.get(amount as usize).map_or((0, 0), |i| (i.2, 0)),
        _ => (1_500_000, 56),
    }
}

/// The ground as the page shows it.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Ground {
    pub club: String,
    pub name: String,
    pub capacity: i32,
    pub seats: i32,
    pub room: i32,
    pub roof: bool,
    pub heating: bool,
    pub average: i32,
    pub balance: i64,
    pub symbol: String,
    pub rate: f64,
    pub works: Vec<(u8, i32, i32)>, // kind, amount, days left
    pub said: String,               // the answer to the last order
    pub colours: [[u8; 3]; 2],      // the club's first two kit colours: the seats
    pub last: Option<i64>,          // the balance at the last month's end (the diary): the trend arrow
    pub next_name: String,          // a new stadium's name, typed at its order ("Fully built" shows it)
    pub shop: u8,                   // the club shop's level, the items on sale (bits), `shop.rs`
    pub items: u8,
    pub merch: i64,                 // merchandising this season
    pub merch_pct: i32,             // the share more this week: shop, items, last week's results
    pub shared: String,             // another club on the same stadium record (Milan and Internazionale): no works
}

/// A club's first two kit colours (club +0x83: six colour records, each red / green / blue at +55);
/// blue and white when they can't be read.
unsafe fn club_colours(club: usize) -> [[u8; 3]; 2] {
    let mut out = [[0x26, 0x52, 0x8e], [0xe8, 0xe8, 0xe8]];
    for (k, o) in out.iter_mut().enumerate() {
        let c = rd::<usize>(club + 0x83 + 4 * k);
        if readable(c, 58) {
            *o = [rd::<u8>(c + 55), rd::<u8>(c + 56), rd::<u8>(c + 57)];
        }
    }
    out
}

impl Ground {
    /// Places ordered and not yet built (the room they take).
    pub fn pending(&self, kinds: &[u8]) -> i32 {
        self.works.iter().filter(|w| kinds.contains(&w.0)).map(|w| w.1).sum()
    }
    /// A work's price: seats by the ground they make, the rest as `price`.
    pub fn cost(&self, kind: u8, amount: i32) -> i64 {
        match kind {
            SEATS => amount as i64 * a_place(self.capacity + self.pending(&[SEATS, STANDING]) + amount) / 2,
            _ => price(kind, amount).0,
        }
    }
    /// Why an order cannot be placed, or None.
    pub fn refusal(&self, kind: u8, amount: i32) -> Option<String> {
        let cost = self.cost(kind, amount);
        let busy = |k: u8| self.works.iter().any(|w| w.0 == k);
        match kind {
            SHOP if busy(SHOP) => return Some("Being built".into()),
            SHOP if amount as u8 != self.shop + 1 || amount > 3 => return Some("Fully built".into()),
            ITEM if self.items & 1 << amount != 0 => return Some("On sale".into()),
            // (name and number, kids' kits: sold in a club shop only)
            ITEM if amount >= 2 && self.shop == 0 => return Some("Needs a club shop".into()),
            _ => {}
        }
        // (a ground two clubs share: no works, they would change the other club's too; a new
        // stadium of its own would first need the club moved to a stadium record of its own)
        if !self.shared.is_empty() && !matches!(kind, SHOP | ITEM) {
            return Some(format!("Shared with {}", self.shared));
        }
        // (the shop goes on while a new stadium is built: it is not the stadium's)
        if busy(NEW) && !matches!(kind, SHOP | ITEM) {
            return Some(if kind == NEW { "Being built" } else { "New stadium being built" }.to_string());
        }
        let reason = match kind {
            NEW if self.capacity + self.pending(&[SEATS, STANDING]) < self.room.min(MOST) => Some("Room left to grow first"),
            NEW if amount <= self.capacity => Some("Not bigger than now"),
            SEATS | STANDING if self.capacity + self.pending(&[SEATS, STANDING]) + amount > self.room.min(MOST) => Some("No room left to build"),
            CONVERT if self.capacity - self.seats - self.pending(&[CONVERT]) < amount => Some("Not enough standing places"),
            ROOF if self.roof => Some("Already covered"),
            HEATING if self.heating => Some("Already has heating"),
            // (ordered and not finished: not "already covered", night 11)
            ROOF | HEATING if busy(kind) => Some("Being built"),
            _ => None,
        };
        let now = if kind == NEW { deposit(cost) } else { cost };
        reason.map(String::from).or_else(|| (self.balance < now).then(|| "Not enough money in the bank".to_string()))
    }
    fn money(&self, v: i64) -> String {
        finreport::Money { symbol: self.symbol.clone(), rate: self.rate }.full(v)
    }
}

fn encode(g: &Ground) -> String {
    let c = g.colours;
    let mut out = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{},{},{},{},{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        g.club, g.name, g.capacity, g.seats, g.room, g.roof as i32, g.heating as i32, g.average, g.balance, g.symbol, g.rate, g.said, c[0][0], c[0][1], c[0][2], c[1][0], c[1][1], c[1][2],
        g.last.map_or(String::new(), |v| v.to_string()),
        g.next_name,
        g.shop,
        g.items,
        g.merch,
        g.merch_pct,
        g.shared
    );
    for w in &g.works {
        out += &format!("{}\t{}\t{}\n", w.0, w.1, w.2);
    }
    out
}

fn decode(text: &str) -> Option<Ground> {
    let mut lines = text.lines();
    let h: Vec<&str> = lines.next()?.split('\t').collect();
    // (13 fields before the trend arrow, night 11)
    if !(13..=20).contains(&h.len()) {
        return None;
    }
    let v: Vec<u8> = h[12].split(',').filter_map(|x| x.parse().ok()).collect();
    let colours = if v.len() == 6 { [[v[0], v[1], v[2]], [v[3], v[4], v[5]]] } else { [[0x26, 0x52, 0x8e], [0xe8, 0xe8, 0xe8]] };
    let mut g = Ground {
        club: h[0].into(),
        name: h[1].into(),
        capacity: h[2].parse().ok()?,
        seats: h[3].parse().ok()?,
        room: h[4].parse().ok()?,
        roof: h[5] == "1",
        heating: h[6] == "1",
        average: h[7].parse().ok()?,
        balance: h[8].parse().ok()?,
        symbol: h[9].into(),
        rate: h[10].parse().ok()?,
        said: h[11].into(),
        works: Vec::new(),
        colours,
        last: h.get(13).and_then(|v| v.parse().ok()),
        next_name: h.get(14).map_or(String::new(), |v| v.to_string()),
        shop: h.get(15).and_then(|v| v.parse().ok()).unwrap_or(0),
        items: h.get(16).and_then(|v| v.parse().ok()).unwrap_or(0),
        merch: h.get(17).and_then(|v| v.parse().ok()).unwrap_or(0),
        merch_pct: h.get(18).and_then(|v| v.parse().ok()).unwrap_or(0),
        shared: h.get(19).map_or(String::new(), |v| v.to_string()),
    };
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 3 {
            g.works.push((f[0].parse().ok()?, f[1].parse().ok()?, f[2].parse().ok()?));
        }
    }
    Some(g)
}

// ---- the host ----

unsafe fn today() -> i32 {
    rd::<i16>(0xae2c92) as i32 * 366 + rd::<i16>(0xae2c90) as i32
}

unsafe fn stadium_of(club: usize) -> Option<usize> {
    let s = rd::<usize>(club + 0x69);
    readable(s, 78).then_some(s)
}

/// Another club whose stadium is the same record (Milan and Internazionale at San Siro): its
/// name, or empty. Works on it would change both clubs' grounds (user 2026-11-25).
unsafe fn sharer(club: usize, s: usize) -> String {
    let (base, count) = (rd::<usize>(CLUB_TABLE), rd::<i32>(CLUB_COUNT).clamp(0, 100_000) as usize);
    (0..count)
        .map(|k| base + k * CLUB_SIZE)
        .find(|&c| c != club && readable(c, 0x6d) && rd::<usize>(c + 0x69) == s)
        .map_or(String::new(), |c| latin1(&game_text(c + 4, 51)).into_owned())
}

/// The page's word once the ground is full (no room left to grow, no new stadium ordered yet):
/// what a new stadium gives, its room to grow by the old ground's level (`new_room`), and the
/// levels after it. None while there is room, or a new stadium is being built.
fn full_note(g: &Ground, small: bool) -> Option<String> {
    if g.capacity + g.pending(&[SEATS, STANDING]) < g.room.min(MOST) || g.works.iter().any(|w| w.0 == NEW) {
        return None;
    }
    let Some(&next) = sizes_above(g.capacity).first() else {
        return Some("The ground is full and as big as a ground gets.".into());
    };
    let room = new_room(g.room, next);
    if small {
        return Some(format!("Full: a new stadium of {}, room to grow to {}.", sep(next), sep(room)));
    }
    let after = match room {
        ..70_000 => " Its next one: room to 70,000, then 150,000.",
        ..MOST => " Its next one: room to 150,000.",
        _ => "",
    };
    Some(format!("Full. A new stadium: from {} seats, room to grow to {}.{}", sep(next), sep(room), after))
}

/// A test's ground (script line `places n`, as `money n`): the human club's stadium n places, a
/// quarter of them seats, no room to grow; in memory only, until the game is saved.
pub unsafe fn test_places(n: i32) -> bool {
    let Some(s) = human_club_ptr().and_then(|c| stadium_of(c)) else { return false };
    for (off, v) in [(0x3c, n), (0x40, n / 4), (0x44, n)] {
        *((s + off) as *mut i32) = v;
    }
    true
}

/// A test's roof (script line `roof 1` / `roof 0`): the human club's ground roofed or not; in
/// memory only, as `test_places`.
pub unsafe fn test_roof(on: bool) -> bool {
    let Some(s) = human_club_ptr().and_then(|c| stadium_of(c)) else { return false };
    *((s + 0x4c) as *mut u8) = on as u8;
    true
}

/// Host: a club's ground as text, after the order (kind 0: none) of the manager whose club it is.
pub unsafe fn remote(club: usize, kind: u8, amount: i32, own: bool, name: &[u8]) -> Option<Vec<u8>> {
    let seed = finreport::seed_of(club)?;
    let s = stadium_of(club)?;
    let mut said = String::new();
    if kind != 0 {
        log(&format!("stadium: order kind {kind} amount {amount} for club {club:#x}, own {own}"));
    }
    if kind != 0 && own {
        let g = ground(club, seed, s, String::new())?;
        said = match g.refusal(kind, amount) {
            Some(why) => why,
            None => {
                let (cost, days) = (g.cost(kind, amount), price(kind, amount).1);
                let now = if kind == NEW { deposit(cost) } else { cost };
                // (cmui.ini stadium_test_days=1: every work done the next day, for a test)
                let days = if setting("stadium_test_days").as_deref() == Some("1") { 1 } else { days };
                if finreport::pay_ground(club, now, false) {
                    #[allow(static_mut_refs)]
                    {
                        if kind == ITEM {
                            let at = diary::MERCH.iter().position(|m| m.0 == seed).unwrap_or_else(|| {
                                diary::MERCH.push((seed, 0, 0));
                                diary::MERCH.len() - 1
                            });
                            diary::MERCH[at].2 |= 1 << amount;
                        } else {
                            diary::WORKS.push((seed, kind, amount, today() + days.max(1)));
                        }
                        if kind == NEW {
                            // (the order's month is the deposit's; a tab or line break would break the diary)
                            let (y, m) = (rd::<i16>(0xae2c92) as i32, diary::month_of(rd::<i16>(0xae2c90) as i32, rd::<i16>(0xae2c92) as i32));
                            diary::PAID.retain(|p| p.0 != seed);
                            diary::PAID.push((seed, y, m));
                            let name: String = latin1(name).chars().filter(|c| !c.is_control()).take(40).collect();
                            diary::NAMES.retain(|n| n.0 != seed);
                            if !name.trim().is_empty() {
                                diary::NAMES.push((seed, name.trim().to_string()));
                            }
                        }
                    }
                    diary::save_now();
                    if kind == ITEM {
                        format!("{} on sale from today", shop::ITEMS[amount as usize].0)
                    } else if kind == NEW {
                        format!("Ordered: ready in {}; the rest in {} monthly payment{}", span(days), instalment_months(days), if instalment_months(days) == 1 { "" } else { "s" })
                    } else {
                        format!("Ordered: ready in {}", span(days))
                    }
                } else {
                    "Not enough money in the bank".into()
                }
            }
        };
    }
    Some(encode(&ground(club, seed, s, said)?).into_bytes())
}

#[allow(static_mut_refs)]
unsafe fn ground(club: usize, seed: usize, s: usize, said: String) -> Option<Ground> {
    let money = finreport::money_now();
    let now = today();
    Some(Ground {
        club: latin1(&game_text(club + 4, 51)).into_owned(),
        name: latin1(&game_text(s + 4, 51)).into_owned(),
        capacity: rd::<i32>(s + 0x3c),
        seats: rd::<i32>(s + 0x40),
        room: rd::<i32>(s + 0x44),
        roof: rd::<u8>(s + 0x4c) != 0,
        heating: rd::<u8>(s + 0x4d) != 0,
        average: rd::<i32>(club + 0x73),
        balance: finreport::balance_now(club).map_or(0, |b| b.0),
        symbol: money.symbol,
        rate: money.rate,
        works: diary::WORKS.iter().filter(|w| w.0 == seed).map(|w| (w.1, w.2, (w.3 - now).max(0))).collect(),
        said,
        colours: club_colours(club),
        last: diary::balances(seed).last().map(|b| b.2),
        next_name: diary::NAMES.iter().find(|n| n.0 == seed).map_or(String::new(), |n| n.1.clone()),
        shop: shop::kit(club).0,
        items: shop::kit(club).1,
        merch: finreport::merch_year(club),
        merch_pct: shop::pct(shop::kit(club).0, shop::kit(club).1) + shop::results_pct(club),
        shared: sharer(club, s),
    })
}

/// The diary's daily tick (host): the works done by today change their stadiums. True: some did.
#[allow(static_mut_refs)]
pub unsafe fn complete() -> bool {
    let now = today();
    let base = rd::<usize>(CLUB_TABLE);
    let done: Vec<(usize, u8, i32, i32)> = diary::WORKS.iter().filter(|w| w.3 <= now).cloned().collect();
    if done.is_empty() {
        return false;
    }
    diary::WORKS.retain(|w| w.3 > now);
    for (seed, kind, amount, _) in done {
        let Some(s) = stadium_of(base + seed * CLUB_SIZE) else { continue };
        let at = |o: usize| (s + o) as *mut i32;
        match kind {
            SEATS => {
                *at(0x3c) += amount;
                *at(0x40) += amount;
            }
            STANDING => *at(0x3c) += amount,
            CONVERT => *at(0x40) = (*at(0x40) + amount).min(*at(0x3c)),
            ROOF => *((s + 0x4c) as *mut u8) = 1,
            SHOP => {
                let at = diary::MERCH.iter().position(|m| m.0 == seed).unwrap_or_else(|| {
                    diary::MERCH.push((seed, 0, 0));
                    diary::MERCH.len() - 1
                });
                diary::MERCH[at].1 = amount as u8;
            }
            NEW => {
                // all seated, roofed, heated, room to grow by the old ground's level (`new_room`);
                // the name typed at the order
                let old_room = *at(0x44);
                *at(0x3c) = amount;
                *at(0x40) = amount;
                *at(0x44) = new_room(old_room, amount);
                *((s + 0x4c) as *mut u8) = 1;
                *((s + 0x4d) as *mut u8) = 1;
                if let Some(n) = diary::NAMES.iter().find(|n| n.0 == seed) {
                    let mut bytes = to_latin1(&n.1);
                    bytes.truncate(50);
                    bytes.push(0);
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), (s + 4) as *mut u8, bytes.len());
                }
                diary::NAMES.retain(|n| n.0 != seed);
                // (the instalments still owed, at once: built sooner than paid, a test's one day)
                let (cost, days) = price(NEW, amount);
                let months = instalment_months(days);
                let done = diary::PAID.iter().filter(|p| p.0 == seed).count() as i32 - 1;
                if done < months {
                    let owed = (cost - deposit(cost)) / months as i64 * (months - done) as i64;
                    finreport::pay_ground(base + seed * CLUB_SIZE, owed, true);
                    log(&format!("stadium: the new stadium's last {} instalments ({owed}) paid at once by club {seed}", months - done));
                }
                diary::PAID.retain(|p| p.0 != seed);
            }
            _ => *((s + 0x4d) as *mut u8) = 1,
        }
        log(&format!("stadium: work {kind} of {amount} done at club {seed}"));
    }
    true
}

/// The diary's daily tick (host): a new stadium's instalment once a month while it is built,
/// paid also into debt (the club signed for it). True: one was paid.
#[allow(static_mut_refs)]
pub unsafe fn instalments(year: i32, month: i32) -> bool {
    let base = rd::<usize>(CLUB_TABLE);
    let mut paid = false;
    for &(seed, kind, amount, _) in diary::WORKS.clone().iter().filter(|w| w.1 == NEW) {
        let (cost, days) = price(kind, amount);
        let months = instalment_months(days);
        let done = diary::PAID.iter().filter(|p| p.0 == seed).count() as i32 - 1; // (the deposit's month)
        if done >= months || diary::PAID.contains(&(seed, year, month)) {
            continue;
        }
        let sum = (cost - deposit(cost)) / months as i64;
        if finreport::pay_ground(base + seed * CLUB_SIZE, sum, true) {
            diary::PAID.push((seed, year, month));
            log(&format!("stadium: instalment {} of {months} ({sum}) paid by club {seed}", done + 1));
            paid = true;
        }
    }
    paid
}

// ---- the page (every machine) ----

static mut OPEN: bool = false;
static mut CLUB: usize = 0;
static mut GROUND: Option<Ground> = None;
static mut BUTTONS: Vec<((i32, i32, i32, i32), u8, i32)> = Vec::new();
static mut BACK: Option<(i32, i32, i32, i32)> = None;
static mut ARMED: Option<u8> = None;
/// The places chosen for seats, standing and converting (1,000 to 10,000, the - and + beside
/// each, user 2026-11-23), and those small buttons: box, kind, step.
static mut AMOUNTS: [i32; 3] = [1000; 3];
static mut STEPS: Vec<((i32, i32, i32, i32), u8, i32)> = Vec::new();
static mut OPENED_AT: Option<std::time::Instant> = None;
/// A client's last order, and the last refresh asked after it (`flip`).
static mut ORDERED_AT: Option<std::time::Instant> = None;
static mut LAST_ASK: u128 = 0;
/// The new stadium: the size picked (of `sizes_above`), its name being typed, Enter pressed, a
/// key since the last frame.
static mut NEW_PICK: usize = 0;
/// The order card shows the club shop; its two pills.
static mut SHOP_VIEW: bool = false;
static mut CARD_PILLS: Vec<((i32, i32, i32, i32), bool)> = Vec::new();
static mut NAMING: bool = false;
static mut TYPED: Vec<u8> = Vec::new();
static mut ORDER_NOW: bool = false;
static mut KEYED: bool = false;
static mut OLD_PROC: isize = 0;

#[link(name = "user32")]
extern "system" {
    fn SetWindowLongA(hwnd: usize, index: i32, value: isize) -> isize;
    fn CallWindowProcA(proc_: isize, hwnd: usize, msg: u32, w: usize, l: isize) -> isize;
}

/// The game window's messages: while a name is typed, the keys are its (as the coach's question).
#[allow(static_mut_refs)]
extern "system" fn window_proc(hwnd: usize, msg: u32, w: usize, l: isize) -> isize {
    unsafe {
        if OPEN && NAMING {
            match msg {
                0x100 => {
                    match w {
                        8 => _ = TYPED.pop(),
                        13 => ORDER_NOW = true,
                        27 => (NAMING, ARMED) = (false, None),
                        _ => {}
                    }
                    KEYED = true;
                    return 0;
                }
                0x101 => return 0,
                0x102 => {
                    if (0x20..0x100).contains(&w) && TYPED.len() < 40 {
                        TYPED.push(w as u8);
                        KEYED = true;
                    }
                    return 0;
                }
                _ => {}
            }
        }
        CallWindowProcA(OLD_PROC, hwnd, msg, w, l)
    }
}

unsafe fn hook_keys() {
    let hwnd = *(GAME_WINDOW as *const usize);
    if OLD_PROC == 0 && hwnd != 0 {
        OLD_PROC = SetWindowLongA(hwnd, -4, window_proc as extern "system" fn(usize, u32, usize, isize) -> isize as usize as isize);
    }
}

/// The tile or step under the mouse (lit a little, as cmui's other buttons).
static mut HOVER: Option<(i32, i32, i32, i32)> = None;
/// "Now | Fully built" over the picture: the ground as it would be with every place it has room
/// for, all seated, roofed (the user's brief: a render with the stands at their room to grow);
/// the two pills.
static mut FULL_VIEW: bool = false;
// the picture over the whole page, the cards hidden (the user, 2026-11-23: "a function that hides
// / shows the right part so you can fully see the stadium and surroundings"); its pill
static mut WIDE: bool = false;
static mut WIDE_PILL: Option<(i32, i32, i32, i32)> = None;
static mut VIEW_PILLS: Vec<((i32, i32, i32, i32), bool)> = Vec::new();

pub fn page_open() -> bool {
    unsafe { OPEN }
}

#[allow(static_mut_refs)]
unsafe fn fetch(kind: u8, amount: i32, name: &[u8]) {
    if remote::CLIENT && kind != 0 {
        (ORDERED_AT, LAST_ASK) = (Some(std::time::Instant::now()), 0);
    }
    let text = if remote::CLIENT {
        let mut k = key(b'!', &[CLUB as u32, kind as u32, amount as u32]);
        k.extend_from_slice(name);
        ask_host(&k)
    } else {
        remote(CLUB, kind, amount, Some(CLUB) == human_club_ptr(), name)
    };
    if let Some(g) = text.and_then(|t| decode(&String::from_utf8_lossy(&t))) {
        GROUND = Some(g);
    }
    REDRAW = true;
}

/// Opens the ground of a club (the Finances tab's Stadium button).
#[allow(static_mut_refs)]
pub unsafe fn open(club: usize) {
    (CLUB, ARMED, GROUND, OPENED_AT) = (club, None, None, Some(std::time::Instant::now()));
    (NAMING, NEW_PICK) = (false, 0);
    TYPED.clear();
    fetch(0, 0, &[]);
    OPEN = GROUND.is_some();
}

/// One stand: its box, the share of seats (from the pitch side), roof, works; side 0 north, 1
/// south, 2 west, 3 east.
unsafe fn stand(r: (i32, i32, i32, i32), seated: f32, roof: bool, building: bool, side: u8, cols: [[u8; 3]; 2]) {
    let (w, h) = (r.2 - r.0, r.3 - r.1);
    if w <= 0 || h <= 0 {
        return;
    }
    // the terraces in rows from the pitch side: seats (blue) first, then standing steps
    // (user 2026-11-23: "mag veel mooier en grafischer")
    let depth = if side < 2 { h } else { w };
    let rows = (depth / 5).max(2);
    // (seats: the club's two colours in blocks by section and every three rows, as real grounds
    // mix them; standing: bare concrete steps with a dark crush barrier every third row, so the two
    // read apart; one colour in stripes read as all white at Heracles, user 2026-11-23)
    let shade = |c: [u8; 3], d: i32| {
        let light = c[0] as i32 + c[1] as i32 + c[2] as i32 > 384;
        c.map(|v| (v as i32 + if light { -d } else { d }).clamp(0, 255) as u8)
    };
    let along = if side < 2 { w } else { h };
    let sections = (along / 70).max(1);
    for k in 0..rows {
        let seat = (k as f32 + 0.5) / rows as f32 <= seated;
        let (a, b) = (k * depth / rows, (k + 1) * depth / rows);
        for seg in 0..sections {
            let (p, q) = (seg * along / sections, (seg + 1) * along / sections);
            let c = if seat {
                shade(cols[((k / 3 + seg) % 2) as usize], if k % 2 == 0 { 0 } else { 18 })
            } else if k % 3 == 0 {
                [0x34, 0x36, 0x3a]
            } else {
                [0x84, 0x86, 0x8a]
            };
            match side {
                0 => fill(r.0 + p, r.3 - b, r.0 + q, r.3 - a, c),
                1 => fill(r.0 + p, r.1 + a, r.0 + q, r.1 + b, c),
                2 => fill(r.2 - b, r.1 + p, r.2 - a, r.1 + q, c),
                _ => fill(r.0 + a, r.1 + p, r.0 + b, r.1 + q, c),
            }
        }
    }
    // aisles across the terraces
    let along = if side < 2 { w } else { h };
    let aisles = (along / 70).max(1);
    for k in 1..aisles {
        let p = k * along / aisles;
        if side < 2 { fill(r.0 + p, r.1, r.0 + p + 2, r.3, [0x1e, 0x22, 0x28]) } else { fill(r.0, r.1 + p, r.2, r.1 + p + 2, [0x1e, 0x22, 0x28]) }
    }
    // a roof: see-through sheeting with its edge on the outside
    if roof {
        blend(r.0, r.1, w, h, |_, _| (rgb([0xdc, 0xe0, 0xe6]), 96));
        let edge = [0xa8, 0xae, 0xb8];
        match side {
            0 => fill(r.0, r.1, r.2, r.1 + 4, edge),
            1 => fill(r.0, r.3 - 4, r.2, r.3, edge),
            2 => fill(r.0, r.1, r.0 + 4, r.3, edge),
            _ => fill(r.2 - 4, r.1, r.2, r.3, edge),
        }
    }
    if building {
        // a crane over the stand
        let line = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_LINE);
        let (mx, my) = (r.0 + w / 3, r.1 + h / 3);
        let jib = if side < 2 { (mx + w / 4, my) } else { (mx, my + h / 4) };
        line(mx, my, jib.0, jib.1, 3, rgb(GOLD) as u32);
        line(mx, my, mx - (jib.0 - mx) / 3, my - (jib.1 - my) / 3, 3, rgb(GOLD) as u32);
        fill(mx - 4, my - 4, mx + 4, my + 4, [0x30, 0x30, 0x30]);
        line(jib.0, jib.1, jib.0, jib.1 + 10, 1, rgb([0x20, 0x20, 0x20]) as u32);
        // the works: yellow stripes along the outside
        if side < 2 {
            let y = if side == 0 { r.1 } else { r.3 - 5 };
            for x in (r.0..r.2).step_by(12) {
                fill(x, y, (x + 6).min(r.2), y + 5, GOLD);
            }
        } else {
            let x = if side == 2 { r.0 } else { r.2 - 5 };
            for y in (r.1..r.3).step_by(12) {
                fill(x, y, x + 5, (y + 6).min(r.3), GOLD);
            }
        }
    }
}

/// Life round the ground (user 2026-11-23: "restaurantjes, catering, zodat het wat levendiger
/// uitziet"): a road with a car park, food stalls with striped awnings, a restaurant, the club
/// shop in the club's colours, trees; placed in the room left round the stadium, the same for a
/// club each time (drawn from its name).
unsafe fn surroundings(area: (i32, i32, i32, i32), foot: (i32, i32, i32, i32), g: &Ground) {
    let mut seed = g.club.bytes().fold(0x9e37_79b9u32, |h, b| h.rotate_left(5) ^ b as u32).max(1);
    let mut next = |n: i32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % n.max(1) as u32) as i32
    };
    let tree = |x: i32, y: i32, r: i32| {
        blend(x - r, y - r, 2 * r, 2 * r, |c, w| {
            let (dx, dy) = (c - r, w - r);
            let inside = dx * dx + dy * dy <= r * r;
            let dark = (dx + r / 3) * (dx + r / 3) + (dy + r / 3) * (dy + r / 3) > r * r * 2 / 3;
            (rgb(if dark { [0x1e, 0x4a, 0x24] } else { [0x2c, 0x66, 0x30] }), if inside { 255 } else { 0 })
        });
    };
    let stall = |x: i32, y: i32, w: i32, h: i32, awning: [u8; 3]| {
        fill(x, y, x + w, y + h, [0xd8, 0xcc, 0xb0]);
        for k in 0..(w / 6).max(1) {
            fill(x + k * 6, y, (x + k * 6 + 3).min(x + w), y + h / 2, awning);
        }
        fill(x, y + h / 2, x + w, y + h / 2 + 1, [0x60, 0x58, 0x48]);
    };
    let (gx1, gy1, gx2, gy2) = area;
    // the road along the bottom, its dashes
    let road = gy2 - 20;
    fill(gx1, road, gx2, gy2, [0x34, 0x36, 0x3a]);
    for x in (gx1..gx2).step_by(24) {
        fill(x, road + 9, x + 12, road + 11, [0xd8, 0xd8, 0xc8]);
    }
    // the car park, left of the stadium down to the road
    if foot.0 - gx1 > 60 {
        // (a car park, not a field of cars: at most 130 wide and half the height)
        let (px1, px2, py1) = ((foot.0 - 10 - 130).max(gx1 + 8), foot.0 - 10, foot.1 + (foot.3 - foot.1) / 2);
        let py2 = (road - 6).min(py1 + (foot.3 - foot.1) / 2 + 30);
        fill(px1, py1, px2, py2, [0x40, 0x42, 0x46]);
        let colours = [[0xb8, 0x30, 0x30], [0x30, 0x58, 0xa8], [0xd8, 0xd8, 0xd8], [0x28, 0x28, 0x28], [0xc8, 0xa8, 0x30], [0x48, 0x78, 0x48]];
        let mut y = py1 + 6;
        while y + 10 < py2 {
            let mut x = px1 + 6;
            while x + 16 < px2 {
                if next(10) < 7 {
                    fill(x, y, x + 14, y + 8, colours[next(6) as usize]);
                    fill(x + 3, y + 2, x + 11, y + 6, [0x20, 0x26, 0x2e]);
                }
                x += 20;
            }
            fill(px1 + 2, y + 11, px2 - 2, y + 12, [0x5a, 0x5c, 0x60]);
            y += 16;
        }
    }
    // the restaurant and the club shop above the stadium, trees between
    if foot.1 - gy1 > 40 {
        let h = (foot.1 - gy1 - 14).min(46);
        let y = foot.1 - 8 - h;
        let (rw, sw) = ((foot.2 - foot.0) / 4, (foot.2 - foot.0) / 5);
        fill(foot.0, y, foot.0 + rw, y + h, [0x7a, 0x4a, 0x34]);
        fill(foot.0 + 4, y + 4, foot.0 + rw - 4, y + h / 2, [0xa8, 0x6a, 0x48]);
        for k in 0..(rw / 14) {
            fill(foot.0 + 6 + k * 14, y + h - 8, foot.0 + 12 + k * 14, y + h - 2, [0xe8, 0xe0, 0xd0]);
        }
        let shop = foot.2 - sw;
        fill(shop, y, foot.2, y + h, g.colours[0]);
        fill(shop + 4, y + 4, foot.2 - 4, y + h / 3, g.colours[1]);
        for k in 0..((shop - foot.0 - rw) / 40).max(0) {
            tree(foot.0 + rw + 20 + k * 40 + next(8), y + h / 2 + next(10) - 5, (h / 3).clamp(6, 14));
        }
    }
    // food stalls along the bottom, between the stadium and the road
    if road - foot.3 > 28 {
        let (h, w) = ((road - foot.3 - 16).min(26), 30);
        let awnings = [[0xc8, 0x30, 0x30], [0x30, 0x80, 0x40], [0xd8, 0x98, 0x20], [0x30, 0x60, 0xb0]];
        let mut x = foot.0 + 10;
        while x + w < foot.2 {
            stall(x, foot.3 + 8, w, h, awnings[next(4) as usize]);
            x += w + 18 + next(16);
        }
    }
    // trees and a stall or two on the right
    if gx2 - foot.2 > 40 {
        let r = ((gx2 - foot.2) / 5).clamp(6, 16);
        let mut y = foot.1 + r;
        while y + r < road - 6 {
            tree(foot.2 + (gx2 - foot.2) / 2 + next(10) - 5, y, r);
            y += r * 2 + 8 + next(10);
        }
        stall(foot.2 + 10, foot.3 - 30, ((gx2 - foot.2) / 2).min(36), 22, g.colours[0]);
    }
}

fn sep(v: i32) -> String {
    let d = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, c) in d.chars().enumerate() {
        if k > 0 && (d.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

// ---------------------------------------------------------------- the rendered ground
/// One picture of Data\cmui_stadium.bin (ui\stadium3d\build_stadium.py, Blender): its place in
/// the full frame, the frame's points (pitch centre, the N S W E stands' middles, the centre
/// circle's radius), its pixels (RGB565 little-endian, alpha, what it is: class | stand << 2 |
/// segment << 4; class 1 and 2 seats in the club's two colours, 3 club colour trim).
struct Pic {
    name: String,
    fw: i32,
    fh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    pts: [(i32, i32); 8],
    px: Vec<u8>,
}

/// The picture made for the page: what it was made for, its size and pixels (RGB565 | alpha <<
/// 16), the frame's scale and offset in it, the frame's points.
struct Shot {
    key: String,
    w: i32,
    h: i32,
    px: Vec<u32>,
    sc: f32,
    ox: f32,
    oy: f32,
    pts: [(i32, i32); 8],
    /// the long roofs' corners (far: top left, top right, bottom left; then the near one) when
    /// the ground has a roof: the words go on them
    roof: Option<[(i32, i32); 8]>,
    /// the amateur ground (`size_of` "a"): one seated stand (north) and a terrace (south)
    amateur: bool,
}

static mut PACK: Option<Option<Vec<Pic>>> = None;
static mut SHOT: Option<Shot> = None;

fn load_pack() -> Option<Vec<Pic>> {
    let d = std::fs::read("Data\\cmui_stadium.bin").ok()?;
    // 'CMS2': each picture's pixels as runs (count, pixel); 'CMST': as they are
    let runs = d.get(..4) == Some(b"CMS2");
    if d.len() < 8 || !(runs || &d[..4] == b"CMST") {
        return None;
    }
    let n = u32::from_le_bytes(d[4..8].try_into().ok()?) as usize;
    let mut out = Vec::new();
    for i in 0..n {
        let e = d.get(8 + 64 * i..8 + 64 * (i + 1))?;
        let u = |k: usize| u16::from_le_bytes([e[16 + 2 * k], e[17 + 2 * k]]) as i32;
        let p = |k: usize| i16::from_le_bytes([e[32 + 2 * k], e[33 + 2 * k]]) as i32;
        let at = u32::from_le_bytes(e[28..32].try_into().ok()?) as usize;
        let (w, h) = (u(4), u(5));
        let want = (w * h * 4) as usize;
        let px = if runs { unruns(d.get(at..)?, want)? } else { d.get(at..at + want)?.to_vec() };
        let name = String::from_utf8_lossy(e[..16].split(|&c| c == 0).next()?).into_owned();
        out.push(Pic { name, fw: u(0), fh: u(1), x: u(2), y: u(3), w, h, pts: std::array::from_fn(|k| (p(2 * k), p(2 * k + 1))), px });
    }
    log(&format!("stadium: {} pictures in Data\\cmui_stadium.bin", out.len()));
    Some(out)
}

/// `want` bytes of 4-byte pixels from runs (count 1..255, pixel).
fn unruns(src: &[u8], want: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(want);
    let mut k = 0;
    while out.len() < want {
        let n = *src.get(k)? as usize;
        let p = src.get(k + 1..k + 5)?;
        for _ in 0..n {
            out.extend_from_slice(p);
        }
        k += 5;
    }
    out.truncate(want);
    Some(out)
}

/// The pack's ground for a capacity: amateur (one small stand, a terrace, rails behind the goals:
/// a friend 2026-11-24, a non-league ground of 1,500 looked like one of 11,000), small, middle,
/// large (two tiers), huge.
fn size_of(capacity: i32) -> &'static str {
    match capacity {
        ..3_000 => "a",
        ..12_000 => "s",
        ..30_000 => "m",
        ..60_000 => "l",
        _ => "x",
    }
}

/// A segment (0..15 along a stand) is standing places by the ground's own share: the ends first
/// (stands 2 and 3, about 30% of the places), from their middle out, then the long sides.
fn standing(stand: u8, seg: u8, seated: f32) -> bool {
    let s = (1.0 - seated).clamp(0.0, 1.0);
    let share = if stand >= 2 { (s / 0.3).min(1.0) } else { ((s - 0.3) / 0.7).max(0.0) };
    (seg as f32 - 7.5).abs() < share * 8.0
}

/// The club's two colours for the mosaic; the second darker (or lighter) when they are alike.
fn mosaic(cols: [[u8; 3]; 2]) -> [[u8; 3]; 2] {
    let d: i32 = (0..3).map(|i| (cols[0][i] as i32 - cols[1][i] as i32).abs()).sum();
    if d >= 90 {
        return cols;
    }
    let lum = cols[0].iter().map(|&v| v as i32).sum::<i32>() / 3;
    [cols[0], cols[0].map(|v| if lum > 128 { (v as i32 * 55 / 100) as u8 } else { (v as i32 + (255 - v as i32) * 6 / 10) as u8 })]
}

/// A pixel of the pack as the page shows it: seats and trim in the club's colours in the light
/// they were rendered in (they are white in the pack), standing segments concrete grey.
fn paint(p: &[u8], cols: [[u8; 3]; 2], seated: f32) -> [u32; 3] {
    let c = u16::from_le_bytes([p[0], p[1]]) as u32;
    let rgb = [(c >> 11 & 31) * 255 / 31, (c >> 5 & 63) * 255 / 63, (c & 31) * 255 / 31];
    let id = p[3];
    let class = id & 3;
    if class == 0 {
        return rgb;
    }
    let lum = (rgb[0] * 3 + rgb[1] * 6 + rgb[2]) / 10;
    // (class 3, trim, fans, the shop's band: the club's main colour, the game's second one: its
    // first is the text on it, Chelsea yellow on blue, 2026-11-23)
    let base = if class == 3 {
        cols[1]
    } else if standing(id >> 2 & 3, id >> 4, seated) {
        [0x70, 0x73, 0x78]
    } else {
        cols[(class - 1) as usize]
    };
    base.map(|v| ((v as u32).max(30) * lum / 205).min(255))
}

/// The pitch's grass in the pack's ids (class 0, "stand" 3, segment 15): rendered one plain green,
/// the club's mowing pattern goes over it here (`Mow`).
const PITCH_ID: u8 = 0xfc;

/// A club's mowing pattern on the pitch (a friend 2026-11-24: "verschillende maaistijlen"; the
/// pack had one, stripes, since the 2026-11-23 restyle; checks before it): one of six, by the
/// club's name, laid on the pitch through its axes in the frame (pack points 0, 6, 7: the centre
/// spot and the centre circle's edge along and across, 9.15 m).
struct Mow {
    c: (f32, f32),
    inv: [f32; 4],
    kind: u32,
}

impl Mow {
    fn new(pts: &[(i32, i32); 8], club: &str) -> Option<Mow> {
        if pts[6] == (0, 0) {
            return None;
        }
        let d = |p: (i32, i32)| ((p.0 - pts[0].0) as f32 / 9.15, (p.1 - pts[0].1) as f32 / 9.15);
        let ((ax, ay), (bx, by)) = (d(pts[6]), d(pts[7]));
        let det = ax * by - bx * ay;
        if det.abs() < 1e-3 {
            return None;
        }
        let kind = club.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 6;
        Some(Mow { c: (pts[0].0 as f32, pts[0].1 as f32), inv: [by / det, -bx / det, -ay / det, ax / det], kind })
    }

    /// The pitch's metres (along, across from the centre spot) at a frame point.
    fn uv(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x - self.c.0, y - self.c.1);
        (self.inv[0] * dx + self.inv[1] * dy, self.inv[2] * dx + self.inv[3] * dy)
    }

    /// The light of the grass at a frame point (1.0 the pack's own green).
    fn shade(&self, x: f32, y: f32) -> f32 {
        let (u, v) = self.uv(x, y);
        let odd = |t: f32| (t.floor() as i32).rem_euclid(2) == 1;
        let pm = |on: bool, d: f32| if on { 1.0 + d } else { 1.0 - d };
        match self.kind {
            // stripes across the pitch, faintly along it too (the restyle's lawn)
            0 => pm(odd(u / 7.5), 0.09) * pm(odd(v / 11.33), 0.025),
            // checks (the first pack's)
            1 => pm(odd(u / 10.5) != odd(v / 11.33), 0.09),
            // diagonal bands
            2 => pm(odd((u + v) / 7.0), 0.085),
            // diamonds
            3 => pm(odd((u + v) / 9.0) != odd((u - v) / 9.0), 0.08),
            // rings round the centre spot
            4 => pm(odd(u.hypot(v) / 6.5), 0.08),
            // stripes along the pitch
            _ => pm(odd(v / 7.56), 0.09),
        }
    }
}

/// The average of a picture's pixels under the frame rectangle [fx0, fx1) x [fy0, fy1): colour
/// (weighted by alpha) and alpha; None where it has nothing. With `mow` the pitch's grass takes
/// the club's mowing pattern.
fn sample(pic: &Pic, f: (f32, f32, f32, f32), cols: [[u8; 3]; 2], seated: f32, mow: Option<&Mow>) -> Option<([f32; 3], f32)> {
    let span = |a: f32, b: f32, at: i32, n: i32| {
        let lo = a.floor() as i32;
        ((lo - at).max(0), ((b.ceil() as i32).max(lo + 1) - at).min(n))
    };
    let ((x0, x1), (y0, y1)) = (span(f.0, f.1, pic.x, pic.w), span(f.2, f.3, pic.y, pic.h));
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (mut acc, mut a_sum, mut n) = ([0f32; 3], 0f32, 0f32);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = &pic.px[((y * pic.w + x) * 4) as usize..][..4];
            n += 1.0;
            if p[2] == 0 {
                continue;
            }
            let a = p[2] as f32 / 255.0;
            let mut c = paint(p, cols, seated);
            if let Some(m) = mow.filter(|_| p[3] == PITCH_ID) {
                let k = m.shade((pic.x + x) as f32 + 0.5, (pic.y + y) as f32 + 0.5);
                c = c.map(|v| ((v as f32 * k) as u32).min(255));
            }
            for i in 0..3 {
                acc[i] += c[i] as f32 * a;
            }
            a_sum += a;
        }
    }
    (a_sum > 0.0).then(|| (acc.map(|v| v / a_sum), a_sum / n))
}

/// The page's picture at w x h, the frame filling it ("cover", its edges cut): the base, the roof
/// when the ground has one (faint while one is built), crane and scaffolding while places are.
fn compose(pack: &[Pic], g: &Ground, w: i32, h: i32) -> Option<Shot> {
    let want = format!("base_{}", size_of(g.capacity));
    let base = pack.iter().find(|p| p.name == want).or_else(|| pack.iter().find(|p| p.name.starts_with("base_")))?;
    let size = &base.name[5..];
    let layer = |kind: &str| pack.iter().find(|p| p.name == format!("{kind}_{size}"));
    let roofing = g.works.iter().any(|w| w.0 == ROOF);
    let roof = if g.roof || roofing { layer("roof") } else { None };
    let works = if g.works.iter().any(|w| matches!(w.0, SEATS | STANDING | CONVERT | ROOF | NEW)) { layer("works") } else { None };
    // (a little closer than the whole frame, round the pitch: the mock's ground fills its column)
    let sc = (w as f32 / base.fw as f32).max(h as f32 / base.fh as f32) * 1.15;
    let centre = |p: i32, size: f32, full: i32| (size / 2.0 - p as f32 * sc).clamp(size - full as f32 * sc, 0.0);
    let (ox, oy) = (centre(base.pts[0].0, w as f32, base.fw), centre(base.pts[0].1, h as f32, base.fh));
    let cols = mosaic(g.colours);
    // (the amateur ground: its one stand always seats, its terrace is drawn as one)
    let amateur = size == "a";
    let seated = if amateur { 1.0 } else if g.capacity > 0 { g.seats as f32 / g.capacity as f32 } else { 1.0 };
    let mow = Mow::new(&base.pts, &g.club);
    let shop_built = g.works.iter().any(|w| w.0 == SHOP);
    // (a megastore from level 2: the bigger building, its own layer; the club shop before)
    let shop_level = g.shop.max(g.works.iter().filter(|w| w.0 == SHOP).map(|w| w.1 as u8).max().unwrap_or(0));
    let shop = match shop_level {
        0 => None,
        1 => layer("shop"),
        _ => layer("mega").or_else(|| layer("shop")),
    };
    let shop = if g.shop > 0 || shop_built { shop } else { None };
    let layers = [(Some(base), 1.0f32), (roof, if g.roof { 1.0 } else { 0.45 }), (works, 1.0), (shop, if g.shop > 0 { 1.0 } else { 0.45 })];
    let (cx, cy) = (ox + base.pts[0].0 as f32 * sc, oy + base.pts[0].1 as f32 * sc);
    let mut px = vec![0u32; (w * h) as usize];
    for dy in 0..h {
        let (fy0, fy1) = ((dy as f32 - oy) / sc, (dy as f32 + 1.0 - oy) / sc);
        for dx in 0..w {
            let (fx0, fx1) = ((dx as f32 - ox) / sc, (dx as f32 + 1.0 - ox) / sc);
            let (mut acc, mut alpha) = ([0f32; 3], 0f32);
            for (pic, fade) in layers {
                let Some(pic) = pic else { continue };
                if let Some((c, a)) = sample(pic, (fx0, fx1, fy0, fy1), cols, seated, mow.as_ref()) {
                    let a = a * fade;
                    for i in 0..3 {
                        acc[i] = acc[i] * (1.0 - a) + c[i] * a;
                    }
                    alpha = alpha * (1.0 - a) + a;
                }
            }
            if alpha > 0.0 {
                // the ground as an island on the page: the land round it fades into the page's
                // dark (the mock's look)
                let (ex, ey) = ((dx as f32 - cx) / (w as f32 * 0.6), (dy as f32 - cy) / (h as f32 * 0.44));
                let d = (ex * ex + ey * ey).sqrt();
                // (weaker since the town is back round the ground, 2026-11-23: from further out,
                // more gently, and never quite the page's dark)
                let fade = ((d - 1.0) / 0.5).clamp(0.0, 1.0);
                let fade = fade * fade * (3.0 - 2.0 * fade) * 0.8;
                let acc = [0, 1, 2].map(|i| acc[i] / alpha * (1.0 - fade) + PAGE[i] as f32 * fade);
                let c = acc.map(|v| v.clamp(0.0, 255.0) as u8);
                px[(dy * w + dx) as usize] = rgb565(c) as u32 | ((alpha * 255.0) as u32) << 16;
            } else {
                // (the page's dark where the pack has nothing: the 2026-11-23 grounds stand on a
                // plinth, see-through round it, and roof_words' scratch text at the top left
                // showed through)
                px[(dy * w + dx) as usize] = rgb565(PAGE) as u32 | 255 << 16;
            }
        }
    }
    let key = String::new();
    let roof_pts = if g.roof { roof.map(|r| r.pts).filter(|p| p[1] != (0, 0)) } else { None };
    Some(Shot { key, w, h, px, sc, ox, oy, pts: base.pts, roof: roof_pts, amateur })
}

/// The rendered ground in r, with the club's badge in the centre circle and each stand's places;
/// false when there is no pack (the drawing then).
#[allow(static_mut_refs)]
unsafe fn rendered(r: (i32, i32, i32, i32), g: &Ground, f2: u32) -> bool {
    let Some(pack) = PACK.get_or_insert_with(load_pack).as_ref() else { return false };
    let (w, h) = (r.2 - r.0, r.3 - r.1);
    if w < 8 || h < 8 {
        return false;
    }
    let works: Vec<u8> = g.works.iter().map(|w| w.0).collect();
    let key = format!("{w}x{h} {} {} {} {:?} {:?} {} {} {}", g.capacity, g.seats, g.roof, g.colours, works, g.shop, g.club, g.name);
    if SHOT.as_ref().is_none_or(|s| s.key != key) {
        let started = std::time::Instant::now();
        SHOT = compose(pack, g, w, h).map(|s| Shot { key: key.clone(), ..s });
        if let Some(s) = SHOT.as_mut() {
            if let Some(c) = s.roof {
                // (the ground's name on the near roof; the club's on the far one went, a friend
                // 2026-11-24: the crest on the pitch says it already)
                // (the amateur ground's one roof is its stand's, the far one)
                let on = if s.amateur { [c[0], c[1], c[2]] } else { [c[3], c[4], c[5]] };
                roof_words(s, r, on, &to_latin1(&g.name.to_uppercase()));
            }
        }
        log(&format!("stadium: picture {key} made in {} ms", started.elapsed().as_millis()));
    }
    let Some(s) = SHOT.as_ref() else { return false };
    blend(r.0, r.1, w, h, |c, row| {
        let v = s.px[(row * w + c) as usize];
        ((v & 0xffff) as u16, v >> 16)
    });
    let at = |p: (i32, i32)| (r.0 + (s.ox + p.0 as f32 * s.sc) as i32, r.1 + (s.oy + p.1 as f32 * s.sc) as i32);
    let (cx, cy) = at(s.pts[0]);
    let rad = (s.pts[5].0 as f32 * s.sc) as i32;
    let key = to_latin1(&g.club);
    if has_crest(&key) && rad > 4 && !(s.pts[6] != (0, 0) && flat_crest(&key, at(s.pts[0]), at(s.pts[6]), at(s.pts[7]))) {
        // (big on the centre spot, as the mock has it; upright when the pack has no pitch axes)
        let size = rad * 26 / 10;
        draw_crest(&key, cx - size / 2, cy - size / 2, size);
    }
    // each stand's places (the long sides 35% each, the ends 15%) on a dark label; the amateur
    // ground's stand its seats, its terrace the standing places, nothing behind the goals
    let labels: Vec<(usize, i32)> = if s.amateur {
        vec![(1, g.seats), (2, g.capacity - g.seats)].into_iter().filter(|l| l.1 > 0).collect()
    } else {
        [(1, 0.35f32), (2, 0.35), (3, 0.15), (4, 0.15)].map(|(k, share)| (k, (g.capacity as f32 * share / 100.0).round() as i32 * 100)).to_vec()
    };
    for (k, places) in labels {
        // (toward the front rows: on the roof they covered the club's name, night 11)
        let (mx, my) = at(s.pts[k]);
        // (smaller on a small screen, and further in: at 800x600 they covered the roof's name)
        let small = screen().1 < 750;
        let (font, inward, ph) = if small { (0xffff0001u32, 60, 16) } else { (f2, 72, 24) };
        let (x, y) = (cx + (mx - cx) * inward / 100, cy + (my - cy) * inward / 100);
        let t = sep(places);
        let tw = text_width(font, t.as_bytes());
        let half = ph / 2;
        if x - tw / 2 - 8 < r.0 || x + tw / 2 + 8 > r.2 || y - half < r.1 || y + half > r.3 {
            continue;
        }
        // (a rounded pill, see-through a little)
        let (pw, rad) = (tw + ph * 3 / 4, half as f32 - 1.0);
        blend(x - pw / 2, y - half, pw, ph, |c, row| {
            let qx = ((c as f32 + 0.5 - pw as f32 / 2.0).abs() - (pw as f32 / 2.0 - rad)).max(0.0);
            let qy = ((row as f32 + 0.5 - ph as f32 / 2.0).abs() - (ph as f32 / 2.0 - rad)).max(0.0);
            let d = qx.hypot(qy) - rad;
            let a = (0.5 - d).clamp(0.0, 1.0);
            let edge = d > -1.5;
            (rgb565(if edge { [0x4a, 0x52, 0x5e] } else { [0x0c, 0x0f, 0x14] }), (a * if edge { 210.0 } else { 200.0 }) as u32)
        });
        text_at(x, y - half, y + half, font, WHITE, t.as_bytes(), 1);
    }
    true
}

/// The crest laid flat on the grass round the centre spot (the mock's look): `x` and `y` the
/// centre circle's edge along the pitch's two axes on screen. Its top goes the way of the axis
/// that points up the screen most, its right the other one, pointing right. False: no crest.
unsafe fn flat_crest(key: &[u8], c: (i32, i32), x: (i32, i32), y: (i32, i32)) -> bool {
    if !on(CRESTS_ON) {
        return false;
    }
    let Some((px, side)) = crest_pixels(key) else { return false };
    let n = side as i32;
    // the crest is 1.35 circle radii from its middle to an edge
    let k = 1.35f32;
    let ax = ((x.0 - c.0) as f32 * k, (x.1 - c.1) as f32 * k);
    let ay = ((y.0 - c.0) as f32 * k, (y.1 - c.1) as f32 * k);
    let cands = [ax, (-ax.0, -ax.1), ay, (-ay.0, -ay.1)];
    let up = *cands.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    let other = if up == ax || up == (-ax.0, -ax.1) { ay } else { ax };
    let right = if other.0 >= 0.0 { other } else { (-other.0, -other.1) };
    let det = right.0 * up.1 - right.1 * up.0;
    if det.abs() < 4.0 {
        return false;
    }
    let xs = [right.0 + up.0, right.0 - up.0, -right.0 + up.0, -right.0 - up.0];
    let ys = [right.1 + up.1, right.1 - up.1, -right.1 + up.1, -right.1 - up.1];
    let (x0, x1) = (xs.iter().cloned().fold(f32::MAX, f32::min) as i32 - 1, xs.iter().cloned().fold(f32::MIN, f32::max) as i32 + 1);
    let (y0, y1) = (ys.iter().cloned().fold(f32::MAX, f32::min) as i32 - 1, ys.iter().cloned().fold(f32::MIN, f32::max) as i32 + 1);
    // two samples a pixel each way: soft edges without a blur
    blend(c.0 + x0, c.1 + y0, x1 - x0, y1 - y0, |col, row| {
        let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
        for (sx, sy) in [(0.25f32, 0.25f32), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
            let (dx, dy) = ((x0 + col) as f32 + sx, (y0 + row) as f32 + sy);
            let u = (dx * up.1 - dy * up.0) / det;
            let v = (right.0 * dy - right.1 * dx) / det;
            if u.abs() >= 1.0 || v.abs() >= 1.0 {
                continue;
            }
            let (cx, cy) = (((u + 1.0) / 2.0 * n as f32) as i32, ((1.0 - (v + 1.0) / 2.0) * n as f32) as i32);
            let p = &px[((cy.min(n - 1) * n + cx.min(n - 1)) * 3) as usize..][..3];
            let (pc, pa) = (u16::from_le_bytes([p[0], p[1]]) as u32, p[2] as u32);
            r += (pc >> 11 & 31) * pa;
            g += (pc >> 5 & 63) * pa;
            b += (pc & 31) * pa;
            a += pa;
        }
        if a == 0 {
            return (0, 0);
        }
        ((((r + a / 2) / a) << 11 | ((g + a / 2) / a) << 5 | (b + a / 2) / a) as u16, a / 4)
    });
    true
}

/// Big white words laid onto a roof (the parallelogram top left, top right, bottom left in the
/// frame): drawn once in the game's biggest font where the picture goes (it is drawn over them
/// right after), read back as a mask, then mapped onto the roof into the made picture.
unsafe fn roof_words(s: &mut Shot, r: (i32, i32, i32, i32), c: [(i32, i32); 3], text: &[u8]) {
    let font = 0xffff0006u32;
    let (tw, th) = (text_width(font, text) + 8, 64);
    if text.is_empty() || tw > r.2 - r.0 || th > r.3 - r.1 {
        return;
    }
    let (x0, y0) = (r.0, r.1);
    fill(x0, y0, x0 + tw, y0 + th, [0, 0, 0]);
    text_at(x0 + 4, y0, y0 + th, font, [0xff, 0xff, 0xff], text, 0);
    let f565 = screen_format() == Some(true);
    let mut mask = vec![0u8; (tw * th) as usize];
    with_back_buffer(|bits, pitch, w, h| {
        for y in 0..th {
            for x in 0..tw {
                let (px, py) = (x0 + x, y0 + y);
                if px < w && py < h {
                    let v = *((bits + (py * pitch + px * 2) as usize) as *const u16);
                    mask[(y * tw + x) as usize] = unpack(v as u32, f565)[1].min(255) as u8;
                }
            }
        }
    });
    // the part that holds the letters
    let rows: Vec<i32> = (0..th).filter(|&y| (0..tw).any(|x| mask[(y * tw + x) as usize] > 60)).collect();
    let cols: Vec<i32> = (0..tw).filter(|&x| (0..th).any(|y| mask[(y * tw + x) as usize] > 60)).collect();
    let (Some(&ry0), Some(&ry1), Some(&cx0), Some(&cx1)) = (rows.first(), rows.last(), cols.first(), cols.last()) else { return };
    let (mw, mh) = ((cx1 - cx0 + 1) as f32, (ry1 - ry0 + 1) as f32);
    // the roof in the made picture; the words in its middle: 60% of its depth high, as wide as
    // that makes them (at most 84% of its length)
    let to = |p: (i32, i32)| (s.ox + p.0 as f32 * s.sc, s.oy + p.1 as f32 * s.sc);
    let (o, a, b) = (to(c[0]), to(c[1]), to(c[2]));
    let (ux, uy, vx, vy) = (a.0 - o.0, a.1 - o.1, b.0 - o.0, b.1 - o.1);
    let det = ux * vy - uy * vx;
    let (ulen, vlen) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
    if det.abs() < 1.0 || ulen < 20.0 || vlen < 6.0 {
        return;
    }
    let v_span = 0.6;
    let u_span = (vlen * v_span * mw / mh / ulen).min(0.84);
    let (u0, v0) = ((1.0 - u_span) / 2.0, (1.0 - v_span) / 2.0);
    let xs = [o.0, a.0, b.0, a.0 + vx];
    let ys = [o.1, a.1, b.1, a.1 + vy];
    let (bx0, bx1) = (xs.iter().cloned().fold(f32::MAX, f32::min).max(0.0) as i32, xs.iter().cloned().fold(f32::MIN, f32::max).min(s.w as f32 - 1.0) as i32);
    let (by0, by1) = (ys.iter().cloned().fold(f32::MAX, f32::min).max(0.0) as i32, ys.iter().cloned().fold(f32::MIN, f32::max).min(s.h as f32 - 1.0) as i32);
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let (dx, dy) = (x as f32 + 0.5 - o.0, y as f32 + 0.5 - o.1);
            let u = (dx * vy - dy * vx) / det;
            let v = (ux * dy - uy * dx) / det;
            let (tu, tv) = ((u - u0) / u_span, (v - v0) / v_span);
            if !(0.0..1.0).contains(&tu) || !(0.0..1.0).contains(&tv) {
                continue;
            }
            let m = mask[((ry0 + (tv * mh) as i32) * tw + cx0 + (tu * mw) as i32) as usize] as u32;
            if m < 20 {
                continue;
            }
            let at = (y * s.w + x) as usize;
            let p = s.px[at];
            let c = p & 0xffff;
            let old = [(c >> 11 & 31) * 255 / 31, (c >> 5 & 63) * 255 / 63, (c & 31) * 255 / 31];
            let k = m * 88 / 100; // a touch of the roof through the paint
            let new = old.map(|v| ((v * (255 - k) + 0xf0 * k) / 255) as u8);
            s.px[at] = rgb565(new) as u32 | (p & 0xffff_0000);
        }
    }
}

// ---------------------------------------------------------------- the page
const PAGE: [u8; 3] = [0x0d, 0x11, 0x17];
const CARD: [u8; 3] = [0x13, 0x18, 0x20];
const EDGE: [u8; 3] = [0x2a, 0x32, 0x3e];
const DIM: [u8; 3] = [0x8c, 0x95, 0xa2];
const GREEN: [u8; 3] = [0x3c, 0xc8, 0x6e];
const RED: [u8; 3] = [0xe0, 0x58, 0x50];
const WORKS: [u8; 3] = [0xe6, 0x86, 0x18];

/// A rounded box from top to bottom colour with a one pixel edge, in one pass.
unsafe fn gradient(r: (i32, i32, i32, i32), top: [u8; 3], bottom: [u8; 3], edge: [u8; 3], radius: i32) {
    let (w, h) = (r.2 - r.0, r.3 - r.1);
    if w <= 2 * radius || h <= 2 * radius {
        return;
    }
    blend(r.0, r.1, w, h, |c, y| {
        let (dx, dy) = ((radius - c).max(c - (w - 1 - radius)).max(0), (radius - y).max(y - (h - 1 - radius)).max(0));
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        if d > radius as f32 + 0.5 {
            return (0, 0);
        }
        let border = c == 0 || y == 0 || c == w - 1 || y == h - 1 || d > radius as f32 - 0.5;
        let t = y as f32 / h as f32;
        let col = if border { edge } else { [0, 1, 2].map(|i| (top[i] as f32 * (1.0 - t) + bottom[i] as f32 * t) as u8) };
        (rgb565(col), 255)
    });
}

unsafe fn card(r: (i32, i32, i32, i32)) {
    gradient(r, CARD, CARD, EDGE, 8);
}

/// A piece of an icon in a unit square: a rounded box (x0, y0, x1, y1, corner), a disc (x, y, r),
/// a triangle, an elliptic ring (x, y, outer rx, ry, inner rx, ry).
enum Shape {
    B(f32, f32, f32, f32, f32),
    C(f32, f32, f32),
    T((f32, f32), (f32, f32), (f32, f32)),
    Ring(f32, f32, f32, f32, f32, f32),
}

impl Shape {
    /// The signed distance from p (< 0 inside).
    fn dist(&self, p: (f32, f32)) -> f32 {
        match *self {
            Shape::B(x0, y0, x1, y1, r) => {
                let (hx, hy) = ((x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r);
                let (qx, qy) = ((p.0 - (x0 + x1) / 2.0).abs() - hx, (p.1 - (y0 + y1) / 2.0).abs() - hy);
                qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
            }
            Shape::C(x, y, r) => (p.0 - x).hypot(p.1 - y) - r,
            Shape::T(a, b, c) => {
                // (Inigo Quilez's triangle)
                let sub = |u: (f32, f32), v: (f32, f32)| (u.0 - v.0, u.1 - v.1);
                let dot = |u: (f32, f32), v: (f32, f32)| u.0 * v.0 + u.1 * v.1;
                let (e0, e1, e2) = (sub(b, a), sub(c, b), sub(a, c));
                let (v0, v1, v2) = (sub(p, a), sub(p, b), sub(p, c));
                let near = |v: (f32, f32), e: (f32, f32)| {
                    let k = (dot(v, e) / dot(e, e)).clamp(0.0, 1.0);
                    sub(v, (e.0 * k, e.1 * k))
                };
                let (q0, q1, q2) = (near(v0, e0), near(v1, e1), near(v2, e2));
                let s = (e0.0 * e2.1 - e0.1 * e2.0).signum();
                let d = dot(q0, q0).min(dot(q1, q1)).min(dot(q2, q2));
                let inside = (s * (v0.0 * e0.1 - v0.1 * e0.0)).min(s * (v1.0 * e1.1 - v1.1 * e1.0)).min(s * (v2.0 * e2.1 - v2.1 * e2.0));
                -d.sqrt() * inside.signum()
            }
            Shape::Ring(x, y, rx, ry, ix, iy) => {
                let outer = ((p.0 - x) / rx).hypot((p.1 - y) / ry) - 1.0;
                let inner = ((p.0 - x) / ix).hypot((p.1 - y) / iy) - 1.0;
                (outer * rx.min(ry)).max(-inner * ix.min(iy))
            }
        }
    }
}

/// The little pictures of the page (after the mock's line icons), size s round (cx, cy), with
/// soft edges: 1..5 the works, 8 a hammer, 9 a ground.
unsafe fn icon(kind: u8, cx: i32, cy: i32, s: i32, c: [u8; 3]) {
    use Shape::*;
    let mut shapes = match kind {
        SEATS => vec![B(0.24, 0.06, 0.76, 0.52, 0.08), B(0.1, 0.42, 0.24, 0.74, 0.06), B(0.76, 0.42, 0.9, 0.74, 0.06), B(0.18, 0.56, 0.82, 0.74, 0.05), B(0.2, 0.72, 0.3, 0.94, 0.03), B(0.7, 0.72, 0.8, 0.94, 0.03)],
        STANDING => vec![C(0.5, 0.2, 0.13), C(0.2, 0.32, 0.1), C(0.8, 0.32, 0.1), B(0.33, 0.38, 0.67, 0.92, 0.14), B(0.04, 0.48, 0.36, 0.88, 0.12), B(0.64, 0.48, 0.96, 0.88, 0.12)],
        CONVERT => vec![B(0.18, 0.06, 0.32, 0.5, 0.03), T((0.06, 0.46), (0.44, 0.46), (0.25, 0.72)), B(0.68, 0.5, 0.82, 0.94, 0.03), T((0.56, 0.54), (0.94, 0.54), (0.75, 0.28))],
        ROOF => vec![T((0.02, 0.5), (0.98, 0.5), (0.5, 0.08)), B(0.14, 0.5, 0.26, 0.94, 0.02), B(0.74, 0.5, 0.86, 0.94, 0.02), B(0.26, 0.62, 0.74, 0.7, 0.02)],
        HEATING => vec![B(0.06, 0.86, 0.94, 0.94, 0.03)],
        8 => vec![B(0.08, 0.1, 0.62, 0.36, 0.05)],
        10 => vec![B(0.28, 0.22, 0.72, 0.94, 0.05), T((0.3, 0.2), (0.02, 0.44), (0.16, 0.58)), T((0.7, 0.2), (0.98, 0.44), (0.84, 0.58)), B(0.4, 0.14, 0.6, 0.24, 0.04)],
        11 => vec![B(0.14, 0.36, 0.86, 0.96, 0.06), Ring(0.5, 0.36, 0.22, 0.24, 0.13, 0.15)],
        12 => vec![B(0.2, 0.46, 0.8, 0.95, 0.07), Ring(0.5, 0.44, 0.2, 0.28, 0.11, 0.19)],
        13 => vec![B(0.12, 0.12, 0.88, 0.32, 0.06), B(0.22, 0.28, 0.42, 0.8, 0.05), B(0.58, 0.28, 0.78, 0.92, 0.05)],
        14 => vec![B(0.06, 0.62, 0.26, 0.96, 0.03), B(0.4, 0.4, 0.6, 0.96, 0.03), B(0.74, 0.12, 0.94, 0.96, 0.03)],
        _ => vec![Ring(0.5, 0.52, 0.48, 0.32, 0.34, 0.19), B(0.34, 0.45, 0.66, 0.59, 0.02)],
    };
    if kind == HEATING {
        // three wavy lines of heat over the base
        for x0 in [0.22f32, 0.5, 0.78] {
            for k in 0..13 {
                let y = 0.08 + k as f32 * 0.058;
                shapes.push(C(x0 + 0.07 * (k as f32 * 0.9).sin(), y, 0.05));
            }
        }
    }
    if kind == 8 {
        // the handle, at a slant
        for k in 0..12 {
            let t = k as f32 / 11.0;
            shapes.push(C(0.36 + t * 0.5, 0.3 + t * 0.6, 0.06));
        }
    }
    let (x0, y0) = (cx - s / 2, cy - s / 2);
    let col = rgb565(c);
    let size = s as f32;
    blend(x0, y0, s, s, |px, py| {
        let p = ((px as f32 + 0.5) / size, (py as f32 + 0.5) / size);
        let d = shapes.iter().map(|sh| sh.dist(p)).fold(f32::MAX, f32::min);
        (col, ((0.5 - d * size).clamp(0.0, 1.0) * 255.0) as u32)
    });
}

/// A refusal as a tile shows it: short, in capitals (the whole sentence five times over was
/// noise next to the mock's calm tiles, night 11); the order's answer keeps the whole one.
fn short(why: &str) -> String {
    match why {
        "Not enough money in the bank" => "NOT ENOUGH MONEY".into(),
        "No room left to build" => "NO ROOM LEFT".into(),
        "Not enough standing places" => "NO STANDING LEFT".into(),
        other => other.to_uppercase(),
    }
}

/// A building time as words: "1 day", "6 days", "9 weeks".
fn span(days: i32) -> String {
    match days {
        ..=1 => "1 day".into(),
        2..=13 => format!("{days} days"),
        _ => format!("{} weeks", (days + 6) / 7),
    }
}

/// The largest of `fonts` (largest first) that fits `s` in `width`; the smallest when none does
/// (800x600: the tiles' words ran past their edges, night 11).
unsafe fn fit(fonts: &[u32], s: &[u8], width: i32) -> u32 {
    fonts.iter().copied().find(|&f| text_width(f, s) <= width).unwrap_or(fonts[fonts.len() - 1])
}

/// The flip pass: the page while it is open (user 2026-11-23, after the Grok mock: the rendered
/// ground as a tall picture at the left, three cards at the right: the ground's figures, the
/// works to order, the key).
#[allow(static_mut_refs)]
pub unsafe fn flip() {
    if !OPEN {
        return;
    }
    // (a client's order: the host's answer comes after the click, so the page is asked for again
    // (the view, never the order: it would be paid twice) for a few seconds, night 12)
    if let Some(t) = ORDERED_AT {
        let ms = t.elapsed().as_millis();
        if ms > 6000 {
            ORDERED_AT = None;
        } else if ms > 400 && ms / 400 != LAST_ASK {
            // (forgotten once, at the first refresh: the order is asked before it, so the host
            // answers this after the order; forgotten each time, its answer was dropped unread)
            let k = key(b'!', &[CLUB as u32, 0, 0]);
            if LAST_ASK == 0 || ms / 400 == 1 {
                remote::forget(&k);
            }
            LAST_ASK = ms / 400;
            if let Some(g) = ask_host(&k).and_then(|t| decode(&String::from_utf8_lossy(&t))) {
                GROUND = Some(g);
                REDRAW = true;
            }
        }
    }
    let Some(g) = GROUND.clone() else { return };
    let (sw, sh) = screen();
    let (f1, f2, f3, f5, f6) = (0xffff0001u32, 0xffff0002u32, 0xffff0003u32, 0xffff0005u32, 0xffff0006u32);
    // (the fonts do not shrink with the screen: smaller ones below 1280x800)
    let small = sh < 750;
    let head = if small { f3 } else { f5 };
    let left = bar_end(sw);
    // (from the menu bar on: the bar stays, as on the other pages)
    fill(left, 0, sw - 1, sh - 1, PAGE);
    let m = (sh / 88).max(6);
    // the header: badge, club, ground; Back at the right
    let bar = sh * 8 / 100;
    fill(left, 0, sw - 1, bar, [0x11, 0x16, 0x1e]);
    fill(left, bar, sw - 1, bar + 1, EDGE);
    let key = to_latin1(&g.club);
    let cs = bar - m;
    let mut tx = left + m * 2;
    if has_crest(&key) {
        draw_crest(&key, tx, (bar - cs) / 2, cs);
        tx += cs + m * 2;
    }
    let b = (sw - m * 2 - sw / 10, m, sw - m * 2, bar - m);
    let club = to_latin1(&g.club.to_uppercase());
    let title = if small { fit(&[f3, f2], &club, b.0 - tx - m) } else { fit(&[f6, f5, f3], &club, b.0 - tx - m) };
    text_at(tx, 2, bar * 62 / 100, title, WHITE, &club, 0);
    let sub = to_latin1(&format!("{}  -  STADIUM", g.name.to_uppercase()));
    text_at(tx, bar * 56 / 100, bar - 2, fit(&[f2, f1], &sub, b.0 - tx - m), DIM, &sub, 0);
    gradient(b, [0x2c, 0x50, 0x8a], [0x1c, 0x36, 0x62], [0x3e, 0x66, 0xa8], 6);
    text_at((b.0 + b.2) / 2, b.1, b.3, f3, WHITE, b"Back", 1);
    BACK = Some(b);
    // the ground, left: the rendered picture (the drawing without the pack)
    let pic = (left, bar + 1, if WIDE { sw } else { left + (sw - left) * 46 / 100 }, sh);
    // (a new stadium being built: "Fully built" is that one)
    let new_size = g.works.iter().find(|w| w.0 == NEW).map(|w| w.1);
    let most = new_size.unwrap_or(g.room.min(MOST)).max(g.capacity);
    let can_grow = most > g.capacity || !g.roof;
    if !can_grow {
        FULL_VIEW = false;
    }
    let next = if new_size.is_some() && !g.next_name.is_empty() { g.next_name.clone() } else { g.name.clone() };
    let shown = if FULL_VIEW { Ground { capacity: most, seats: most, roof: true, heating: true, works: Vec::new(), name: next, ..g.clone() } } else { g.clone() };
    if !rendered(pic, &shown, f2) {
        flat(pic.0 + m, pic.1 + m, pic.2 - m, pic.3 - m, m, &g, f2);
    }
    VIEW_PILLS.clear();
    if can_grow {
        // the pills at the picture's top right
        let labels: [(&[u8], bool); 2] = [(b"NOW", false), (b"FULLY BUILT", true)];
        let ph = (sh / 30).max(18);
        let mut px = pic.2 - m * 2;
        for (label, full) in labels.iter().rev() {
            let w = text_width(f1, label) + m * 3;
            let r = (px - w, pic.1 + m * 2, px, pic.1 + m * 2 + ph);
            let on = FULL_VIEW == *full;
            gradient(r, if on { [0x2e, 0x58, 0x98] } else { [0x14, 0x19, 0x21] }, if on { [0x1a, 0x36, 0x66] } else { [0x10, 0x14, 0x1a] }, if on { [0x44, 0x72, 0xb8] } else { EDGE }, ph / 2 - 1);
            text_at((r.0 + r.2) / 2, r.1, r.3, f1, if on { WHITE } else { DIM }, label, 1);
            VIEW_PILLS.push((r, *full));
            px = r.0 - m / 2;
        }
        if FULL_VIEW {
            // (a new stadium: its own room to grow by the old ground's level, as `complete` sets it)
            let note = match new_size {
                Some(n) => format!("{} PLACES, ALL SEATED, ROOFED  -  ROOM TO GROW TO {}", sep(n), sep(new_room(g.room, n))),
                None => format!("{} PLACES, ALL SEATED, ROOFED", sep(most)),
            };
            // (on a dark band: the town under it made the gold hard to read)
            let (ny0, ny1) = (pic.1 + m * 3 + ph, pic.1 + m * 3 + ph * 2);
            let nx0 = pic.2 - m * 3 - text_width(f1, note.as_bytes());
            blend(nx0, ny0, pic.2 - m - nx0, ny1 - ny0, |_, _| (rgb565([0x0c, 0x0f, 0x14]), 200));
            text_at(pic.2 - m * 2, ny0, ny1, f1, GOLD, note.as_bytes(), 2);
        }
    }
    // FULL SCREEN / SHOW CARDS at the picture's top left
    {
        let ph = (sh / 30).max(18);
        let label: &[u8] = if WIDE { b"SHOW CARDS" } else { b"FULL SCREEN" };
        let r = (pic.0 + m * 2, pic.1 + m * 2, pic.0 + m * 2 + text_width(f1, label) + m * 3, pic.1 + m * 2 + ph);
        gradient(r, [0x14, 0x19, 0x21], [0x10, 0x14, 0x1a], EDGE, ph / 2 - 1);
        text_at((r.0 + r.2) / 2, r.1, r.3, f1, LIGHT, label, 1);
        WIDE_PILL = Some(r);
    }
    if WIDE {
        // (no cards: none of their buttons may take a click)
        (BUTTONS, STEPS, CARD_PILLS) = (Vec::new(), Vec::new(), Vec::new());
        return;
    }
    // the cards, right
    let (rx1, rx2) = (pic.2 + m * 2, sw - m * 2);
    let (y0, y_end) = (bar + m * 2, sh - m * 2);
    let l = (y_end - y0) / 25;
    let pad = m * 2;
    // THE GROUND: six figures
    let a = (rx1, y0, rx2, y0 + l * 9);
    card(a);
    icon(9, a.0 + pad + l / 2, a.1 + l, l * 8 / 10, [0x9c, 0xb8, 0xe0]);
    text_at(a.0 + pad + l + m, a.1 + l / 3, a.1 + l * 5 / 3, head, WHITE, b"THE GROUND", 0);
    text_at(a.2 - pad, a.1 + l / 3, a.1 + l * 5 / 3, f1, DIM, &to_latin1(&g.name.to_uppercase()), 2);
    let full = if g.capacity > 0 { g.average * 100 / g.capacity } else { 0 };
    let pct = |v: i32| if g.capacity > 0 { format!("{:.1}% OF TOTAL", v as f32 * 100.0 / g.capacity as f32) } else { String::new() };
    let cells = [
        ("CAPACITY", sep(g.capacity), "TOTAL CAPACITY".to_string(), WHITE),
        ("SEATED CAPACITY", sep(g.seats), pct(g.seats), WHITE),
        ("STANDING CAPACITY", sep(g.capacity - g.seats), pct(g.capacity - g.seats), WHITE),
        ("ROOM TO GROW", sep(g.room.min(MOST)), "MOST THE GROUND CAN HOLD".to_string(), WHITE),
        ("AVERAGE CROWD", sep(g.average), format!("{full}% FULL ON AVERAGE"), WHITE),
        ("BANK BALANCE", g.money(g.balance), if g.balance >= 0 { "IN THE BANK".to_string() } else { "OVERDRAWN".to_string() }, if g.balance >= 0 { GREEN } else { RED }),
    ];
    let (gy, rh) = (a.1 + l * 2, (a.3 - a.1 - l * 2 - m) / 3);
    let cw = (a.2 - a.0 - pad * 2) / 2;
    for (k, (label, value, sub, colour)) in cells.iter().enumerate() {
        let (x, y) = (a.0 + pad + (k as i32 % 2) * cw, gy + (k as i32 / 2) * rh);
        if k % 2 == 0 {
            fill(a.0 + pad, y, a.2 - pad, y + 1, EDGE);
        }
        text_at(x, y + rh * 6 / 100, y + rh * 30 / 100, f1, DIM, label.as_bytes(), 0);
        // (the mock's figures are big: the title font from 1050 lines up)
        let value = to_latin1(value);
        let sizes: Vec<u32> = if sh >= 1000 { vec![f6, f5, f3] } else if small { vec![f3, f2] } else { vec![f5, f3] };
        let big = fit(&sizes, &value, cw - m);
        text_at(x, y + rh * 24 / 100, y + rh * 76 / 100, big, *colour, &value, 0);
        // the bank's trend since the last month's end, an arrow after the figure (the mock's ↑)
        if k == 5 {
            if let Some(last) = g.last.filter(|&v| v != g.balance) {
                let up = g.balance > last;
                let (ax, ay, s) = (x + text_width(big, &value) + m, y + rh / 2, (rh * 34 / 100).max(8));
                let ink = if up { GREEN } else { RED };
                blend(ax, ay - s / 2, s, s, |c, r| {
                    let (u, v) = ((c as f32 + 0.5) / s as f32, (r as f32 + 0.5) / s as f32);
                    let v = if up { v } else { 1.0 - v };
                    // a head (the top 45%) and a shaft
                    let head = v < 0.45 && (u - 0.5).abs() < v / 0.45 * 0.5;
                    let shaft = v >= 0.45 && (u - 0.5).abs() < 0.13;
                    (rgb565(ink), if head || shaft { 255 } else { 0 })
                });
            }
        }
        text_at(x, y + rh * 70 / 100, y + rh * 94 / 100, f1, DIM, &to_latin1(sub), 0);
    }
    // ORDER WORKS
    let legend = (rx1, y_end - l * 2, rx2, y_end);
    let o = (rx1, a.3 + m * 2, rx2, legend.1 - m * 2);
    card(o);
    icon(8, o.0 + pad + l / 2, o.1 + l, l * 8 / 10, [0x6c, 0xa8, 0xf0]);
    text_at(o.0 + pad + l + m, o.1 + l / 3, o.1 + l * 5 / 3, head, WHITE, if SHOP_VIEW { b"CLUB SHOP".as_slice() } else { b"ORDER WORKS".as_slice() }, 0);
    // WORKS | CLUB SHOP, at the card's top right
    CARD_PILLS.clear();
    {
        let ph = l * 9 / 10;
        let mut px = o.2 - pad;
        for (label, shop_view) in [(b"CLUB SHOP".as_slice(), true), (b"WORKS".as_slice(), false)] {
            let w = text_width(f1, label) + m * 3;
            let r = (px - w, o.1 + l / 2, px, o.1 + l / 2 + ph);
            let on = SHOP_VIEW == shop_view;
            gradient(r, if on { [0x2e, 0x58, 0x98] } else { [0x14, 0x19, 0x21] }, if on { [0x1a, 0x36, 0x66] } else { [0x10, 0x14, 0x1a] }, if on { [0x44, 0x72, 0xb8] } else { EDGE }, ph / 2 - 1);
            text_at((r.0 + r.2) / 2, r.1, r.3, f1, if on { WHITE } else { DIM }, label, 1);
            CARD_PILLS.push((r, shop_view));
            px = r.0 - m / 2;
        }
    }
    text_at(o.0 + pad, o.1 + l * 5 / 3, o.1 + l * 5 / 2, f1, DIM, if SHOP_VIEW { b"MERCHANDISE: SELL MORE TO YOUR FANS".as_slice() } else { b"INVEST IN STADIUM IMPROVEMENTS".as_slice() }, 0);
    BUTTONS.clear();
    STEPS.clear();
    // (less room left than chosen: the order is what is left, at its own price, user 2026-11-23:
    // Heracles at 14,500 of 15,000 could not order the last 500)
    let grow_left = (g.room.min(MOST) - g.capacity - g.pending(&[SEATS, STANDING])).max(0);
    let convert_left = (g.capacity - g.seats - g.pending(&[CONVERT])).max(0);
    let fit_room = |chosen: i32, left: i32| if left > 0 && left < chosen { left } else { chosen };
    let amounts = [fit_room(AMOUNTS[0], grow_left), fit_room(AMOUNTS[1], grow_left), fit_room(AMOUNTS[2], convert_left)];
    let gap = m * 3 / 2;
    let tw = (o.2 - o.0 - pad * 2 - gap * 2) / 3;
    let (ty, th) = (o.1 + l * 3, l * 4);
    let blue = ([0x2e, 0x58, 0x98], [0x1a, 0x36, 0x66], [0x44, 0x72, 0xb8]);
    let gold = ([0x8a, 0x66, 0x14], [0x52, 0x3a, 0x0a], [0xe0, 0xb8, 0x50]);
    let off = ([0x22, 0x27, 0x2f], [0x1a, 0x1e, 0x25], [0x30, 0x36, 0x40]);
    let lit = ([0x3a, 0x6c, 0xb4], [0x22, 0x44, 0x7e], [0x6a, 0x98, 0xe0]);
    if SHOP_VIEW {
        shop_view(&g, o, (ty, th, tw, gap, pad, l, m), (f1, f2, f3), [blue, gold, off, lit]);
    } else {
    let sizes = sizes_above(g.capacity.max(new_size.unwrap_or(0) - 1));
    let show_new = (grow_left == 0 && g.pending(&[SEATS, STANDING]) == 0 && !sizes.is_empty()) || new_size.is_some();
    if show_new {
        NEW_PICK = NEW_PICK.min(sizes.len().saturating_sub(1));
        let size = new_size.or_else(|| sizes.get(NEW_PICK).copied()).unwrap_or(0);
        let (cost, days) = price(NEW, size);
        let refused = g.refusal(NEW, size);
        let armed = ARMED == Some(NEW);
        if refused.is_some() {
            NAMING = false;
        }
        let r = (o.0 + pad, ty, o.0 + pad + tw * 2 + gap, ty + th);
        let (top, bottom, edge) = if armed { gold } else if refused.is_some() { off } else if HOVER == Some(r) { lit } else { blue };
        gradient(r, top, bottom, edge, 6);
        let ink = if refused.is_some() { [0x6a, 0x72, 0x7c] } else { WHITE };
        let cx = (r.0 + r.2) / 2;
        let room = r.2 - r.0 - 16;
        icon(9, cx, ty + th * 19 / 100, th * 28 / 100, ink);
        let (line1, line2) = if NAMING {
            let typed = latin1(&TYPED).into_owned();
            ("NAME YOUR STADIUM".to_string(), if typed.is_empty() { "type a name (empty: keep the old one)_".to_string() } else { format!("{typed}_") })
        } else if new_size.is_some() {
            ("NEW STADIUM".to_string(), format!("{} SEATS, BEING BUILT", sep(size)))
        } else {
            ("NEW STADIUM".to_string(), format!("{} SEATS, ALL COVERED", sep(size)))
        };
        let (l1, l2) = (to_latin1(&line1), to_latin1(&line2));
        // (800x600: f3 ran into the line under it)
        text_at(cx, ty + th * 36 / 100, ty + th * 52 / 100, if small { fit(&[f2, f1], &l1, room) } else { fit(&[f3, f2, f1], &l1, room) }, ink, &l1, 1);
        text_at(cx, ty + th * 51 / 100, ty + th * 66 / 100, fit(&[f2, f1], &l2, room), if NAMING { GOLD } else { ink }, &l2, 1);
        // (the cost alone on its line, the deposit under it: one line ran past the tile, 2026-11-23)
        let cost_line = to_latin1(&format!("COST: {}", g.money(cost)));
        text_at(cx, ty + th * 68 / 100, ty + th * 83 / 100, fit(&[f2, f1], &cost_line, room), ink, &cost_line, 1);
        let (under, under_ink) = match (&refused, armed, NAMING) {
            (_, _, true) => ("CLICK AGAIN OR PRESS ENTER TO ORDER (ESC: CANCEL)".to_string(), GOLD),
            (Some(why), _, _) => (short(why), [0x8a, 0x92, 0x9c]),
            _ => (format!("{} NOW, THE REST MONTHLY  -  {} WEEKS", g.money(deposit(cost)), (days + 6) / 7), [0xb8, 0xc8, 0xe0]),
        };
        let under = to_latin1(&under);
        // (the smallest font, and the words up to what fits)
        let mut under = under;
        while text_width(f1, &under) > room && under.contains(&b' ') {
            let cut = under.iter().rposition(|&c| c == b' ').unwrap();
            under.truncate(cut);
        }
        text_at(cx, ty + th * 83 / 100, ty + th * 97 / 100, f1, under_ink, &under, 1);
        if refused.is_none() {
            BUTTONS.push((r, NEW, size));
        }
        // - and + under it: the next size down or up
        if new_size.is_none() {
            let (sy, half) = (ty + th + m / 2, (r.2 - r.0 - m) / 2);
            for (j, (sign, d)) in [("-", -1), ("+", 1)].into_iter().enumerate() {
                let s = (r.0 + j as i32 * (half + m), sy, r.0 + j as i32 * (half + m) + half, sy + l);
                let can = (0..sizes.len() as i32).contains(&(NEW_PICK as i32 + d));
                let on = can && HOVER == Some(s);
                gradient(s, if on { [0x3a, 0x44, 0x52] } else if can { [0x2a, 0x31, 0x3c] } else { [0x18, 0x1c, 0x22] }, if on { [0x30, 0x38, 0x44] } else if can { [0x22, 0x28, 0x31] } else { [0x18, 0x1c, 0x22] }, EDGE, 4);
                text_at(s.0 + half / 2, s.1, s.3, f3, if can { WHITE } else { [0x4a, 0x50, 0x58] }, sign.as_bytes(), 1);
                if can {
                    STEPS.push((s, NEW, d));
                }
            }
        }
    }
    for (k, kind) in [SEATS, STANDING, CONVERT].into_iter().enumerate() {
        if show_new && kind != CONVERT {
            continue;
        }
        let amount = amounts[k];
        let (cost, days) = (g.cost(kind, amount), price(kind, amount).1);
        let refused = g.refusal(kind, amount);
        let armed = ARMED == Some(kind);
        let x = o.0 + pad + k as i32 * (tw + gap);
        let r = (x, ty, x + tw, ty + th);
        let (top, bottom, edge) = if armed { gold } else if refused.is_some() { off } else if HOVER == Some(r) { lit } else { blue };
        gradient(r, top, bottom, edge, 6);
        let ink = if refused.is_some() { [0x6a, 0x72, 0x7c] } else { WHITE };
        let cx = x + tw / 2;
        icon(kind, cx, ty + th * 19 / 100, th * 28 / 100, ink);
        let (line1, line2) = match kind {
            SEATS => (format!("ADD {}", sep(amount)), "SEATS"),
            STANDING => (format!("ADD {}", sep(amount)), "STANDING PLACES"),
            _ => (format!("CONVERT {}", sep(amount)), "STANDING TO SEATS"),
        };
        let room = tw - 10;
        text_at(cx, ty + th * 36 / 100, ty + th * 52 / 100, fit(&[f3, f2, f1], line1.as_bytes(), room), ink, line1.as_bytes(), 1);
        text_at(cx, ty + th * 51 / 100, ty + th * 66 / 100, fit(&[f2, f1], line2.as_bytes(), room), ink, line2.as_bytes(), 1);
        let (under, under_ink) = match (&refused, armed) {
            (_, true) => ("CLICK AGAIN TO ORDER".to_string(), GOLD),
            (Some(why), _) => (short(why), [0x8a, 0x92, 0x9c]),
            _ => (format!("{} WEEKS", (days + 6) / 7), [0xb8, 0xc8, 0xe0]),
        };
        let cost_line = to_latin1(&format!("COST: {}", g.money(cost)));
        text_at(cx, ty + th * 68 / 100, ty + th * 83 / 100, fit(&[f2, f1], &cost_line, room), ink, &cost_line, 1);
        // (too long for the tile at 800x600: the reason's first words)
        let under = to_latin1(&under);
        let under = if text_width(f1, &under) > room { under.split(|&c| c == b' ').take(3).collect::<Vec<_>>().join(&b' ') } else { under };
        text_at(cx, ty + th * 83 / 100, ty + th * 97 / 100, f1, under_ink, &under, 1);
        if refused.is_none() {
            BUTTONS.push((r, kind, amount));
        }
        // - and + under the tile (1,000 to 10,000 places)
        let sy = ty + th + m / 2;
        let half = (tw - m) / 2;
        let left_now = if kind == CONVERT { convert_left } else { grow_left };
        let chosen = AMOUNTS[k];
        for (j, (sign, d)) in [("-", -1000), ("+", 1000)].into_iter().enumerate() {
            let s = (x + j as i32 * (half + m), sy, x + j as i32 * (half + m) + half, sy + l);
            let can = (1000..=10_000).contains(&(chosen + d)) && (d < 0 || chosen + d <= left_now);
            let on = can && HOVER == Some(s);
            gradient(s, if on { [0x3a, 0x44, 0x52] } else if can { [0x2a, 0x31, 0x3c] } else { [0x18, 0x1c, 0x22] }, if on { [0x30, 0x38, 0x44] } else if can { [0x22, 0x28, 0x31] } else { [0x18, 0x1c, 0x22] }, EDGE, 4);
            text_at(s.0 + half / 2, s.1, s.3, f3, if can { WHITE } else { [0x4a, 0x50, 0x58] }, sign.as_bytes(), 1);
            if can {
                STEPS.push((s, kind, d));
            }
        }
    }
    // the roof and the heating, a row of two
    let ry = ty + th + m / 2 + l + m * 2;
    let rw = (o.2 - o.0 - pad * 2 - gap) / 2;
    let names = if small { [(ROOF, "A ROOF"), (HEATING, "HEATING")] } else { [(ROOF, "A ROOF ON EVERY STAND"), (HEATING, "UNDER-SOIL HEATING")] };
    for (k, (kind, title)) in names.into_iter().enumerate() {
        let (cost, days) = price(kind, 0);
        let refused = g.refusal(kind, 0);
        let armed = ARMED == Some(kind);
        let x = o.0 + pad + k as i32 * (rw + gap);
        let r = (x, ry, x + rw, ry + l * 2);
        let (top, bottom, edge) = if armed { gold } else if refused.is_some() { off } else if HOVER == Some(r) { lit } else { blue };
        gradient(r, top, bottom, edge, 6);
        let ink = if refused.is_some() { [0x6a, 0x72, 0x7c] } else { WHITE };
        icon(kind, x + l, ry + l, l * 9 / 10, ink);
        text_at(x + l * 2, ry + l / 6, ry + l, fit(&[f2, f1], title.as_bytes(), rw - l * 2 - 8), ink, title.as_bytes(), 0);
        let under = match (&refused, armed) {
            (_, true) => "CLICK AGAIN TO ORDER".to_string(),
            (Some(why), _) => short(why),
            _ => format!("COST: {}  -  {} WEEKS", g.money(cost), (days + 6) / 7),
        };
        text_at(x + l * 2, ry + l, ry + l * 2 - l / 6, f1, if armed { GOLD } else { [0xb0, 0xbc, 0xcc] }, &to_latin1(&under), 0);
        if refused.is_none() {
            BUTTONS.push((r, kind, 0));
        }
    }
    // a word on how it works (or the answer to the last order), then what is being built
    let y = ry + l * 2 + m * 2;
    let circle = l * 7 / 10;
    // (a smooth disc with an i, as the mock's)
    let (ix, iy) = (o.0 + pad + circle / 2, y + l / 2);
    blend(ix - circle / 2, iy - circle / 2, circle, circle, |c, r| {
        let d = ((c as f32 + 0.5 - circle as f32 / 2.0).hypot(r as f32 + 0.5 - circle as f32 / 2.0)) - circle as f32 / 2.0;
        (rgb565([0x7a, 0x82, 0x8c]), ((0.5 - d).clamp(0.0, 1.0) * 255.0) as u32)
    });
    text_at(ix, y, y + l, f1, CARD, b"i", 1);
    // (a full ground: how it goes on from here, the user 2026-11-24; `full_note`)
    let full = full_note(&g, small);
    let note = match (g.said.is_empty(), small) {
        (false, _) => g.said.clone(),
        (true, _) if full.is_some() => full.unwrap_or_default(),
        (true, false) => "Paid at once from the bank; the works are finished in the weeks shown.".to_string(),
        (true, true) => "Paid at once; ready in the weeks shown.".to_string(),
    };
    let note = to_latin1(&note);
    text_at(o.0 + pad + circle + m, y, y + l, fit(&[f2, f1], &note, o.2 - pad - (o.0 + pad + circle + m)), if g.said.is_empty() { [0xb0, 0xb8, 0xc4] } else { GOLD }, &note, 0);
    }
    // what is being built: a panel over the picture's bottom left, a bar for each work (the card
    // had no room left for it, night 11)
    if !g.works.is_empty() && !FULL_VIEW {
        // (at the bottom, in the faded land: the crane and the stands stay in view)
        let pw = ((pic.2 - pic.0) * 55 / 100).max(200);
        let shown = g.works.len().min(4) as i32;
        let row = l * 13 / 10;
        // (bottom right: the club shop stands at the bottom left, 2026-11-23)
        let (px, py) = (pic.2 - m * 2 - pw, pic.3 - m * 2 - (l + row * shown + m));
        let panel = (px, py, px + pw, py + l + row * shown + m);
        blend(panel.0, panel.1, panel.2 - panel.0, panel.3 - panel.1, |c, r| {
            let edge = c == 0 || r == 0 || c == panel.2 - panel.0 - 1 || r == panel.3 - panel.1 - 1;
            (rgb565(if edge { WORKS } else { [0x0c, 0x0f, 0x14] }), if edge { 200 } else { 215 })
        });
        text_at(px + m, py + 2, py + l, f1, WORKS, b"UNDER CONSTRUCTION", 0);
        let mut wy = py + l;
        for w in g.works.iter().take(4) {
            let what = match w.0 {
                SEATS => format!("{} seats", sep(w.1)),
                STANDING => format!("{} standing places", sep(w.1)),
                CONVERT => format!("{} places to seats", sep(w.1)),
                ROOF => "The roof".into(),
                NEW => format!("New stadium, {} seats", sep(w.1)),
                SHOP => shop::LEVELS[(w.1 as usize).min(3)].0.to_string(),
                _ => "Under-soil heating".into(),
            };
            let left_days = if w.2 == 1 { "1 day".to_string() } else { format!("{} days", w.2) };
            text_at(px + m, wy, wy + row * 6 / 10, fit(&[f2, f1], &to_latin1(&what), pw - m * 3 - text_width(f1, left_days.as_bytes())), WHITE, &to_latin1(&what), 0);
            text_at(px + pw - m, wy, wy + row * 6 / 10, f1, LIGHT, left_days.as_bytes(), 2);
            // how far it is: the days gone of the days it takes
            let total = price(w.0, w.1).1.max(1);
            let done = ((total - w.2).clamp(0, total) as i64 * (pw - m * 2) as i64 / total as i64) as i32;
            let (by1, by2) = (wy + row * 66 / 100, wy + row * 66 / 100 + (l / 6).max(3));
            fill(px + m, by1, px + pw - m, by2, [0x2a, 0x2f, 0x38]);
            fill(px + m, by1, px + m + done.max(2), by2, WORKS);
            wy += row;
        }
    }
    // the key
    card(legend);
    let cols = mosaic(g.colours);
    let mut kx = legend.0 + pad;
    let sq = l * 7 / 10;
    let ky = (legend.1 + legend.3) / 2;
    let items: [(&[u8], [u8; 3], [u8; 3]); 4] = [
        (b"SEATED", cols[0], cols[1]),
        (b"STANDING", [0x70, 0x73, 0x78], [0x70, 0x73, 0x78]),
        (b"ROOF", [0x3a, 0x3e, 0x44], [0x3a, 0x3e, 0x44]),
        (b"WORKS", WORKS, WORKS),
    ];
    let step = (legend.2 - legend.0 - pad * 2) / 4;
    for (label, c1, c2) in items {
        // (seats: the club's two colours side by side, as in the mosaic)
        fill(kx, ky - sq / 2, kx + sq / 2, ky + sq / 2, c1);
        fill(kx + sq / 2, ky - sq / 2, kx + sq, ky + sq / 2, c2);
        text_at(kx + sq + m / 2, ky - l / 2, ky + l / 2, fit(&[f2, f1], label, step - sq - m), LIGHT, label, 0);
        kx += step;
    }
}

/// The ground drawn flat (no Data\cmui_stadium.bin): the four stands round the pitch.
#[allow(static_mut_refs)]
unsafe fn flat(dx1: i32, top: i32, dx2: i32, bottom: i32, m: i32, g: &Ground, f2: u32) {
    let g = g.clone();
    // the drawing, left: the stands as deep as the ground is big (125,000 fills the panel)
    rounded(dx1, top, dx2, bottom, AREA, 6);
    let (cx, cy) = ((dx1 + dx2) / 2, (top + bottom) / 2 - m * 2);
    let (pw, ph) = ((dx2 - dx1) * 44 / 100, (bottom - top) * 36 / 100);
    let pitch = (cx - pw / 2, cy - ph / 2, cx + pw / 2, cy + ph / 2);
    let room = ((dx2 - dx1) / 2 - pw / 2 - 16).min((bottom - top) / 2 - ph / 2 - 40).max(10);
    let scale = (g.capacity.max(0) as f32 / MOST as f32).sqrt();
    let (long, end) = (((room as f32 * scale) as i32).max(8), ((room as f32 * scale * 0.7) as i32).max(6));
    let seated = if g.capacity > 0 { g.seats as f32 / g.capacity as f32 } else { 0.0 };
    let building = g.works.iter().any(|w| w.0 == SEATS || w.0 == STANDING);
    let roofing = g.works.iter().any(|w| w.0 == ROOF);
    // the ground's plot: grass round it, a concrete apron under the stadium, the corners filled
    let foot = (pitch.0 - end - 14, pitch.1 - long - 14, pitch.2 + end + 14, pitch.3 + long + 14);
    fill(dx1 + 10, top + 10, dx2 - 10, bottom - m * 6, [0x1c, 0x2c, 0x20]);
    surroundings((dx1 + 10, top + 10, dx2 - 10, bottom - m * 6), foot, &g);
    rounded(foot.0, foot.1, foot.2, foot.3, [0x3a, 0x3e, 0x46], 10);
    for (x, y) in [(foot.0 + 4, foot.1 + 4), (pitch.2 + 4, foot.1 + 4), (foot.0 + 4, pitch.3 + 4), (pitch.2 + 4, pitch.3 + 4)] {
        fill(x, y, x + end + 6, y + long + 6, [0x2e, 0x32, 0x38]);
    }
    stand((pitch.0, pitch.1 - long - 4, pitch.2, pitch.1 - 4), seated, g.roof, building || roofing, 0, g.colours);
    stand((pitch.0, pitch.3 + 4, pitch.2, pitch.3 + long + 4), seated, g.roof, building || roofing, 1, g.colours);
    stand((pitch.0 - end - 4, pitch.1, pitch.0 - 4, pitch.3), seated, g.roof, building || roofing, 2, g.colours);
    stand((pitch.2 + 4, pitch.1, pitch.2 + end + 4, pitch.3), seated, g.roof, building || roofing, 3, g.colours);
    // floodlights at the four corners, their light a soft glow
    for (x, y) in [(foot.0 + 6, foot.1 + 6), (foot.2 - 6, foot.1 + 6), (foot.0 + 6, foot.3 - 6), (foot.2 - 6, foot.3 - 6)] {
        let r = (long / 3).clamp(8, 26);
        blend(x - r, y - r, 2 * r, 2 * r, |c, w| {
            let (dx, dy) = (c - r, w - r);
            let d = ((dx * dx + dy * dy) as f32).sqrt() / r as f32;
            (rgb([0xff, 0xf2, 0xb0]), if d < 1.0 { ((1.0 - d) * 110.0) as u32 } else { 0 })
        });
        fill(x - 3, y - 3, x + 4, y + 4, [0xe8, 0xe8, 0xe0]);
    }
    // the pitch: mown stripes and its white lines
    let stripes = 12;
    for k in 0..stripes {
        let (a, b) = (pitch.0 + k * pw / stripes, pitch.0 + (k + 1) * pw / stripes);
        fill(a, pitch.1, b, pitch.3, if k % 2 == 0 { [0x2e, 0x7d, 0x40] } else { [0x36, 0x8c, 0x48] });
    }
    let line = game::<extern "C" fn(i32, i32, i32, i32, u32, u32)>(DRAW_LINE);
    let white = rgb([0xe8, 0xf0, 0xe8]) as u32;
    let (lx1, ly1, lx2, ly2) = (pitch.0 + 6, pitch.1 + 6, pitch.2 - 6, pitch.3 - 6);
    for (a, b, c, d) in [(lx1, ly1, lx2, ly1), (lx1, ly2, lx2, ly2), (lx1, ly1, lx1, ly2), (lx2, ly1, lx2, ly2), (cx, ly1, cx, ly2)] {
        line(a, b, c, d, 2, white);
    }
    let rad = (ly2 - ly1) / 6;
    for k in 0..32 {
        let (t1, t2) = (k as f32 * std::f32::consts::TAU / 32.0, (k + 1) as f32 * std::f32::consts::TAU / 32.0);
        line(cx + (rad as f32 * t1.cos()) as i32, cy + (rad as f32 * t1.sin()) as i32, cx + (rad as f32 * t2.cos()) as i32, cy + (rad as f32 * t2.sin()) as i32, 2, white);
    }
    let (bw, bh) = ((lx2 - lx1) * 16 / 100, (ly2 - ly1) * 44 / 100);
    for (x, dir) in [(lx1, 1), (lx2, -1)] {
        let bx = x + dir * bw;
        line(x, cy - bh / 2, bx, cy - bh / 2, 2, white);
        line(x, cy + bh / 2, bx, cy + bh / 2, 2, white);
        line(bx, cy - bh / 2, bx, cy + bh / 2, 2, white);
        // the goal, just outside the line
        fill(x - dir.max(0) * 5 - (1 - dir.max(0)) * 0, cy - bh / 7, x + (1 - dir.max(0)) * 5, cy + bh / 7, [0xf0, 0xf0, 0xf0]);
    }
    // the club's badge in the centre circle
    let key = to_latin1(&g.club);
    if has_crest(&key) {
        let s = rad * 17 / 10;
        draw_crest(&key, cx - s / 2, cy - s / 2, s);
    }
    // each stand's places written on it (the long sides 35% each, the ends 15%)
    let places = |share: f32| sep((g.capacity as f32 * share / 100.0).round() as i32 * 100);
    for (x, y, s) in [(cx, pitch.1 - long / 2 - 4, places(0.35 * 100.0 / 100.0)), (cx, pitch.3 + long / 2 + 4, places(0.35)), (pitch.0 - end / 2 - 4, cy, places(0.15)), (pitch.2 + end / 2 + 4, cy, places(0.15))] {
        // (on a dark label in a larger font: unreadable on the seats, user 2026-11-23)
        let tw = text_width(f2, s.as_bytes());
        rounded(x - tw / 2 - 8, y - 12, x + tw / 2 + 8, y + 12, [0x12, 0x14, 0x18], 5);
        text_at(x, y - 12, y + 12, f2, WHITE, s.as_bytes(), 1);
    }
    if g.heating {
        for (a, b, c, d) in [(pitch.0, pitch.1, pitch.2, pitch.1 + 2), (pitch.0, pitch.3 - 2, pitch.2, pitch.3), (pitch.0, pitch.1, pitch.0 + 2, pitch.3), (pitch.2 - 2, pitch.1, pitch.2, pitch.3)] {
            fill(a, b, c, d, [0xe8, 0x8a, 0x3a]);
        }
    }
    // the key under the drawing
    let ky = bottom - m * 4;
    let mut kx = dx1 + m * 2;
    for (c, label) in [(g.colours[0], "Seats"), ([0x84, 0x86, 0x8a], "Standing"), ([0xc8, 0xcc, 0xd2], "Roof"), ([0xe8, 0x8a, 0x3a], "Heating"), (GOLD, "Works")] {
        fill(kx, ky + 4, kx + 14, ky + 18, c);
        text_at(kx + 20, ky, ky + 22, f2, LIGHT, label.as_bytes(), 0);
        kx += 20 + text_width(f2, label.as_bytes()) + 24;
    }
}

/// The mouse while the page is open: every click is its own but the menu bar's (which leaves it).
#[allow(static_mut_refs)]
pub unsafe fn input(mx: i32, my: i32, released: bool) -> Option<bool> {
    if !OPEN {
        return None;
    }
    let inside = |r: (i32, i32, i32, i32)| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    // (the name's keys: Enter orders, Escape cancels; a key typed draws the page again)
    if std::mem::take(&mut ORDER_NOW) && NAMING {
        order_new();
        return Some(true);
    }
    if std::mem::take(&mut KEYED) {
        return Some(true);
    }
    if mx < bar_end(screen().0) {
        if released {
            NAMING = false;
            OPEN = false;
            REDRAW = true;
        }
        return None;
    }
    *(LEFT_CLICK as *mut u32) = 0;
    let lit = BUTTONS.iter().chain(STEPS.iter()).find(|b| inside(b.0)).map(|b| b.0);
    if lit != HOVER {
        HOVER = lit;
        REDRAW = true;
    }
    if !released || OPENED_AT.is_some_and(|t| t.elapsed().as_millis() < 300) {
        return Some(false);
    }
    if let Some(&(_, shop_view)) = CARD_PILLS.iter().find(|v| inside(v.0)) {
        (SHOP_VIEW, ARMED, NAMING) = (shop_view, None, false);
        return Some(true);
    }
    if let Some(&(_, full)) = VIEW_PILLS.iter().find(|v| inside(v.0)) {
        FULL_VIEW = full;
        return Some(true);
    }
    if WIDE_PILL.is_some_and(inside) {
        (WIDE, ARMED, NAMING) = (!WIDE, None, false);
        return Some(true);
    }
    if BACK.is_some_and(inside) {
        (OPEN, NAMING) = (false, false);
        return Some(true);
    }
    if let Some(&(_, kind, d)) = STEPS.iter().find(|s| inside(s.0)) {
        if kind == NEW {
            NEW_PICK = (NEW_PICK as i32 + d).max(0) as usize;
            (ARMED, NAMING) = (None, false);
            return Some(true);
        }
        let k = (kind - 1) as usize;
        AMOUNTS[k] = (AMOUNTS[k] + d).clamp(1000, 10_000);
        ARMED = None;
        return Some(true);
    }
    if let Some(&(_, kind, amount)) = BUTTONS.iter().find(|b| inside(b.0)) {
        // (a new stadium: the first click asks its name, the second (or Enter) orders)
        if kind == NEW {
            if NAMING {
                order_new();
            } else {
                (ARMED, NAMING) = (Some(NEW), true);
                TYPED.clear();
                hook_keys();
            }
            return Some(true);
        }
        NAMING = false;
        // (real money: the first click asks, the second orders; an item: the same item twice)
        if ARMED == Some(kind) && (kind != ITEM || ARMED_ITEM == amount as usize) {
            ARMED = None;
            fetch(kind, amount, &[]);
        } else {
            ARMED = Some(kind);
            ARMED_ITEM = amount as usize;
        }
        return Some(true);
    }
    (ARMED, NAMING) = (None, false);
    Some(true)
}

/// A layer of the pack (the club shop's building) fitted into r, for the shop's card; faint while
/// it is being built.
#[allow(static_mut_refs)]
unsafe fn thumb(kind: &str, g: &Ground, r: (i32, i32, i32, i32), faint: bool) -> bool {
    let Some(Some(pack)) = PACK.as_ref() else { return false };
    let want = format!("{kind}_{}", size_of(g.capacity));
    let Some(pic) = pack.iter().find(|p| p.name == want) else { return false };
    let (w, h) = (r.2 - r.0, r.3 - r.1);
    if w < 4 || h < 4 || pic.w < 2 || pic.h < 2 {
        return false;
    }
    let sc = (w as f32 / pic.w as f32).min(h as f32 / pic.h as f32);
    let (ox, oy) = ((w as f32 - pic.w as f32 * sc) / 2.0, (h as f32 - pic.h as f32 * sc) / 2.0);
    let cols = mosaic(g.colours);
    blend(r.0, r.1, w, h, |c, row| {
        let (fx0, fy0) = (pic.x as f32 + (c as f32 - ox) / sc, pic.y as f32 + (row as f32 - oy) / sc);
        match sample(pic, (fx0, fx0 + 1.0 / sc, fy0, fy0 + 1.0 / sc), cols, 1.0, None) {
            Some((rgb, a)) => (rgb565(rgb.map(|v| v.clamp(0.0, 255.0) as u8)), (a * if faint { 110.0 } else { 255.0 }) as u32),
            None => (0, 0),
        }
    });
    true
}

/// The shop's item pictures (stadium3d\build_merch.py `pack`, after the user's Grok merchandise):
/// shirt, scarf, name and number, kit, lock; 96x96 pixels of (light, alpha, class: 1 the club's
/// first colour, 2 its second, 0 grey as drawn), rows top down.
const MERCH: &[u8; 5 * 96 * 96 * 3] = include_bytes!("merch.bin");

/// Item picture `k` centred in r, in the colours `cols` (or all in `tint`: the lock), faded to
/// `fade` (0..255).
unsafe fn merch_icon(k: usize, r: (i32, i32, i32, i32), cols: [[u8; 3]; 2], tint: Option<[u8; 3]>, fade: u32) {
    const N: i32 = 96;
    let size = (r.2 - r.0).min(r.3 - r.1);
    if size < 4 || k > 4 {
        return;
    }
    let (x, y) = (r.0 + (r.2 - r.0 - size) / 2, r.1 + (r.3 - r.1 - size) / 2);
    let px = &MERCH[k * (N * N * 3) as usize..][..(N * N * 3) as usize];
    blend(x, y, size, size, |col, row| {
        // (the source pixels under this one, averaged by their alpha: a 96 picture drawn at 50)
        let (x0, x1) = (col * N / size, ((col + 1) * N + size - 1) / size);
        let (y0, y1) = (row * N / size, ((row + 1) * N + size - 1) / size);
        let (mut acc, mut a, mut n) = ([0u32; 3], 0u32, 0u32);
        for sy in y0..y1.min(N) {
            for sx in x0..x1.min(N) {
                let p = &px[((sy * N + sx) * 3) as usize..][..3];
                n += 1;
                if p[1] == 0 {
                    continue;
                }
                let light = p[0] as u32;
                let c = match (tint, p[2]) {
                    (Some(t), _) => t.map(|v| (v as u32 * light / 255).min(255)),
                    (None, 1 | 2) => cols[p[2] as usize - 1].map(|v| ((v as u32).max(30) * light / 205).min(255)),
                    _ => [light; 3],
                };
                for i in 0..3 {
                    acc[i] += c[i] * p[1] as u32;
                }
                a += p[1] as u32;
            }
        }
        if a == 0 {
            return (0, 0);
        }
        (rgb565(acc.map(|v| (v / a) as u8)), a / n.max(1) * fade / 255)
    });
}

/// The order card as the club shop (the user's and Grok's notes, 2026-11-23, in the page's own
/// style): one card for the shop (its building from the pack, its level and status, the weekly
/// boost split into the shop's and the results' part, the next upgrade), then the items with a
/// small picture each: active, available or locked.
#[allow(static_mut_refs)]
unsafe fn shop_view(g: &Ground, o: (i32, i32, i32, i32), (ty, _th, _tw, gap, pad, l, m): (i32, i32, i32, i32, i32, i32, i32), (f1, f2, f3): (u32, u32, u32), [blue, gold, off, lit]: [([u8; 3], [u8; 3], [u8; 3]); 4]) {
    let building = g.works.iter().find(|w| w.0 == SHOP).copied();
    let level = g.shop.min(3);
    let base = shop::pct(g.shop, g.items);
    let results = g.merch_pct - base;
    let green = ([0x1c, 0x3a, 0x2a], [0x14, 0x2a, 0x1e], [0x3c, 0xc8, 0x6e]);
    // (800x600: no picture, no tags, shorter words: the fonts do not shrink)
    let small = screen().1 < 750;
    // the shop's card
    let ch = l * 4;
    let s = (o.0 + pad, ty, o.2 - pad, ty + ch);
    gradient(s, [0x18, 0x1e, 0x28], [0x12, 0x17, 0x1e], EDGE, 6);
    let t = if small { (s.0, s.1 + m, s.0, s.3 - m) } else { (s.0 + m, s.1 + m, s.0 + m + ch * 13 / 10, s.3 - m) };
    if !small {
        gradient(t, [0x0c, 0x10, 0x16], [0x0a, 0x0d, 0x12], [0x22, 0x28, 0x32], 5);
    }
    let shown = level.max(building.map_or(0, |w| w.1 as u8));
    let kind = match shown {
        0 => "",
        1 => "shop",
        _ => "mega",
    };
    if small {
    } else if kind.is_empty() || !thumb(kind, g, (t.0 + 3, t.1 + 3, t.2 - 3, t.3 - 3), level < shown) {
        icon(11, (t.0 + t.2) / 2, (t.1 + t.3) / 2, (t.3 - t.1) * 55 / 100, [0x4a, 0x52, 0x5e]);
    }
    let tx = t.2 + m * 2;
    let ub = (s.2 - m - (s.2 - s.0) * if small { 42 } else { 30 } / 100, s.1 + m, s.2 - m, s.3 - m);
    let text_w = ub.0 - m - tx;
    let name = to_latin1(&shop::LEVELS[level as usize].0.to_uppercase());
    text_at(tx, s.1 + ch * 5 / 100, s.1 + ch * 27 / 100, fit(&[f3, f2, f1], &name, text_w), WHITE, &name, 0);
    // the level as a pill, the status beside it
    let lv = to_latin1(&format!("LEVEL {level} OF 3"));
    let (py0, py1) = (s.1 + ch * 29 / 100, s.1 + ch * 45 / 100);
    let pill = (tx, py0, tx + text_width(f1, &lv) + m * 2, py1);
    let (pt, pb, pe) = if level > 0 { green } else { off };
    gradient(pill, pt, pb, pe, (py1 - py0) / 2 - 1);
    text_at((pill.0 + pill.2) / 2, py0, py1, f1, if level > 0 { GREEN } else { DIM }, &lv, 1);
    let status = match (building, level) {
        (Some(w), _) => format!("{} BEING BUILT: {}", shop::LEVELS[(w.1 as usize).min(3)].0.to_uppercase(), span(w.2).to_uppercase()),
        (None, 3) => "FULLY BUILT".into(),
        (None, 0) => "NOT BUILT YET".into(),
        (None, _) => "OPEN".into(),
    };
    let status = to_latin1(&status);
    if !small {
        text_at(pill.2 + m, py0, py1, fit(&[f1], &status, text_w - (pill.2 - tx) - m), if building.is_some() { WORKS } else { LIGHT }, &status, 0);
    }
    // the weekly boost: the shop's and the items' share, the results' share
    let boost = to_latin1(&format!("WEEKLY BOOST {:+}%", g.merch_pct));
    text_at(tx, s.1 + ch * 48 / 100, s.1 + ch * 66 / 100, fit(&[f3, f2, f1], &boost, text_w), if g.merch_pct >= 0 { GREEN } else { RED }, &boost, 0);
    // (base: the shop and the items; form: this week's results)
    let parts = to_latin1(&format!("BASE {base:+}%  -  FORM {results:+}%"));
    text_at(tx, s.1 + ch * 66 / 100, s.1 + ch * 80 / 100, fit(&[f1], &parts, text_w), DIM, &parts, 0);
    // (shorter where the long one does not fit: 1280x800)
    let season = to_latin1(&format!("MERCHANDISING THIS SEASON: {}", g.money(g.merch)));
    let season = if text_width(f1, &season) > text_w { to_latin1(&format!("THIS SEASON: {}", g.money(g.merch))) } else { season };
    text_at(tx, s.1 + ch * 81 / 100, s.1 + ch * 96 / 100, fit(&[f1], &season, text_w), LIGHT, &season, 0);
    // the next upgrade, at the card's right
    let next = g.shop + 1;
    let (bh, bx) = (ub.3 - ub.1, (ub.0 + ub.2) / 2);
    let room = ub.2 - ub.0 - 10;
    if next <= 3 && building.is_none() {
        let (cost, days) = price(SHOP, next as i32);
        let refused = g.refusal(SHOP, next as i32);
        let armed = ARMED == Some(SHOP);
        let (top, bottom, edge) = if armed { gold } else if refused.is_some() { off } else if HOVER == Some(ub) { lit } else { blue };
        gradient(ub, top, bottom, edge, 6);
        let ink = if refused.is_some() { [0x6a, 0x72, 0x7c] } else { WHITE };
        let head = to_latin1(if small { ["", "CLUB SHOP", "MEGASTORE", "WEBSHOP"][next as usize] } else { ["", "BUILD A CLUB SHOP", "UPGRADE TO A MEGASTORE", "ADD A WEBSHOP"][next as usize] });
        text_at(bx, ub.1 + bh * 6 / 100, ub.1 + bh * 30 / 100, fit(&[f2, f1], &head, room), ink, &head, 1);
        let gain = shop::LEVELS[next as usize].1 - shop::LEVELS[level as usize].1;
        let more = to_latin1(&if small { format!("+{gain}%") } else { format!("+{gain}% MERCHANDISE") });
        text_at(bx, ub.1 + bh * 30 / 100, ub.1 + bh * 50 / 100, fit(&[f1], &more, room), if refused.is_none() { GREEN } else { ink }, &more, 1);
        let cost_line = to_latin1(&if small { g.money(cost) } else { format!("{}  -  {}", g.money(cost), span(days).to_uppercase()) });
        text_at(bx, ub.1 + bh * 52 / 100, ub.1 + bh * 72 / 100, fit(&[f2, f1], &cost_line, room), ink, &cost_line, 1);
        let under = match (&refused, armed) {
            (_, true) => "CLICK AGAIN TO ORDER".to_string(),
            (Some(why), _) => short(why),
            _ if small => span(days).to_uppercase(),
            _ => "CLICK TO ORDER".to_string(),
        };
        let under = to_latin1(&under);
        text_at(bx, ub.1 + bh * 74 / 100, ub.1 + bh * 94 / 100, fit(&[f1], &under, room), if armed { GOLD } else { [0xb8, 0xc8, 0xe0] }, &under, 1);
        if refused.is_none() {
            BUTTONS.push((ub, SHOP, next as i32));
        }
    } else {
        gradient(ub, [0x14, 0x19, 0x21], [0x10, 0x14, 0x1a], EDGE, 6);
        icon(14, bx, ub.1 + bh * 38 / 100, bh * 40 / 100, if building.is_some() { WORKS } else { GREEN });
        let words: &[u8] = if building.is_some() { b"BEING BUILT" } else { b"ALL BUILT" };
        text_at(bx, ub.1 + bh * 66 / 100, ub.1 + bh * 90 / 100, f1, DIM, words, 1);
    }
    // the items, two by two
    let hy = s.3 + m;
    text_at(o.0 + pad, hy, hy + l * 8 / 10, f1, DIM, b"MERCHANDISE", 0);
    let gy = hy + l * 8 / 10;
    let note_y = o.3 - m - l;
    let rh = ((note_y - m - gy - m) / 2).max(l);
    let iw = (o.2 - o.0 - pad * 2 - gap) / 2;
    for (k, (name, more, cost)) in shop::ITEMS.iter().enumerate() {
        let x = o.0 + pad + (k % 2) as i32 * (iw + gap);
        let y = gy + (k / 2) as i32 * (rh + m);
        let r = (x, y, x + iw, y + rh);
        let owned = g.items & 1 << k != 0;
        let refused = g.refusal(ITEM, k as i32);
        let locked = refused.as_deref() == Some("Needs a club shop");
        let armed = ARMED == Some(ITEM) && ARMED_ITEM == k;
        let (top, bottom, edge) = if owned { green } else if armed { gold } else if refused.is_some() { off } else if HOVER == Some(r) { lit } else { blue };
        gradient(r, top, bottom, edge, 6);
        let ib = (r.0 + m / 2, r.1 + m / 2, r.0 + m / 2 + rh - m, r.3 - m / 2);
        gradient(ib, [0x0c, 0x10, 0x16], [0x0a, 0x0d, 0x12], [0x22, 0x28, 0x32], 5);
        let ink = if locked { [0x6a, 0x72, 0x7c] } else { WHITE };
        // the item in the club's colours (the away shirt the other way round); a locked one faint
        // under a lock
        let cols = mosaic(g.colours);
        let inner = (ib.0 + 2, ib.1 + 2, ib.2 - 2, ib.3 - 2);
        // (the main colour is the game's second, the text colour its first: home items blue for
        // Chelsea, the away shirt the other way round)
        merch_icon(k.min(3), inner, if k == 0 { cols } else { [cols[1], cols[0]] }, None, if locked { 70 } else { 255 });
        if locked {
            let (cx, cy, s) = ((ib.0 + ib.2) / 2, (ib.1 + ib.3) / 2, (ib.3 - ib.1) * 55 / 100);
            merch_icon(4, (cx - s / 2, cy - s / 2, cx + s / 2, cy + s / 2), cols, Some([0xc8, 0xcc, 0xd4]), 255);
        }
        let tx = ib.2 + m;
        // the state, a tag at the top right
        let (tag, tag_bg, tag_ink): (&[u8], [u8; 3], [u8; 3]) = if owned {
            (b"ON SALE", [0x1f, 0x7a, 0x45], WHITE)
        } else if locked {
            (b"LOCKED", [0x30, 0x36, 0x40], [0x9a, 0xa0, 0xa8])
        } else {
            (b"AVAILABLE", [0x2a, 0x48, 0x78], WHITE)
        };
        let tg = if small { (r.2, r.1, r.2, r.1) } else { (r.2 - m / 2 - text_width(f1, tag) - m * 2, r.1 + m / 2, r.2 - m / 2, r.1 + m / 2 + l * 7 / 10) };
        if !small {
            gradient(tg, tag_bg, tag_bg, tag_bg, 3);
            text_at((tg.0 + tg.2) / 2, tg.1, tg.3, f1, tag_ink, tag, 1);
        }
        // (a short tile, 1280x800 and down: the name with its share, then the state; no middle line)
        let compact = rh < l * 2;
        let n = to_latin1(&name.to_uppercase());
        let name_box = if compact { (r.1 + rh * 4 / 100, r.1 + rh * 50 / 100) } else { (r.1 + rh * 6 / 100, r.1 + rh * 38 / 100) };
        text_at(tx, name_box.0, name_box.1, fit(&[f2, f1], &n, tg.0 - m - tx), ink, &n, 0);
        if !compact {
            let p = to_latin1(&format!("+{more}% MERCHANDISE"));
            text_at(tx, r.1 + rh * 38 / 100, r.1 + rh * 64 / 100, fit(&[f1], &p, r.2 - m - tx), if owned { GREEN } else { [0xb0, 0xbc, 0xcc] }, &p, 0);
        }
        let under = if owned && small {
            "ON SALE".to_string()
        } else if locked && small {
            "NEEDS A SHOP".to_string()
        } else if owned {
            "ACTIVE: SELLING EVERY WEEK".to_string()
        } else if armed {
            "CLICK AGAIN TO UNLOCK".to_string()
        } else if locked {
            format!("NEEDS A CLUB SHOP  -  {}", g.money(*cost))
        } else if let Some(why) = &refused {
            short(why)
        } else if small {
            g.money(*cost)
        } else {
            format!("UNLOCK FOR {}", g.money(*cost))
        };
        let under = to_latin1(&under);
        let under = if compact { [format!("+{more}%  ").as_bytes(), &under].concat() } else { under };
        let under_box = if compact { (r.1 + rh * 50 / 100, r.1 + rh * 96 / 100) } else { (r.1 + rh * 64 / 100, r.1 + rh * 94 / 100) };
        text_at(tx, under_box.0, under_box.1, fit(&[f1], &under, r.2 - m - tx), if armed { GOLD } else if owned { GREEN } else if refused.is_some() { [0x8a, 0x92, 0x9c] } else { WHITE }, &under, 0);
        if refused.is_none() && !owned {
            BUTTONS.push((r, ITEM, k as i32));
        }
    }
    // how it works (or the answer to the last order)
    let note = to_latin1(match (g.said.is_empty(), small) {
        (false, _) => g.said.as_str(),
        (true, false) => "Fans buy more when you win: +3% a win, +1% a draw, -2% a defeat that week.",
        (true, true) => "Fans buy more when you win.",
    });
    text_at(o.0 + pad, note_y, note_y + l, fit(&[f2, f1], &note, o.2 - o.0 - pad * 2), if g.said.is_empty() { [0xb0, 0xb8, 0xc4] } else { GOLD }, &note, 0);
}

/// Which item is armed (the two clicks of an order: kind ITEM and its index).
static mut ARMED_ITEM: usize = 0;

/// Orders the new stadium of the size picked, with the name typed.
#[allow(static_mut_refs)]
unsafe fn order_new() {
    let Some(g) = GROUND.clone() else { return };
    let sizes = sizes_above(g.capacity);
    let Some(&size) = sizes.get(NEW_PICK.min(sizes.len().saturating_sub(1))) else { return };
    let name = std::mem::take(&mut TYPED);
    (ARMED, NAMING) = (None, false);
    fetch(NEW, size, &name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mowing_patterns_lie_on_the_pitch() {
        // a frame where 9.15 m is 20 px along (x) and 10 px across, tilted: pitch metres come back
        let mut pts = [(0, 0); 8];
        pts[0] = (500, 400);
        pts[6] = (520, 404);
        pts[7] = (497, 410);
        let m = Mow::new(&pts, "Ajax").unwrap();
        let at = |u: f32, v: f32| (500.0 + u * 20.0 / 9.15 - v * 3.0 / 9.15, 400.0 + u * 4.0 / 9.15 + v * 10.0 / 9.15);
        let (x, y) = at(30.0, -12.0);
        let (u, v) = m.uv(x, y);
        assert!((u - 30.0).abs() < 0.01 && (v + 12.0).abs() < 0.01, "{u} {v}");
        // every club gets one of the six, each with light and dark grass
        assert!(m.kind < 6);
        for kind in 0..6 {
            let m = Mow { kind, ..Mow::new(&pts, "Ajax").unwrap() };
            let shades: Vec<f32> = (0..40).map(|k| { let (x, y) = at(-50.0 + k as f32 * 2.6, -30.0 + k as f32 * 1.5); m.shade(x, y) }).collect();
            assert!(shades.iter().any(|&s| s > 1.0) && shades.iter().any(|&s| s < 1.0), "kind {kind}");
        }
        assert!(Mow::new(&[(0, 0); 8], "Ajax").is_none());
    }

    #[test]
    fn the_games_prices_and_the_rules() {
        // the board's own sums for 1,000 places
        assert_eq!(price(SEATS, 1000).1, 63);
        assert_eq!(price(STANDING, 1000), ((1000 + 2000) * 50, 42));
        assert_eq!(price(CONVERT, 1000), (1000 * 75 + 2 * 50_000, 31));
        // the more places, the longer: 5,000 seats take 21 weeks
        assert_eq!(price(SEATS, 5000).1, 147);
        assert_eq!((price(ROOF, 0).0, price(HEATING, 0).0), (7_500_000, 1_500_000));
        let g = Ground { capacity: 20_000, seats: 15_000, room: 21_000, balance: 2_000_000, symbol: "E".into(), rate: 1.0, ..Default::default() };
        // seats by the ground they make: 1,000 onto 20,000 at (60 + 21,000 / 16) / 2 a seat
        assert_eq!(g.cost(SEATS, 1000), 686_000);
        assert_eq!(Ground { capacity: 3_000, ..g.clone() }.cost(SEATS, 1000), 1000 * (60 + 250) / 2);
        assert_eq!(g.refusal(SEATS, 1000), None);
        let mut busy = g.clone();
        busy.works.push((STANDING, 1000, 10));
        assert_eq!(busy.refusal(SEATS, 1000).as_deref(), Some("No room left to build"));
        assert_eq!(g.refusal(ROOF, 0).as_deref(), Some("Not enough money in the bank"));
        assert_eq!(Ground { seats: 20_000, ..g.clone() }.refusal(CONVERT, 1000).as_deref(), Some("Not enough standing places"));
        assert_eq!(decode(&encode(&busy)).unwrap(), busy);
        assert_eq!((sep(1625000), sep(-500), sep(12)), ("1,625,000".into(), "-500".into(), "12".into()));
        // the rendered ground: all seated, no grey; 15% standing = the ends' middle half; 100% all
        assert!((0..4).all(|s| (0..16).all(|k| !standing(s, k, 1.0))));
        assert!(standing(2, 7, 0.85) && standing(3, 8, 0.85) && !standing(2, 1, 0.85) && !standing(0, 7, 0.85));
        assert!((0..4).all(|s| (0..16).all(|k| standing(s, k, 0.0))));
        assert_eq!((size_of(9_000), size_of(15_000), size_of(45_000), size_of(80_000)), ("s", "m", "l", "x"));
        // alike colours: the mosaic's second one darker (white) or lighter (black)
        assert_eq!(mosaic([[255, 255, 255], [250, 250, 250]])[1], [140, 140, 140]);
        assert_eq!(mosaic([[0, 0, 0], [255, 255, 255]]), [[0, 0, 0], [255, 255, 255]]);
        // the pack's runs: two of one pixel, one of another; a short pack is no picture
        assert_eq!(unruns(&[2, 1, 2, 3, 4, 1, 9, 9, 9, 9], 12), Some(vec![1, 2, 3, 4, 1, 2, 3, 4, 9, 9, 9, 9]));
        assert_eq!(unruns(&[2, 1, 2, 3, 4], 12), None);
        // the new stadium: only once the ground is full, sizes above it, 30% now, the rest monthly
        assert_eq!(sizes_above(1_000)[..3], [3_000, 5_000, 8_000]);
        assert_eq!(sizes_above(7_500)[0], 12_000);
        assert_eq!(sizes_above(14_000), vec![20_000, 25_000, 30_000, 40_000, 50_000, 60_000, 75_000, 90_000, 110_000, 125_000]);
        // the price a place rises with the size: 3,000 for 0.74 million, 30,000 for 58, 125,000
        // for nearly a billion
        assert_eq!(price(NEW, 3_000).0, 741_000);
        assert_eq!(price(NEW, 30_000), (58_050_000, 665));
        assert_eq!(price(NEW, 125_000).0, 984_000_000);
        // its room to grow by the old ground's: a level 7 ground of 1,000 to 30,000, then 70,000,
        // then the top; never under a quarter more than its places
        assert_eq!((new_room(1_000, 15_000), new_room(20_000, 25_000), new_room(60_000, 90_000)), (30_000, 70_000, 150_000));
        assert_eq!(new_room(1_000, 40_000), 50_000);
        assert_eq!(new_room(100_000, 125_000), 150_000);
        assert_eq!((deposit(58_050_000), instalment_months(665)), (17_415_000, 21));
        let full = Ground { capacity: 7_500, seats: 5_000, room: 7_500, balance: 40_000_000, symbol: "E".into(), rate: 1.0, ..Default::default() };
        assert_eq!(full.refusal(NEW, 30_000), None);
        // the full ground's word: the next stadium and its room; none while there is room
        assert_eq!(full_note(&full, false).as_deref(), Some("Full. A new stadium: from 12,000 seats, room to grow to 30,000. Its next one: room to 70,000, then 150,000."));
        assert_eq!(full_note(&Ground { room: 9_000, ..full.clone() }, false), None);
        assert_eq!(full.refusal(NEW, 60_000).as_deref(), Some("Not enough money in the bank"));
        assert_eq!(Ground { room: 9_000, ..full.clone() }.refusal(NEW, 30_000).as_deref(), Some("Room left to grow first"));
        let building = Ground { works: vec![(NEW, 30_000, 600)], ..full.clone() };
        assert_eq!(building.refusal(ROOF, 0).as_deref(), Some("New stadium being built"));
        assert_eq!(building.refusal(NEW, 30_000).as_deref(), Some("Being built"));
        // the club shop: one level at a time, not while one is built, goes on during a new stadium;
        // an item once
        assert_eq!(full.refusal(SHOP, 1), None);
        assert_eq!(full.refusal(SHOP, 2).as_deref(), Some("Fully built"));
        assert_eq!(Ground { shop: 3, ..full.clone() }.refusal(SHOP, 4).as_deref(), Some("Fully built"));
        assert_eq!(Ground { works: vec![(SHOP, 1, 30)], ..full.clone() }.refusal(SHOP, 1).as_deref(), Some("Being built"));
        assert_eq!(building.refusal(SHOP, 1), None);
        assert_eq!(building.refusal(ITEM, 0), None);
        assert_eq!(full.refusal(ITEM, 3).as_deref(), Some("Needs a club shop"));
        assert_eq!(Ground { shop: 1, ..full.clone() }.refusal(ITEM, 3), None);
        assert_eq!(Ground { items: 0b0100, shop: 1, ..full.clone() }.refusal(ITEM, 2).as_deref(), Some("On sale"));
        assert_eq!(price(SHOP, 1), (1_500_000, 120));
        assert_eq!(price(ITEM, 2), (100_000, 0));
        let g = Ground { shop: 2, items: 0b1001, merch: 123, merch_pct: 59, next_name: "X".into(), ..full.clone() };
        assert_eq!(decode(&encode(&g)).unwrap(), g);
    }
}
