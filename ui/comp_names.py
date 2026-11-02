"""comp_names.py <Data folder> <out tsv>: the table src\\challenge\\comps_<database>.tsv, from the
folder's club_comp.dat + nation_comp.dat. A line is `name<TAB>family`: 0 CL, 1 UEFA Cup, 2 European
Supercup, 3 club world cup, 4 World Cup, 5 continental tournament, 7 national supercup, - nothing
(mod.rs `family`). Listed: every international competition, and every national one the old word
rules took for a supercup. A national main cup is not listed: it counts by `top` in the game.
Run it, then read the result by eye: the old word rules plus the fixes in `fix`."""
import re
import struct
import sys


def read(path):
    b = open(path, 'rb').read()
    for k in range(len(b) // 107):
        r = b[k * 107:(k + 1) * 107]
        yield r[4:55].split(b'\0')[0].decode('cp1252'), struct.unpack('<i', r[0x5d:0x61])[0]


def has(n, *words):
    return any(w in n for w in words)


def old(n, national):
    """The word rules `family` had before the tables."""
    if national:
        return 7 if has(n, 'super', 'shield', 'schaal', 'trophy', 'supertaca') else None
    if has(n, 'african', 'asian', 'concacaf', 'cis ', 'afro', 'arab', 'oceania', 'american') and not has(n, 'nations', 'asian cup', 'gold cup', 'copa america'):
        return None
    for fam, words in [(0, ('european champions cup', 'uefa champions', 'champions league')), (1, ('uefa cup', 'europa league')), (2, ('european super', 'uefa super')),
                       (3, ('club world', 'world club', 'intercontinental')), (4, ('world cup',)), (5, ('european championship', 'copa america', 'nations', 'asian cup', 'gold cup'))]:
        if has(n, *words):
            return fam


def fix(n, national, fam):
    """What the words got wrong: a qualifying or play-off is not its tournament, another confederation's
    champions league is not ours, Copa America has an accent, the original's European Football Championship an extra word, a national league or lower cup is no supercup."""
    if national:
        return 7 if re.search(r'super ?cup|supercop|supercoup|superpuchar|superta|schaal|charity shield|community shield|champions trophy|superbowl|e des champions', n) else None
    if has(n, 'qualifying', 'play-off', 'winners'):
        return None
    if fam == 0 and re.search(r'\b(afc|caf|ofc|gcc)\b', n):
        return None
    return 5 if n in ('copa américa', 'european football championship') else fam


if __name__ == '__main__':
    out = ['# name<TAB>family, made by comp_names.py (see there); read by mod.rs `family`\n']
    for f in ('club_comp.dat', 'nation_comp.dat'):
        for name, nation in read(sys.argv[1] + '/' + f):
            national = nation > 0
            n = name.lower()
            if national and old(n, True) is None and 'des champions' not in n:
                continue
            fam = fix(n, national, old(n, national))
            out.append('%s\t%s\n' % (name, '-' if fam is None else fam))
    open(sys.argv[2], 'w', encoding='cp1252', newline='\n').write(''.join(out))
