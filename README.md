# CM Modern UI

Two separate add-ons for Championship Manager 01/02 (version 3.9.68), with their source code.
Neither needs the other, and they work together. The game itself is not included: you need your
own copy.

| Add-on | What it does | Ready to use | Source |
|---|---|---|---|
| **cmui** | A modern dark look as a DLL for the original exe: attribute bars, club crests, new squad, tactics, match, news and competition screens, the Assistant's team picks and more. Every feature has its own switch. | [`cmui_package/dist/cmui`](cmui_package/dist/cmui) | [`ui/`](ui) |
| **CM Network** | Network games over the internet, not only Hamachi or LAN. Listens on all addresses, opens the router's ports with UPnP, accepts names in the join screen and compresses the game's traffic. | [`net/package`](net/package) | [`net/`](net) |

## CM Challenge

Part of cmui: a competition between the managers of one game, kept by the game itself. It was made
for a group of friends who play a long network career together and kept their score on a website
by hand. Now a button under the menu bar opens it as a game screen, and nobody types in a result.

- **Points from your career.** Titles, cups, supercups, European and world trophies, promotions,
  league finishes and changes of club are read from each manager's history in the save and turned
  into points (the Rules tab lists what each is worth). The Dashboard ranks the managers from
  Bronze to Legend; Season and History show every season of every manager, Stats each one's points
  by season, numbers and trophy cabinet.
- **RAD challenges.** Each season every manager spins a wheel for a random challenge on top of his
  job: "Max 1 purchase", "Play with a sweeper", "5 players from South America", "No staff"... 58 in
  all, worth 3 to 9 points. The game checks 23 of them itself and settles them when the season
  turns; the host ticks the rest.
- **Career goals** (switch `career_goals`). Each manager picks ten lifelong goals from 48, worth 10
  to 150 points: from "win the league title in a country of your choice" to "take a club from the
  lowest playable division to the Champions League" or "win De Klassieker ten times". The game ticks
  off 46 of them itself, from the history, league tables, awards and club records.

In a network game the host works it all out and keeps the league in a file next to his exe;
clients with the same cmui only read it, so everyone sees the same table. In single player it
tracks your own career, with RAD as an optional way to set yourself challenges. `challenge=0` in
`cmui.ini` switches it off. How it works, and how to take it out of the code:
[`ui/docs/challenge.md`](ui/docs/challenge.md).

## Patch your game

Nothing here needs building: both add-ons are ready to use from the folders above.

**cmui**, either way:

- With Python: `python cmui_package/dist/cmui/install.py "<game folder>"`, or run `install.bat`
  there. Add `--crests` to fetch logos and flags for that game's database. Start the game with
  `cm0102_ui.exe`. Your `cm0102.exe` is not changed.
- With Nick's CM0102Patcher: copy `cmui.dll` into the game folder and apply the misc patches in
  [`cmui_package/dist/cmui/patches`](cmui_package/dist/cmui/patches).

The exe may already carry other patches. `install.py --check` tests every place cmui hooks and says
which features would work. See
[`HOW TO INSTALL.txt`](<cmui_package/dist/cmui/HOW TO INSTALL.txt>).

**CM Network:** copy `wsock32.dll` into the game folder, on the host and on every client. See
[`README CM Network.txt`](<net/package/README CM Network.txt>).

## Edit the source

Both need Rust with the 32-bit Windows target (`rustup target add i686-pc-windows-msvc`):

```
cd ui
cargo build --release --target i686-pc-windows-msvc

cd ../net
cargo build --release --target i686-pc-windows-msvc
```

`python ui/install.py "<game folder>"` installs your own build. Working with an AI assistant? Start
it on [`ui/AGENTS.md`](ui/AGENTS.md). [`ui/docs/INDEX.md`](ui/docs/INDEX.md) maps every screen to its code,
and [`ui/FIXES.md`](ui/FIXES.md) lists every fix with its cause.

## Logos

No club or competition logos are included, because their licence does not allow it (see
[`cmui_package/LICENCES.md`](cmui_package/LICENCES.md)). `install.py --crests` fetches them for
your own use. Please do not share the generated `Data\cmui_crests.bin`.

## Credits

- Design: mike_appguy's modern look for CM0102 (champman0102.co.uk, topic 11030)
- Flags: flagcdn.com / flagpedia.net (fetched, not included)
- cmfix Display Fix and the CM0102 community; Nick's CM0102Patcher

This is an unofficial fan project, not affiliated with Sports Interactive, Eidos or any club.
