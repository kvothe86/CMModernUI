//! The staff's monthly reports (user 2026-11-24): instead of the Assistant menu's Medical, Youth
//! and Scouting report pages, three news items in each human manager's inbox at the start of
//! every month, written by the staff member himself: the best physio on injuries and fitness, the
//! coach best with youngsters and at judging potential on the young players, and the chief scout
//! (the best judge, `diary::assign`) on this month's three names. Each has a portrait as its badge,
//! one of nine photos picked by his name (so he keeps his face), dressed in the club's colours.
//!
//! The items ride on the Pete Cheesecorner machinery (`cheesecorner.rs`): the same message type
//! and hooks; our second mark tells them apart. Their figures come from the host's own report
//! builders (`finreport::medical_remote` ...), their texts are kept per game in
//! `cmui_staff_<game>.txt`.
use crate::*;

const NEWS_NEW: usize = 0x77bbe0;
const SEND: usize = 0x7861e0;
const NEWS: usize = 0xb7431c;
const MARK: u32 = 0x5341_414b; // (Pete Cheesecorner's first mark)
const MARK2: u32 = 0x4646_4154; // "TAFF"
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The faces: 9 x 80 x 80 pixels of R, G, B, main (shirt), trim, shade (`staff_faces.py`).
const FACES: &[u8] = include_bytes!("staff_faces.bin");
const N: usize = 80;

struct Report {
    n: u32,
    head: String,
    face: usize,
    colours: [[u8; 3]; 2],
    text: String,
    /// The people it names, as the text writes them, and their staff ids: links in the story.
    links: Vec<(String, i32)>,
}

static mut REPORTS: Vec<Report> = Vec::new();
static mut LAST: i32 = 0;
static mut FILE: String = String::new();

/// The report the news screen shows now (0 physio, 1 youth coach, 2 chief scout) and its club:
/// what its Full Report button opens. Noted when the news screen asks for the item's button with
/// its own label buffer (0xde1f64); the read marker and the urgent check ask with theirs.
static mut SHOWN: Option<(u8, usize)> = None;
const SCREEN_LABEL: usize = 0xde1f64;
const ADD_LINK: usize = 0x4ae8a0; // Text::add_link(this, phrase, kind, target), thiscall ret 0xc
const STAFF_LINK: i32 = 0x7d7; // its kind for a person (comp_stats' player links)

/// `NewsItem::get_action_button` for our items (`cheesecorner::action`): "Full Report", a view
/// (1), the handler opening the full page (`finreport`), as the next opposition's scout report.
#[allow(static_mut_refs)]
pub unsafe fn button(item: usize, manager: usize, label: *mut u8, size: i32, screen: *mut usize) -> Option<u8> {
    let n = ours(item)?;
    load();
    let r = REPORTS.iter().find(|r| r.n == n)?;
    let kind = if r.head.starts_with("Physio") { 0 } else if r.head.starts_with("Youth") { 1 } else { 2 };
    if label as usize == SCREEN_LABEL && readable(manager, 0x3d) {
        SHOWN = Some((kind, rd::<usize>(manager + 0x39)));
    }
    if !label.is_null() && size > 12 {
        std::ptr::copy_nonoverlapping(b"Full Report\0".as_ptr(), label, 12);
    }
    if !screen.is_null() {
        *screen = full_report as usize;
    }
    Some(1)
}

/// The Full Report button's handler (the game passes the item's keys: not needed).
unsafe extern "C" fn full_report(_a: usize, _b: usize) {
    // (the host running a client's click, its screen channel > 0 as in `export::open`: the client
    // opens its own page, `client_full_report`; here it opened over the host's screen, user
    // 2026-11-26)
    if !remote::CLIENT && remote::network_game() && rd::<i16>(SCREEN_MANAGER + 0x382a) > 0 {
        return;
    }
    match SHOWN {
        Some((0, club)) => finreport::open_medical(club),
        Some((1, club)) => finreport::open_youth(club),
        Some((_, club)) => finreport::open_scouting(club),
        None => {}
    }
    // (a network client: the host's page comes after this click, opened when it does, night 12)
    if let (Some((kind, club)), false) = (SHOWN, finreport::page_shown()) {
        finreport::ask_again(finreport::STAFF_PAGES[kind.min(2) as usize], club);
    }
}

/// A network client's click on the "Full Report" button of a staff report it shows: the page of
/// the report whose headline is the story's title (the lowest headline on screen: the list rows
/// above it carry them too), for the client's own club, asked of the host. True: taken.
#[allow(static_mut_refs)]
pub unsafe fn client_full_report(mx: i32, my: i32) -> bool {
    let inside = |b: &(i32, i32, i32, i32, String)| (b.0..=b.2).contains(&mx) && (b.1..=b.3).contains(&my);
    if !crate::LABELS.iter().any(|b| b.4 == "full report" && inside(b)) {
        return false;
    }
    let heads = ["physio's monthly report - ", "youth coach's monthly report - ", "chief scout's monthly report - "];
    let Some(kind) = crate::LABELS.iter().filter_map(|b| heads.iter().position(|h| b.4.starts_with(h)).map(|k| (b.1, k))).max().map(|(_, k)| k) else { return false };
    let Some(club) = crate::human_club_ptr() else { return false };
    let page = finreport::STAFF_PAGES[kind];
    match kind {
        0 => finreport::open_medical(club),
        1 => finreport::open_youth(club),
        _ => finreport::open_scouting(club),
    }
    if !finreport::page_shown() {
        finreport::ask_again(page, club);
    }
    true
}

/// Links the people an item of ours names in its story text `out` (a game Text, just set).
#[allow(static_mut_refs)]
pub unsafe fn add_links(item: usize, out: usize) {
    let Some(n) = ours(item) else { return };
    let Some(r) = REPORTS.iter().find(|r| r.n == n) else { return };
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    for (name, id) in &r.links {
        let staff = base + (*id).max(0) as usize * 0x6e;
        if (0..count).contains(id) && readable(staff, 0x6e) {
            let mut phrase = to_latin1(name);
            phrase.push(0);
            game::<extern "thiscall" fn(usize, *const u8, i32, usize) -> i32>(ADD_LINK)(out, phrase.as_ptr(), STAFF_LINK, staff);
        }
    }
}

/// One of our items: its report number.
pub unsafe fn ours(item: usize) -> Option<u32> {
    let slot = |k: usize| ((item + 5 + 4 * k) as *const u32).read_unaligned();
    (item != 0 && mapped(item, 0xe0) && rd::<i32>(item) == 0x1388 && slot(2) == MARK && slot(20) == MARK2).then(|| slot(21))
}

/// An item's headline (`full` false) or its report, when it is ours (the news writer's hook).
#[allow(static_mut_refs)]
pub unsafe fn text(item: usize, full: bool) -> Option<String> {
    let n = ours(item)?;
    load();
    let r = REPORTS.iter().find(|r| r.n == n);
    Some(match (r, full) {
        (Some(r), false) => r.head.clone(),
        (Some(r), true) => r.text.clone(),
        (None, _) => "The report is lost.".into(),
    })
}

#[allow(static_mut_refs)]
unsafe fn load() {
    let Some(game) = challenge::game_number() else { return };
    let file = format!("cmui_staff_{game:08x}.txt");
    if file == FILE {
        return;
    }
    REPORTS.clear();
    LAST = 0;
    let colour = |t: &str| -> [u8; 3] {
        let v: Vec<u8> = t.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() == 3 { [v[0], v[1], v[2]] } else { [0x80; 3] }
    };
    for l in std::fs::read_to_string(&file).unwrap_or_default().lines() {
        if let Some(v) = l.strip_prefix("LAST\t") {
            LAST = v.parse().unwrap_or(0);
            continue;
        }
        // (n, head, face, colours, colours, text[, links "name=id;..."]: the first month's have none)
        let f: Vec<&str> = l.split('\t').collect();
        if !(6..=7).contains(&f.len()) {
            continue;
        }
        let links = f.get(6).map_or(Vec::new(), |t| {
            t.split(';').filter_map(|p| p.rsplit_once('=')).filter_map(|(name, id)| Some((name.to_string(), id.parse().ok()?))).collect()
        });
        if let (Ok(n), Ok(face)) = (f[0].parse(), f[2].parse::<usize>()) {
            REPORTS.push(Report { n, head: f[1].into(), face: face % 9, colours: [colour(f[3]), colour(f[4])], text: f[5].replace('|', "\n"), links });
        }
    }
    FILE = file;
}

#[allow(static_mut_refs)]
unsafe fn save() {
    let mut out = format!("LAST\t{LAST}\n");
    let c = |x: [u8; 3]| format!("{},{},{}", x[0], x[1], x[2]);
    for r in &REPORTS {
        let links: Vec<String> = r.links.iter().map(|(name, id)| format!("{name}={id}")).collect();
        out += &format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.n, r.head, r.face, c(r.colours[0]), c(r.colours[1]), r.text.replace('\n', "|"), links.join(";"));
    }
    let _ = std::fs::write(&FILE, out);
}

/// A club's first two kit colours (club +0x83: colour records, red / green / blue at +55), as
/// `stadium::club_colours` reads them; blue and white when they can't be read.
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

/// His face (0..9): from his name, the same each month.
fn face_of(name: &str) -> usize {
    name.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193)) as usize % 9
}

/// A face's pixel (0..80) dressed in `colours`: the shirt in the first, the trim in the second,
/// its folds kept (`staff_faces.py` `dress`, the same sums).
fn dress(face: usize, col: usize, row: usize, colours: [[u8; 3]; 2]) -> [u8; 3] {
    let p = &FACES[((face * N + row) * N + col) * 6..][..6];
    let (wm, wt, sh) = (p[3] as f32 / 255.0, p[4] as f32 / 255.0, p[5] as f32 / 128.0);
    let tint = |c: u8| (c as f32 * (0.35 + 0.65 * sh) + 18.0 * (sh - 1.0)).clamp(0.0, 255.0);
    let mut out = [0u8; 3];
    for k in 0..3 {
        let v = p[k] as f32 * (1.0 - wm - wt) + wm * tint(colours[0][k]) + wt * tint(colours[1][k]);
        out[k] = v.clamp(0.0, 255.0) as u8;
    }
    out
}

/// The portrait of a headline that is ours, in a box of `size`; false when it is not ours.
#[allow(static_mut_refs)]
pub unsafe fn photo(t: &[u8], x: i32, y: i32, size: i32) -> bool {
    let t = latin1(t);
    let (face, colours) = match REPORTS.iter().rev().find(|r| !r.head.is_empty() && t.starts_with(r.head.as_str())) {
        Some(r) => (r.face, r.colours),
        // (a network client has no reports file: the face from the name in the headline, the
        // shirt in its own club's colours, as the host chose them; night 12)
        None if remote::CLIENT && ["Physio's monthly report - ", "Youth coach's monthly report - ", "Chief scout's monthly report - "].iter().any(|p| t.starts_with(p)) => {
            let who = t.split(" - ").nth(1).unwrap_or("").trim();
            (face_of(who), crate::human_club_ptr().map_or([[0x26, 0x52, 0x8e], [0xe8, 0xe8, 0xe8]], |c| club_colours(c)))
        }
        None => return false,
    };
    let size = size.max(8);
    blend(x, y, size, size, |col, row| {
        let (c, r) = ((col as usize * N / size as usize).min(N - 1), (row as usize * N / size as usize).min(N - 1));
        (rgb(dress(face, c, r, colours)), 255)
    });
    true
}

/// The diary's daily tick (host), after the scouts' assignments: on the first day seen of a new
/// month the three reports to every human manager of a club. The game's first month gets none.
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
        let colours = club_colours(club);
        let seed = (now as u32).wrapping_mul(0x9e37_79b9) ^ rd::<i32>(staff) as u32;
        let mut used: Vec<usize> = Vec::new();
        let named = people(club);
        let reports = [physio(club, seed), youth(club, seed), if on(SCOUT_SCREEN) { scout(club, seed, year, month) } else { None }];
        for (who, head, body) in reports.into_iter().flatten() {
            // (his face; another one when a colleague this month already has it)
            let mut face = face_of(&who);
            while used.contains(&face) {
                face = (face + 1) % 9;
            }
            used.push(face);
            let n = REPORTS.iter().map(|r| r.n).max().unwrap_or(0) + 1;
            // (each person the text names, once; the longest names first, so "Jack Janssen" is not
            // taken for a "Janssen" link)
            let mut links: Vec<(String, i32)> = named.iter().filter(|(name, _)| body.contains(name.as_str())).cloned().collect();
            links.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
            links.dedup_by(|a, b| a.0 == b.0);
            REPORTS.push(Report { n, head: head.clone(), face, colours, text: body, links });
            let mut item = [0u8; 0xe0];
            let it = item.as_mut_ptr() as usize;
            game::<extern "thiscall" fn(usize, i32, i32) -> usize>(NEWS_NEW)(it, 0x1388, 1);
            let slot = |k: usize, v: u32| ((it + 5 + 4 * k) as *mut u32).write_unaligned(v);
            slot(0, rd::<i32>(staff) as u32);
            slot(1, 1);
            slot(2, MARK);
            let short = to_latin1(&head);
            std::ptr::copy_nonoverlapping(short.as_ptr(), (it + 5 + 12) as *mut u8, short.len().min(67));
            slot(20, MARK2);
            slot(21, n);
            game::<extern "thiscall" fn(usize, usize, usize) -> i32>(SEND)(news, it, club);
            log(&format!("staff: report {n} {head}"));
        }
    }
    save();
}

/// A person a staff page names (its name as the page writes it), among the club's people.
pub unsafe fn person_named(club: usize, name: &str) -> Option<usize> {
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    let id = people(club).into_iter().find(|(n, _)| n == name)?.1;
    (0..count).contains(&id).then(|| base + id as usize * 0x6e).filter(|&s| readable(s, 0x6e))
}

/// Who a month's reports may name, with their staff ids: the club's squad and the chief scout's
/// players (`diary::scouted`). Names as the reports write them (`staff_name`).
unsafe fn people(club: usize) -> Vec<(String, i32)> {
    let mut staff = challenge::squad_of(club);
    let (base, count) = (rd::<usize>(STAFF_TABLE), rd::<i32>(STAFF_COUNT));
    if let Some(seed) = finreport::seed_of(club) {
        for a in diary::scouted(seed) {
            staff.extend(a.4.iter().filter(|id| (0..count).contains(*id)).map(|&id| base + id as usize * 0x6e));
        }
    }
    // (the file keeps them as "name=id;...")
    staff.into_iter().filter(|&s| readable(s, 0x6e)).map(|s| (latin1(&staff_name(s)).replace(['=', ';', '\t'], " "), rd::<i32>(s))).collect()
}

/// The head physio's report: (his name, headline, text).
unsafe fn physio(club: usize, seed: u32) -> Option<(String, String, String)> {
    let md = finreport::decode_medical(&String::from_utf8_lossy(&finreport::medical_remote(club)?))?;
    if md.rows.is_empty() {
        return None;
    }
    let who = md.physio.clone();
    Some((who.clone(), format!("Physio's monthly report - {who}"), physio_words(&md, seed)))
}

fn physio_words(md: &finreport::Medical, seed: u32) -> String {
    let r = |salt: u32, options: &[&'static str]| cheesecorner::pick(seed, salt, options).to_string();
    let mut s = r(1, &[
        "Boss, my monthly word on the squad's bodies: who is out, and who I would keep an eye on.",
        "Here is where the squad stands physically at the start of the month.",
        "The treatment room's monthly report. Short version: read the names below before you pick the team.",
        "My look at the squad's fitness this month: the injured, and the ones I am worried about.",
    ]);
    // (short, the names linked: the whole squad is on the Full Report page, user 2026-11-24)
    let mut injured: Vec<&finreport::MedRow> = md.rows.iter().filter(|r| r.out > 0).collect();
    injured.sort_by_key(|r| -r.out);
    s += "\n\n";
    if injured.is_empty() {
        s += &r(2, &["Nobody is injured: the whole squad trains.", "The treatment room is empty. Long may it last.", "Not a single injury. Enjoy it, boss, it never lasts."]);
    } else {
        s += &format!("Injured ({}):", injured.len());
        for m in injured.iter().take(3) {
            s += &format!("\n- {} ({}): out {} days", m.name, m.pos, m.out);
        }
    }
    let mut watch: Vec<&finreport::MedRow> = md.rows.iter().filter(|r| r.to_watch()).collect();
    watch.sort_by_key(|r| (-r.risk(), r.condition));
    if !watch.is_empty() {
        s += "\n\n";
        s += &r(3, &["The ones I would keep an eye on:", "To watch this month:", "These worry me a little:"]);
        for m in watch.iter().take(3) {
            s += &format!("\n- {} ({}): {}% fit, {} risk", m.name, m.pos, m.condition, m.risk_word().to_lowercase());
        }
    }
    let fit = md.rows.iter().filter(|r| r.out == 0).count() - watch.len();
    s += &format!("\n\nThe other {fit} are fit and fresh. Every player is in my full report.\n\n{}, head physio", md.physio);
    s
}

/// The youth coach's report: (his name, headline, text).
unsafe fn youth(club: usize, seed: u32) -> Option<(String, String, String)> {
    let y = finreport::decode_youth(&String::from_utf8_lossy(&finreport::youth_remote(club)?))?;
    let who = y.coach.clone();
    Some((who.clone(), format!("Youth coach's monthly report - {who}"), youth_words(&y, seed)))
}

fn youth_words(y: &finreport::Youth, seed: u32) -> String {
    let r = |salt: u32, options: &[&'static str]| cheesecorner::pick(seed, salt, options).to_string();
    let mut s = r(1, &[
        "My monthly word on the youngsters: where they stand now, and how far I think they can go.",
        "The kids, boss. Here is how I rate every player of 19 and under at the club this month.",
        "Another month with the young ones. My honest view on each of them below.",
    ]);
    if y.rows.is_empty() {
        s += "\n\nWe have no players of 19 or under at the club. I would like a few: give me some and I will make players of them.";
    } else {
        // (only the first letter down: "a future star. keep him close", night 12)
        let line = |p: &finreport::YouthRow| format!("\n- {} ({}, {}): {}{}", p.name, p.pos, p.age, p.word()[..1].to_lowercase(), &p.word()[1..]);
        // (short, the names linked: every youngster is on the Full Report page, user 2026-11-24)
        let mut new: Vec<&finreport::YouthRow> = y.rows.iter().filter(|r| r.new).collect();
        new.sort_by_key(|r| -r.seen);
        if !new.is_empty() {
            s += &format!("\n\nNew this month ({}):", new.len());
            new.iter().take(3).for_each(|p| s += &line(p));
        }
        let mut best: Vec<&finreport::YouthRow> = y.rows.iter().filter(|r| !r.new && r.seen >= 110).collect();
        best.sort_by_key(|r| -r.seen);
        if !best.is_empty() {
            s += "\n\n";
            s += &r(2, &["The ones I believe in:", "Who could make it:", "My pick of the bunch:"]);
            best.iter().take(3).for_each(|p| s += &line(p));
        }
        s += &format!("\n\nAll {} of them, with a loan or a farewell where I would give one, are in my full report.", y.rows.len());
    }
    s += &format!("\n\n{}, youth coach", y.coach);
    s
}

/// The chief scout's report on this month's assignment: (his name, headline, text).
unsafe fn scout(club: usize, seed: u32, year: i32, month: i32) -> Option<(String, String, String)> {
    let sc = finreport::decode_scouting(&String::from_utf8_lossy(&finreport::scouting_remote(club)?))?;
    let this = format!("{} {year}", MONTHS[(month as usize).clamp(1, 12) - 1]);
    let who = sc.scout.clone();
    Some((who.clone(), format!("Chief scout's monthly report - {who}"), scout_words(&sc, &this, seed)))
}

fn scout_words(sc: &finreport::Scouting, this: &str, seed: u32) -> String {
    let r = |salt: u32, options: &[&'static str]| cheesecorner::pick(seed, salt, options).to_string();
    let rows: Vec<_> = sc.rows.iter().filter(|r| r.0 == this).collect();
    let mut s = String::new();
    match rows.first() {
        None => s += &r(1, &[
            "I have been out all month, boss, but I found nobody better than what we have for the money we have. I will keep looking.",
            "Nothing to report this month: nobody I saw is better than ours and within our means.",
        ]),
        Some(first) => {
            s += &r(1, &[
                "My monthly report. Where we need it most is {line}, so that is where I have been looking.",
                "Boss, I looked at {line} this month: our best there is the weakest link in the side.",
                "This month's assignment was {line}. These are the ones I would bring in.",
            ])
            .replace("{line}", &first.1.to_lowercase());
            s += "\n";
            for p in rows.iter().take(3) {
                let stars = if p.7 % 2 == 1 { format!("{}.5", p.7 / 2) } else { (p.7 / 2).to_string() };
                s += &format!("\n- {} ({}, {}), {}: {} stars", p.2, p.3, p.4, p.5, stars);
            }
            s += "\n\n";
            s += &r(2, &["The stars are how I rate them, not what the papers say.", "Say the word and I will go back for another look.", "I would not wait too long on the first one."]);
            s += " Their values, and the past months' names, are in my full report.";
        }
    }
    s += &format!("\n\n{}, chief scout", sc.scout);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_dress_and_reports_read() {
        assert_eq!(FACES.len(), 9 * N * N * 6);
        // a shirt pixel takes the club's colour; the face stays the photo
        let shirt = (0..N * N).find(|&k| FACES[k * 6 + 3] == 255).unwrap();
        let red = dress(0, shirt % N, shirt / N, [[200, 0, 0], [255, 255, 255]]);
        assert!(red[0] > red[1] + 40, "{red:?}");
        let skin = N * 30 + 40; // the middle of the face
        assert_eq!(dress(0, skin % N, skin / N, [[200, 0, 0], [255, 255, 255]]), [FACES[skin * 6], FACES[skin * 6 + 1], FACES[skin * 6 + 2]]);
        let row = |name: &str, out, cond, jaded| finreport::MedRow { name: name.into(), pos: "D".into(), age: 25, condition: cond, jaded, prone: 5, out, trains: 3, ..Default::default() };
        let md = finreport::Medical { physio: "Doc".into(), skill: 15, rows: vec![row("Hurt", 10, 50, 50), row("Tired", 0, 70, 70), row("Fine", 0, 99, 40)], ..Default::default() };
        let s = physio_words(&md, 1);
        for part in ["Hurt", "Tired", "other 1 are fit", "Doc, head physio"] {
            assert!(s.contains(part), "{part} missing in:\n{s}");
        }
        let y = finreport::Youth { coach: "Coach".into(), rows: vec![finreport::YouthRow { name: "Kid".into(), pos: "S".into(), age: 17, now: 60, seen: 150, new: false }], ..Default::default() };
        assert!(youth_words(&y, 2).contains("Kid (S, 17): first-team player"));
        let sc = finreport::Scouting { scout: "Eye".into(), rows: vec![("May 2027".into(), "Defence".into(), "Target".into(), "D".into(), 24, "Club".into(), "1M".into(), 7)], ..Default::default() };
        let s = scout_words(&sc, "May 2027", 3);
        assert!(s.contains("Target (D, 24), Club: 3.5 stars") && s.contains("defence") && s.contains("full report"), "{s}");
        assert!(scout_words(&sc, "June 2027", 3).contains("nobody"));
    }
}
