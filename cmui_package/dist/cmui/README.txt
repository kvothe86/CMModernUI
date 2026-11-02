cmui - a modern dark look for Championship Manager 01/02
=========================================================

New here? Read "HOW TO INSTALL.txt" first: the short step-by-step guide. This file is the
full reference.

cmui restyles CM 01/02 in the spirit of mike_appguy's modern look: flat dark theme,
attribute bars and CA/PA stars on the player profile, form/morale/condition bars, a new
menu bar, club crests and league logos, nicer squad lists, tactics pitch, match screen,
news and a player compare screen, competition screens, plus an "Ability" view in player
search and the squad, better scout stars and an assistant who picks your team.
It works next to cmfix (the Display Fix, dsound.dll) and any resolution.

It does NOT change your cm0102.exe. The installer makes a copy, cm0102_ui.exe, that loads
cmui.dll first. You start the game with cm0102_ui.exe instead of cm0102.exe.


REQUIREMENTS
------------
- Windows, and CM 01/02 version 3.9.68 (the exe may be patched, e.g. with Nick's
  CM0102Patcher, or the plain 3.9.68 exe). The installer checks the exe first.
- Python 3 (https://www.python.org/downloads/ ; tick "Add python.exe to PATH") and:
      pip install -r requirements.txt        (pefile, Pillow)
- Internet, only if you want club crests (see below).


INSTALL
-------
1. Unzip this folder anywhere (NOT inside the game folder is fine; so is outside).
2. Check your exe first (changes nothing):
      python install.py "C:\Games\CM0102" --check
3. Install:
      install.bat "C:\Games\CM0102"
   or: python install.py "C:\Games\CM0102"
   Options:  --off tabs,news     switch features off (names below)
             --crests            also build crests, league logos and flags (see below)
4. Start cm0102_ui.exe.

OR WITH NICK'S CM0102PATCHER (no Python needed)
-----------------------------------------------
The folder "patches" holds cmui as misc patches in the patcher's own format:
  "cmui Loader"        makes cm0102.exe itself load cmui.dll (the game runs as usual
                       without cmui.dll); every feature is off until its patch is applied
  "cmui - <feature>"   one per feature below; apply the ones you want
1. Copy cmui.dll into the game folder.
2. In the patcher: Tools... > Apply Patchfile..., and pick "cmui - Everything.patch" (the
   Loader and every feature), or "cmui Loader" and then the feature patches you want, one a
   click. Holding Ctrl+Shift while clicking Apply Patchfile... unapplies a patch.
3. Start cm0102.exe. cmui writes Data\gcui.mbr etc. (the flat menu bar) itself.
cmui.ini can still switch a feature off again (name=0), not on. Crests: build them with
install.py --crests (below), or copy Data\cmui_crests.bin from an install.

CRESTS AND LOGOS
----------------
Club crests, competition logos and flags are NOT included in this package. The logo
site's terms (https://football-logos.cc/license/) allow its logos only for informational,
non-commercial and fan projects and say nothing about passing them on, so each player
builds their own copy for their own database:
      python install.py "C:\Games\CM0102" --crests
This downloads the matching pictures from football-logos.cc and flagcdn.com (a few
minutes, a few hundred MB of downloads the first time) and writes Data\cmui_crests.bin
in your game folder. Without it everything else works; crests are simply not shown.
Downloaded pictures are cached in a "logos" folder NEXT TO the cmui folder (one level up).
Personal use only; do not redistribute cmui_crests.bin.

Flags are flagpedia.net's (public domain), fetched at 640 pixels; national teams get their
country's flag at the left of the title bar and their federation's badge at the right.

YOUR OWN BADGE PACK (e.g. an FM logo pack)
------------------------------------------
      python install.py "C:\Games\CM0102" --badges "D:\FM logos" names.csv
FM packs name each picture by FM's ID (clubs\normal\1234.png), which CM does not know, so
names.csv gives the names: one row per ID, "id,name,nation" (nation optional but makes the
matching surer; comma, semicolon or tab). Export it from FM (a search view or the editor).
Pictures named after the club ("Ajax.png") need no list. The pack's clubs and competitions
are matched to your game's database by name; with --crests as well, the pack's picture
wins where both have one. The matches are listed in the game folder's cmui_logos folder
(badge_match.tsv, badge_comp_match.tsv).

CHECK / UNINSTALL
-----------------
      python install.py "C:\Games\CM0102" --check        which of the 18 hook sites match
      python install.py "C:\Games\CM0102" --uninstall    lists cmui's files, removes them on y
Uninstall removes cm0102_ui.exe, cmui.dll, cmui.ini, cmui.log, Data\gcui.mbr, mcui.mbr,
mcui_.mbr, cmui_crests.bin and cmui_logos. cm0102.exe and your saves are never touched.
To update cmui: unzip the new version and run the installer again.

THE MAIN FEATURE SWITCHES (30 in all, the rest in cmui.ini; 1 = on, 0 = off; restart the game;
                         with the patcher, a feature is on only if its patch is applied too)
-----------------------------------------------------------------------------------------
theme            dark palette and flat background (0 keeps the game's own colours)
tabs             flat tabs with a gold underline
attribute_bars   attribute bars on the player profile
crests           club crests and league/competition logos, flags (needs --crests)
profile          profile layout: info strip, reputation, status and positions panels, ability stars,
                 an injury cross by the condition (Injuries & Bans)
stats_table      flat season stats table on the profile; competition screens: league table
                 with crests, form and goal difference, stats views with medals, rating pills,
                 bars and cards, results/fixtures with crests, history with crests and title
                 counts, records in colour; Latest Scores (screen and match tab) with crests,
                 gold scores and icons for goals, red cards, injuries and missed penalties
news            news screen: type icons, unread dots, crests, short dates, Mark all read,
                 stories laid out with clickable names, transfer/record/mood cards
club_header      club header: league logo, crest, league position line
squad_list       squad lists: gold positions, coloured fitness %
tactics_pitch    tactics pitch with shirts in the club's colours (the keeper his own), name
                 pills and extra columns (Av R, Tck, Drb)
match_screen     match screen: score squares, events with icons, key stats, last 5 minutes bar
compare_screen   player compare screen with radar and positions
menu_fixes       menu bar without boxes (an icon over each item), dark dropdown menus
status_bars      form tiles, morale and condition bars
search_ability   an "Ability" view (true CA/PA stars) in player search and squad, sortable
                 by Current and Potential in both
scout_stars      scouts' Rec stars from Mad Scientist's intrinsic weights, rated against the
                 scout's own club's squad; a scout's judging sets how far off he is
assistant_pick   an Assistant button on the squad screen and the pre-match tactics screen:
                 best team, most fit team, squad rotation or clear team, as your assistant
                 manager judges the players; he also sets the captain, free kick, corner and
                 penalty takers and says what he did and why; he names only as many subs as
                 the next match allows and leaves banned players out (on a network client
                 too, squad and tactics screen; the host makes the pick)
challenge        CM Challenge: a button that opens a league of the human managers, results
                 from the game's own history; in a network game the host keeps the league
                 file and a client only reads it (needs the same cmui); in single player a
                 tracker of your own career, with optional RAD challenges

KNOWN LIMITS
------------
- Built for 3.9.68. If a hook site does not match your exe, that feature stays off
  (see cmui.log); if the core ones do not match, nothing is installed.
- The untouched 2001 exe: all hooks fit, but that exe stops at its own CD check.
- Game Credits crashes the 1680x1050 patched exe with or without cmui.
- Competition logos for World Cup, Euro, Copa America and Africa Cup are not included.
- The game folder needs to be writable (not Program Files without admin rights).
- Network games (the game's own behaviour, not cmui): with Game Settings > Screen Mode on
  Full on the host, a save switches the joined players' windows to full screen and a load
  back to windowed. Keep Screen Mode on Windowed and get full screen from ddraw.ini
  (fullscreen=true with windowed=true: borderless). A player's match can also race to the
  end while the host sits on a paused screen (a tactics change after a red card).
- Tested on a patched 1680x1050 install, on "CM0102 - Restructured 2025-2026" (800x600)
  and on the untouched 2001 exe; network games (host and joined player) at 800x600,
  1280x800 and 1680x1050, with the exe made by the patcher (resolution change + patch files).

CREDITS
-------
- Design: mike_appguy's modern look for CM0102 -
  https://www.champman0102.co.uk (topic 11030)
- Logos: football-logos.cc (not included, fetched by you)
- Flags: flagcdn.com / flagpedia.net (not included, fetched by you)
- cmfix Display Fix and the CM0102 community; Nick's CM0102Patcher.
cmui is an unofficial fan mod, not affiliated with Sports Interactive, Eidos or any club.
