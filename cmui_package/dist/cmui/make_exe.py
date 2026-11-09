"""Make cm0102_ui.exe: a copy of the played cm0102.exe whose entry point first loads
cmui.dll (LoadLibraryA from the import table), then jumps to the game's own entry point.
The stub sits in free space at the end of the .nick section the community patcher adds; an exe
without one (the untouched 2001 exe) gets a small .cmui section of its own for it. After the
stub come the mark "CMUI" and one switch byte per feature (all on here; cmui.ini switches them
off). If cmui.dll is missing the game starts as usual.

usage: python make_exe.py "<game folder>"
"""
import os
import struct
import sys
import pefile

NICK_STUB = 0xdf0000
FEATURES = 40


def align(v, a):
    return (v + a - 1) // a * a


def add_section(data):
    """Append an executable 0x1000-byte section .cmui; its virtual address."""
    pe = pefile.PE(data=bytes(data), fast_load=True)
    fa, sa = pe.OPTIONAL_HEADER.FileAlignment, pe.OPTIONAL_HEADER.SectionAlignment
    last = pe.sections[-1]
    header = last.get_file_offset() + 40
    assert header + 40 <= pe.OPTIONAL_HEADER.SizeOfHeaders, 'no room for another section header'
    va = align(last.VirtualAddress + max(last.Misc_VirtualSize, last.SizeOfRawData), sa)
    raw = align(len(data), fa)
    data += bytes(raw - len(data) + 0x1000)
    data[header:header + 40] = struct.pack('<8sIIIIIIHHI', b'.cmui', 0x1000, va, 0x1000, raw, 0, 0, 0, 0, 0x60000020)
    count_at = pe.FILE_HEADER.get_file_offset() + 2
    struct.pack_into('<H', data, count_at, pe.FILE_HEADER.NumberOfSections + 1)
    struct.pack_into('<I', data, pe.OPTIONAL_HEADER.get_file_offset() + 56, align(va + 0x1000, sa))  # SizeOfImage
    return va


def loader(pe, stub):
    """The stub's bytes at `stub` (without the switches): push "cmui.dll"; call [LoadLibraryA];
    jmp to the game's entry; the name; the mark."""
    pe.parse_data_directories()
    entry = pe.OPTIONAL_HEADER.ImageBase + pe.OPTIONAL_HEADER.AddressOfEntryPoint
    assert entry != stub, 'this cm0102.exe already loads cmui (the CM0102Patcher cmui patches): start it as it is'
    load = next(i.address for e in pe.DIRECTORY_ENTRY_IMPORT for i in e.imports if i.name == b'LoadLibraryA')
    code = (b'\x68' + struct.pack('<I', stub + 16)              # push "cmui.dll"
            + b'\xff\x15' + struct.pack('<I', load)            # call [LoadLibraryA]
            + b'\xe9' + struct.pack('<i', entry - (stub + 16)))  # jmp to the game's entry
    return code + b'\x90' * (16 - len(code)) + b'cmui.dll\0\0\0\0CMUI'


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    game = sys.argv[1]
    src, dst = os.path.join(game, 'cm0102.exe'), os.path.join(game, 'cm0102_ui.exe')
    data = bytearray(open(src, 'rb').read())
    pe = pefile.PE(data=bytes(data), fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    if any(s.Name.startswith(b'.nick') for s in pe.sections):
        stub = NICK_STUB
    else:
        stub = base + add_section(data)
        pe = pefile.PE(data=bytes(data), fast_load=True)
    code = loader(pe, stub) + b'\x01' * FEATURES
    off = pe.get_offset_from_rva(stub - base)
    assert pe.__data__[off:off + len(code)] == bytes(len(code)), 'stub space not free'
    pe.set_bytes_at_offset(off, code)
    pe.OPTIONAL_HEADER.AddressOfEntryPoint = stub - base
    pe.write(dst)
    print(f'{dst}: entry -> stub {stub:#x}')


if __name__ == '__main__':
    main()
