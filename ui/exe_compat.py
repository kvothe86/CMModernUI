"""Is a new patched cm0102.exe safe for cmui / CM Network? Diffs it against a known-good exe and
lists every address the cmui and cmnet sources name (0x4xxxxx..0xbxxxxx) that falls in a changed
byte range (code changed under a hook or a read) or past the known exe's end (new code/data).
usage: python exe_compat.py <known good cm0102.exe> <new cm0102.exe> [src dirs...]"""
import os
import re
import sys
import pefile


def image(path):
    """The exe as it lies in memory: {va: byte} as one bytearray from ImageBase, with raw section data."""
    pe = pefile.PE(path, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    mem = bytearray(pe.OPTIONAL_HEADER.SizeOfImage)
    for s in pe.sections:
        raw = s.get_data()[:max(s.Misc_VirtualSize, s.SizeOfRawData)]
        mem[s.VirtualAddress:s.VirtualAddress + len(raw)] = raw
    names = [(base + s.VirtualAddress, base + s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData),
              s.Name.rstrip(b'\0').decode()) for s in pe.sections]
    return base, mem, names


def changed_ranges(a, b, base, gap=16):
    n = min(len(a), len(b))
    # fast: compare in 64-byte blocks first
    out, i = [], 0
    while i < n:
        if a[i:i + 64] == b[i:i + 64]:
            i += 64
            continue
        j = i
        while j < min(i + 64, n):
            if a[j] != b[j]:
                k = j
                while k < n and (a[k] != b[k] or a[k:k + gap] != b[k:k + gap]):
                    k += 1
                if out and j - out[-1][1] + base <= gap:
                    out[-1][1] = base + k
                else:
                    out.append([base + j, base + k])
                j = k
            else:
                j += 1
        i = j
    return out


def addresses(dirs):
    found = {}
    for d in dirs:
        for root, _, files in os.walk(d):
            if 'target' in root:
                continue
            for f in files:
                if f.endswith('.rs'):
                    p = os.path.join(root, f)
                    for ln, line in enumerate(open(p, encoding='utf8', errors='replace'), 1):
                        for m in re.finditer(r'0x([4-9a-b][0-9a-f]{5})\b', line):
                            found.setdefault(int(m.group(1), 16), []).append(f'{os.path.relpath(p, d)}:{ln}')
    return found


def main():
    known, new = sys.argv[1], sys.argv[2]
    dirs = sys.argv[3:] or [os.path.join(os.path.dirname(os.path.abspath(__file__)), 'src')]
    base, a, secs_a = image(known)
    base_b, b, secs_b = image(new)
    assert base == base_b
    print('known sections:', [(hex(s), hex(e), n) for s, e, n in secs_a])
    print('new sections:  ', [(hex(s), hex(e), n) for s, e, n in secs_b])
    ranges = changed_ranges(a, b, base)
    print(len(ranges), 'changed ranges,', sum(e - s for s, e in ranges), 'bytes')
    sec = lambda va: next((n for s, e, n in secs_b if s <= va < e), '?')
    by_sec = {}
    for s, e in ranges:
        by_sec[sec(s)] = by_sec.get(sec(s), 0) + e - s
    print('changed bytes by section:', by_sec)
    addrs = addresses(dirs)
    end_known = base + len(a)
    hits = []
    for va, where in sorted(addrs.items()):
        # an address cmui hooks/reads: flag if any changed byte lies within 16 bytes after it
        for s, e in ranges:
            if s < va + 16 and va < e:
                hits.append((va, s, e, where))
                break
    print(f'{len(addrs)} addresses named in the sources; {len(hits)} touch a changed range:')
    for va, s, e, where in hits:
        print(f'  {va:#x} [{sec(va)}] changed {s:#x}..{e:#x} ({e - s} b)  used at {", ".join(where[:3])}{" ..." if len(where) > 3 else ""}')
    past = [va for va in addrs if va >= end_known]
    if past:
        print('addresses past the known image end:', [hex(v) for v in past][:20])


if __name__ == '__main__':
    main()
