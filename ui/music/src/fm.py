"""Eight original orchestral menu tracks for cmui (football manager feel). Themes are hand-written
scale degrees (1 = tonic, 8 = an octave up, '7#' raised, 'r' rest) over hand-written chord plans.
usage: python fm.py <n|all>   -> out/NN Title.mp3 + out/NN.json (sections)"""
import json, os, subprocess, sys
import numpy as np
from scipy.signal import butter, sosfilt, oaconvolve
from sfz import SR
from inst import load

TRACKS = [
    dict(n=1, title="Kick Off", tonic=62, mode="maj", bpm=132, feel="drive",
         ca="I V vi IV I V IV V", cb="vi IV I V vi IV I V",
         ta="5:1.5 5:.5 6:1 5:1 | 7:1.5 7:.5 6:1 5:1 | 6:1.5 6:.5 8:1 6:1 | 6:1 5:1 4:2 | 5:1.5 5:.5 6:1 5:1 | 7:1.5 8:.5 9:1 7:1 | 8:1.5 8:.5 6:1 4:1 | 5:4",
         tb="3:1 3:1 3:.5 2:.5 1:1 | 1:1 1:1 2:.5 3:.5 4:1 | 5:2 3:1 1:1 | 2:3 r:1 | 3:1 3:1 3:.5 4:.5 5:1 | 6:1 6:1 5:.5 4:.5 3:1 | 5:1.5 4:.5 3:1 2:1 | 2:2 0:2"),
    dict(n=2, title="Matchday Anthem", tonic=58, mode="maj", bpm=100, feel="anthem",
         ca="I IV vi V I IV V I", cb="vi IV I V vi IV V V",
         ta="3:2 2:1 1:1 | 6:2 5:1 4:1 | 3:1 5:1 6:2 | 5:3 r:1 | 3:2 2:1 1:1 | 6:1 8:1 8:1 6:1 | 7:1.5 6:.5 5:1 7:1 | 8:4",
         tb="6:1 6:1 5:1 6:1 | 8:2 6:2 | 5:1 5:1 3:1 5:1 | 2:3 r:1 | 6:1 6:1 5:1 6:1 | 8:1 9:1 8:1 6:1 | 7:2 9:2 | 7:2 5:2"),
    dict(n=3, title="Deadline Day", tonic=64, mode="min", bpm=148, feel="gallop",
         ca="i VI III VII i VI iv V", cb="VI VII i i VI VII V V",
         ta="5:.5 5:.5 5:1 8:1 7:1 | 6:1 5:1 3:2 | 5:.5 5:.5 5:1 7:1 8:1 | 7:2 4:2 | 5:.5 5:.5 5:1 8:1 10:1 | 10:1 9:1 8:2 | 8:1 6:1 4:1 6:1 | 7#:2 5:2",
         tb="3:1.5 3:.5 5:1 6:1 | 7:1.5 7:.5 6:1 4:1 | 5:4 | 5:1 6:1 7:1 8:1 | 10:1.5 10:.5 9:1 8:1 | 9:1.5 9:.5 8:1 7:1 | 7#:2 9:2 | 5:3 r:1"),
    dict(n=4, title="Cup Run", tonic=67, mode="maj", bpm=138, feel="march",
         ca="I I IV I vi ii V I", cb="IV V iii vi IV V I I",
         ta="1:1 3:1 5:1.5 3:.5 | 5:1 8:1 7:1 5:1 | 6:1 8:1 8:1.5 6:.5 | 5:3 r:1 | 6:1 8:1 10:1.5 8:.5 | 9:1 8:1 6:1 4:1 | 5:1.5 7:.5 9:1 7:1 | 8:3 r:1",
         tb="6:1.5 6:.5 8:2 | 7:1.5 7:.5 9:2 | 7:1 5:1 3:1 5:1 | 6:3 r:1 | 6:1.5 6:.5 8:1 6:1 | 7:1 9:1 12:2 | 10:2 8:2 | 8:4"),
    dict(n=5, title="Under the Floodlights", tonic=69, mode="min", bpm=116, feel="drive",
         ca="i iv VII III VI iv V V", cb="III VII i i VI III iv V",
         ta="5:2 3:1 1:1 | 1:1 4:1 6:2 | 7:1 9:1 11:2 | 10:2 8:1 7:1 | 8:1.5 6:.5 8:1 10:1 | 11:1.5 10:.5 8:1 6:1 | 7#:2 9:1 7#:1 | 5:4",
         tb="3:1 5:1 7:1 5:1 | 7:2 4:2 | 3:1.5 2:.5 1:2 | 5:4 | 6:1 8:1 10:2 | 10:1 9:1 7:2 | 8:1 6:1 4:2 | 7#:3 r:1"),
    dict(n=6, title="Open-Top Bus", tonic=65, mode="maj", bpm=106, feel="march",
         ca="I IV I V I IV V I", cb="vi iii IV I ii V I V",
         ta="1:.75 1:.25 3:1 5:2 | 6:.75 6:.25 4:1 8:2 | 5:.75 5:.25 3:1 1:1 3:1 | 2:2 5:2 | 1:.75 1:.25 3:1 5:2 | 6:.75 6:.25 8:1 11:2 | 9:1 7:1 5:1 7:1 | 8:3 r:1",
         tb="6:2 5:1 3:1 | 5:2 3:1 2:1 | 4:1 6:1 8:1 6:1 | 5:3 r:1 | 2:1 4:1 6:1 9:1 | 7:2 5:2 | 8:1.5 7:.5 6:1 5:1 | 5:2 2:2"),
    dict(n=7, title="Academy Dreams", tonic=63, mode="maj", bpm=96, feel="light",
         ca="I vi IV V I iii IV V", cb="IV V iii vi ii V I I",
         ta="5:1 3:.5 2:.5 1:2 | 3:1 6:1 8:2 | 6:1 4:1 6:1 8:1 | 7:2 5:2 | 5:1 3:.5 2:.5 1:2 | 3:1 5:1 7:2 | 8:1 6:1 4:1 6:1 | 5:4",
         tb="8:1.5 6:.5 4:2 | 5:1.5 7:.5 9:2 | 7:1 5:1 3:2 | 6:3 r:1 | 4:1 6:1 9:2 | 9:1 7:1 5:2 | 8:2 5:2 | 8:3 r:1"),
    dict(n=8, title="Final Whistle", tonic=62, mode="min", bpm=124, feel="gallop",
         ca="i VI III VII i iv V i", cb="VI VII III VI iv V i V",
         ta="1:1 3:1 5:1.5 8:.5 | 6:2 5:1 3:1 | 3:1 5:1 7:1 5:1 | 4:2 7:2 | 8:1.5 7:.5 5:1 8:1 | 11:1 10:1 8:1 6:1 | 7#:2 9:1 7#:1 | 8:3 r:1",
         tb="10:2 8:1 6:1 | 7:2 11:2 | 10:1.5 9:.5 7:1 5:1 | 6:3 r:1 | 4:1 6:1 8:1 11:1 | 9:2 7#:2 | 8:1 5:1 3:1 5:1 | 5:3 r:1"),
]

SCALE = {"maj": [0, 2, 4, 5, 7, 9, 11], "min": [0, 2, 3, 5, 7, 8, 10]}
CHORD = {"I": (0, "M"), "ii": (2, "m"), "iii": (4, "m"), "IV": (5, "M"), "V": (7, "M"), "vi": (9, "m"),
         "i": (0, "m"), "iv": (5, "m"), "III": (3, "M"), "VI": (8, "M"), "VII": (10, "M")}
GAIN = dict(vn1=1, vn2=1, va=1, vc=1, cb=1, trem=0.9, vc_st=1.1, cb_st=1.1, hn=1, hn_st=1, tpt=0.95, tbn=1, tbn_st=1,
            tuba=1, choir=1.3, timp=1.6, drums=1.5, cym=1.2, harp=0.85, pno=1.1, cel=0.8)
PAN = dict(vn1=-0.5, vn2=-0.2, va=0.15, vc=0.4, cb=0.55, trem=-0.45, vc_st=0.4, cb_st=0.55, hn=-0.3, hn_st=-0.3,
           tpt=0.25, tbn=0.4, tbn_st=0.4, tuba=0.5, harp=-0.55, cel=-0.35, choir=0, drums=0, pno=0, timp=0.1, cym=0)


def theme(s):
    bars = []
    for bar in s.split("|"):
        notes = []
        for tok in bar.split():
            d, b = tok.split(":")
            notes.append((None if d == "r" else d, float(b)))
        assert abs(sum(b for _, b in notes) - 4) < 1e-6, (s, bar)
        bars.append(notes)
    assert len(bars) == 8
    return bars


class Track:
    def __init__(self, spec, orch):
        self.s, self.o = spec, orch
        self.beat = 60 / spec["bpm"]
        self.bar = 4 * self.beat
        self.ev = []           # (t, inst, midi, dur, vel, rel)
        self.sections = []
        self.rng = np.random.default_rng(spec["n"])
        self.shift = 0         # key change in semitones
        self.ta, self.tb = theme(spec["ta"]), theme(spec["tb"])
        self.ca, self.cb = spec["ca"].split(), spec["cb"].split()

    # --- pitch helpers
    def deg(self, d, base):
        sharp = d.endswith("#")
        k = int(d.rstrip("#"))
        o, i = divmod(k - 1, 7)
        return base + 12 * o + SCALE[self.s["mode"]][i] + (1 if sharp else 0) + self.shift

    def mel_base(self):
        m = 60 + (self.s["tonic"] % 12)
        return m + 12 if m < 65 else m

    def chord_pcs(self, sym, final=False):
        root, q = CHORD[sym]
        if final and sym in ("i", "I"):
            q = "M"            # picardy third at the very end of a minor track
        pc = (self.s["tonic"] + root + self.shift) % 12
        third = 4 if q == "M" else 3
        return pc, [pc, (pc + third) % 12, (pc + 7) % 12]

    def fold(self, inst, m):
        i = self.o[inst]
        while m < i.lo: m += 12
        while m > i.hi: m -= 12
        return m

    def add(self, t, inst, m, dur, vel, rel=None):
        self.ev.append((t, inst, self.fold(inst, m), dur, min(1.0, vel), rel))

    # --- writing helpers
    def line(self, t0, bars, insts, octs, vel, accent=True):
        t = t0
        flat = [(n, b) for bar in bars for n, b in bar]
        for k, (d, b) in enumerate(flat):
            dur = b * self.beat
            if d is not None:
                nxt = flat[k + 1][0] if k + 1 < len(flat) else None
                legato = nxt is not None
                v = vel * (1.06 if accent and abs((t - t0) / self.beat % 2) < 1e-6 else 1.0)
                for inst, o in zip(insts, octs):
                    m = self.deg(d, self.mel_base()) + o
                    self.add(t, inst, m, dur + (0.06 if legato else 0), v, 0.12 if legato else None)
            t += dur

    def voicing(self, pcs, low):
        out, m = [], low
        while len(out) < 4:
            if m % 12 in pcs: out.append(m)
            m += 1
        return out

    def pad(self, t, sym, insts, low, vel, beats=4, final=False):
        _, pcs = self.chord_pcs(sym, final)
        for inst in insts:
            for m in self.voicing(pcs, low[inst] if isinstance(low, dict) else low):
                self.add(t, inst, m, beats * self.beat + 0.05, vel)

    def bass(self, t, sym, insts, vel, beats=4):
        pc, _ = self.chord_pcs(sym)
        for inst in insts:
            self.add(t, inst, 36 + pc if pc < 8 else 24 + pc, beats * self.beat + 0.05, vel)

    def ostinato(self, t, sym, vel, feel):
        pc, pcs = self.chord_pcs(sym)
        root = 36 + pc if pc < 8 else 24 + pc
        fifth = root + 7
        if feel == "gallop":
            hits = [(0, 0.5), (0.5, 0.25), (0.75, 0.25)] * 4
            hits = [(i // 3 + o, d) for i, (o, d) in enumerate(hits)]
        elif feel == "march":
            hits = [(x * 0.5, 0.5) for x in range(8)]
        elif feel == "anthem":
            hits = [(0, 1), (1, 1), (2, 1), (3, 0.5), (3.5, 0.5)]
        else:   # drive: straight eighths, accents 3+3+2
            hits = [(x * 0.5, 0.5) for x in range(8)]
        acc = {0, 1.5, 3}
        for k, (o, d) in enumerate(hits):
            m = fifth if (feel == "drive" and k in (2, 5)) else root
            v = vel * (1.15 if o in acc else 0.85)
            self.add(t + o * self.beat, "vc_st", m + 12, d * self.beat, v)
            self.add(t + o * self.beat, "cb_st", m, d * self.beat, v * 0.9)

    def drums(self, t, sym, vel, feel, fill=False):
        b = self.beat
        pc, _ = self.chord_pcs(sym)
        timp = 36 + (pc + 12 - 0) % 12 if 36 + pc <= 48 else 36 + pc - 12
        if feel == "march":
            for x in range(8):
                self.add(t + x * 0.5 * b, "drums", 49, 0.2, vel * (0.85 if x % 2 else 0.6))
            for x in (0, 2):
                self.add(t + x * b, "drums", 48, 0.4, vel)
        elif feel == "anthem":
            self.add(t, "drums", 48, 0.4, vel)
            self.add(t + 2 * b, "drums", 49, 0.3, vel * 0.9)
            self.add(t + 2.5 * b, "drums", 48, 0.4, vel * 0.7)
        elif feel == "light":
            self.add(t, "drums", 48, 0.4, vel * 0.7)
            self.add(t + 2 * b, "drums", 48, 0.4, vel * 0.5)
        else:   # drive / gallop: bass drum 1 and 3, snare backbeat
            for x in (0, 2):
                self.add(t + x * b, "drums", 48, 0.4, vel)
            for x in (1, 3):
                self.add(t + x * b, "drums", 49, 0.3, vel * 0.95)
            if feel == "gallop":
                self.add(t + 2.5 * b, "drums", 48, 0.4, vel * 0.75)
        for x in (0, 2.5) if feel != "light" else (0,):
            self.add(t + x * b, "timp", timp, 0.6, vel * 0.85)
        if fill:
            for x in range(4):
                self.add(t + (3 + x * 0.25) * b, "drums", 49, 0.15, vel * (0.6 + 0.12 * x))

    def timp_roll(self, t, beats, sym, v0, v1):
        pc, _ = self.chord_pcs(sym)
        m = 36 + pc if 36 + pc <= 48 else 24 + pc
        n = int(beats * 4)
        for k in range(n):
            self.add(t + k * self.beat / 4, "timp", m, 0.2, v0 + (v1 - v0) * k / max(1, n - 1))

    def crash(self, t, vel=0.95):
        self.add(t, "cym", 55, 2.5, vel)

    def swell(self, t_end):
        self.ev.append((t_end, "SWELL", 48, 0, 0.8, None))

    def harp_arp(self, t, sym, vel):
        _, pcs = self.chord_pcs(sym)
        notes = self.voicing(pcs, 55) + self.voicing(pcs, 67)
        for k, m in enumerate(notes):
            self.add(t + k * self.beat / 2, "harp", m, self.beat * 2, vel)

    # --- the arc: 128 bars
    def section(self, name, bars, desc):
        self.sections.append([round(self.t + 0.3, 2), name, desc])
        t = self.t
        self.t += bars * self.bar
        return t

    def compose(self):
        s, feel = self.s, self.s["feel"]
        B = self.bar
        self.t = 0.0
        lowpad = {"va": 55, "hn": 53, "choir": 57, "vn2": 62, "tbn": 48, "trem": 67}
        # Intro (8): fanfare of the theme's head on brass over a timpani roll
        t = self.section("Fanfare", 8, "horns, trumpets, timpani roll")
        head = [self.ta[0], self.ta[1]]
        for rep in range(2):
            t0 = t + rep * 4 * B
            self.timp_roll(t0, 8, self.ca[0], 0.3, 0.75)
            self.line(t0 + 2 * B, head, ["hn", "tpt", "tbn"], [-12, 0, -24], 0.78)
            self.pad(t0, self.ca[0], ["va", "vc"], {"va": 55, "vc": 48}, 0.45, 8)
            self.bass(t0, self.ca[0], ["cb"], 0.5, 8)
        self.swell(t + 8 * B)
        # A1 (8): theme on violins, ostinato starts
        t = self.section("Kick-off", 8, "violins, staccato celli + basses, drums")
        self.crash(t, 0.85)
        self.line(t, self.ta, ["vn1", "vn2"], [0, -12], 0.62)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.6, feel); self.pad(tb, c, ["va"], 55, 0.38); self.bass(tb, c, ["cb"], 0.45)
            self.drums(tb, c, 0.55, feel, fill=(k == 7))
        # A2 (8): theme on brass, strings pad
        t = self.section("Brass Statement", 8, "horns + trumpets carry the theme, choir pad")
        self.crash(t)
        self.line(t, self.ta, ["tpt", "hn", "tbn"], [0, -12, -24], 0.74)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.7, feel); self.pad(tb, c, ["vn2", "choir"], lowpad, 0.5); self.bass(tb, c, ["cb", "tuba"], 0.6)
            self.drums(tb, c, 0.7, feel, fill=(k == 7))
        # B1 (8): second theme on horns + celli
        t = self.section("Second Half", 8, "second theme on horns + celli, violins pad")
        self.line(t, self.tb, ["hn", "vc"], [-12, -12], 0.66)
        for k, c in enumerate(self.cb):
            tb = t + k * B
            self.ostinato(tb, c, 0.6, feel); self.pad(tb, c, ["vn2", "va"], lowpad, 0.42); self.bass(tb, c, ["cb"], 0.5)
            self.drums(tb, c, 0.6, feel, fill=(k == 7))
        # A3 (8): theme on violins + horns, fuller
        t = self.section("Momentum", 8, "violins + horns in octaves, full drums")
        self.crash(t)
        self.line(t, self.ta, ["vn1", "vn2", "hn"], [0, -12, -12], 0.72)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.72, feel); self.pad(tb, c, ["va", "choir"], lowpad, 0.5); self.bass(tb, c, ["cb", "tuba"], 0.62)
            self.drums(tb, c, 0.75, feel, fill=(k == 7))
        # Quiet (16): harp, celeste theme, soft strings, no drums
        t = self.section("Half Time", 16, "harp, celeste, soft strings and choir")
        self.line(t, self.tb, ["cel"], [0], 0.5)
        self.line(t + 8 * B, self.ta, ["vc"], [-12], 0.45)
        for k, c in enumerate(self.cb + self.ca):
            tb = t + k * B
            self.harp_arp(tb, c, 0.42); self.pad(tb, c, ["va", "choir"] if k >= 8 else ["va"], lowpad, 0.3); self.bass(tb, c, ["cb"], 0.32)
        # Build (8): ostinato alone, rising timpani, trem strings, swell
        t = self.section("Second Wind", 8, "staccato ostinato, tremolo violins, timpani build")
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.5 + 0.04 * k, feel); self.bass(tb, c, ["cb"], 0.45 + 0.03 * k)
            self.pad(tb, c, ["trem"], 67, 0.35 + 0.05 * k)
            if k >= 4: self.drums(tb, c, 0.5 + 0.06 * (k - 4), feel, fill=(k == 7))
        self.line(t + 4 * B, self.ta[:4], ["hn"], [-12], 0.6)
        self.timp_roll(t + 6 * B, 8, self.ca[7], 0.4, 0.9)
        self.swell(t + 8 * B)
        # B2 (8): second theme tutti
        t = self.section("Pressure", 8, "second theme on violins + trumpets, brass stabs")
        self.crash(t)
        self.line(t, self.tb, ["vn1", "vn2", "tpt"], [0, -12, -12], 0.76)
        for k, c in enumerate(self.cb):
            tb = t + k * B
            self.ostinato(tb, c, 0.78, feel); self.pad(tb, c, ["hn", "choir"], lowpad, 0.55); self.bass(tb, c, ["cb", "tuba"], 0.66)
            self.drums(tb, c, 0.8, feel, fill=(k == 7))
        # A4 (8): tutti theme, choir on the melody
        t = self.section("Glory", 8, "tutti: strings, brass and choir on the theme")
        self.crash(t, 1.0)
        self.line(t, self.ta, ["vn1", "vn2", "tpt", "hn", "tbn", "choir"], [0, -12, 0, -12, -24, -12], 0.85)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.85, feel); self.pad(tb, c, ["va"], 55, 0.6); self.bass(tb, c, ["cb", "tuba"], 0.75)
            self.drums(tb, c, 0.88, feel, fill=(k == 7))
        self.swell(t + 8 * B)
        # Key change up a tone (24): A, B, A tutti
        self.shift = 2
        t = self.section("Extra Time", 8, "a tone higher: theme on trumpets + violins")
        self.crash(t, 1.0)
        self.line(t, self.ta, ["vn1", "vn2", "tpt", "hn", "choir"], [0, -12, 0, -12, -12], 0.88)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.86, feel); self.pad(tb, c, ["va", "tbn"], lowpad, 0.6); self.bass(tb, c, ["cb", "tuba"], 0.78)
            self.drums(tb, c, 0.9, feel, fill=(k == 7))
        t = self.section("Last Attack", 8, "second theme, horns and choir, tremolo strings")
        self.line(t, self.tb, ["hn", "choir", "vn1"], [-12, -12, 0], 0.82)
        for k, c in enumerate(self.cb):
            tb = t + k * B
            self.ostinato(tb, c, 0.82, feel); self.pad(tb, c, ["trem"], 67, 0.5); self.bass(tb, c, ["cb", "tuba"], 0.72)
            self.drums(tb, c, 0.86, feel, fill=(k == 7))
        t = self.section("Trophy", 8, "full orchestra, the theme one last time")
        self.crash(t, 1.0)
        self.line(t, self.ta, ["vn1", "vn2", "tpt", "hn", "tbn", "choir"], [0, -12, 0, -12, -24, -12], 0.92)
        for k, c in enumerate(self.ca):
            tb = t + k * B
            self.ostinato(tb, c, 0.9, feel); self.pad(tb, c, ["va"], 55, 0.65); self.bass(tb, c, ["cb", "tuba"], 0.8)
            self.drums(tb, c, 0.95, feel, fill=(k == 7))
        # Outro (8): the head twice, then the last chord held
        t = self.section("Final Whistle", 8, "fanfare head, held tonic chord, cymbal")
        self.crash(t, 1.0)
        self.line(t, head, ["tpt", "hn", "tbn", "vn1"], [0, -12, -24, 0], 0.9)
        for k in range(2):
            self.pad(t + k * B, self.ca[k], ["va", "choir"], lowpad, 0.6); self.bass(t + k * B, self.ca[k], ["cb", "tuba"], 0.75)
        end = t + 2 * B
        self.timp_roll(end, 8, "I" if s["mode"] == "maj" else "i", 0.5, 0.95)
        self.pad(end + 2 * B, "I" if s["mode"] == "maj" else "i", ["vn1", "vn2", "va", "hn", "tpt", "tbn", "choir"],
                 {"vn1": 74, "vn2": 67, "va": 57, "hn": 53, "tpt": 64, "tbn": 48, "choir": 57}, 0.85, 12, final=True)
        self.bass(end + 2 * B, "I" if s["mode"] == "maj" else "i", ["cb", "tuba", "vc"], 0.8, 12)
        self.crash(end + 2 * B, 1.0)
        self.length = end + 2 * B + 12 * self.beat + 5.0

    # --- render
    def render(self, path):
        o = self.o
        n = int((self.length + 1) * SR)
        mix = np.zeros((n, 2), np.float32)
        roll = o["cym"].render(48, 0.8, 4.0)
        peak = int(np.argmax(np.abs(roll).sum(axis=1)))
        ramp = np.linspace(0.2, 1, peak)[:, None]
        for (t, inst, m, dur, vel, rel) in self.ev:
            if inst == "SWELL":
                y = roll[:peak] * ramp * 1.2
                s0 = int((t + 0.3) * SR) - peak
            else:
                jitter = 0 if inst in ("timp", "drums", "cym") else self.rng.normal(0, 0.006)
                y = o[inst].render(m, vel, dur, rel)
                if y is None:
                    continue
                y = y * GAIN[inst]
                p = PAN.get(inst, 0)
                if p:
                    y = y.copy(); y[:, 0] *= 1 - max(0, p) * 0.7; y[:, 1] *= 1 + min(0, p) * 0.7
                s0 = int((t + 0.3 + jitter) * SR)
            if s0 < 0:
                y = y[-s0:]; s0 = 0
            e = min(n, s0 + len(y))
            mix[s0:e] += y[:e - s0]
        # hall
        ir_n = int(3.6 * 1.6 * SR)
        noise = np.random.default_rng(7).standard_normal((ir_n, 2)).astype(np.float32)
        noise = sosfilt(butter(4, 5200, "low", fs=SR, output="sos"), noise, axis=0)
        ir = noise * np.exp(-np.arange(ir_n) / (3.6 / 6.9 * SR))[:, None]
        ir = np.vstack([np.zeros((int(0.012 * SR), 2)), ir]).astype(np.float32)
        ir /= np.sqrt((ir ** 2).sum(axis=0))
        wet = np.stack([oaconvolve(mix[:, c], ir[:, c])[:n] for c in range(2)], axis=1)
        out = 0.78 * mix + 0.38 * wet * (np.abs(mix).max() / max(1e-9, np.abs(wet).max()))
        out = sosfilt(butter(2, 28, "high", fs=SR, output="sos"), out, axis=0)
        f = int(5 * SR)
        out[-f:] *= np.linspace(1, 0, f)[:, None]
        out = np.tanh(1.5 * out / np.abs(out).max())
        out = (out / np.abs(out).max() * 0.89).astype(np.float32)
        raw = path + ".f32"
        out.tofile(raw)
        subprocess.run(["ffmpeg", "-y", "-v", "quiet", "-f", "f32le", "-ar", str(SR), "-ac", "2", "-i", raw,
                        "-b:a", "224k", "-metadata", f"title={self.s['title']}", "-metadata", "artist=cmui orchestra", path], check=True)
        os.remove(raw)


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    orch = load()
    os.makedirs("out", exist_ok=True)
    for spec in TRACKS:
        if which != "all" and str(spec["n"]) != which:
            continue
        tr = Track(spec, orch)
        tr.compose()
        path = f"out/{spec['n']:02d} {spec['title']}.mp3"
        tr.render(path)
        json.dump({"title": spec["title"], "length": round(tr.length, 1), "sections": tr.sections, "events": len(tr.ev)},
                  open(f"out/{spec['n']:02d}.json", "w"), indent=1)
        print(spec["n"], spec["title"], round(tr.length / 60, 2), "min", len(tr.ev), "events", flush=True)
