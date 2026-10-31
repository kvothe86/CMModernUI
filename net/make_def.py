"""wsock32.def for the CM Network stand-in: every export of Windows' own (32-bit) wsock32.dll
with its ordinal; forwarded ones forwarded the same way, its four own functions and the ones CM
Network changes (bind, connect, listen) to this DLL's cmnet_* functions.
usage: python make_def.py   (reads C:\\Windows\\SysWOW64\\wsock32.dll, writes wsock32.def here)"""
import os
import pefile

OURS = {'getsockopt', 'recv', 'recvfrom', 'setsockopt', 'bind', 'connect', 'listen', 'accept', 'send'}
pe = pefile.PE(r'C:\Windows\SysWOW64\wsock32.dll')
lines = ['LIBRARY wsock32', 'EXPORTS']
for e in sorted(pe.DIRECTORY_ENTRY_EXPORT.symbols, key=lambda e: e.ordinal):
    name = e.name.decode()
    if name in OURS:
        lines.append(f'    {name}=cmnet_{name} @{e.ordinal}')
    elif e.forwarder:
        # (the module lower case: link.exe took "MSWSOCK.AcceptEx" for a symbol, not a forward)
        module, func = e.forwarder.decode().split('.', 1)
        lines.append(f'    {name}={module.lower()}.{func} @{e.ordinal}')
    else:
        raise SystemExit(f'{name} is wsock32\'s own and not handled')
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'wsock32.def'), 'w') as f:
    f.write('\n'.join(lines) + '\n')
print(len(lines) - 2, 'exports')
