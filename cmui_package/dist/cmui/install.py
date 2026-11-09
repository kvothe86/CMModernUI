"""Install cmui into a CM 01/02 game folder.

usage: python install.py "<game folder>" [--check] [--off name,name] [--crests] [--badges folder [names.csv]] [--uninstall]

  --check      only test the game's cm0102.exe at every place cmui hooks, and say which
               features would work
  --off ...    features to switch off in cmui.ini (names: python install.py x --list)
  --crests     also match and pack club crests, competition logos and flags for this game's
               own database (needs the internet the first time; pictures are cached in ..\\logos)
  --badges <pack folder> [names.csv]   use a badge pack you have (an FM logo pack, pictures named
               by FM ID with names.csv as id,name[,nation] rows, or pictures named after clubs);
               its pictures go before football-logos.cc's (with --crests) for the same club
  --uninstall  list the files cmui added and remove them after you confirm
  --list       the feature names

What it does: checks the exe; writes cm0102_ui.exe, a copy of cm0102.exe that loads cmui.dll
first (make_exe.py; cm0102.exe itself is never changed); copies cmui.dll; writes cmui.ini with
every feature on unless switched off. cmui.dll writes the flat menu bar pictures at start.
For Nick's CM0102Patcher instead: make_patches.py.
Start the game with cm0102_ui.exe. Built and tested for the 3.9.68 exe (the community patcher's
builds of it included), any resolution.
"""
import os
import shutil
import struct
import subprocess
import sys
import winreg

import pefile

HERE = os.path.dirname(os.path.abspath(__file__))
DLL = next((p for p in (os.path.join(HERE, 'cmui.dll'), os.path.join(HERE, 'target', 'i686-pc-windows-msvc', 'release', 'cmui.dll'))
            if os.path.exists(p)), 'cmui.dll')  # the package's, else the build's
FEATURES = ['theme', 'tabs', 'attribute_bars', 'crests', 'profile', 'stats_table', 'news', 'club_header',
            'squad_list', 'tactics_pitch', 'match_screen', 'compare_screen', 'menu_fixes', 'status_bars',
            'search_ability', 'scout_stars', 'assistant_pick', 'team_report', 'honours', 'export_button',
            'injury_history', 'transfer_history', 'compare_report', 'scout_screen', 'bigger_text', 'squad_table',
            'table_icons', 'background_show', 'modern_buttons', 'challenge', 'career_goals', 'coach',
            'weather_icons', 'form_timeline', 'match_card', 'season_review', 'national_pool',
            'saved_lineups']

# Where cmui hooks (from DllMain in src\lib.rs): calls it redirects (function, its length, the
# called function, how many calls) and function entries it detours (their first bytes; another
# mod's jump there is fine too). The feature that needs each one.
CALLS = [
    (0x5e2620, 1696, 0x5e4560, 1, 'theme (palette)'),
    (0x55f300, 800, 0x5e3fd0, 1, 'theme (background)'),
    (0x55f300, 800, 0x5e2eb0, 1, 'all overlays (flip)'),
    (0x402e70, 816, 0x5e3730, 2, 'theme (grid lines)'),
    (0x5ee590, 1680, 0x5e3ce0, 5, 'menu_fixes (arrows)'),
    (0x80a774, 5, 0x55f990, 1, 'search_ability (search View item)'),
    (0x808e47, 5, 0x804a40, 1, 'search_ability (sorting)'),
    (0x456f7f, 5, 0x55f990, 1, 'search_ability (squad View item)'),
    (0x484865, 5, 0x945333, 1, 'search_ability (squad Ability view sorting)'),
    (0x561388, 5, 0x5e3dd0, 1, 'all (drag outline fitted to the slot)'),
]
ENTRIES = [
    (0x4671f0, bytes([0x6a, 0xff, 0x64, 0xa1, 0, 0, 0, 0]), 'search_ability (squad clicks)'),
    (0x89f710, bytes([0x6a, 0xff, 0x68, 0xfe, 0x35, 0x96, 0x00]), 'assistant_pick (tactics clicks)'),
    (0x7892b0, bytes([0x83, 0xec, 0x10, 0x0f, 0xbf, 0x05, 0x7a, 0x7c, 0xb7, 0x00]), 'news (club row)'),
    (0x80eb00, bytes([0x81, 0xec, 0x94, 0x07, 0, 0]), 'search_ability (search clicks)'),
    (0x5e42e0, bytes([0xa1, 0x0c, 0xbe, 0xae, 0x00]), 'theme (panels)'),
    (0x5e5df0, bytes([0xa1, 0x0c, 0xbe, 0xae, 0x00]), 'theme (panels)'),
    (0x55f620, bytes([0x81, 0xec, 0, 2, 0, 0]), 'clicks on cmui buttons'),
    (0x5e6d80, bytes([0x6a, 0xff, 0x68, 0x1e, 0xa9, 0x95, 0x00]), 'all text features'),
]
# the files cmui adds to a game folder (relative to it)
ADDED = ['cm0102_ui.exe', 'cmui.dll', 'cmui.ini', 'cmui.log', 'Data/gcui.mbr', 'Data/mcui.mbr',
         'Data/mcui_.mbr', 'Data/cmui_crests.bin', 'cmui_logos']


def check(exe_path):
    """Every hook site: (ok, what). The DLL makes the same checks when it starts and leaves a
    site alone (logged in cmui.log) when it does not match."""
    pe = pefile.PE(exe_path, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    read = lambda va, n: pe.get_data(va - base, n)
    results = []
    for func, length, target, count, what in CALLS:
        code = read(func, length)
        found = sum(1 for i in range(len(code) - 4)
                    if code[i] == 0xe8 and (func + i + 5 + struct.unpack_from('<i', code, i + 1)[0]) & 0xffffffff == target)
        results.append((found == count, f'{func:#x} calls {target:#x} x{count}: {what}'))
    for func, moved, what in ENTRIES:
        code = read(func, len(moved))
        results.append((code == moved or code[0] == 0xe9, f'{func:#x} entry: {what}'))
    return results


def other_install(exe_path, game):
    """The folder the exe moves to as it starts when that is not `game` (it is not portable and
    the registry's install path is elsewhere, so it would load that folder's data), else None."""
    pe = pefile.PE(exe_path, fast_load=True)
    if pe.get_data(0x9f17a0 - pe.OPTIONAL_HEADER.ImageBase, 8) != b'Location':
        return None  # the patcher's portable patch makes it "Aocation"
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\Sports Interactive Ltd\Championship Manager 01-02',
                            0, winreg.KEY_READ | winreg.KEY_WOW64_32KEY) as key:
            location = winreg.QueryValueEx(key, 'Location')[0]
    except OSError:
        return None
    return location if os.path.normcase(os.path.normpath(location)) != os.path.normcase(os.path.normpath(game)) else None


def run(script, *args, env=None):
    subprocess.run([sys.executable, script, *args], cwd=HERE, check=True, env=env)


def main():
    args = sys.argv[1:]
    if '--list' in args:
        print('\n'.join(FEATURES))
        return
    if not args or args[0].startswith('--'):
        sys.exit(__doc__)
    game = os.path.abspath(args[0])
    exe = os.path.join(game, 'cm0102.exe')
    if not os.path.exists(exe):
        sys.exit(f'no cm0102.exe in {game}')

    if '--uninstall' in args:
        present = [p for p in ADDED if os.path.exists(os.path.join(game, p))]
        print('cmui added these to', game, ':\n  ' + '\n  '.join(present or ['(nothing)']))
        if present and input('Remove them? [y/N] ').strip().lstrip('﻿').lower().startswith('y'):
            for p in present:
                full = os.path.join(game, p)
                shutil.rmtree(full) if os.path.isdir(full) else os.remove(full)
            print('removed; cm0102.exe and the rest of the game are as they were')
        return

    results = check(exe)
    for ok, what in results:
        print(('  ok    ' if ok else '  NO    ') + what)
    bad = [w for ok, w in results if not ok]
    print(f'{len(results) - len(bad)} of {len(results)} hook sites match' + ('' if not bad else ': the features above marked NO stay off'))
    elsewhere = other_install(exe, game)
    if elsewhere:
        print(f'WARNING: this exe is not portable, so the game would run from {elsewhere} and load that folder\'s data.\n'
              '         In CM0102Patcher tick "Make Executable Portable" and apply it to cm0102.exe first.\n'
              '         Then switch off Windows 95 compatibility mode for the exe (Properties > Compatibility), or the game crashes at start.')
    if '--check' in args:
        return
    if any('all text features' in w or 'flip' in w for w in bad):
        sys.exit('this exe is too different from 3.9.68 for cmui; nothing installed')

    if not os.path.exists(DLL):
        sys.exit(f'{DLL} is missing: build it first (cargo build --release --target i686-pc-windows-msvc)')
    if os.path.exists(os.path.join(game, 'cm0102_ui.exe')):
        os.remove(os.path.join(game, 'cm0102_ui.exe'))  # made again from the current cm0102.exe
    run('make_exe.py', game)
    shutil.copy2(DLL, os.path.join(game, 'cmui.dll'))

    off = set()
    if '--off' in args:
        off = {f.strip() for f in args[args.index('--off') + 1].split(',')}
        unknown = off - set(FEATURES)
        if unknown:
            sys.exit(f'unknown feature(s): {", ".join(sorted(unknown))} (see --list)')
    ini = os.path.join(game, 'cmui.ini')
    keep = [l for l in open(ini).read().splitlines() if l.split('=')[0].strip() not in FEATURES] if os.path.exists(ini) else []
    with open(ini, 'w', newline='\r\n') as f:
        f.write('\n'.join(keep + [f'{n}={0 if n in off else 1}' for n in FEATURES]) + '\n')
    print('cmui.ini:', 'all features on' if not off else 'off: ' + ', '.join(sorted(off)))

    env = dict(os.environ, CMUI_GAME=game)
    if '--crests' in args:
        if not os.path.exists(os.path.join(HERE, '..', 'logos', 'index.tsv')):
            run('logos.py', 'index', env=env)
        for step in ('match', 'comps', 'flags', 'fetch'):
            run('logos.py', step, env=env)
    if '--badges' in args:
        at = args.index('--badges')
        names = args[at + 2:at + 3] if args[at + 2:at + 3] and not args[at + 2].startswith('--') else []
        run('logos.py', 'badges', os.path.abspath(args[at + 1]), *map(os.path.abspath, names), env=env)
    if '--crests' in args or '--badges' in args:
        run('logos.py', 'pack', env=env)
    elif not os.path.exists(os.path.join(game, 'Data', 'cmui_crests.bin')):
        print('no crests for this game yet: run again with --crests to make them')
    print(f'installed: start {os.path.join(game, "cm0102_ui.exe")}')


if __name__ == '__main__':
    main()
