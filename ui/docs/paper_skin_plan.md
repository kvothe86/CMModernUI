# Paper skin for cmui (plan only, 2026-11-22)

Goal: a second look for cmui modelled on the Jonukas FM27 skin's *backgrounds*: screens as
sheets of light paper (cream, dark ink), optionally on a desk. Every cmui switch and drawing
(bars, crests, pills, stars, cards, dials) stays as it is; only the colours it draws with and
what lies behind them change. Chosen with a skin switcher.

## Status (2026-11-22): built, tried, undone

**Undone on the user's word the same day**: source back to HEAD, `skin.rs` deleted, E:\CM0102
back on its old DLL, config tool and ini. What was built, kept here in case it comes back:

Built as a **display filter**, not as the colour map at `rgb()` described below. Reading the code
showed 18 `pixel_at` read-backs that fill with what they read, colour tests on the game's text
colours and menu restores from saved pictures. A map at the sinks would have mapped those a
second time. So `src\skin.rs` hooks the entry of `graphics_flip_rect` 0x5e2eb0, which every
present goes through, partial ones included. It saves the rectangle, sends it through a 64K table
built from `paper()`, lets the game copy it to the screen, then puts the dark pixels back. cmui
keeps drawing and testing in dark; only what is shown is paper.

- Protected (shown as drawn): crests and flags (`blend_picture`), the tactics pitch (`PITCH`), a
  title band in a club's or competition's own colours (not a theme slot colour). Emptied at each
  full frame (`background`).
- `skin=dark|paper` in `cmui.ini`, read at start and every half second (live switch + `REDRAW`);
  `cmui_config.ps1` has a Skin dropdown. Dark (or no line): the hook passes straight on.
- `paper()`: greys round a curve (luma 0x1a -> 0xf7, 0x2b -> 0xee, 0x3a -> 0xe4, 0x5a -> 0xcf,
  0x80 -> 0xa0, 0xb8 -> 0x50, 0xd8 -> 0x30) warmed by g x0.96, b x0.87; dark colours (luma under
  0x58) the curve's level plus 45 % of their chroma; bright ones luma 0x30 + 45 % of (luma - 0x58),
  chroma kept. The hook's prologue to move: `a1 0c be ae 00`.
- What the filter cannot reach: the game's own pictures (match screen parts, bitmaps) are turned
  round like everything else unless protected; `cmui.shot` saves the dark back buffer.
- Steps 2-5 below stand (crawl, grain, Dark Paper, Wood). Grain fits in the same table pass.

## What the reference does (fmportugal.net, Jonukas Skin FM27)

Four looks, picked in Preferences: **Wood** (light sheets on a wooden desk), **Paper** (plain
light paper, brighter), **Dark** (dark, coloured numbers pop), **Dark Paper** (dark paper,
"magazine at night"). Panels are separate sheets; small status boxes (happiness, fitness,
form) look like cuttings; tables sit on sheets with dark numbers; attributes have hand-marker
colour marks behind them. The page has no hex values or usable screenshots (lazy-loaded), so
the colours below are mine, to be tuned on screen.

Map to cmui: Dark = today's theme. Paper, Dark Paper and Wood are new.

## How cmui colours work today (what makes this cheap or not)

| Where | What | Count |
|---|---|---|
| `THEME` table, `lib.rs` ~198 | 11 game palette slots overwritten by `apply_palette` (the game's own panels, title bars, text) | 11 |
| `BACKGROUND`, `SIDEBAR`, `AREA`, `AZURE_*` consts | flat screen fill, left bar, grid areas | ~6 |
| colour literals `[0x.., 0x.., 0x..]` across 15 modules | cmui's own drawing | ~390 uses, 191 distinct colours |
| sinks they pass through | `rgb()` (59 calls), `rgb565()` (25), `pack()` (20), GDI `SetTextColor` (2) | 4 functions |
| game colour detection | `is_colour` / `px565` compare the game's *input* colours | 15 sites |
| erase by flat fill | `fill(.., BACKGROUND/AREA/SIDEBAR)` | ~51 sites |

The key finding: nearly every colour cmui puts on screen goes through 4 functions. So a skin is
**a colour map at those sinks**, not an edit of 390 literals.

## Design

1. **`skin` setting**: `skin=dark|paper|dark_paper|wood` in `cmui.ini`, default `dark` (no
   change for anyone who does nothing). A setting, not one of the 40 on/off switches: the
   switch table is bits and a skin has four values. Read with the half-second ini poll that
   `background_opacity` already uses, so it can change while the game runs (re-run
   `apply_palette`, rebuild the look-up table, `REDRAW`).
2. **`ink(c) -> c`**: one function called in `rgb()`, `rgb565()`, theme `pack()` and
   `SetTextColor`. For `dark`, it returns `c` unchanged (zero risk to today's look). For a paper skin:
   - low-saturation colours (the greys of panels, text, lines) get their lightness turned
     around onto a paper ramp: dark background → cream `#efe6d2`, panel → sheet `#f7f1e3`,
     light text `#d8dce2` → ink `#2b2620`, mid greys → warm greys;
   - saturated accents (gold selection, red/green form, attribute colour scale, pills) keep
     their hue and get darker by about 25-35 %, so they read on cream;
   - a short exceptions table (`[(from, to)]`) for the cases the rule gets wrong, filled while
     testing. Expect 20-60 entries.
   - `px565` / `is_colour` (game input detection) are **not** mapped: they read the game's
     colours, not cmui's.
   - crests, flags and pictures never go through `ink` (they use `pack` on picture pixels: that
     path needs a flag so only theme colours are mapped).
3. **The game's own text**: most comes through the palette slots (mapped by 1). Text the game
   draws in a fixed white/near-white goes through `text_box_hooked`, where the colour
   argument is visible: one rule there (paper skin and near-white → ink) catches all of it
   without a per-screen crawl.
4. **Paper feel without breaking erases**: the ~51 erase-by-fill sites must stay flat fills. So
   texture is added at the flip pass, the way `background_show` already does: every pixel of
   exactly the background colour is swapped for a texture pixel (`THROUGH`-style look-up, one
   full-frame pass, already measured as fine). The texture is a procedural grain (hash noise,
   ±2-3 levels in 565, no file). Sheets: panel fill stays flat sheet colour plus a 1 px darker
   edge and a 2-3 px soft shadow bottom-right (`panel` hook, sizes via `sx`/`sy`).
   Known limit: a list redrawn alone (a scroll) has no flip pass, so it shows flat until the next
   full frame. Same as today's `background_show`.
5. **Wood**: same swap, but the background pixels take a desk picture (`cmui_wood.bmp`, tiled,
   shipped with the crests). The sheets stay opaque. This is the only skin that needs a file.
6. **Dark Paper**: the paper rule with a dark warm ramp (`#2a2621` desk, `#34302a` sheet, light
   warm ink) plus the grain. It costs almost nothing once Paper works.

### Skin switcher

- **`cmui_config.ps1`**: a "Skin" ComboBox next to the opacity slider (the Coach model box is
  already a ComboBox to copy), saved as `skin=`. About 15 lines. This is "the tool".
- **In the game** (optional): a key or a row by Game Options that cycles the skin and writes it
  with `set_setting`. Takes effect live through the ini poll.
- **CM0102Patcher misc patches**: leave out for now. If wanted later, it is one byte after the
  switch bytes (`make_patches.py`), with 0-3 for the skin.
- Network: the skin is local drawing only. Host and client each read their own ini; no packets
  change.

## How big this is

| Step | Work | Size | Risk |
|---|---|---|---|
| 0 Plumbing | `skin` setting, `ink()` in the 4 sinks, the `pack` picture flag, live reload, config ComboBox | ~120 lines, `lib.rs` + `cmui_config.ps1` | low: `dark` is unchanged |
| 1 Flat Paper | paper ramp + accent darkening, THEME slots, BACKGROUND/SIDEBAR/AREA, the near-white text rule | ~80 lines | **medium-high: this is where the work is**, see risks |
| 2 Tuning crawl | every screen at 1680, 1280, 800 and on a client; exceptions table filled | 1 night crawl (night 6 style) + 1 session fixes | medium |
| 3 Paper feel | grain swap at flip, sheet edge + shadow in `panel` | ~100 lines | low-medium (frame time: measure with `cmui.timing`) |
| 4 Dark Paper | second ramp | ~20 lines + short crawl | low |
| 5 Wood | desk picture swap + bmp | ~40 lines + picture | low |

Total: about 350-400 lines, nearly all in `lib.rs`, plus the config tool. Roughly **3 working
sessions plus one night crawl**. Steps 0-2 give a usable flat paper skin. 3-5 can each be
left out.

## Risks (what the crawl must check)

1. **Readability of colour codes**: attribute scale, form W/D/L, status badges (21 codes), news
   chips, CM Challenge wheels were all picked against dark. The darkening rule handles most;
   the rest go in the exceptions table.
2. **Game pictures with dark assumptions**: button bitmaps, match screen parts, the game's own
   icons. `modern_buttons` / `flat_buttons` already draw most buttons flat; the rest will show.
   Without `modern_buttons`, paper will look half done; say so in the tool's tooltip.
3. **Exact-colour logic**: `background_show`'s swap and some text-hook checks match pixels to
   `rgb(BACKGROUND)`. They stay right as long as both sides go through the mapped `rgb()`. Read
   the 15 `is_colour` / `px565` sites once in step 0.
4. **Antialiased text** (GDI icons, `bigger_text`) blends with the real pixels, so it is fine on
   cream. The game's bitmap font in dark ink still needs a look at 800x600.
5. **`background_show` with Background Changes on**: on paper the picture through the sheets
   must be blended toward cream, not toward `AREA`. `keep_picture` reads `AREA`, which `ink`
   maps, so it should follow without changes. Check it.

Out of scope: the HTML export dashboard (`dash.html`, its own CSS; a paper stylesheet would
be a separate small job), dials and graphics, layout changes.

## Order if it goes ahead

Step 0 + 1 as one build → screenshots of 10 key screens at 1680 (squad, profile, tactics,
match, news, table, finances, compare, Challenge, menus) → the user's look → crawl → 3 → 4/5.
Every install also goes to main `E:\CM0102` (hard rule), never on a running game.
