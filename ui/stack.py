# Where is a hung game? Read-only: reads each thread's registers and stack, no debugger attached.
# python stack.py <pid> [words]   prints per thread EIP and the code addresses found on its stack.
# ponytail: a stack scan, so stale return addresses show up too; a real walk needs symbols we lack.
import ctypes, sys
from ctypes import wintypes as w

k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.restype = w.HANDLE
k.OpenThread.restype = w.HANDLE
k.CreateToolhelp32Snapshot.restype = w.HANDLE
for f in ('Thread32First', 'Thread32Next'):
    getattr(k, f).argtypes = [w.HANDLE, ctypes.c_void_p]
k.Wow64GetThreadContext.argtypes = [w.HANDLE, ctypes.c_void_p]
k.Wow64SuspendThread.argtypes = [w.HANDLE]
k.ResumeThread.argtypes = [w.HANDLE]
k.ReadProcessMemory.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]


class THREADENTRY32(ctypes.Structure):
    _fields_ = [('dwSize', w.DWORD), ('cntUsage', w.DWORD), ('th32ThreadID', w.DWORD),
                ('th32OwnerProcessID', w.DWORD), ('tpBasePri', w.LONG), ('tpDeltaPri', w.LONG), ('dwFlags', w.DWORD)]


def threads(pid):
    snap = k.CreateToolhelp32Snapshot(4, 0)
    e = THREADENTRY32()
    e.dwSize = ctypes.sizeof(e)
    ok = k.Thread32First(snap, ctypes.byref(e))
    out = []
    while ok:
        if e.th32OwnerProcessID == pid:
            out.append(e.th32ThreadID)
        ok = k.Thread32Next(snap, ctypes.byref(e))
    k.CloseHandle(snap)
    return out


def modules(proc):
    # 32-bit modules of a WOW64 process: EnumProcessModulesEx with LIST_MODULES_32BIT
    psapi = ctypes.WinDLL('psapi', use_last_error=True)
    psapi.EnumProcessModulesEx.argtypes = [w.HANDLE, ctypes.c_void_p, w.DWORD, ctypes.POINTER(w.DWORD), w.DWORD]
    psapi.GetModuleBaseNameW.argtypes = [w.HANDLE, ctypes.c_void_p, w.LPWSTR, w.DWORD]

    class MI(ctypes.Structure):
        _fields_ = [('base', ctypes.c_void_p), ('size', w.DWORD), ('entry', ctypes.c_void_p)]
    psapi.GetModuleInformation.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MI), w.DWORD]
    hs = (ctypes.c_void_p * 512)()
    need = w.DWORD()
    psapi.EnumProcessModulesEx(proc, hs, ctypes.sizeof(hs), ctypes.byref(need), 1)
    out = []
    for h in hs[:need.value // ctypes.sizeof(ctypes.c_void_p)]:
        mi = MI()
        name = ctypes.create_unicode_buffer(260)
        psapi.GetModuleBaseNameW(proc, h, name, 260)
        psapi.GetModuleInformation(proc, h, ctypes.byref(mi), ctypes.sizeof(mi))
        out.append((mi.base or 0, (mi.base or 0) + mi.size, name.value))
    return sorted(out)


def where(mods, a):
    for lo, hi, n in mods:
        if lo <= a < hi:
            return n, a - lo
    return None


def main():
    pid = int(sys.argv[1])
    words = int(sys.argv[2]) if len(sys.argv) > 2 else 400
    proc = k.OpenProcess(0x0410, False, pid)  # QUERY_INFORMATION | VM_READ
    if not proc:
        sys.exit(f'OpenProcess: {ctypes.get_last_error()}')
    mods = modules(proc)
    for tid in threads(pid):
        th = k.OpenThread(0x005a, False, tid)  # SUSPEND_RESUME | GET/SET_CONTEXT | QUERY
        ctx = (ctypes.c_uint32 * 180)()  # WOW64_CONTEXT, 716 bytes
        ctx[0] = 0x00010003  # WOW64_CONTEXT_CONTROL | INTEGER
        k.Wow64SuspendThread(th)
        ok = k.Wow64GetThreadContext(th, ctx)
        k.ResumeThread(th)
        if not ok:
            print(f'thread {tid}: no context ({ctypes.get_last_error()})')
            continue
        # offsets in DWORDs: Eip 46, Esp 49, Ebp 45
        eip, esp, ebp = ctx[46], ctx[49], ctx[45]
        print(f'thread {tid}: eip {eip:08x} {where(mods, eip)} esp {esp:08x} ebp {ebp:08x}')
        buf = (ctypes.c_uint32 * words)()
        got = ctypes.c_size_t()
        k.ReadProcessMemory(proc, esp, buf, ctypes.sizeof(buf), ctypes.byref(got))
        seen = 0
        for v in buf[:got.value // 4]:
            w_ = where(mods, v)
            if w_ and not w_[0].lower().startswith(('ntdll', 'wow64')):
                print(f'    {v:08x} {w_[0]}+{w_[1]:x}')
                seen += 1
                if seen >= 24:
                    break
        k.CloseHandle(th)


main()
