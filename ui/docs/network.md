# cmui in a network game

Background and plan: `..\NETWORK_PLAN.md`. Code: `src\remote.rs` (transport), `src\prefetch.rs` (answers sent with the screen) and, in `lib.rs`,
`host_answer`, `served_node`, `ask_host`, `screen_sent`, `display_reset`, `client_loop`, `send_click`,
`client_rects_as_sent`, `contacting_box_lower`; `news.rs` / `comp.rs` for the block.

## Model

The host runs the game and builds every client's screens; the client is a terminal. cmui on the
client has no game data, so reads go through `readable` / `rd` / `data_bytes` / `game_text`, which
in a client (`remote::CLIENT`, set while `ScreenManager::run_network_client` 0x7fdc00 runs, via
`client_loop`) ask the host. Packets use the game's own connection (`Network::send_packet` 0x77ac30)
with opcode `0x7c`, taken at the entry of `Display::process_command_packet` 0x564080 (`packet`
hook, `remote::received`). Client: `remote::get` marks a key wanted, `remote::ask()` (end of
`flip_hooked`) sends it as `[0x7c,'q']`; host `host_answer(channel, key)` replies `[0x7c,'a']`; answers
cached 2 s, a change sets `ARRIVED` and `poll_input` redraws. `cmui.noremote` in the game folder
silences the client's questions (test switch).

## Keys (`host_answer`, key = letter + u32 args little-endian)

| Key | Asks | Answer / use |
|---|---|---|
| `R` addr | 256-byte memory chunk | `remote::chunk`, behind `rd` etc. |
| `P` n | screen parameter n of the client's channel | only for the channel being served and with a screen (else the screen manager stops the game) |
| `C` name | club by name | pointer |
| `G` / `M` name | staff by row name / compare pair | `staff_by_row_name`, `compare_players` |
| `N` staff | national team? | `national_team` |
| `F` staff | condition | `condition` |
| `S` staff, cat | season stat | `season_stat` |
| `W` / `V` staff | wage / value text | `money_texts` |
| `A` staff | 31 attributes | `attributes` |
| `O` staff | positions text | `positions_text` |
| `Y` text | a story's crest from its links | `story_link_crest` |
| `L` | link list of the story on the channel's display | `news::link_list` (old route, now `story_links` on a client) |
| `Z` kind, object | a link's name + crest key | `news::link_names` |
| `X` cell | open the story link in that cell, by the click that follows | sets `news::PENDING_CELL`; `news_input` calls `menubar_handle_object_link` 0x7643d0 |
| `K` club | open a club (news club row, chips, word-found clubs) | sets `PENDING_CLUB`; `news_input` calls `open_club_squad_screen` 0x453d60, returns -4 |
| `Q` | Mark all read | sets `news::PENDING_MARK` |
| `H` words | Assistant pick request (mode, number, unavailable players) | `assistant::remote_request` |
| `J` n | the Assistant's note | `assistant::remote_note` |
| `T` n, count, players | the Assistant's team report (n: a new one each time, unavailable players as `H`) | `assistant::remote_report`, text; only while `served_node(channel)`, else the client asks again |
| `I` staff | a player's injuries, bans and moves | `history::rows`; text, a row a line |
| `D` staff, staff | what the channel's club's staff say on a compare pair | `assistant::staff_views`; text, a line each |
| `B` n, club | the scout's report on a club for the channel's manager (n: a new one each time) | `assistant::remote_scout`, as `T` but reads no screen parameter |
| `U` club | a club's three best by the intrinsic score | `scout::best_three`, staff pointers |
| `b` (not a key) | packet `[0x7c,'b', block]`, host to client, unasked | `remote::BLOCK` |

Host-side checks: staff pointers must lie in the staff table (`valid_staff`), mapped memory only.

## The cmui block ('b')

Sent by the host right after each client screen: `screen_sent` hooks the call at 0x7fca5c in
`SCREEN_MANAGER::monitor` (wraps `Display::log_screen_sent` 0x5622a0), finds the channel whose
display it is, and sends `[0x7c,'b'] + news::screen_block(display) + comp::history_block()`
(38,000 bytes at most, `cmui.trace` logs "cmui block to channel ..."). The client stores it in
`remote::BLOCK` and clears it in `display_reset` (a new screen; also `remote::new_screen` ages
all cached answers).

| Section | Writer (host) | Reader (client) | Content |
|---|---|---|---|
| Links | `news::screen_block` | `news::block_links`, `block_words` | u16 count; per link: kind u16, object u32, cell u16, text, name, crest key (length-byte strings). Menu bar's own-text items left out. |
| History | `comp::history_block` | `comp::titles` via `news::block_tail` | u16 count; per winner row: season i16, winner u32, titles u16. Empty unless screen param 0 is a competition. |

`block_words` places each link on the story's words (by name, key, or shorter parts, last item
first); stories the game links nothing in fall back to `word_links` (clubs open by `K`).

## Prefetch with the screen

`src\prefetch.rs` (host) and `remote.rs` (client). The host sends a client, with each screen, the
answers its cmui asked for the last times it was on that kind of screen, so the first frame of
squad, tactics, live tactics, profile and search needs no round trip. `remote.rs`'s questions stay
as the fallback for whatever the prefetch missed or the cap cut.

**Learning (host, `prefetch::asked`, called for every key of a 'q' packet before `host_answer`).**
A screen kind is the input handler of the channel's screen node (`[SCREEN_MANAGER + 0x300 *
channel + 0x7e2] + 8`: club 0x4671f0, tactics 0x89f710, ...), per channel. Each question is kept
for the kind of the last screen sent to that channel, as a template where it is about a staff
member of that screen, else as it was:

| Template | From a question | Sent for the next screen of the kind |
|---|---|---|
| Staff(letter, rest) | `F` / `O` / `A` / `W` / `V` / `S field` of a screen staff member | the same key for every screen staff member |
| Name(k) | `G` with a screen staff member's row name k (`row_names`) | `G` with each one's name k |
| Record | `R` chunk overlapping a screen staff member's record (0x6e bytes) | the chunks of every one's record |
| Ptr(o, d) | `R` chunk d after the one his field at +o points to (any byte offset; the nearest d only) | that chunk for every one (names +4/+8/+0xc, club +0x39, player +0x61 ...) |
| Exact | anything else (`P` screen parameters, globals, a club's chunks not reached by a field) | the key itself; 800 per kind, the longest-unasked go |

Action and news keys (`X Q H K J L Y Z T`) are never kept. Templates count their hits (spurious
pointer matches stay low).

**Screen staff** (`prefetch::screen_staff`): the staff objects of the display's items (+0x48),
screen parameter 0 when it is a staff member (profile), the squad of the club in parameter 0
(club +0xd7, 50; squad / club screens), and on in-match tactics (kind 0x89f710, parameter 10 = 6)
the engine's player rows (+0x69). At most 160.

**Sending (host, `prefetch::send`, in `screen_sent` before the block).** Only while the screen
manager serves that channel and it has a screen (`served_node`, the old 'P' guard), so `P` answers
are that channel's. Keys by priority: `P`; lookups (Staff, Name, exact); memory: the live engine's
rows (`E + 0x4796`, 40 x 0x1be), the tactics screen's copy (`T + 0x2735`, 20 x 0x1be), `T`, `E +
0x8eb3..`; then Record, Ptr by hits, exact `R` newest first. Answers the host has nothing for are
left out (the client asks; not cached as "none"). Cap `MAX` 160,000 bytes a screen (48,000 until
2026-10-30); four or five 'a' packets under `PACKET_MAX` 40,000 (`remote::send_answers`, the format
of an asked answer), all before the block. The CM Network wsock32.dll zstd-packs the wire (~10x). `cmui.trace`: "prefetch to
channel c: n keys, b bytes (screen kind, staff, templates)".

**Preset (host, `cmui.prefetch` in the game folder).** What is learned outlives the session and
the channel: a kind new on a channel starts with the preset's templates for that kind id (screens
0, never asked, the saved hits), so even the first visit in a new session gets a prefetch. Kept:
Staff, Name, Record, Ptr, and the portable Exact keys (`prefetch::portable`): `P`, `G` (a row
name) and `R` chunks in the exe image 0x400000..0xe00000 (the channels' displays and other
globals, the same address every session); other exact keys are this session's heap. Read once at the first prefetch ("prefetch preset: n templates for k screen kinds" in
cmui.log); every 20 screens sent the host writes the preset and all channels' templates merged
(hits the max, Ptr templates only with 2 hits or more, 1200 a kind, most hits first) to a temp file, then renames it
(`cmui.trace`: "prefetch preset saved: n templates"). One line a template, tab-separated: kind
hex, type (`E` exact key, `S` staff letter + rest, `G` name k, `R` record, `P` o d), fields as
hex bytes, hits; e.g. `89f710 P 3901 14` (tabs). Bad lines are skipped; delete the file to start
cold. Night 3 run C: the preset helps (profile 9,043 -> 7,254 B out on a first visit, tactics 8,769
-> 3,598) but a kind learns 500-690 templates and only ~10 are portable: the rest are exact keys.
`cmui.trace` on the host now shows, per question, "prefetch asked on kind k: letter args hex
(template | exact, where: exe / club i +off (screen's) / staff i +off / other)", and per screen
"prefetch by class (P, lookups, records + live, pointers, exact R, pointers d >= 1) as
keys/bytes: sent [...], cut [...]" (keys past the cap counted, not sent).

Night 3 run D (those lines, `ui\night3\D\asked_by_step.txt`): a first visit's ~500 questions were
70-87% exact `R` chunks in the exe image (350-437), then heap 33-87, staff-table records 19-55,
club records 1-17, and on the search screens 117-126 `G` names: hence the exe chunks and `G` in
the preset. Second visits asked nearly the same keys again (tactics 459 of 460) although the
prefetch held them: the 300 ms hold ran out on 11 of the 12 screens with a prefetch of ~150 kB or
more and on none of the small ones, so the client asked before the prefetch was in (its answers
then count as "unasked", twice); `HOLD_MAX` is 1 s now. At the cap the squad screen cut 79-169
exact chunks while sending the chunk after a name pointer for every staff member: those (Ptr d >=
1) go last now.

Night 3 run E (E1 teaches, E2 measures with E1's 780-line preset): second visits near 0 on
profile / tactics / search opens and no hold ran out, but first visits still asked ~500. Those
were the same exe chunks as E1's first visits (profile 393 of 454, tactics 425 of 501, search 347
of 495), yet not in the preset: asked once in E1, then prefetched on the second visit and never
asked again, they stayed at 1 hit, under the save's 2 (tactics kept 0 exact keys, profile 47).
Now only Ptr templates need 2 hits. And 112 `G` keys in the preset were asked on every search
visit: the client tries each cell ("23", "£4.1M", "95%") as a row name, the host has none, and
the prefetch left "none" out; `G` answers are now sent even when none (a fixed index, so the
same answer a question gets).

**Client.** The prefetch comes after the game's screen (after `display_reset` -> `new_screen` aged
the cache), so its answers are fresh and not asked for. Once a host has sent a block, `ask()` waits
after a new screen until its block (which follows the prefetch) is in, 1 s at most (300 ms until
2026-10-30)
(`remote::HOLD`, `hold_over` redraws when it runs out), and drops wanted keys that came fresh.
`cmui.trace`: per screen, when the next begins, "client screen n: a keys asked (f in its first
2 s), u answers came unasked (prefetch)". The first visit of a kind teaches the host; from the
second, f should be near 0.

**Measured** (night2 runs A, B, `ui\night2\`): both lines are logged without `cmui.trace` too,
for the first 500 screens. "keys asked" counts the keys wanted, cached ones included, so it stays
around 520; the client's bytes out (cmnet.log) are the measure, ~0.1 KB on a second visit. Every
prefetch sits at the 48 kB cap. Back on a second visit: profile 350 -> 79 KB, tactics 227 -> 76,
search 193 -> 81. The game itself re-sends 180-330 KB on every Back / Cancel / view change.

**How often a client asks** (`remote.rs`, since 2026-10-31, night 4 run D): an unanswered question
is repeated after `UNANSWERED` (15 s), an answered key goes old after `FRESH` (2 s), on a screen
older than `SETTLED` (10 s) after `REFRESH` (10 s). The game's send on the host (0x8ae8d0) spins on
a busy socket on the game thread and the client loop 0x7fdc00 reads one packet per pass, so
questions the host cannot keep up with hold up the whole game.

**The speed patch** is the divider of the game's millisecond clock `0x947299` (0x9472ce: original 10000, x200 = 50, x4 = 2500), so every timer in the game runs that much faster. Night 4's pair ran on x200: its 5 s below were 25 ms of real time. Check this byte first when timing on a pair looks wrong.

**A match day with many matches** (night 4 runs E-H, 2026-10-31, all on x200): Latest Scores refreshes every 5 s
(`handle_latest_scores_cycle` 0x725860 -> `refresh_latest_scores_rows` 0x724520), every widget
change a packet of its own (`ScreenManager::set_widget_*` 0x800890-0x8022d0), ~1 MB a refresh on a
Dutch Cup day of 8 pages, and the client loop slept in every pass (`NtDelayExecution`; most likely
the patcher's Idle Sensitivity, the exe imports no Sleep; not proven). The host stood in `send`,
"Not responding", 14 to 45+ minutes, with or without cmui's questions. Now: the client's `packet`
takes all that has come in one pass, 20 ms at a time (`one_packet`, `READ_PACKET`), and cmnet sends
a packet's four length bytes (`send_packet` 0x77ac30 sends them one by one) with the packet and
waits on a busy socket with select. The host's `ScreenManager::monitor` 0x7fc7e0 reads a network
packet first and polls its own input only in a pass without one, and a refresh is one long turn of
the client's channel: on such a day the host can still take a while to answer a click (~3 min on
13.8 in run H).

**A hang:** a file `cmui.stack` in the game folder logs the game thread's EIP, the code addresses
on its stack and the monitor's state; `cmui.shot` (holding a bmp name) saves the back buffer from a
side thread, also while the game polls nothing. `ui\stack.py <pid>` reads stacks from outside,
which fails on these games here (access denied).

## Click routes on a client

| Click | Route |
|---|---|
| story link from the block | `news::clicked` -> `X` cell, then `send_click` (widget 0) -> host `news_input` |
| club chip / club row / word-found club | `K` club, then click |
| Mark all read button | `Q`, then click |
| Assistant button | `assistant::client_request` -> `H`, then click (host's club / tactics input runs `run_pending`) |
| Team report | no click: the client's overlay asks `T` while drawing (`assistant::report`); the host answers in the packet handler, nothing changes there and no screen is sent |
| ordinary widget | game's `Display::send_widget_click` 0x568560, wrapped by `send_click` (clears 0xb67a34 / 0xb67a38 so a held button does not fire on the host's next screen) |

## Other client fixes

`client_rects_as_sent`: the client's `add_area` / `add_item` calls skip a resolution patch's
scaling (rects were scaled twice). `contacting_box_lower`: "Contacting Server..." rows moved below
the menu bar items (rows 0x7fe162..0x7fe1b4). Dropdowns: the client redraws only two rows, so
`menu_row` shades a lone row from `MENU_ROWS`.

## Status on a client

| Works | Not yet / open |
|---|---|
| squad header + fit %, profile (strip, bars, CA/PA, panels), tactics pitch + list, search + squad Ability view, compare, match overview + live match, national team header, news crests / links / chips / clicks / Mark all read (confirmed 2026-10-29), Assistant on squad and tactics (checked), history title counts (block) | news: header badge click, initials ("M. Tel Aviv"), competition and staff links; live tactics need the match engine (too big to ask), see `..\NETWORK_PLAN.md` steps 4-6 (squad / tactics / profile from the block, traffic measure); dragging cannot be scripted |

Inconsistency: `NETWORK_PLAN.md` says history title counts (step 5) are open, but `history_block`
is built and read (FIXES "History title count"); the plan was not updated.
