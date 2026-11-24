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
  size, roof, heating, the name written into the game's stadium record (+4): the game's own General
  Info shows it ("60000 all seater with retractable roof and under-soil heating"). "Fully built"
  shows the new one with its name and room. Test commands in cmui.click: `money n` (the human
  club's balance), `places n` (its ground n places, a quarter seated, full), `roof 0|1`; the last
  two in memory only, never save after them.
- **Room to grow of a new stadium** (`new_room`, user 2026-11-24: a level 7 club of 1,000 places
  had to build five new stadiums): by the old ground's room, not the size picked: under 15,000 ->
  30,000 (professional), under 50,000 -> 70,000 (big club), else 150,000 (`MOST`, was 125,000);
  never under a quarter over its seats. Once the ground is full the info line says what comes
  next (`full_note`): "Full. A new stadium: 15,000 seats, room to grow to 30,000. Its next one:
  room to 70,000, then 150,000."
- Roof words: only the ground's name, on the near roof (the far roof's club name went, a friend
  2026-11-24: the crest on the pitch says it); on the amateur ground the stand's roof.
- Mowing (`Mow`): the pack's pitch is one plain green with its own id (class 0, stand 3, seg 15 =
  `PITCH_ID`); the DLL lays one of six patterns over it by the club's name (stripes across, checks,
  diagonal, diamonds, rings, stripes along) through the pitch axes (pack points 0/6/7).
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
<out>`; env `STADIUM_ONLY=m` for one size; the whole pack about 70 s) builds the ground in five
sizes (`a` amateur < 3,000, `s` < 12,000, `m` < 30,000, `l` < 60,000 two tiers, `x` above), each
from one orthographic 3/4 camera, 780x1080.

Amateur `a` (2026-11-24, after the user's reference picture, `sides_for` / `amateur_ground`): a
46 m covered stand with six rows of seats on the far side (roof on front posts, `RH` 2.6), open
concrete terraces with crush barriers on the near side and the east end, the west end open, a
concrete walkway behind a low wall round the pitch, slim floodlight masts, a palisade fence, a gate
in brick pillars, a brick clubhouse with a slate roof at the south-east, the ground in grass. On
the page its stand always seats (labels: seats on the stand, standing on the terrace).

The town (2026-11-24, after a friend's asset pictures; the stadium itself unchanged, checked by a
pixel diff of old and new renders): kerbs and white edge lines on the ring road, kerbs, edge lines
and pavements on the main road, kerbed car parks with paved edges and hatchback cars, leafy trees
(14 shapes, `tree_shapes`, placed as linked copies: one mesh of a thousand trees took minutes),
terraced brick / rendered houses with gables, chimneys, front gardens and railings
(`terrace_row`), food vans in club colours on paved pads instead of kiosks and trucks
(`food_van`), training pitches with cut corners, goals with nets, penalty boxes, slalom poles and
small dummies; the megastore 33 m wide (was 24).

| Picture | Engine | What |
|---|---|---|
| `base_<size>` | Eevee | stands (seats white, vomitories, entrance with club-colour canopy), striped pitch, corners, a dark paved octagonal plinth with kerb and trees, lattice floodlights |
| `id_<size>` | Workbench, flat, no smoothing | per pixel: R class (0 as drawn, 1/2 seat colour A/B, 3 club-colour trim) x80, G stand (N S W E) x80, B segment 0..15 x16 |
| `roof_<size>` | Eevee, the rest held out | the roofs (drawn over when the ground has one; faint while one is built) |
| `works_<size>` | Eevee, the rest held out | crane and scaffolding (while places or a roof are built) |
| `top_<size>` | Eevee, straight down | the clean top-down view of the brief (not in the pack, not used by the page) |
| `shop_` / `mega_<size>` + `idshop_` / `idmega_` | Eevee held out + Workbench ids | club shop / megastore beside the south stand's west end (after `ref\megastore.webp`): pillars, sign band, roof panel in club colour A (class 3), the name in colour B (class 2) |

Pictures are 1560x1080 since 2026-11-23: the page's column cuts the middle 780 as before, FULL
SCREEN (`WIDE`, a pill at the picture's top left) hides the cards and shows the whole frame. The
town round the plinth (`town`: grass, ring road, car parks, kiosks, fan zone with food trucks,
beer tent, parasols and fans, houses, training pitches) grows with the size (`LEVEL`: s one car
park and two kiosks, x the biggest; a friend's idea), and size x has seated, roofed corners
(`corners`) with the floodlights on the corner roofs. Club colours: class 3 (trim, fans, the
shop's band) takes the game's second colour (the club's main one, Chelsea blue), the shop's name
class 1; the item pictures likewise. Test setting `stadium_test_club=<name>` in cmui.ini: the
Stadium button opens that club's ground (computer clubs have no Finances screen); big computer
clubs (reputation 8,500+) have the full store.

The look follows `stadium3d\ref\stadium.webp` (user 2026-11-23: cleaner, modern, darker set, lit
pitch); the camera and frame are unchanged. `build_merch.py` (same Blender call, `-- <out>`, env
`MERCH_ONLY`) renders the shop's item icons after `ref\merch.webp` / `ref\lock.png` into
`stadium3d\merch\`: shirt, scarf, mug, jacket (green and white, 512x512, see-through) and lock
(white glyph). Not used by the DLL yet.

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

`preview.py <out> <size> [roof] [works] [shop|mega] [red] [seated]`: the same look without the
game (`red`: red and white, the reference's club).
`rle_measure.py`: what the runs save.
