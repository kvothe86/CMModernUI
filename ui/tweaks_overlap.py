"""Do cmui's hooks touch code that CM0102-Rust-Tweaks checks or replaces?
cmui: patch_call(s)(func, len, ...) and hook_entry(func, moved) regions, names resolved from `const X: usize = 0x..`.
tweaks: every (0xADDR, LEN) pair in their sources (Parte bordas/corpo and the swap tables).
usage: python tweaks_overlap.py <cmui src dir> <tweaks src dir>"""
import os
import re
import sys


def rs_files(d):
    for root, _, files in os.walk(d):
        for f in files:
            if f.endswith('.rs'):
                yield os.path.join(root, f), open(os.path.join(root, f), encoding='utf-8', errors='replace').read()


def num(tok, consts):
    tok = tok.strip().split('::')[-1]
    if tok.startswith('0x'):
        return int(tok.replace('_', ''), 16)
    if tok.isdigit():
        return int(tok)
    return consts.get(tok)


def cmui_regions(d):
    srcs = list(rs_files(d))
    consts = {}
    for _, t in srcs:
        for n, v in re.findall(r'const (\w+): usize = (0x[0-9a-fA-F_]+)', t):
            consts[n] = int(v.replace('_', ''), 16)
    out = []
    for path, t in srcs:
        for m in re.finditer(r'patch_calls?\(([^,]+),\s*([^,]+),', t):
            a, n = num(m.group(1), consts), num(m.group(2), consts)
            if a and n:
                out.append((a, n, f'{os.path.basename(path)} patch_call {m.group(1).strip()}'))
        for m in re.finditer(r'hook_entry\(([^,]+),\s*&\[([^\]]*)\]', t):
            a = num(m.group(1), consts)
            if a:
                out.append((a, m.group(2).count(',') + 1, f'{os.path.basename(path)} hook_entry {m.group(1).strip()}'))
    return out


def tweak_regions(d):
    out = []
    for path, t in rs_files(d):
        if path.endswith('referencia.rs'):  # their map of the whole exe (functions vs 3.9.68), not hooks
            continue
        for a, n in re.findall(r'\((0x[4-9a-b][0-9a-f]{5}),\s*(0x[0-9a-f]+|\d+)\b', t):
            if int(n, 0) < 0x10000:  # (addr, addr) pairs are not regions
                out.append((int(a, 16), int(n, 0), os.path.relpath(path, d)))
        for a in re.findall(r'(?:desviar|chamar|trampolim|trocar_call|trocar\("[^"]*",)\s*\(?(0x[4-9a-b][0-9a-f]{5})', t):
            out.append((int(a, 16), 5, os.path.relpath(path, d)))
        for a, n in re.findall(r'nops\((0x[0-9a-f]+),\s*(0x[0-9a-f]+|\d+)', t):
            out.append((int(a, 16), int(n, 0), os.path.relpath(path, d)))
    return out


def main():
    c, t = cmui_regions(sys.argv[1]), tweak_regions(sys.argv[2])
    print(f'cmui regions: {len(c)}, tweaks regions: {len(t)}')
    hits = sorted({(a, n, w, b, m, v) for a, n, w in c for b, m, v in t if a < b + m and b < a + n})
    for a, n, w, b, m, v in hits:
        print(f'OVERLAP cmui {a:#x}+{n} ({w})  <->  tweaks {b:#x}+{m} ({v})')
    if not hits:
        print('no overlap')


if __name__ == '__main__':
    main()
