"""check_switches.py folder [folder ...]: every cmui feature switch byte of each folder's
cm0102_ui.exe set to 1 (the features `FEATURES` in src/lib.rs lists; a byte past the stub's
count is free space, 0, until set). Prints each switch it had to turn on, and FAILED when an exe
has no cmui stub. The safeguard against folders with different features (user, 2026-11-07:
Career Goals was off in three folders for days). Run by install_all.ps1 after the DLL copy;
cmui.ini can still switch a feature off per folder. No backup is kept: only switch bytes change,
and the check below makes sure nothing else did."""
import os
import re
import sys
import pefile

HERE = os.path.dirname(os.path.abspath(__file__))


def features():
    src = open(os.path.join(HERE, 'src', 'lib.rs'), encoding='utf-8').read()
    body = re.search(r'const FEATURES: \[&str; (\d+)\] = \[(.*?)\];', src, re.S)
    names = re.findall(r'"([a-z_]+)"', body.group(2))
    assert len(names) == int(body.group(1)), 'FEATURES count and names differ'
    return names


def check(folder, names):
    exe = os.path.join(folder, 'cm0102_ui.exe')
    if not os.path.exists(exe):
        return f'missing   {folder}'
    pe = pefile.PE(exe, fast_load=True)
    stub = pe.OPTIONAL_HEADER.AddressOfEntryPoint
    if pe.get_data(stub + 16, 16) != b'cmui.dll\0\0\0\0CMUI':
        return f'FAILED    {exe}: no cmui stub'
    at = pe.get_offset_from_rva(stub + 0x20)
    pe.close()
    data = bytearray(open(exe, 'rb').read())
    old = bytes(data)
    turned = []
    for k, name in enumerate(names):
        if data[at + k] not in (0, 1):
            return f'FAILED    {exe}: byte {k} ({name}) is {data[at + k]}, not a switch'
        if data[at + k] == 0:
            data[at + k] = 1
            turned.append(f'{k} {name}')
    # (cmui.ini's own switch-offs are the player's choice: shown, not changed)
    ini = os.path.join(folder, 'cmui.ini')
    lines = open(ini, encoding='latin-1').read().splitlines() if os.path.exists(ini) else []
    offs = [n for n in names if f'{n}=0' in (l.strip() for l in lines)]
    note = f' (cmui.ini switches off: {", ".join(offs)})' if offs else ''
    if turned:
        changed = [i for i in range(len(old)) if old[i] != data[i]]
        assert all(at <= i < at + len(names) for i in changed), 'other bytes changed'
        open(exe, 'wb').write(data)
        return f'TURNED ON {folder}: ' + ', '.join(turned) + note
    return f'= all {len(names)} {folder}{note}'


if __name__ == '__main__':
    names = features()
    for folder in sys.argv[1:]:
        print(check(folder, names))
