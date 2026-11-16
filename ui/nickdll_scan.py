"""Where NickDLL (the patcher's runtime DLL) and cmui touch the same game code: every 32-bit value in
NickDLL.dll that is an address cmui's sources name, or lies in a game function cmui hooks or
redirects calls in (install.py's CALLS / ENTRIES), or within 16 bytes before such an address.
usage: python nickdll_scan.py <NickDLL.dll> [src dirs...]"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import exe_compat  # noqa: E402
import install  # noqa: E402


def main():
    data = open(sys.argv[1], 'rb').read()
    dirs = sys.argv[2:] or [os.path.join(os.path.dirname(os.path.abspath(__file__)), 'src')]
    named = exe_compat.addresses(dirs)
    funcs = [(f, f + n, what) for f, n, _, _, what in install.CALLS] + [(f, f + len(b), what) for f, b, what in install.ENTRIES]
    values = {}
    for i in range(len(data) - 3):
        v = struct.unpack_from('<I', data, i)[0]
        if 0x401000 <= v < 0xfe7000:
            values.setdefault(v, []).append(i)
    hits = []
    for v in sorted(values):
        why = []
        if v in named:
            why.append('named in cmui: ' + ', '.join(named[v][:2]))
        else:
            near = [a for a in named if 0 < a - v <= 16]
            if near:
                why.append(f'16 b before {near[0]:#x} ({named[near[0]][0]})')
        why += [f'inside {f:#x} ({w})' for f, e, w in funcs if f <= v < e]
        if why:
            hits.append((v, why))
    print(len(values), 'game addresses in the DLL;', len(hits), 'meet cmui:')
    for v, why in hits:
        print(f'  {v:#x}  ' + ' | '.join(why))


if __name__ == '__main__':
    main()
