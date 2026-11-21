"""Builds challenge_editor.html: an editor outside the game for CM Challenge's RAD challenges (with
category and sub-category), career goals and rule book lines (user 2026-11-21). One HTML file, no
install: open it in Chrome or Edge, open the game folder's cmui_rules.tsv (or start from the built-in
rules baked in here), edit, save. The host's game reads cmui_rules.tsv at each game load
(challenge/mod.rs RULES_FILE); prizes and ranks (P, G lines) are kept as they are.
A text the game knows (found literally in track.rs / goals.rs) is marked "game"; any other is the
host's to tick. Run again after a change to rules.tsv or the challenge code.
usage: python make_challenge_editor.py   (writes ui/challenge_editor.html)"""
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, 'src', 'challenge')


def rust_text(path):
    """A Rust source with its \\u{..} escapes turned into characters (the texts as the game has them)."""
    text = open(path, encoding='utf-8').read()
    return re.sub(r'\\u\{([0-9a-fA-F]+)\}', lambda m: chr(int(m.group(1), 16)), text)


rules = open(os.path.join(SRC, 'rules.tsv'), 'rb').read().decode('cp1252')
track, goals = rust_text(os.path.join(SRC, 'track.rs')), rust_text(os.path.join(SRC, 'goals.rs'))
known_rad = [l.split('\t')[2] for l in rules.splitlines() if l.startswith('C\t')]
known_goal = [l.split('\t')[2] for l in rules.splitlines() if l.startswith('Q\t')]
# (the rules file has the euro sign as cp1252 0x80, the Rust source as \u{80})
fix = lambda t: t.replace('\u20ac', '\x80')
followed_rad = sorted({t for t in known_rad if f'"{fix(t)}"' in track})
followed_goal = sorted({t for t in known_goal if f'"{fix(t)}"' in goals})

page = r'''<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>CM Challenge editor</title>
<style>
:root { --bg:#14181f; --card:#1d232d; --line:#2c3440; --ink:#e6e9ee; --muted:#8a94a3; --gold:#e8b63b; --cyan:#4fc3d9; --bad:#e0605a; --ok:#4cc07a; }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--ink); font:14px/1.4 "Segoe UI", system-ui, sans-serif; }
header { position:sticky; top:0; z-index:2; background:#10141a; border-bottom:1px solid var(--line); padding:10px 18px; display:flex; gap:10px; align-items:center; flex-wrap:wrap; }
h1 { font-size:17px; margin:0 14px 0 0; color:var(--gold); }
button { background:#2b4766; color:var(--ink); border:0; border-radius:6px; padding:6px 12px; font:inherit; cursor:pointer; }
button:hover { background:#3a5c82; } button.quiet { background:transparent; color:var(--muted); padding:2px 6px; } button.quiet:hover { color:var(--bad); }
#status { color:var(--muted); margin-left:auto; } #status.dirty { color:var(--gold); }
nav { display:flex; gap:4px; padding:12px 18px 0; }
nav button { background:var(--card); border-radius:6px 6px 0 0; } nav button.on { background:#2b4766; color:var(--gold); }
main { padding:0 18px 40px; }
section { display:none; background:var(--card); border-radius:0 8px 8px 8px; padding:14px; } section.on { display:block; }
.tools { display:flex; gap:10px; margin-bottom:10px; align-items:center; flex-wrap:wrap; }
input, textarea { background:#121720; color:var(--ink); border:1px solid var(--line); border-radius:5px; padding:4px 6px; font:inherit; }
input:focus, textarea:focus { outline:1px solid var(--cyan); }
table { width:100%; border-collapse:collapse; } th { text-align:left; color:var(--muted); font-weight:600; font-size:12px; padding:4px; border-bottom:1px solid var(--line); }
td { padding:3px 4px; border-bottom:1px solid #232a34; vertical-align:middle; } td input { width:100%; }
td.n input { width:64px; } td.c { width:150px; } td.s { width:150px; } td.chk { width:70px; text-align:center; } td.del { width:30px; }
tr.group td { color:var(--cyan); font-weight:600; padding-top:12px; border-bottom:1px solid var(--line); }
tr.archived td input:not([type=checkbox]) { color:var(--muted); text-decoration:line-through; }
.badge { font-size:11px; border-radius:4px; padding:1px 6px; white-space:nowrap; } .game { background:#1f4a33; color:#9ee6b8; } .host { background:#4a3b1f; color:#f1cf86; }
.note textarea { width:100%; min-height:54px; } .hint { color:var(--muted); font-size:12px; margin:6px 0 12px; }
</style></head><body>
<header>
  <h1>CM Challenge editor</h1>
  <button id="open">Open cmui_rules.tsv</button>
  <button id="builtin">Start from the built-in rules</button>
  <button id="save">Save</button>
  <button id="saveas">Save as...</button>
  <span id="status">Nothing open</span>
</header>
<nav><button data-tab="rad" class="on">RAD challenges</button><button data-tab="goals">Career goals</button><button data-tab="notes">Rule book</button></nav>
<main>
<section id="rad" class="on">
  <div class="tools"><button id="addrad">Add challenge</button><input id="radfilter" placeholder="Filter..." size="30"><span class="hint">Category and sub-category are the wheel's first two spins. "Can be drawn" off: kept for old rows, never drawn again. <span class="badge game">game</span> the game judges it; <span class="badge host">host ticks</span> a text the game does not know.</span></div>
  <table><thead><tr><th>Category</th><th>Sub-category</th><th>Challenge</th><th>Points</th><th>Can be drawn</th><th></th><th></th></tr></thead><tbody id="radrows"></tbody></table>
</section>
<section id="goals">
  <div class="tools"><button id="addgoal">Add goal</button><input id="goalfilter" placeholder="Filter..." size="30"><span class="hint">Each manager picks ten. A goal the game does not know is ticked by the host.</span></div>
  <table><thead><tr><th>Category</th><th>Goal</th><th>Points</th><th></th><th></th></tr></thead><tbody id="goalrows"></tbody></table>
</section>
<section id="notes">
  <div class="tools"><button id="addnote">Add line</button><span class="hint">Shown at the top of the Rules tab, under "How the RAD challenges count".</span></div>
  <div id="noterows"></div>
</section>
</main>
<datalist id="cats"></datalist><datalist id="subs"></datalist><datalist id="gcats"></datalist>
<script>
const BUILTIN = __BUILTIN__;
const FOLLOWED_RAD = new Set(__RAD__);
const FOLLOWED_GOAL = new Set(__GOAL__);
let keep = [], rad = [], goals = [], notes = [], handle = null, dirty = false;
const $ = id => document.getElementById(id);
// the file is Windows-1252 (the euro sign 0x80): decode and encode by its table
const dec = new TextDecoder('windows-1252');
const enc = new Map([...Array(256).keys()].map(b => [dec.decode(new Uint8Array([b])), b]));
const toBytes = s => new Uint8Array([...s].map(ch => enc.get(ch) ?? 63));

function parse(text) {
  keep = []; rad = []; goals = []; notes = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const c = line.split('\t');
    if (c[0] === 'C') rad.push({ cat: c[1] || '', text: c[2] || '', points: +c[3] || 0, archived: c[4] === '1', sub: c[5] || '' });
    else if (c[0] === 'Q') goals.push({ cat: c[1] || '', text: c[2] || '', points: +c[3] || 0 });
    else if (c[0] === 'X') notes.push(c.slice(1).join(' '));
    else keep.push(line);
  }
  render(); setDirty(false);
}
function serialise() {
  const clean = s => String(s).replace(/[\t\r\n]+/g, ' ').trim();
  const out = keep.filter(l => !l.startsWith('C\t') && !l.startsWith('Q\t') && !l.startsWith('X\t'));
  notes.filter(n => clean(n)).forEach(n => out.push('X\t' + clean(n)));
  rad.filter(r => clean(r.text)).forEach(r => out.push(['C', clean(r.cat), clean(r.text), Math.round(r.points) || 0, r.archived ? 1 : 0, clean(r.sub)].join('\t')));
  goals.filter(g => clean(g.text)).forEach(g => out.push(['Q', clean(g.cat), clean(g.text), Math.round(g.points) || 0].join('\t')));
  return out.join('\n') + '\n';
}
function setDirty(d) { dirty = d; const s = $('status'); s.className = d ? 'dirty' : ''; s.textContent = (handle ? handle.name : (keep.length ? 'built-in rules (not saved yet)' : 'Nothing open')) + (d ? ' - unsaved changes' : ''); }
function lists() {
  const fill = (id, vals) => $(id).innerHTML = [...new Set(vals.filter(Boolean))].sort().map(v => `<option value="${esc(v)}">`).join('');
  fill('cats', rad.map(r => r.cat)); fill('subs', rad.map(r => r.sub)); fill('gcats', goals.map(g => g.cat));
}
const esc = s => String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
function input(obj, key, attrs = '') {
  const el = document.createElement('input'); el.value = obj[key];
  if (attrs) attrs.split(' ').forEach(a => { const [k, v] = a.split('='); el.setAttribute(k, v ?? ''); });
  el.oninput = () => { obj[key] = el.type === 'number' ? +el.value : el.value; setDirty(true); if (key !== 'points') { lists(); badge(el); } };
  return el;
}
function badge(el) { const tr = el.closest('tr'); if (tr && tr._badge) tr._badge(); }
function render() {
  lists();
  const f = $('radfilter').value.toLowerCase(), body = $('radrows'); body.innerHTML = '';
  const order = rad.map((r, i) => i).sort((a, b) => (rad[a].cat + '\u0001' + rad[a].sub).localeCompare(rad[b].cat + '\u0001' + rad[b].sub) || a - b);
  let group = null;
  for (const i of order) {
    const r = rad[i];
    if (f && !(r.cat + ' ' + r.sub + ' ' + r.text).toLowerCase().includes(f)) continue;
    const g = r.cat + ' / ' + (r.sub || '-');
    if (g !== group) { group = g; const tr = body.insertRow(); tr.className = 'group'; const td = tr.insertCell(); td.colSpan = 7; td.textContent = g; }
    const tr = body.insertRow(); if (r.archived) tr.className = 'archived';
    tr.insertCell().className = 'c'; tr.cells[0].append(input(r, 'cat', 'list=cats'));
    tr.insertCell().className = 's'; tr.cells[1].append(input(r, 'sub', 'list=subs'));
    tr.insertCell().append(input(r, 'text'));
    tr.insertCell().className = 'n'; tr.cells[3].append(input(r, 'points', 'type=number min=-50 max=200'));
    const chk = document.createElement('input'); chk.type = 'checkbox'; chk.checked = !r.archived;
    chk.onchange = () => { r.archived = !chk.checked; tr.className = r.archived ? 'archived' : ''; setDirty(true); };
    tr.insertCell().className = 'chk'; tr.cells[4].append(chk);
    const b = tr.insertCell(); tr._badge = () => b.innerHTML = FOLLOWED_RAD.has(r.text) ? '<span class="badge game">game</span>' : '<span class="badge host">host ticks</span>'; tr._badge();
    const del = document.createElement('button'); del.className = 'quiet'; del.textContent = '\u2715'; del.title = 'Delete (old rows with it become the host\'s)';
    del.onclick = () => { if (confirm('Delete "' + r.text + '"? Old rows with it become the host\'s to tick. Unticking "Can be drawn" keeps them judged.')) { rad.splice(i, 1); setDirty(true); render(); } };
    tr.insertCell().className = 'del'; tr.cells[6].append(del);
  }
  const gf = $('goalfilter').value.toLowerCase(), gb = $('goalrows'); gb.innerHTML = '';
  goals.forEach((g, i) => {
    if (gf && !(g.cat + ' ' + g.text).toLowerCase().includes(gf)) return;
    const tr = gb.insertRow();
    tr.insertCell().className = 'c'; tr.cells[0].append(input(g, 'cat', 'list=gcats'));
    tr.insertCell().append(input(g, 'text'));
    tr.insertCell().className = 'n'; tr.cells[2].append(input(g, 'points', 'type=number min=0 max=1000'));
    const b = tr.insertCell(); tr._badge = () => b.innerHTML = FOLLOWED_GOAL.has(g.text) ? '<span class="badge game">game</span>' : '<span class="badge host">host ticks</span>'; tr._badge();
    const del = document.createElement('button'); del.className = 'quiet'; del.textContent = '\u2715';
    del.onclick = () => { if (confirm('Delete "' + g.text + '"? Managers who picked it lose it.')) { goals.splice(i, 1); setDirty(true); render(); } };
    tr.insertCell().className = 'del'; tr.cells[4].append(del);
  });
  const nb = $('noterows'); nb.innerHTML = '';
  notes.forEach((n, i) => {
    const d = document.createElement('div'); d.className = 'note';
    const t = document.createElement('textarea'); t.value = n; t.oninput = () => { notes[i] = t.value; setDirty(true); };
    const del = document.createElement('button'); del.className = 'quiet'; del.textContent = '\u2715 remove line';
    del.onclick = () => { notes.splice(i, 1); setDirty(true); render(); };
    d.append(t, del); nb.append(d);
  });
}
async function writeTo(h) {
  const w = await h.createWritable(); await w.write(toBytes(serialise())); await w.close();
  handle = h; setDirty(false);
}
function download() {
  const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([toBytes(serialise())])); a.download = 'cmui_rules.tsv'; a.click();
  setDirty(false); $('status').textContent = 'Downloaded cmui_rules.tsv: put it in the game folder (next to cm0102_ui.exe)';
}
const picker = { types: [{ description: 'CM Challenge rules', accept: { 'text/tab-separated-values': ['.tsv'] } }], suggestedName: 'cmui_rules.tsv' };
$('open').onclick = async () => {
  if (dirty && !confirm('Unsaved changes: open another file anyway?')) return;
  if (window.showOpenFilePicker) {
    try { const [h] = await showOpenFilePicker(picker); parse(dec.decode(await (await h.getFile()).arrayBuffer())); handle = h; setDirty(false); } catch (e) { if (e.name !== 'AbortError') alert(e); }
  } else {
    const i = document.createElement('input'); i.type = 'file'; i.accept = '.tsv';
    i.onchange = async () => { parse(dec.decode(await i.files[0].arrayBuffer())); handle = null; setDirty(false); }; i.click();
  }
};
$('builtin').onclick = () => { if (dirty && !confirm('Unsaved changes: replace them with the built-in rules?')) return; handle = null; parse(BUILTIN); setDirty(true); };
$('save').onclick = async () => { if (!keep.length && !rad.length) return; try { if (handle) await writeTo(handle); else $('saveas').onclick(); } catch (e) { alert(e); } };
$('saveas').onclick = async () => {
  if (window.showSaveFilePicker) { try { await writeTo(await showSaveFilePicker(picker)); } catch (e) { if (e.name !== 'AbortError') alert(e); } }
  else download();
};
$('addrad').onclick = () => { const last = rad[rad.length - 1] || {}; rad.push({ cat: last.cat || '', sub: last.sub || '', text: 'New challenge', points: 3, archived: false }); setDirty(true); render(); };
$('addgoal').onclick = () => { goals.push({ cat: (goals[goals.length - 1] || {}).cat || '', text: 'New goal', points: 25 }); setDirty(true); render(); };
$('addnote').onclick = () => { notes.push(''); setDirty(true); render(); };
$('radfilter').oninput = render; $('goalfilter').oninput = render;
document.querySelectorAll('nav button').forEach(b => b.onclick = () => {
  document.querySelectorAll('nav button, section').forEach(x => x.classList.remove('on'));
  b.classList.add('on'); $(b.dataset.tab).classList.add('on');
});
window.onbeforeunload = e => { if (dirty) { e.preventDefault(); e.returnValue = ''; } };
document.addEventListener('keydown', e => { if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); $('save').onclick(); } });
</script></body></html>
'''
page = (page.replace('__BUILTIN__', json.dumps(rules))
        .replace('__RAD__', json.dumps(followed_rad)).replace('__GOAL__', json.dumps(followed_goal)))
out = os.path.join(HERE, 'challenge_editor.html')
open(out, 'w', encoding='utf-8').write(page)
print(out, len(page) // 1024, 'kB;', len(followed_rad), 'of', len(known_rad), 'RAD and', len(followed_goal), 'of', len(known_goal), 'goals followed')
