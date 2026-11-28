"""Which bytes of the exe did a DLL change in the running game? Compares live memory with the exe file.
usage: python memdiff.py <exe path> <process name> <va> <len> [<va> <len> ...]
Prints each changed run, the live bytes, and for a jmp/call (e8/e9) the module its target lies in."""
import ctypes
import subprocess
import sys

import pefile

k32 = ctypes.WinDLL('kernel32', use_last_error=True)


def pid_of(name):
    out = subprocess.run(['powershell', '-NoProfile', '-Command', f'(Get-Process {name}).Id'], capture_output=True, text=True).stdout
    return int(out.split()[0])


def modules(name):
    ps = f'(Get-Process {name}).Modules | ForEach-Object {{ "{{0}} {{1}} {{2}}" -f $_.BaseAddress.ToInt64(), $_.ModuleMemorySize, $_.ModuleName }}'
    out = subprocess.run(['powershell', '-NoProfile', '-Command', ps], capture_output=True, text=True).stdout
    return [(int(a), int(n), m) for a, n, m in (l.split(' ', 2) for l in out.splitlines() if l.strip())]


def owner(mods, a):
    for b, n, m in mods:
        if b <= a < b + n:
            return f'{m}+{a - b:#x}'
    return 'no module (allocated memory)'


def main():
    exe, name, rest = sys.argv[1], sys.argv[2], sys.argv[3:]
    pe = pefile.PE(exe, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    h = k32.OpenProcess(0x10 | 0x400, False, pid_of(name))
    mods = modules(name)
    for va, n in zip(rest[::2], rest[1::2]):
        va, n = int(va, 0), int(n, 0)
        disk = pe.get_data(va - base, n)
        buf = ctypes.create_string_buffer(n)
        got = ctypes.c_size_t()
        k32.ReadProcessMemory(h, ctypes.c_void_p(va), buf, n, ctypes.byref(got))
        live = buf.raw
        i = 0
        while i < n:
            if live[i] == disk[i]:
                i += 1
                continue
            j = i
            while j < n and live[j] != disk[j]:
                j += 1
            s = max(i - 1, 0)
            note = ''
            if live[s] not in (0xe8, 0xe9) and live[i] in (0xe8, 0xe9):
                s = i
            if live[s] in (0xe8, 0xe9) and s + 5 <= n:
                t = (va + s + 5 + int.from_bytes(live[s + 1:s + 5], 'little', signed=True)) & 0xffffffff
                note = f' -> {t:#x} {owner(mods, t)}'
            print(f'{va + i:#x} +{j - i}: disk {disk[s:j].hex()} live {live[s:j].hex()}{note}')
            i = j


if __name__ == '__main__':
    main()
