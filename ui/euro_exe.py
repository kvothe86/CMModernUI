"""Currency sign of the euro in the game exes: "Euro" at file offset 0x58b458 becomes "<A4>uro", the byte
the community packs and cm25 use, which the patched fonts (euro_fonts.py, fnt_euro.py) draw as the sign.

usage: python euro_exe.py [--undo] <exe>...      (copies to <exe>.before_euro once; refuses an unknown exe)"""
import sys, os, shutil

# (offset, plain bytes, euro bytes): the currency's name in Game Options, and the sign before amounts
# (the setter 0x43f7f0 case 7 copies it to 0xad9ca8; the original 3.9.68 has 0xA4 here, a patch made it "E")
SITES = [(0x58B458, b"Euro", b"\xa4uro"), (0x58B37F, b"\x00E\x00", b"\x00\xa4\x00")]


def patch(path, undo):
    out = []
    data = open(path, "rb").read()
    for off, plain, euro in SITES:
        want, now = (plain, euro) if undo else (euro, plain)
        cur = data[off:off + len(want)]
        if cur == want:
            out.append(f"0x{off:x} already")
            continue
        if cur != now:
            out.append(f"0x{off:x} unknown {cur!r}, left alone")
            continue
        if not os.path.exists(path + ".before_euro"):
            shutil.copyfile(path, path + ".before_euro")
        with open(path, "r+b") as f:
            f.seek(off)
            f.write(want)
        out.append(f"0x{off:x} {'undone' if undo else 'patched'}")
    return "; ".join(out)


if __name__ == "__main__":
    for p in [a for a in sys.argv[1:] if a != "--undo"]:
        try:
            print(os.path.basename(p), "->", patch(p, "--undo" in sys.argv))
        except OSError as ex:
            print(os.path.basename(p), "-> FAILED", ex)
