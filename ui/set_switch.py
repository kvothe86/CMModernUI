"""set_switch.py folder index [0|1]: sets one cmui switch byte (0-based, after "cmui.dll" and
"CMUI" at the exe's entry stub) in the folder's cm0102_ui.exe as it is now, after a copy to
cm0102_ui_before_switch<index>.exe; checks that no other byte changed. Never rebuild an exe
with make_exe.py to add a switch: that drops patches made to the exe itself (a resolution)."""
import os
import shutil
import sys
import pefile

folder, index = sys.argv[1], int(sys.argv[2])
value = int(sys.argv[3]) if len(sys.argv) > 3 else 1
exe = os.path.join(folder, 'cm0102_ui.exe')
backup = os.path.join(folder, f'cm0102_ui_before_switch{index}.exe')
pe = pefile.PE(exe, fast_load=True)
stub = pe.OPTIONAL_HEADER.AddressOfEntryPoint
assert pe.get_data(stub + 16, 16) == b'cmui.dll\0\0\0\0CMUI', 'no cmui stub'
at = pe.get_offset_from_rva(stub + 0x20 + index)
pe.close()
data = bytearray(open(exe, 'rb').read())
assert data[at] in (0, 1), f'byte {index} is {data[at]}: not a switch'
if data[at] == value:
    print(exe, 'switch', index, 'already', value)
    sys.exit()
if not os.path.exists(backup):
    shutil.copy2(exe, backup)
data[at] = value
open(exe, 'wb').write(data)
old = open(backup, 'rb').read()
diff = [i for i in range(len(old)) if old[i] != data[i]]
assert len(old) == len(data) and diff == [at], f'other bytes changed: {diff[:8]}'
print(exe, 'switch', index, '=', value, 'at file offset', hex(at))
