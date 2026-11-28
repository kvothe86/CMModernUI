"""Put a euro sign at character 0xA4 of TrueType-style font files (.ttf, and the game's .t2k, which are
sfnt files): the game's currency string is "?uro" with 0xA4 as the first byte (cm25, Restructured packs),
and a font draws the sign only if its glyph for 0xA4 is a euro.

usage: python euro_fonts.py [--dry] <font file>...
Each changed file is first copied to <file>.before_euro (once). The glyph comes from the font's own
U+20AC when it has one, else from a donor (Arial's euro, scaled to the font's em)."""
import sys, os, shutil
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.recordingPen import DecomposingRecordingPen

DONOR = r"C:\Windows\Fonts\arial.ttf"


def donor_glyph(font):
    don = TTFont(DONOR)
    dset = don.getGlyphSet()
    dname = don.getBestCmap()[0x20AC]
    rec = DecomposingRecordingPen(dset)
    dset[dname].draw(rec)
    s = font["head"].unitsPerEm / don["head"].unitsPerEm
    pen = TTGlyphPen(None)
    rec.replay(TransformPen(pen, (s, 0, 0, s, 0, 0)))
    return pen.glyph(), int(don["hmtx"][dname][0] * s)


def fix(path, dry):
    f = TTFont(path)
    if "glyf" not in f:
        return "no glyf table (skipped)"
    cmap = f.getBestCmap()
    a4, e = cmap.get(0xA4), cmap.get(0x20AC)
    if e is None:
        e = cmap.get(0x80) if cmap.get(0x80, "").lower() in ("uni0080", "euro") else None
    if e == a4 and a4 is not None:
        return "already a euro at 0xA4"
    if e is not None:  # the font has the glyph: just let 0xA4 reach it, nothing is redrawn
        for t in f["cmap"].tables:
            if t.isUnicode():
                t.cmap[0xA4] = e
        how = f"0xA4 now points at the font's own euro ({e})"
    elif a4 is None:
        return "no euro and no glyph at 0xA4 (skipped)"
    else:
        glyf, hmtx = f["glyf"], f["hmtx"]
        g, adv = donor_glyph(f)
        g.recalcBounds(glyf)
        glyf[a4] = g
        hmtx[a4] = (adv, g.xMin)
        how = "drawn from Arial's euro"
    if not dry:
        if not os.path.exists(path + ".before_euro"):
            shutil.copyfile(path, path + ".before_euro")
        f.save(path)
    return how


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--dry"]
    for p in args:
        try:
            print(os.path.basename(p), "->", fix(p, "--dry" in sys.argv))
        except Exception as ex:
            print(os.path.basename(p), "-> FAILED", type(ex).__name__, ex)
