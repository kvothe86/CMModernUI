# Stadium page (switch `stadium`, index 40)

A Stadium button on the club's Finances view opens the ground (own club; a network client asks the
host, key `!`). Built 2026-11-23 like USM 2's builder; restyled the same night after the user's
mock picture (generated, not in the repository; its left menu ignored).

## The page (`stadium.rs` `flip`)

- Header: crest, club, ground, Back. From `bar_end` on (cmui's menu bar stays).
- Left (46% of the width): the rendered ground (below). Each stand's places on a dark label,
  the crest on the centre spot, the club's name and the ground's on the two long roofs when the
  ground has a roof, a panel "Under construction" with a progress bar per work.
- Right: THE GROUND (capacity, seated, standing, room to grow, average crowd, bank), ORDER WORKS
  (seats / standing / convert tiles with - and +, 1,000..10,000, or what room is left; roof and
  heating; first click arms (gold), second orders), the key.
- "Now | Fully built" pills over the picture: the ground at its room to grow, all seated, roofed.
- **New stadium** (job 6, `NEW`; user 2026-11-23): once the ground is at its room to grow, a wide
  tile replaces seats / standing. Sizes 15,000 .. 125,000 above the ground (- / +); price 25 million
  + 2,500 a place (game units), a year + 10 days a thousand. First click: type the name (Enter or a
  second click orders, Escape cancels; the keys through our window proc). 30% at the order (must be
  in the bank), the rest in monthly instalments into debt if need be (`instalments`, diary `Q`
  lines; the name in `N` lines). Every other work waits. Done (`complete`): capacity = seats =
  size, room to grow a quarter more, roof, heating, the name written into the game's stadium record
  (+4): the game's own General Info shows it ("60000 all seater with retractable roof and
  under-soil heating"). "Fully built" shows the new one with its name and room. Test command:
  `money n` in cmui.click (the human club's balance).
- The crest lies flat on the centre spot (`flat_crest`, the pack's pitch axes); the bank balance
  has a green / red arrow against the last month's end (the diary's `B` lines, `Ground.last`).
- Tiles light up under the mouse; refusals short ("NOT ENOUGH MONEY").
- **Club shop** (`shop.rs`, user 2026-11-23): the order card's pills WORKS | CLUB SHOP. The shop
  now (level, % more merchandise this week, merchandising this season), its next level (Club shop
  +30%, Megastore +45%, Megastore and webshop +60%; 1.5 / 4 / 2 million, 120 / 180 / 90 days; job
  `SHOP`, amount = level) and four items bought at once (`ITEM`: away shirt +8%, scarves and caps
  +4%, name and number +8%, kids' kits +6%). Kept in the diary (`M` lines: seed, level, item bits),
  not in the stadium record: a new stadium keeps it. Money: a hook on the game's Sunday
  merchandise (0x59b7e0, thiscall(table, club)) adds the share to the same line (finance record
  +0xbc month, +0x15c year, the bank), plus the week's results (+3 a win, +1 a draw, -2 a defeat,
  at most 8). Computer clubs by reputation (+0x80): 6,000+ a club shop, 3,000+ away shirt and
  scarves. The building is the pack's `shop_<size>` layer, `mega_<size>` from level 2 (faint while
  built). The card (after the user's and Grok's notes): the building cut from the pack, LEVEL n OF
  3, status, WEEKLY BOOST split into BASE (shop + items) and FORM (the week's results), the next
  upgrade at the right; the items two by two, ON SALE / AVAILABLE (unlock price) / LOCKED (name and
  number, kids' kits: "Needs a club shop"). 800x600: no picture or tags, shorter words.
- A new stadium finished sooner than paid (a test's one day) pays the instalments still owed at
  once (`complete`).
- Test: cmui.ini `stadium_test_days=1` makes every work take one day; `money n` in cmui.click.
- `fit()` picks the largest font that fits; shorter words below 1280x800. Seen at 1680x1050,
  1280x800 and 800x600.
- The background picture pass (`let_picture_through`) skips the page.

## The rendered ground (`ui\stadium3d`)

`build_stadium.py` (Blender 5.x, no window: `blender -b --factory-startup -P build_stadium.py --
<out>`; env `STADIUM_ONLY=m` for one size) builds the ground in four sizes (`s` < 12,000, `m` <
30,000, `l` < 60,000 two tiers, `x` above), each from one orthographic 3/4 camera, 780x1080:

| Picture | Engine | What |
|---|---|---|
| `base_<size>` | Eevee | stands (seats white), pitch, corners, plaza, roads, car park, trees, floodlights |
| `id_<size>` | Workbench, flat, no smoothing | per pixel: R class (0 as drawn, 1/2 seat colour A/B, 3 club-colour trim) x80, G stand (N S W E) x80, B segment 0..15 x16 |
| `roof_<size>` | Eevee, the rest held out | the roofs (drawn over when the ground has one; faint while one is built) |
| `works_<size>` | Eevee, the rest held out | crane and scaffolding (while places or a roof are built) |
| `top_<size>` | Eevee, straight down | the clean top-down view of the brief (not in the pack, not used by the page) |

Materials: plain Principled BSDF; all but the seats, the club-colour trim and the lines get a
noise on colour and roughness (not plastic). Light: one sun, a spot light from each floodlight.

The pack `cmui_stadium.bin` ('CMS2'): count, 64-byte entries (name, frame size, crop x y w h,
offset, eight points: pitch centre, the four stands' middles, centre circle radius; a roof
entry's points are its two long roofs' top left / top right / bottom left), then each picture's
4-byte pixels (RGB565, alpha, id) as runs (count, pixel). About 9 MB. `install_all.ps1` copies
`stadium3d\out\cmui_stadium.bin` to every folder's `Data\`. Without it the page draws the old flat
plan (`flat`).

The DLL (`compose`, cached per size of the page, colours, capacity, roof, works): the seats in
the club's first two kit colours as a mosaic (`mosaic`: alike colours get a darker or lighter
second), by the light they were rendered in; standing segments grey by the ground's own share
(`standing`: the ends' middles first, then the long sides'); the land fades into the page (the
mock's island).

`preview.py <out> <size> [roof] [works] [seated]`: the same look without the game.
`rle_measure.py`: what the runs save.
