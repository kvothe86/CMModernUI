//! Export as an HTML dashboard (switch `export_button`): the game's own printout is written as
//! before (its txt has everything: the scorers of each fixture, the transfers, the comp tables),
//! then turned into one page next to it (`print\<name>.html`: flat design, CSS and a small script
//! inline, the Outfit font from Google Fonts when online) and opened in the browser (single player only: on a network game the file lands on
//! the host). Club screen: Squad, Stats, Transfers, Fixtures, All (0x7d4460); competition and
//! cup screens: Table, Goals, Assists, Average Ratings, Man of Match, All (0x7d7c80).
//!
//! The txt: a "****" box with the title, sections between "====" lines, tables of a header line
//! and a dash line, rows in the header's columns, a dash line inside a table a separator (the
//! league's zones), a blank line its end.
use crate::*;

/// The club screen's print (club, ?, entry mask, file, printer flag), its call at 0x4672f3.
pub extern "C" fn club(a: usize, b: usize, mask: u32, file: *const u8, flag: u32) -> i32 {
    let r = unsafe { game::<extern "C" fn(usize, usize, u32, *const u8, u32) -> i32>(0x7d4460)(a, b, mask, file, flag) };
    if r != 0 {
        unsafe { convert(file) };
    }
    r
}

/// The competition screen's print (competition, entry, file, printer flag), its call at 0x496654.
pub extern "C" fn comp(c: usize, mask: u32, file: *const u8, flag: u32) -> i32 {
    let r = unsafe { game::<extern "C" fn(usize, u32, *const u8, u32) -> i32>(0x7d7c80)(c, mask, file, flag) };
    unsafe { convert(file) };
    r
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteA(hwnd: usize, op: *const u8, file: *const u8, params: *const u8, dir: *const u8, show: i32) -> usize;
}

unsafe fn convert(file: *const u8) {
    if file.is_null() {
        return;
    }
    let path = std::path::PathBuf::from(latin1(std::ffi::CStr::from_ptr(file as *const i8).to_bytes()).into_owned());
    let Ok(txt) = std::fs::read(&path) else {
        log(&format!("export: {} not readable", path.display()));
        return;
    };
    let html = path.with_extension("html");
    if std::fs::write(&html, to_html(&txt)).is_err() {
        log(&format!("export: {} not written", html.display()));
        return;
    }
    if trace() {
        log(&format!("export: {} written", html.display()));
    }
    if !remote::network_game() {
        let full = std::env::current_dir().map(|d| d.join(&html)).unwrap_or(html);
        let c = std::ffi::CString::new(full.to_string_lossy().as_bytes()).unwrap_or_default();
        ShellExecuteA(0, b"open\0".as_ptr(), c.as_ptr() as *const u8, std::ptr::null(), std::ptr::null(), 1);
    }
}

// ---- the txt read back ----

#[derive(Debug, Default)]
struct Table {
    head: Vec<String>,
    rows: Vec<Option<Vec<String>>>, // None: a separator line
}

enum Block {
    Text(String),
    Table(Table),
}

#[derive(Default)]
struct Section {
    title: String,
    blocks: Vec<Block>,
}

/// cp1252 as the game writes it (0x80 the euro, the rest as latin1).
fn text(b: &[u8]) -> String {
    b.iter().map(|&c| if c == 0x80 { '\u{20ac}' } else { c as char }).collect()
}

fn rule(l: &str, c: char) -> bool {
    l.len() >= 10 && l.trim_end().chars().all(|x| x == c)
}

/// Columns start at 0 and at each word of the header after two spaces, or after one when every
/// row has a gap there too ("Caps Gls" two columns, "Av R" and "Player In" one).
fn columns(head: &str, rows: &[&str]) -> Vec<usize> {
    let h = head.as_bytes();
    let gap = |i: usize| rows.iter().all(|r| r.as_bytes().get(i - 1).is_none_or(|&c| c == b' '));
    let mut s = vec![0];
    s.extend((2..h.len()).filter(|&i| h[i] != b' ' && h[i - 1] == b' ' && (h[i - 2] == b' ' || gap(i))));
    s
}

fn cut(line: &str, starts: &[usize]) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    (0..starts.len())
        .map(|k| {
            let (a, b) = (starts[k].min(chars.len()), starts.get(k + 1).map_or(chars.len(), |&e| e.min(chars.len())));
            chars[a..b].iter().collect::<String>().trim().to_string()
        })
        .collect()
}

fn parse(txt: &[u8]) -> (String, Vec<Section>) {
    let all = text(txt);
    let lines: Vec<&str> = all.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut title = String::new();
    let mut out: Vec<Section> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if rule(l, '*') {
            if title.is_empty() && i + 1 < lines.len() && !rule(lines[i + 1], '*') {
                title = lines[i + 1].trim().to_string();
                i += 1;
            }
        } else if rule(l, '=') && i + 2 < lines.len() && rule(lines[i + 2], '=') {
            out.push(Section { title: lines[i + 1].trim().to_string(), blocks: Vec::new() });
            i += 2;
        } else if i + 1 < lines.len() && !l.trim().is_empty() && rule(lines[i + 1], '-') {
            let body: Vec<&str> = lines[i + 2..].iter().take_while(|r| !r.trim().is_empty()).filter(|r| !rule(r, '-')).copied().collect();
            let starts = columns(l, &body);
            let mut t = Table { head: cut(l, &starts), rows: Vec::new() };
            i += 2;
            while i < lines.len() && !lines[i].trim().is_empty() {
                t.rows.push(if rule(lines[i], '-') { None } else { Some(cut(lines[i], &starts)) });
                i += 1;
            }
            while t.rows.last().is_some_and(|r| r.is_none()) {
                t.rows.pop();
            }
            if out.is_empty() {
                out.push(Section::default());
            }
            out.last_mut().unwrap().blocks.push(Block::Table(t));
            continue;
        } else if !l.trim().is_empty() {
            if out.is_empty() {
                out.push(Section::default());
            }
            out.last_mut().unwrap().blocks.push(Block::Text(l.trim().to_string()));
        }
        i += 1;
    }
    (title, out)
}

// ---- numbers ----

fn num(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    t.parse().ok()
}

/// "E3.5M", "$200K", "£1.2M" -> (currency, value).
fn money(s: &str) -> Option<(String, f64)> {
    let s = s.trim();
    let at = s.find(|c: char| c.is_ascii_digit())?;
    let v = num(&s[at..])?;
    let scale = match s.trim_end().chars().last() {
        Some('K') => 1e3,
        Some('M') => 1e6,
        _ => 1.0,
    };
    Some((s[..at].to_string(), v * scale))
}

fn money_text(cur: &str, v: f64) -> String {
    let a = v.abs();
    let body = if a >= 1e6 { format!("{:.1}M", a / 1e6) } else if a >= 1e3 { format!("{:.0}K", a / 1e3) } else { format!("{a:.0}") };
    format!("{}{cur}{body}", if v < 0.0 { "-" } else { "" })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn col(t: &Table, name: &str) -> Option<usize> {
    t.head.iter().position(|h| h == name)
}

fn rows(t: &Table) -> impl Iterator<Item = &Vec<String>> {
    t.rows.iter().flatten()
}

// ---- the page ----

fn tile(label: &str, value: &str) -> String {
    format!("<div class=tile><b>{}</b><span>{}</span></div>", esc(value), esc(label))
}

/// A bar chart of the top ten (label, value), `fmt` the value's text.
fn bars(title: &str, mut items: Vec<(String, f64)>, fmt: fn(f64) -> String) -> String {
    items.retain(|i| i.1 > 0.0);
    if items.is_empty() {
        return String::new();
    }
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
    items.truncate(10);
    let top = items[0].1;
    let mut h = format!("<div class=chart><h3>{}</h3>", esc(title));
    for (l, v) in &items {
        // (a click filters the section's table by the name, the club in brackets left out)
        let q = l.split(" (").next().unwrap_or(l);
        h += &format!("<div class=bar data-q=\"{}\" title=\"Filter by {0}\"><span class=l>{}</span><span class=t><i style=\"width:{:.1}%\"></i></span><b>{}</b></div>", esc(q), esc(l), v / top * 100.0, esc(&fmt(*v)));
    }
    h + "</div>"
}

fn whole(v: f64) -> String {
    format!("{v:.0}")
}
fn two(v: f64) -> String {
    format!("{v:.2}")
}

/// The name a row is about: player, then his club if the table has one.
fn who(t: &Table, r: &[String]) -> Option<String> {
    let n = ["Name", "Player", "Team"].iter().find_map(|c| col(t, c))?;
    let name = r[n].split("  ").next().unwrap_or("").to_string();
    Some(match col(t, "Club") {
        Some(c) if !r[c].is_empty() => format!("{name} ({})", r[c]),
        _ => name,
    })
}

/// Counts of a column's values, for a bar chart.
fn tally<'a>(values: impl Iterator<Item = &'a str>) -> Vec<(String, f64)> {
    let mut n: Vec<(String, f64)> = Vec::new();
    for v in values {
        match n.iter_mut().find(|x| x.0 == v) {
            Some(x) => x.1 += 1.0,
            None => n.push((v.to_string(), 1.0)),
        }
    }
    n
}

/// What a table adds above itself: tiles and charts by the columns it has.
fn extras(t: &Table) -> String {
    let mut h = String::new();
    let mut charts = String::new();
    // the squad
    if let (Some(pos), Some(age), Some(wages), Some(value)) = (col(t, "Position(s)"), col(t, "Age"), col(t, "Wages"), col(t, "Value")) {
        let rs: Vec<_> = rows(t).collect();
        let ages: Vec<f64> = rs.iter().filter_map(|r| num(&r[age])).collect();
        let sum = |c: usize| rs.iter().filter_map(|r| money(&r[c])).fold((String::new(), 0.0), |a, m| (if a.0.is_empty() { m.0 } else { a.0 }, a.1 + m.1));
        let (cw, w) = sum(wages);
        let (cv, v) = sum(value);
        h += "<div class=tiles>";
        h += &tile("players", &rs.len().to_string());
        if !ages.is_empty() {
            h += &tile("average age", &format!("{:.1}", ages.iter().sum::<f64>() / ages.len() as f64));
        }
        h += &tile("wage bill", &money_text(&cw, w));
        h += &tile("squad value", &money_text(&cv, v));
        h += "</div>";
        charts += &bars("Positions", tally(rs.iter().map(|r| r[pos].split(['/', ' ']).next().unwrap_or(""))), whole);
        if let Some(nat) = col(t, "Nat") {
            charts += &bars("Nationalities", tally(rs.iter().map(|r| r[nat].as_str())), whole);
        }
        charts += &bars("Most valuable", rs.iter().filter_map(|r| Some((who(t, r)?, money(&r[value])?.1))).collect(), |v| money_text("", v));
    }
    // player and team stats
    for (c, label, fmt) in [("Gls", "Goals", whole as fn(f64) -> String), ("Asts", "Assists", whole), ("Av R", "Average rating", two), ("MoM", "Man of the match", whole), ("Pts", "Points", whole)] {
        if let Some(k) = col(t, c) {
            // (the squad's goals are the caps' goals)
            let label = if c == "Gls" && col(t, "Caps").is_some() { "International goals" } else { label };
            charts += &bars(label, rows(t).filter_map(|r| Some((who(t, r)?, num(&r[k])?))).collect(), fmt);
        }
    }
    // transfers
    if let Some(fee) = col(t, "Fee") {
        let (cur, total) = rows(t).filter_map(|r| money(&r[fee])).fold((String::new(), 0.0), |a, m| (if a.0.is_empty() { m.0 } else { a.0 }, a.1 + m.1));
        let n = rows(t).count();
        let inward = t.head.iter().any(|x| x.contains("In"));
        h += "<div class=tiles>";
        h += &tile(if inward { "signed" } else { "sold" }, &n.to_string());
        h += &tile(if inward { "spent" } else { "received" }, &money_text(&cur, total));
        h += "</div>";
        if let Some(p) = t.head.iter().position(|x| x.starts_with("Player")) {
            charts += &bars("Biggest fees", rows(t).filter_map(|r| Some((r[p].clone(), money(&r[fee])?.1))).collect(), |v| money_text("", v));
        }
    }
    // fixtures and results
    if let (Some(res), Some(comp)) = (col(t, "Res"), col(t, "Competition")) {
        let played: Vec<(&Vec<String>, i32, i32)> = rows(t)
            .filter_map(|r| {
                let (a, b) = r[res].split_once(':')?;
                Some((r, a.trim().parse().ok()?, b.trim().parse().ok()?))
            })
            .collect();
        let real: Vec<_> = played.iter().filter(|p| !p.0[comp].starts_with("Friendly")).collect();
        let (w, d, l) = (real.iter().filter(|p| p.1 > p.2).count(), real.iter().filter(|p| p.1 == p.2).count(), real.iter().filter(|p| p.1 < p.2).count());
        h += "<div class=tiles>";
        h += &tile("played", &real.len().to_string());
        h += &tile("won", &w.to_string());
        h += &tile("drawn", &d.to_string());
        h += &tile("lost", &l.to_string());
        h += &tile("goals for", &real.iter().map(|p| p.1).sum::<i32>().to_string());
        h += &tile("goals against", &real.iter().map(|p| p.2).sum::<i32>().to_string());
        if let (Some(ven), Some(att)) = (col(t, "Ven"), col(t, "Attend")) {
            let home: Vec<f64> = real.iter().filter(|p| p.0[ven] == "H").filter_map(|p| num(&p.0[att])).collect();
            if !home.is_empty() {
                h += &tile("home crowd (avg)", &format!("{:.0}", home.iter().sum::<f64>() / home.len() as f64));
            }
        }
        h += "</div>";
        if !real.is_empty() {
            h += "<div class=form>";
            for p in &real {
                let (c, x) = if p.1 > p.2 { ("w", "W") } else if p.1 == p.2 { ("d", "D") } else { ("l", "L") };
                h += &format!("<a class={c} href=\"#m{}\" title=\"{}\">{x}</a>", p.0[0].replace('.', "-"), esc(&format!("{} {}", p.0[1].split("  ").next().unwrap_or(""), p.0[res])));
            }
            h += "</div>";
            // goals a match: for up, against down
            let n = real.len() as i32;
            let mut svg = format!("<div class=chart><h3>Goals a match</h3><svg viewBox=\"0 0 {} 120\" preserveAspectRatio=\"none\"><line x1=0 y1=60 x2={} y2=60 class=axis />", n * 14, n * 14);
            for (k, p) in real.iter().enumerate() {
                let x = k as i32 * 14 + 2;
                svg += &format!("<rect x={x} y={} width=10 height={} class=gf /><rect x={x} y=60 width=10 height={} class=ga />", 60 - p.1.min(5) * 11, p.1.min(5) * 11, p.2.min(5) * 11);
            }
            charts += &(svg + "</svg></div>");
        }
        if let Some(sc) = col(t, "Scorers") {
            let mut goals: Vec<(String, f64)> = Vec::new();
            for p in &played {
                for s in p.0[sc].split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let s = s.trim_end_matches(" pen").trim_end_matches(" og");
                    let (name, k) = match s.rsplit_once(' ') {
                        Some((n, k)) if k.chars().all(|c| c.is_ascii_digit()) => (n, k.parse().unwrap_or(1.0)),
                        _ => (s, 1.0),
                    };
                    match goals.iter_mut().find(|x| x.0 == name) {
                        Some(x) => x.1 += k,
                        None => goals.push((name.to_string(), k)),
                    }
                }
            }
            charts += &bars("Scorers (all matches)", goals, whole);
        }
    }
    if !charts.is_empty() {
        h += &format!("<div class=charts>{charts}</div>");
    }
    h
}

/// The filter box over a section's first table, and for a fixture list a chip a competition.
fn tools_html(t: &Table) -> String {
    let mut h = String::from("<div class=tools><input class=find type=search placeholder=\"Filter rows&hellip;\">");
    if let (Some(_), Some(c)) = (col(t, "Res"), col(t, "Competition")) {
        let mut comps: Vec<&str> = Vec::new();
        for r in rows(t) {
            if !comps.contains(&r[c].as_str()) {
                comps.push(&r[c]);
            }
        }
        h += "<button class=\"chip on\" data-c=\"\">All</button>";
        for c in comps {
            h += &format!("<button class=chip data-c=\"{0}\">{0}</button>", esc(c));
        }
    }
    h + "</div>"
}

fn table_html(t: &Table) -> String {
    let (res, comp) = (col(t, "Res"), col(t, "Competition"));
    // a column of numbers (money, places, ratings) is right-aligned and sorts high first
    let numeric: Vec<bool> = (0..t.head.len())
        .map(|k| {
            let cells: Vec<&str> = rows(t).map(|r| r[k].as_str()).filter(|c| !c.is_empty() && *c != "-" && *c != "----").collect();
            let n = cells.iter().filter(|c| c.trim_start_matches(['$', '£', '€', 'E']).starts_with(|x: char| x.is_ascii_digit()) && c.matches('.').count() < 2).count();
            !cells.is_empty() && n * 10 >= cells.len() * 8
        })
        .collect();
    let mut h = String::from("<div class=scroll><table><thead><tr>");
    for (k, c) in t.head.iter().enumerate() {
        h += &format!("<th{}>{}</th>", if numeric[k] { " class=num" } else { "" }, esc(c));
    }
    h += "</tr></thead><tbody>";
    for r in &t.rows {
        let Some(r) = r else {
            h += &format!("<tr class=sep><td colspan={}></td></tr>", t.head.len());
            continue;
        };
        let class = res.and_then(|k| r[k].split_once(':')).and_then(|(a, b)| Some((a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?))).map_or("", |(a, b)| if a > b { " class=w" } else if a == b { " class=d" } else { " class=l" });
        // (a fixture: its competition for the chips, its date as the form strip's anchor)
        let extra = match (res, comp) {
            (Some(_), Some(c)) => format!(" id=m{} data-c=\"{}\"", r[0].replace('.', "-"), esc(&r[c])),
            _ => String::new(),
        };
        h += &format!("<tr{class}{extra}>");
        for (k, c) in r.iter().enumerate() {
            // (two spaces inside a column: a second value, the opponent's nation)
            let mut parts = c.split("  ").map(str::trim).filter(|p| !p.is_empty());
            let first = parts.next().unwrap_or("");
            let first = if first == "-" || first == "----" { String::from("<span class=dim>-</span>") } else { esc(first) };
            let rest: String = parts.map(|p| format!(" <em>{}</em>", esc(p))).collect();
            h += &format!("<td{}>{first}{rest}</td>", if numeric[k] { " class=num" } else { "" });
        }
        h += "</tr>";
    }
    h + "</tbody></table></div>"
}

/// Flat design (user 2026-11-09): one plane, no shadows or gradients, colour blocks for structure,
/// one 8 px radius, Outfit (Segoe UI offline). Primary the game's orange made AA-safe for white
/// text (#C2410C, 5.2:1); results #047857 / #4B5563 / #DC2626, all AA with white.
const CSS: &str = r#"
:root{--p:#C2410C;--p2:#9A3412;--pt:#FFEDD5;--a:#F59E0B;--w:#047857;--d:#4B5563;--l:#DC2626;--gf:#10B981;--ga:#EF4444;--m:#F3F4F6;--b:#E5E7EB;--t:#111827;--t2:#4B5563;--r:8px}
*{box-sizing:border-box}
body{margin:0;background:#fff;color:var(--t);font:15px/1.5 'Outfit','Segoe UI',system-ui,sans-serif}
header{background:var(--p);color:#fff;padding:44px 48px 40px;position:relative;overflow:hidden}
header::before{content:'';position:absolute;right:-90px;top:-140px;width:380px;height:380px;border-radius:50%;background:#fff;opacity:.08}
header::after{content:'';position:absolute;right:220px;bottom:-110px;width:200px;height:200px;background:#fff;opacity:.08;transform:rotate(45deg)}
header h1{margin:0;font-size:44px;font-weight:800;letter-spacing:-.02em;line-height:1.1}
header p{margin:8px 0 0;font-size:16px;font-weight:600;color:var(--pt)}
nav{position:sticky;top:0;z-index:2;background:var(--m);padding:12px 48px;display:flex;gap:8px;flex-wrap:wrap}
nav a{color:var(--t);text-decoration:none;font-weight:600;font-size:14px;padding:8px 14px;border-radius:var(--r);background:#fff;transition:transform .15s,background .15s}
nav a:hover{background:var(--p);color:#fff;transform:scale(1.05)}
main{max-width:1440px;margin:0 auto;padding:40px 48px;display:grid;gap:48px}
section h2{display:flex;justify-content:space-between;align-items:center;margin:0 0 20px;background:var(--p);color:#fff;font-size:22px;font-weight:800;letter-spacing:-.01em;padding:14px 20px;border-radius:var(--r);cursor:pointer;user-select:none}
section h2::after{content:'\2212';font-size:24px;line-height:1}section.closed h2::after{content:'+'}section.closed>:not(h2){display:none}
h3{margin:0 0 12px;font-size:12px;font-weight:700;text-transform:uppercase;letter-spacing:.08em;color:var(--t2)}
.tiles{display:flex;gap:12px;flex-wrap:wrap;margin-bottom:20px}
.tile{background:var(--m);border-radius:var(--r);padding:16px 22px;min-width:140px}
.tile:first-child{background:var(--p)}.tile:first-child b,.tile:first-child span{color:#fff}
.tile b{display:block;font-size:30px;font-weight:800;letter-spacing:-.02em;line-height:1.15;color:var(--t)}
.tile span{font-size:12px;font-weight:600;text-transform:uppercase;letter-spacing:.08em;color:var(--t2)}
.charts{display:grid;grid-template-columns:repeat(auto-fill,minmax(340px,1fr));gap:16px;margin-bottom:24px}
.chart{background:var(--m);border-radius:var(--r);padding:20px}
.bar{display:grid;grid-template-columns:170px 1fr 60px;gap:10px;align-items:center;font-size:14px;padding:3px 6px;margin:0 -6px;border-radius:6px;cursor:pointer;transition:background .15s}
.bar:hover{background:#fff}.bar:hover i{background:var(--p)}
.bar .l{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.bar .t{background:var(--b);border-radius:4px;height:12px}.bar i{display:block;height:100%;border-radius:4px;background:var(--a);transition:background .15s}
.bar b{text-align:right;font-weight:800;font-variant-numeric:tabular-nums}
svg{width:100%;height:140px;display:block}.axis{stroke:var(--b);stroke-width:2}.gf{fill:var(--gf)}.ga{fill:var(--ga)}
.form{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:20px}
.form a{width:30px;height:30px;border-radius:6px;display:grid;place-items:center;font-size:13px;font-weight:800;color:#fff;text-decoration:none;transition:transform .15s}
.form a:hover{transform:scale(1.15)}.form .w{background:var(--w)}.form .d{background:var(--d)}.form .l{background:var(--l)}
.tools{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin-bottom:14px}
.find{font:inherit;font-size:14px;padding:9px 14px;border:3px solid var(--b);border-radius:var(--r);width:280px;outline:none;background:#fff;color:var(--t)}.find:focus{border-color:var(--p)}
.chip{font:inherit;font-size:13px;font-weight:600;padding:9px 14px;border:0;border-radius:var(--r);background:var(--m);color:var(--t);cursor:pointer;transition:transform .15s,background .15s}
.chip:hover{transform:scale(1.05)}.chip.on{background:var(--p);color:#fff}
.scroll{overflow-x:auto;margin-bottom:20px}
table{border-collapse:separate;border-spacing:0;width:100%;font-size:14px}
th{background:var(--p);color:#fff;text-align:left;font-size:12px;font-weight:700;text-transform:uppercase;letter-spacing:.06em;padding:12px 14px;white-space:nowrap;cursor:pointer;user-select:none}
th:hover{background:var(--p2)}th:first-child{border-radius:var(--r) 0 0 0}th:last-child{border-radius:0 var(--r) 0 0}
th.asc::after{content:' \25B2'}th.desc::after{content:' \25BC'}
td{padding:9px 14px;white-space:nowrap}tbody tr:nth-child(even){background:var(--m)}tbody tr:hover{background:var(--pt)}
.num{text-align:right;font-variant-numeric:tabular-nums}
td em{font-style:normal;font-size:11px;font-weight:700;color:var(--t2);background:var(--b);border-radius:4px;padding:1px 6px;margin-left:6px}
tr.sep td{padding:0;height:4px;background:var(--p)}
tr.w td:first-child{border-left:6px solid var(--w)}tr.d td:first-child{border-left:6px solid var(--d)}tr.l td:first-child{border-left:6px solid var(--l)}
tr:target{background:#FEF3C7!important}
.dim{color:#9CA3AF}p.note{color:var(--t2);margin:0 0 12px}
footer{background:var(--m);color:var(--t2);text-align:center;padding:20px;font-size:13px;font-weight:600}
"#;

/// Sorting (a header: numbers high first, money, "1st", dates d.m.yy), a filter box a section, the
/// fixtures' competition chips, a bar filters its section by its name, a section's bar folds it.
const JS: &str = r#"
const val=s=>{s=s.trim();if(!s||s=='-'||s=='----')return null;let m=s.match(/^(\d+)\.(\d+)\.(\d+)$/);if(m)return +m[3]*1e4+ +m[2]*100+ +m[1];
m=s.match(/^[^\d]{0,2}(\d+(?:\.\d+)?)\s*([KM])?/);if(!m)return s.toLowerCase();return +m[1]*(m[2]=='M'?1e6:m[2]=='K'?1e3:1)};
document.querySelectorAll('th').forEach(th=>th.onclick=()=>{const tr=th.parentNode,tb=th.closest('table').tBodies[0],i=[...tr.children].indexOf(th);
let d=th.classList.contains('desc')?1:th.classList.contains('asc')?-1:(th.classList.contains('num')?-1:1);
tr.querySelectorAll('th').forEach(x=>x.classList.remove('asc','desc'));th.classList.add(d>0?'asc':'desc');
tb.querySelectorAll('tr.sep').forEach(r=>r.remove());const rows=[...tb.rows];
rows.sort((a,b)=>{const x=val(a.cells[i].textContent),y=val(b.cells[i].textContent);if(x===null&&y===null)return 0;if(x===null)return 1;if(y===null)return -1;
if(typeof x!=typeof y)return typeof x=='number'?-1:1;return (x<y?-1:x>y?1:0)*d});rows.forEach(r=>tb.appendChild(r))});
document.querySelectorAll('section').forEach(sec=>{const inp=sec.querySelector('.find'),chips=sec.querySelectorAll('.chip');let comp='';
const run=()=>{const q=(inp?inp.value:'').toLowerCase();sec.querySelectorAll('tbody tr:not(.sep)').forEach(r=>{r.hidden=!(r.textContent.toLowerCase().includes(q)&&(!comp||r.dataset.c==comp))})};
if(inp)inp.oninput=run;chips.forEach(c=>c.onclick=()=>{comp=c.dataset.c;chips.forEach(x=>x.classList.toggle('on',x==c));run()});
sec.querySelectorAll('.bar[data-q]').forEach(b=>b.onclick=()=>{if(!inp)return;inp.value=inp.value==b.dataset.q?'':b.dataset.q;run();inp.scrollIntoView({behavior:'smooth',block:'center'})});
sec.querySelector('h2').onclick=()=>sec.classList.toggle('closed')});
"#;

pub fn to_html(txt: &[u8]) -> String {
    let (title, sections) = parse(txt);
    let (name, date) = title.split_once(" - ").unwrap_or((title.as_str(), ""));
    let mut h = format!(
        "<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{}</title>\
         <link rel=preconnect href=\"https://fonts.googleapis.com\"><link rel=stylesheet href=\"https://fonts.googleapis.com/css2?family=Outfit:wght@400;600;700;800&display=swap\">\
         <style>{CSS}</style></head><body>",
        esc(&title)
    );
    h += &format!("<header><h1>{}</h1><p>{}</p></header><nav>", esc(name), esc(date));
    for (k, s) in sections.iter().enumerate() {
        h += &format!("<a href=\"#s{k}\">{}</a>", esc(if s.title.is_empty() { "Report" } else { &s.title }));
    }
    h += "</nav><main>";
    for (k, s) in sections.iter().enumerate() {
        h += &format!("<section id=s{k}><h2>{}</h2>", esc(&s.title));
        let mut tools = true;
        for b in &s.blocks {
            match b {
                Block::Text(t) => h += &format!("<p class=note>{}</p>", esc(t)),
                Block::Table(t) => {
                    h += &extras(t);
                    // (one filter box a section, over its first table; a fixture list's chips)
                    if std::mem::take(&mut tools) {
                        h += &tools_html(t);
                    }
                    h += &table_html(t);
                }
            }
        }
        h += "</section>";
    }
    h + &format!("</main><footer>Championship Manager 01/02 &middot; exported by cmui &middot; click a header to sort, a bar to filter, a title to fold</footer><script>{JS}</script></body></html>")
}

/// The printouts of a folder as pages, to look at (`CMUI_PRINT=<folder> cargo test -- --ignored`).
#[test]
#[ignore]
fn folder_to_pages() {
    let dir = std::env::var("CMUI_PRINT").unwrap_or_else(|_| "../print".into());
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "txt") {
            std::fs::write(p.with_extension("html"), to_html(&std::fs::read(&p).unwrap())).unwrap();
        }
    }
}

#[test]
fn printout_to_page() {
    let txt = b"****************************************\r\n Feyenoord - Monday 11th August 2003\r\n****************************************\r\n\r\n\
========================================\r\n Squad\r\n========================================\r\n\r\n\
No  Name                       Position(s)  Nat  Born      Age  Caps Gls  Wages     Expires   Value\r\n\
------------------------------------------------------------\r\n\
-   Adolfo                     F C          ESP  21.5.79   24   -    -    $8K       18.6.05   $200K     \r\n\
-   Ahannach, Alami            M LC         MOR  20.9.69   33   1    -    $325K     4.6.04    $1.5M     \r\n\r\n\
========================================\r\n 2001/2 Fixtures\r\n========================================\r\n\r\n\
Date      Opposition                 Ven  Competition                     Res    Attend  Scorers\r\n\
------------------------------------------------------------\r\n\
24.7.01   Marseille             FRA   H   Friendly                        1:0    18482   Tomasson\r\n\
19.8.01   Twente                      A   Premier Division                2:0    13405   Rzasa pen, van Hooijdonk\r\n\
26.8.01   Ajax                        H   Premier Division                1:1    49115   van Hooijdonk\r\n";
    let (title, s) = parse(txt);
    assert_eq!(title, "Feyenoord - Monday 11th August 2003");
    assert_eq!(s.len(), 2);
    let Block::Table(q) = &s[0].blocks[0] else { panic!() };
    assert_eq!(q.head[6..8], ["Caps", "Gls"]);
    let Block::Table(t) = &s[1].blocks[0] else { panic!() };
    assert_eq!(t.head, ["Date", "Opposition", "Ven", "Competition", "Res", "Attend", "Scorers"]);
    let r = t.rows[0].as_ref().unwrap();
    assert_eq!((r[1].as_str(), r[2].as_str(), r[6].as_str()), ("Marseille             FRA", "H", "Tomasson"));
    assert_eq!(money("$1.5M"), Some(("$".into(), 1_500_000.0)));
    let page = to_html(txt);
    assert!(page.contains("<b>2</b><span>played</span>"), "friendlies left out of the record");
    assert!(page.contains("<b>$1.7M</b><span>squad value</span>"));
    assert!(page.contains("<span class=l>van Hooijdonk</span>"));
    assert!(page.contains("Marseille <em>FRA</em>"));
}
