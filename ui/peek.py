"""Print bytes from the running cm0102_ui.exe: what the code at hooked addresses is now.

usage: python peek.py 0x5e5df0 0x5e6d80 [count]
"""
import ctypes
import subprocess
import sys

pid = int(subprocess.run(['powershell', '-NoProfile', '-Command', '(Get-Process cm0102_ui).Id'],
                         capture_output=True, text=True).stdout.split()[0])
k32 = ctypes.windll.kernel32
h = k32.OpenProcess(0x0410, False, pid)  # VM_READ | QUERY_INFORMATION
if not h:
    sys.exit(f'OpenProcess failed: {k32.GetLastError()}')
args = sys.argv[1:]
n = int(args.pop()) if len(args) > 1 and not args[-1].startswith('0x') else 16
for a in args:
    buf, got = ctypes.create_string_buffer(n), ctypes.c_size_t()
    ok = k32.ReadProcessMemory(h, ctypes.c_void_p(int(a, 16)), buf, n, ctypes.byref(got))
    print(a, buf.raw[:got.value].hex(' ') if ok else f'read failed {k32.GetLastError()}')
