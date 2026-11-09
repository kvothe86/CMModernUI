"""Write cmui as misc patches for Nick's CM0102Patcher (github nckstwrt/CM0102Patcher), its
`OFFSET: OLD NEW` byte format, into patches\\: "cmui Loader" makes cm0102.exe load cmui.dll
(the stub of make_exe.py in the .nick section, all switches off) and each "cmui - <feature>"
patch turns one feature's switch byte on. cmui.dll (and Data\\cmui_crests.bin for crests) must
be in the game folder; without cmui.dll the game starts as usual.

usage: python make_patches.py "<cm0102.exe>"   (a 3.9.68 exe the patcher has expanded)
       python make_patches.py --test "<cm0102.exe>" "<out.exe>" [feature,feature]   applies the
           loader and the feature patches (all if none named) the way the patcher does
"""
import os
import sys

import pefile

from make_exe import NICK_STUB, loader

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'patches')
FEATURES = [
    ('theme', 'Theme', 'Dark palette, flat background and flat menu bar (written to Data\\gcui.mbr etc. at start).'),
    ('tabs', 'Tabs', 'Flat tab buttons with the active tab marked.'),
    ('attribute_bars', 'Attribute Bars', 'Attributes as bars with the value right of the bar, hidden attributes included.'),
    ('crests', 'Crests', 'Club crests, competition logos and flags (needs Data\\cmui_crests.bin, made by install.py --crests).'),
    ('profile', 'Player Profile', "Player profile in mike_appguy's layout: info strip, reputation / status / positions panels, CA/PA stars, an injury cross by the condition."),
    ('stats_table', 'Stats Table', 'Player stats table with pills and flat -/+ buttons.'),
    ('news', 'News', 'News restyled: gold selection, flat dates, story header with crests, clickable club row.'),
    ('club_header', 'Club Header', 'Club header band with crest on club screens.'),
    ('squad_list', 'Squad List', 'Squad list with fitness, form boxes and uniform headers.'),
    ('tactics_pitch', 'Tactics Pitch', 'Tactics pitch with shirts and pills; in a match live condition and position.'),
    ('match_screen', 'Match Screen', 'Match overview: crests by the scores, events, key stats, last-5 bar, HALF/FULL TIME.'),
    ('compare_screen', 'Compare Screen', 'Player comparison: P1/P2 boxes, radar, pitches, season.'),
    ('menu_fixes', 'Menu Fixes', 'Menu bar without boxes: an icon over each item, the open one marked gold; dark dropdown menus, arrow icons.'),
    ('status_bars', 'Status Bars', 'Morale and condition as bars.'),
    ('search_ability', 'Ability View', 'An "Ability" view in player search and the squad screen: position, age, CA and PA stars, value.'),
    ('scout_stars', 'Scout Stars', "Scouts' Rec stars from Mad Scientist's intrinsic weights, rated against the scout's own club's squad; a scout's judging sets how far off he is."),
    ('assistant_pick', 'Assistant Pick', 'An Assistant button on the squad screen: best team, most fit team, squad rotation or clear team, with captain and set piece takers, as the assistant manager judges the players.'),
    ('team_report', 'Team Report', "A Team report item in the squad screen's Assistant menu (needs Assistant Pick): the assistant's best eleven and best backup eleven on their own screen. The selection is not changed."),
    ('honours', 'Honours', "A club's Honours list with a cup in gold, silver or bronze and the number of times after each place (needs Stats Table)."),
    ('export_button', 'Export Button', 'The Print dropdown of club and competition screens is called Export; on club screens it stands clear of the crest and always writes a file.'),
    ('injury_history', 'Injury History', "A player's last injuries and bans on Injuries & Bans."),
    ('transfer_history', 'Transfer History', "A player's moves since the game began on the Transfer tab."),
    ('compare_report', 'Compare Report', "A line from the assistant and two coaches on who is better under the pitches of Compare Players (needs Compare Screen)."),
    ('scout_screen', 'Scout Screen', 'A Full scout report chip on "<club> scouting completed" stories: the eleven the scout expects, in the shape the manager prefers, without those injured or banned, and his report.'),
    ('bigger_text', 'Bigger Text', "At 1280 wide or more: buttons, tabs, the rows of a profile's tabs and the Position(s) view in a larger font where the text fits its box."),
    ('squad_table', 'Squad Table', "At 1280 wide or more: the squad's Position(s) view in two wide columns, a table of position, CA stars, average rating, condition, goals and appearances."),
    ('table_icons', 'Table Icons', 'Needs Stats Table. The league table\'s C, P, Pl and R boxes as a cup, a green arrow up, two amber arrows and a red arrow down; in a continental league phase Q as a green tick and Pl as a blue PO pill.'),
    ('background_show', 'Background Show', 'With Background Changes on: the dark panels let the picture through.'),
    ('modern_buttons', 'Modern Buttons', "The game's grey buttons, dropdowns and list cells in steel blue, column headers flat, and no box around an empty stats cell."),
    ('challenge', 'CM Challenge', "In a network game: a button under the menu bar opens the managers' league, prizes and points from the game's own history, RAD challenges and the seasons so far."),
    ('career_goals', 'Career Goals', 'Needs CM Challenge. A Goals tab: the host picks ten goals for a whole career per manager, from 48 worth 10 to 150 points; the game ticks off the 46 it can read (history, league tables, awards, club records), the host the rest.'),
    ('coach', 'Coach', 'At 1680x1050 and up, with claude_api_key=<your Anthropic API key> in cmui.ini: a Coach button under the menu bar. Claude gives a pre-match briefing, a post-match analysis, an expanded team report and answers typed questions. Each answer is one paid API request.'),
    ('weather_icons', 'Weather Icons', "Needs Stats Table. An icon for the match's weather (sun, cloud, rain, snow) by the weather the game names on fixtures, results and the match screen."),
    ('form_timeline', 'Form Timeline', "A strip on a club's screen: its last ten results as W, D and L pills with the opponent's crest, and its league place week by week as a line."),
    ('match_card', 'Match Card', 'Before a match of the human manager: a card with both clubs, their last five results, their league places and the last meetings.'),
    ('season_review', 'Season Review', "On 1 July a card on the human manager's season: final place, record, goals, biggest win, cup run, top scorer, assists, best average, signings, CM Challenge points and the last six results."),
    ('national_pool', 'National Pool', "Needs Assistant Pick. A National pool item in a national team's Assistant menu: every player of the country (the first nationality), rated by the assistant, best 100 a position group for a big country; + puts one on the Short List."),
    ('saved_lineups', 'Saved Line-ups', "Needs Assistant Pick. Three line-up slots and Fill subs on the squad screen: save your eleven and bench, load them back with the assistant standing in for whoever cannot play, and let him fill only the empty bench places."),
    ('flat_buttons', 'Flat Buttons', 'Needs Modern Buttons. The buttons flat with soft corners and small gaps, no bevel; the tabs one dark bar with the open tab in gold; the clickable cells of a list (positions, dates) without a box, their text light blue.'),
    ('pl_report', 'Profit and Loss Report',"A P&L report button on a club's Finances & Info tab: a full profit and loss account on a page signed by the made-up firm Accountants & Co, with notes written from how the club is doing. Needs Crests and Profile."),
]


def lines(old, new, at):
    return [f'{at + i:08X}: {o:02X} {n:02X}' for i, (o, n) in enumerate(zip(old, new)) if o != n]


def patches(exe):
    """{file name: patch text}"""
    pe = pefile.PE(exe, fast_load=True)
    assert any(s.Name.startswith(b'.nick') for s in pe.sections), 'not an exe the patcher has expanded (no .nick section)'
    base, data = pe.OPTIONAL_HEADER.ImageBase, pe.__data__
    stub, entry = pe.get_offset_from_rva(NICK_STUB - base), pe.OPTIONAL_HEADER.get_file_offset() + 16
    code = loader(pe, NICK_STUB)
    assert data[stub:stub + len(code) + len(FEATURES)] == bytes(len(code) + len(FEATURES)), 'stub space not free'
    out = {'cmui Loader': ['# cmui: load cmui.dll (put it in the game folder) before the game starts', 'EXPANDEXE']
           + lines(data[entry:entry + 4], (NICK_STUB - base).to_bytes(4, 'little'), entry) + lines(bytes(len(code)), code, stub)}
    for k, (name, title, _) in enumerate(FEATURES):
        out[f'cmui - {title}'] = [f'# cmui feature switch {name} (needs the cmui Loader patch)', 'EXPANDEXE'] + lines(b'\0', b'\1', stub + len(code) + k)
    # all of it in one file, for the patcher's Tools > Apply Patchfile (one file a click)
    out['cmui - Everything'] = ['# cmui: the Loader and every feature'] + out['cmui Loader'][1:] + [
        line for t in FEATURES for line in out[f'cmui - {t[1]}'][2:]]
    return {name: '\n'.join(text) + '\n' for name, text in out.items()}


def apply(data, text):
    """The patcher's ApplyPatch for these files: EXPANDEXE (the exe is expanded already) and bytes."""
    for line in text.splitlines():
        if line and not line.startswith('#') and line != 'EXPANDEXE':
            at, _, new = line.replace(':', '').split()
            data[int(at, 16)] = int(new, 16)


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    if args[0] == '--test':
        src, dst = args[1], args[2]
        wanted = {t for n, t, _ in FEATURES if len(args) < 4 or n in args[3].split(',')}
        data = bytearray(open(src, 'rb').read())
        for name, text in patches(src).items():
            if name == 'cmui Loader' or (name[7:] in wanted and name != 'cmui - Everything'):
                apply(data, text)
        open(dst, 'wb').write(data)
        print(f'{dst}: cmui Loader and {len(wanted)} feature patches applied')
        return
    os.makedirs(OUT, exist_ok=True)
    notes = {'cmui Loader': "cmui, a modern look for CM 01/02 (after mike_appguy's design). This patch makes cm0102.exe load\n"
                            'cmui.dll from the game folder before the game starts; without cmui.dll the game runs as usual.\n'
                            'Every feature has its own "cmui - ..." patch: apply the ones you want (none is on without its patch).\n'
                            'cmui.ini in the game folder can switch a feature off again (name=0). Unapply the loader to remove cmui.\n'}
    notes.update({f'cmui - {t}': f'{text}\nNeeds the "cmui Loader" patch and cmui.dll. cmui.ini: {n}=0 switches it off again.\n' for n, t, text in FEATURES})
    notes['cmui - Everything'] = ('cmui with every feature: the "cmui Loader" patch and all "cmui - ..." feature patches in one\n'
                                  'file. Needs cmui.dll in the game folder. cmui.ini can switch features off again (name=0).\n')
    for name, text in patches(args[0]).items():
        open(os.path.join(OUT, name + '.patch'), 'w', newline='\r\n').write(text)
        open(os.path.join(OUT, name + '.txt'), 'w', newline='\r\n').write(notes[name])
    print(f'{len(notes)} patches and their notes in {OUT}')


if __name__ == '__main__':
    main()
