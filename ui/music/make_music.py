"""make_music.py: renders cmui's menu music (switch `music`, src\\music.rs) into music\\out\\:
four original synth tracks in the spirit of late-90s football-management menus, each one
seamless loop (events past the end wrap round to the start, so do reverb and echo tails).
Written as mono 22050 Hz IMA ADPCM WAV (4 bits a sample, about 11 KB a second; Windows' own
players play it too). numpy only.

usage: python make_music.py [--pcm]   --pcm also writes 16-bit previews to music\\out\\pcm
"""
import os
import struct
import sys

import numpy as np

SR = 22050
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'out')
rng = np.random.default_rng(1998)

# ---------------------------------------------------------------- notes and chords
PC = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
QUALITY = {'': (0, 4, 7), 'm': (0, 3, 7), '7': (0, 4, 7, 10), 'maj7': (0, 4, 7, 11), 'm7': (0, 3, 7, 10), 'sus4': (0, 5, 7)}


def pitch(name):
    """'F#5' -> midi 78"""
    pc, rest = PC[name[0]], name[1:]
    while rest[:1] in ('#', 'b'):
        pc += 1 if rest[0] == '#' else -1
        rest = rest[1:]
    return 12 * (int(rest) + 1) + pc


def chord(name):
    """'F#m7' -> (root pitch class, intervals)"""
    pc, rest = PC[name[0]], name[1:]
    if rest[:1] in ('#', 'b'):
        pc += 1 if rest[0] == '#' else -1
        rest = rest[1:]
    return pc % 12, QUALITY[rest]


def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def place(pc, lo):
    """the pitch of class pc in [lo, lo + 12)"""
    return lo + (pc - lo) % 12


# ---------------------------------------------------------------- synthesis
def tt(n):
    return np.arange(n) / SR


def osc_phase(f, n, vib=0.0, vib_delay=0.2):
    """phase 0..1 and per-sample increment; vib = vibrato depth in cents (5.5 Hz, faded in)"""
    t = tt(n)
    fr = np.full(n, f)
    if vib:
        ramp = np.clip((t - vib_delay) / 0.3, 0, 1)
        fr = f * 2 ** (vib / 1200 * np.sin(2 * np.pi * 5.5 * t) * ramp)
    inc = fr / SR
    return (np.cumsum(inc) + rng.random()) % 1.0, inc


def blep(p, dt):
    r = np.zeros_like(p)
    m = p < dt
    x = p[m] / dt[m]
    r[m] = x + x - x * x - 1
    m = p > 1 - dt
    x = (p[m] - 1) / dt[m]
    r[m] = x * x + x + x + 1
    return r


def saw(p, dt):
    return 2 * p - 1 - blep(p, dt)


def square(p, dt):
    return 0.5 * (saw(p, dt) - saw((p + 0.5) % 1.0, dt))


def filt(x, fc, q=0.707, kind='lp'):
    """2-pole filter through the FFT (padded so the response does not wrap)"""
    n = len(x)
    N = 1 << int(np.ceil(np.log2(n + SR // 8)))
    f = np.fft.rfftfreq(N, 1 / SR)
    s = 1j * f / fc
    H = (1 if kind == 'lp' else s * s) / (s * s + s / q + 1)
    return np.fft.irfft(np.fft.rfft(x, N) * H, N)[:n]


def adsr(gate, a, d, s, r):
    """envelope over gate seconds plus r seconds of release"""
    g, rn = max(int(gate * SR), 1), max(int(r * SR), 1)
    t = tt(g + rn)
    e = np.where(t < a, t / a, s + (1 - s) * np.exp(-(t - a) / max(d, 1e-4)))
    x = np.arange(rn) / rn
    e[g:] = e[g - 1] * np.exp(-4 * x) * (1 - x)
    return e


def detuned(f, n, cents, wave=saw, vib=0.0):
    x = np.zeros(n)
    for c in cents:
        p, dt = osc_phase(f * 2 ** (c / 1200), n, vib)
        x += wave(p, dt)
    return x / len(cents)


def bass(f, dur, vel):
    e = adsr(dur, 0.003, 0.35, 0.7, 0.05)
    n = len(e)
    p, dt = osc_phase(f, n)
    x = 0.55 * saw(p, dt) + 0.45 * square(p, dt) + 0.5 * np.sin(2 * np.pi * p)
    bright, dark = filt(x, 2000, 1.1), filt(x, 240, 0.8)
    y = dark + (bright - dark) * np.exp(-tt(n) / 0.08) * (0.4 + 0.6 * vel)
    return y * e * vel


def pad(f, dur, vel):
    e = adsr(dur, 0.35, 1.0, 0.85, 0.7)
    n = len(e)
    x = detuned(f, n, (-11, -4, 4, 11))
    return filt(x, 1400, 0.7) * e * vel


def strings(f, dur, vel):
    e = adsr(dur, 0.18, 1.0, 0.9, 0.45)
    n = len(e)
    x = detuned(f, n, (-7, 0, 7), vib=6)
    return filt(x, 2600, 0.6) * e * vel


def lead(f, dur, vel):
    e = adsr(dur, 0.008, 0.25, 0.8, 0.12)
    n = len(e)
    x = detuned(f, n, (-7, 7), square, vib=14) + 0.3 * detuned(2 * f, n, (0,), saw, vib=14)
    return filt(x, 3800, 0.9) * e * vel


def brass(f, dur, vel):
    e = adsr(dur, 0.03, 0.4, 0.85, 0.18)
    n = len(e)
    t = tt(n)
    x = detuned(f, n, (-10, 0, 10), vib=8)
    bright, dark = filt(x, 3400, 0.8), filt(x, 650, 0.8)
    swell = (1 - np.exp(-t / 0.07)) * (0.55 + 0.45 * np.exp(-t / 0.45))
    return (dark + (bright - dark) * swell * vel) * e * vel


def piano(f, dur, vel):
    """the house piano stab: inharmonic partials, the upper ones dying first"""
    rel = adsr(dur, 0.002, 10, 1, 0.16)
    n = len(rel)
    t = tt(n)
    y = np.zeros(n)
    k = 1
    while k * f < SR * 0.42 and k <= 16:
        fk = k * f * np.sqrt(1 + 0.0004 * k * k)
        tau = 1.4 / (1 + 0.4 * k) * (220 / f) ** 0.3
        y += np.sin(2 * np.pi * fk * t + rng.random() * 6.3) * np.exp(-t / tau) / k ** (1.35 - 0.4 * vel)
        k += 1
    y += filt(rng.standard_normal(n), 3000) * np.exp(-t / 0.004) * 0.25
    return y * rel * vel * 0.6


def epiano(f, dur, vel):
    """FM electric piano: index falls fast, a short metal tine on top"""
    rel = adsr(dur, 0.002, 10, 1, 0.3)
    n = len(rel)
    t = tt(n)
    index = (0.8 + 2.2 * vel) * np.exp(-t / 0.22) + 0.25
    y = np.sin(2 * np.pi * f * t + index * np.sin(2 * np.pi * f * t))
    y += 0.12 * np.sin(2 * np.pi * f * 14 * t) * np.exp(-t / 0.03)
    y *= np.exp(-t / 1.6) * (1 + 0.12 * np.sin(2 * np.pi * 4.5 * t))
    return y * rel * vel * 0.7


def pluck(f, dur, vel):
    rel = adsr(dur, 0.002, 10, 1, 0.06)
    n = len(rel)
    t = tt(n)
    p, dt = osc_phase(f, n)
    x = 0.7 * saw(p, dt) + 0.3 * square(p, dt)
    bright, dark = filt(x, 4200, 1.4), filt(x, 520, 1.0)
    y = dark + (bright - dark) * np.exp(-t / 0.05)
    return y * np.exp(-t / 0.22) * rel * vel


def bell(f, dur, vel):
    rel = adsr(dur, 0.001, 10, 1, 0.6)
    n = len(rel)
    t = tt(n)
    index = 2.0 * np.exp(-t / 0.5)
    y = np.sin(2 * np.pi * f * t + index * np.sin(2 * np.pi * f * 3.5 * t))
    return y * np.exp(-t / 0.9) * rel * vel * 0.6


def flute(f, dur, vel):
    e = adsr(dur, 0.06, 0.3, 0.85, 0.15)
    n = len(e)
    p, _ = osc_phase(f, n, vib=18, vib_delay=0.25)
    w = 2 * np.pi * p
    y = np.sin(w) + 0.18 * np.sin(2 * w) + 0.05 * np.sin(3 * w)
    y += filt(rng.standard_normal(n), 2800, 0.7) * 0.05
    return y * e * vel


# drums: made once, played as samples
def noise(sec):
    return rng.standard_normal(int(sec * SR))


def make_drums():
    d = {}
    t = tt(int(0.5 * SR))
    fr = 46 + 120 * np.exp(-t / 0.03)
    k = np.sin(2 * np.pi * np.cumsum(fr) / SR) * np.exp(-t / 0.26)
    k += filt(noise(0.5), 4000) * np.exp(-t / 0.003) * 0.4
    d['kick'] = np.tanh(1.8 * k) * 0.95
    t = tt(int(0.35 * SR))
    body = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05) * 0.6
    d['snare'] = body + filt(filt(noise(0.35), 1400, kind='hp'), 7500) * np.exp(-t / 0.12) * 0.9
    d['rim'] = (np.sin(2 * np.pi * 1700 * t) * 0.4 + filt(noise(0.35), 2500, kind='hp') * 0.6) * np.exp(-t / 0.012)
    burst = np.zeros(len(t))
    for s in (0.0, 0.010, 0.021):
        m = t >= s
        burst[m] += np.exp(-(t[m] - s) / 0.0045)
    burst += np.exp(-t / 0.09) * 0.55 * (t >= 0.021)
    d['clap'] = filt(filt(noise(0.35), 900, 0.9, kind='hp'), 3200, 0.9) * burst * 1.3
    t = tt(int(0.4 * SR))
    hiss = filt(noise(0.4), 7000, kind='hp')
    d['hat'] = hiss * np.exp(-t / 0.03) * 0.55
    d['ohat'] = hiss * np.exp(-t / 0.18) * 0.45
    d['shaker'] = filt(noise(0.4), 5000, kind='hp') * np.minimum(t / 0.012, 1) * np.exp(-t / 0.05) * 0.4
    t = tt(int(2.4 * SR))
    d['crash'] = filt(noise(2.4), 3200, kind='hp') * np.exp(-t / 0.9) * 0.45
    t = tt(int(0.6 * SR))
    d['tom'] = np.sin(2 * np.pi * np.cumsum(90 + 60 * np.exp(-t / 0.05)) / SR) * np.exp(-t / 0.3) * 0.8
    return d


DRUMS = make_drums()


# ---------------------------------------------------------------- the song
class Song:
    TAIL = 5.0  # seconds a sound may ring past the loop's end (it wraps round)

    def __init__(self, name, bpm, bars, prog):
        self.name, self.bpm, self.bars, self.prog = name, bpm, bars, prog
        self.spb = 60.0 / bpm
        self.n = int(round(bars * 4 * self.spb * SR))
        self.bus = {b: np.zeros(self.n + int(self.TAIL * SR)) for b in ('drums', 'duck', 'mel', 'dry')}
        self.kicks = []

    def sec(self, bar, beat=0.0):
        return (bar * 4 + beat) * self.spb

    def put(self, bus, sec, y, gain=1.0):
        i = int(round(sec * SR)) % self.n
        y = y[:len(self.bus[bus]) - i]
        self.bus[bus][i:i + len(y)] += y * gain

    def chord_at(self, bar, beat=0.0):
        cell = self.prog[bar % len(self.prog)].split()
        return chord(cell[min(int(beat * len(cell) / 4), len(cell) - 1)])

    def note(self, inst, bus, bar, beat, m, beats, vel, gain=1.0, jitter=0.004):
        y = inst(hz(m), beats * self.spb, vel)
        self.put(bus, self.sec(bar, beat) + abs(rng.normal(0, jitter)), y, gain)

    def hit(self, name, bar, beat, vel=1.0, bus='drums'):
        if name == 'kick':
            self.kicks.append(self.sec(bar, beat))
        self.put(bus, self.sec(bar, beat), DRUMS[name], vel)

    def melody(self, inst, bus, bar, text, gain=1.0, shift=0, vel=0.8):
        """bars of 'NOTE:beats' split by |, r a rest; connected notes overlap a little (legato)"""
        notes = []
        for b, cell in enumerate(text.split('|')):
            beat = 0.0
            for tok in cell.split():
                name, beats = tok.split(':')
                beats = float(beats)
                if name != 'r':
                    notes.append([bar + b, beat, pitch(name) + shift, beats])
                beat += beats
            assert abs(beat - 4) < 1e-6, f'{self.name}: bar {b} of a melody is {beat} beats'
        for i, (nb, beat, m, beats) in enumerate(notes):
            nxt = notes[i + 1] if i + 1 < len(notes) else None
            legato = nxt and abs((nxt[0] * 4 + nxt[1]) - (nb * 4 + beat + beats)) < 1e-6
            accent = 1.0 if beat % 1 == 0 else 0.88
            self.note(inst, bus, nb, beat, m, beats + (0.06 / self.spb if legato else -0.05), vel * accent, gain)
        return bar + len(text.split('|'))

    # parts over bars [a, b)
    def pads(self, a, b, inst=pad, lo=55, vel=0.6, gain=1.0, bus='duck'):
        for bar in range(a, b):
            cells = self.prog[bar % len(self.prog)].split()
            for k in range(len(cells)):
                beat, beats = 4 * k / len(cells), 4 / len(cells)
                root, iv = self.chord_at(bar, beat)
                for v in iv:
                    self.note(inst, bus, bar, beat, place((root + v) % 12, lo), beats, vel, gain)

    def bassline(self, a, b, style, vel=0.9, gain=0.55):
        pats = {
            'offbeat': [(0.5, 0.4, 0), (1.5, 0.4, 0), (2.5, 0.4, 0), (3.5, 0.4, 12)],
            'octave16': [(i * 0.25, 0.2, 12 * (i % 2)) for i in range(16)],
            'drive': [(i * 0.5, 0.42, 0) for i in range(8)],
            'chill': [(0, 1.4, 0), (1.5, 0.45, 7), (2.5, 0.9, 12), (3.5, 0.45, 10)],
            'anthem': [(0, 0.45, 0), (0.75, 0.2, 0), (1.5, 0.45, 0), (2, 0.45, 12), (2.75, 0.2, 0), (3.5, 0.45, 7)],
        }
        for bar in range(a, b):
            for beat, beats, up in pats[style]:
                root, _ = self.chord_at(bar, beat)
                self.note(bass, 'duck', bar, beat, place(root, 36) + up, beats, vel * (1 if beat % 1 == 0 else 0.85), gain, 0)

    def stabs(self, a, b, beats_at, inst=piano, lo=60, vel=0.7, length=0.3, gain=1.0, bus='mel'):
        for bar in range(a, b):
            for beat in beats_at:
                root, iv = self.chord_at(bar, beat)
                for v in iv[:4]:
                    self.note(inst, bus, bar, beat, place((root + v) % 12, lo), length, vel, gain)

    def arp(self, a, b, inst=pluck, lo=64, step=0.25, vel=0.55, gain=1.0, order=(0, 1, 2, 3, 2, 1)):
        for bar in range(a, b):
            for i in range(int(4 / step)):
                beat = i * step
                root, iv = self.chord_at(bar, beat)
                tones = sorted(place((root + v) % 12, lo) for v in iv[:3]) + [place(root, lo) + 12]
                m = tones[order[i % len(order)] % len(tones)]
                self.note(inst, 'mel', bar, beat, m, step * 0.9, vel * (1 if i % 4 == 0 else 0.8), gain, 0.002)

    def beat(self, a, b, style, vel=1.0):
        for bar in range(a, b):
            if style in ('house', 'intro', 'drive'):
                for q in range(4):
                    self.hit('kick', bar, q, vel)
                for q in range(4):
                    self.hit('ohat', bar, q + 0.5, 0.5 * vel)
                if style != 'intro':
                    for q in (1, 3):
                        self.hit('clap', bar, q, 0.8 * vel)
                    for s in range(16):
                        self.hit('hat', bar, s * 0.25, (0.35 if s % 2 else 0.22) * vel)
                if style == 'drive':
                    for q in (1, 3):
                        self.hit('snare', bar, q, 0.5 * vel)
            elif style == 'chill':
                for q in (0, 1.75, 2.5):
                    self.hit('kick', bar, q, 0.8 * vel)
                for q in (1, 3):
                    self.hit('rim', bar, q, 0.7 * vel)
                for s in range(8):
                    swing = 0.08 if s % 2 else 0
                    self.hit('shaker', bar, s * 0.5 + swing, (0.5 if s % 2 else 0.8) * vel)
            elif style == 'shaker':
                for s in range(8):
                    self.hit('shaker', bar, s * 0.5 + (0.08 if s % 2 else 0), 0.6 * vel)

    def roll(self, bar, start=2.0):
        """a snare roll up to the next bar, getting louder"""
        steps = int((4 - start) / 0.25)
        for i in range(steps):
            self.hit('snare', bar, start + i * 0.25, 0.25 + 0.6 * i / steps)

    def crash(self, bar):
        self.hit('crash', bar, 0, 0.9)

    # ------------------------------------------------------------ mixdown
    def render(self, duck=0.55, reverb=0.25, echo=0.3):
        n = self.n
        b = {}
        for k, x in self.bus.items():
            y = x[:n].copy()
            y[:len(x) - n] += x[n:]  # the tail wraps round: a seamless loop
            b[k] = y
        # sidechain: bass and pads dip under each kick
        g = np.ones(n)
        curve = 1 - duck * np.exp(-tt(int(0.3 * SR)) / 0.09)
        for s in self.kicks:
            i = int(round(s * SR)) % n
            idx = (np.arange(len(curve)) + i) % n
            g[idx] = np.minimum(g[idx], curve)
        b['duck'] *= g
        # echo on the melody bus (a dotted eighth), circular
        d = int(round(0.75 * self.spb * SR))
        wet = np.roll(b['mel'], d) * echo + np.roll(b['mel'], 2 * d) * echo * echo
        b['mel'] += filt(wet, 2500)
        # a small hall, circular convolution over the loop
        t = tt(int(2.2 * SR))
        ir = filt(rng.standard_normal(len(t)), 4200) * np.exp(-t / 0.42)
        ir[:int(0.015 * SR)] = 0
        ir /= np.sqrt(np.sum(ir ** 2))
        send = b['mel'] + 0.6 * b['duck'] + 0.25 * b['drums']
        hall = np.fft.irfft(np.fft.rfft(send) * np.fft.rfft(ir, n), n)
        mix = 0.75 * b['drums'] + b['duck'] + 1.6 * b['mel'] + b['dry'] + reverb * hall
        # (high-pass at 30 Hz, circular too)
        X = np.fft.rfft(mix)
        f = np.fft.rfftfreq(n, 1 / SR)
        s = 1j * f / 30
        mix = np.fft.irfft(X * (s * s / (s * s + s / 0.707 + 1)), n)
        mix *= 0.17 / np.sqrt(np.mean(mix ** 2))
        mix = np.tanh(mix * 1.2) / 1.2
        return mix * (0.92 / max(np.max(np.abs(mix)), 0.92))


# ---------------------------------------------------------------- the tracks (all original)
def kick_off():
    a = ['D', 'Bm', 'G', 'A', 'D', 'F#m', 'G', 'A']
    b = ['Em', 'A', 'F#m', 'Bm', 'G', 'A', 'Bm', 'A']
    s = Song('Kick Off', 124, 32, a + a + b + a)
    A = ('A4:1 D5:.5 E5:.5 F#5:1.5 E5:.5 | D5:1 B4:1 D5:.5 F#5:1 r:.5 | G5:1.5 F#5:.5 E5:1 D5:1 | E5:3 r:1 |'
         ' A4:1 D5:.5 E5:.5 F#5:1.5 A5:.5 | A5:1 F#5:1 C#5:1 F#5:1 | B5:1.5 A5:.5 G5:1 F#5:1 | E5:2 F#5:.5 E5:.5 C#5:1')
    B = ('B4:.5 E5:.5 G5:1 F#5:.5 E5:.5 D5:1 | C#5:1.5 E5:.5 A5:2 | F#5:.5 A5:.5 C#6:1 B5:.5 A5:.5 F#5:1 |'
         ' B5:2 r:.5 F#5:.5 B5:.5 C#6:.5 | D6:1.5 B5:.5 G5:1 B5:1 | A5:1 E5:1 C#6:1 A5:1 | B5:1 F#5:1 D6:1 B5:1 | C#6:3 r:1')
    stab = [0.5, 1.5, 2.25, 2.75, 3.5]
    # intro 0-3 / theme 4-11 / second theme 12-19 / break 20-23 / theme 24-31
    s.pads(0, 32, vel=0.45)
    s.beat(0, 4, 'intro')
    s.bassline(0, 4, 'offbeat', 0.7)
    s.roll(3, 3)
    s.crash(4)
    s.beat(4, 20, 'house')
    s.bassline(4, 20, 'anthem')
    s.stabs(4, 20, stab, vel=0.65)
    s.melody(lead, 'mel', 4, A, 0.55)
    s.crash(12)
    s.melody(brass, 'mel', 12, B, 0.5)
    s.arp(12, 20, lo=66, vel=0.4, gain=0.6)
    s.arp(20, 24, lo=66, vel=0.45, gain=0.7)
    s.stabs(20, 24, [0, 2.5], vel=0.4, length=1.5)
    s.roll(23, 2)
    s.crash(24)
    s.beat(24, 32, 'house')
    s.bassline(24, 32, 'anthem')
    s.stabs(24, 32, stab, vel=0.7)
    s.melody(lead, 'mel', 24, A, 0.5)
    s.melody(brass, 'mel', 24, A, 0.35, -12)
    return s


def deadline_day():
    a = ['Em', 'C', 'D', 'Bm', 'Em', 'C', 'Am', 'B']
    b = ['C', 'D', 'Em', 'Em', 'Am', 'B', 'Em', 'B']
    s = Song('Deadline Day', 132, 32, a + a + b + a)
    A = ('E5:.75 E5:.75 G5:.5 B5:1 A5:.5 G5:.5 | E5:1.5 D5:.5 C5:1 E5:1 | F#5:.75 F#5:.75 A5:.5 D6:1 C6:.5 A5:.5 | B5:3 r:1 |'
         ' E5:.75 E5:.75 G5:.5 B5:1 D6:.5 B5:.5 | C6:1.5 B5:.5 G5:1 E5:1 | A5:1 C6:1 E6:1 C6:1 | D#6:2 B5:1 F#5:1')
    B = 'G5:4 | A5:4 | B5:3 G5:1 | E5:4 | A5:2 C6:2 | B5:2 D#6:2 | E6:4 | B5:2 F#5:2'
    s.arp(0, 32, lo=59, vel=0.5, gain=0.75, order=(0, 1, 2, 1, 3, 1, 2, 1))
    s.beat(0, 4, 'intro', 0.85)
    s.roll(3, 2)
    s.crash(4)
    s.beat(4, 20, 'drive')
    s.bassline(4, 20, 'octave16', 0.8)
    s.melody(lead, 'mel', 4, A, 0.5, -12)
    s.pads(12, 24, strings, lo=55, vel=0.5, gain=0.8)
    s.melody(brass, 'mel', 12, B, 0.55)
    s.melody(bell, 'mel', 20, 'E6:1 B5:1 G5:1 B5:1 | C6:1 G5:1 E5:1 G5:1 | A5:1 E5:1 C5:1 E5:1 | D#6:2 F#5:2', 0.5)
    s.roll(23, 0)
    s.crash(24)
    s.beat(24, 32, 'drive')
    s.bassline(24, 32, 'octave16', 0.8)
    s.melody(lead, 'mel', 24, A, 0.45, -12)
    s.melody(brass, 'mel', 24, A, 0.3)
    s.pads(24, 32, strings, lo=52, vel=0.45, gain=0.6)
    return s


def sunday_league():
    p = ['Fmaj7', 'Em7 A7', 'Dm7', 'Cm7 F7', 'Bbmaj7', 'Am7', 'Gm7', 'C7']
    s = Song('Sunday League', 98, 32, p * 4)
    A = ('A5:1.5 G5:.5 E5:1 C5:1 | D5:1 E5:1 G5:1 C#5:1 | D5:2 F5:1 A5:1 | G5:1.5 Eb5:.5 A5:1 C6:1 |'
         ' D6:2 C6:.5 Bb5:.5 A5:1 | G5:1 E5:1 C5:1 E5:1 | F5:1.5 G5:.5 Bb5:1 D5:1 | E5:3 r:1')
    B = ('C6:1 A5:.5 C6:.5 E6:2 | D6:1 B5:1 C#6:1 E6:1 | F6:1.5 E6:.5 D6:1 A5:1 | Bb5:1 G5:1 A5:1 C6:1 |'
         ' D6:1.5 F6:.5 D6:1 Bb5:1 | C6:1 E6:1 A5:2 | Bb5:1 D6:1 F6:1 D6:1 | E6:2 C6:1 G5:1')
    comp = [0, 1.5, 2.5, 3.75]
    s.stabs(0, 32, comp, epiano, lo=57, vel=0.5, length=0.9, gain=0.8, bus='duck')
    s.beat(0, 4, 'shaker')
    s.beat(4, 28, 'chill')
    s.bassline(4, 28, 'chill', 0.8)
    s.melody(flute, 'mel', 4, A, 0.55)
    s.melody(flute, 'mel', 12, B, 0.5)
    s.melody(bell, 'mel', 12, 'A5:4 | G5:2 C#6:2 | F5:4 | Eb5:2 A5:2 | D5:4 | E5:4 | D5:4 | E5:2 G5:2', 0.25)
    s.pads(12, 28, pad, lo=53, vel=0.35, gain=0.7)
    s.melody(flute, 'mel', 20, A, 0.45)
    s.melody(bell, 'mel', 20, A, 0.25, 12)
    s.beat(28, 32, 'shaker')
    s.bassline(28, 32, 'chill', 0.6)
    s.crash(4)
    return s


def matchday():
    a = ['G', 'D', 'Em', 'C', 'G', 'D', 'C', 'D']
    b = ['Em', 'C', 'G', 'D', 'Em', 'C', 'Am', 'D']
    s = Song('Matchday', 128, 40, a + a + b + a + b)
    A = ('D5:1 G5:1 B5:1.5 A5:.5 | A5:2 F#5:1 D5:1 | E5:1 G5:1 B5:1 E6:1 | E6:1.5 D6:.5 C6:1 G5:1 |'
         ' D5:1 G5:1 B5:1.5 D6:.5 | D6:2 A5:1 F#5:1 | G5:1 E5:1 C6:1.5 B5:.5 | A5:3 r:1')
    B = ('B5:1.5 G5:.5 E5:2 | C6:1.5 G5:.5 E5:2 | D6:1 B5:1 G5:1 B5:1 | A5:1 F#5:1 A5:1 D6:1 |'
         ' E6:2 D6:1 B5:1 | C6:2 E6:2 | E6:1 D6:.5 C6:.5 A5:2 | F#5:1 A5:1 D6:2')
    stab = [0, 0.75, 1.5, 2.5, 3.25]
    # intro 0-3 / A 4-11 / B 12-19 / break 20-23 / A 24-31 / B 32-39
    s.pads(0, 40, strings, lo=55, vel=0.45, gain=0.8)
    for bar in range(0, 4):
        for q in (0, 1, 2, 3):
            s.hit('tom', bar, q, 0.5 + 0.1 * bar)
    s.bassline(0, 4, 'drive', 0.6)
    s.roll(3, 2)
    for start, end in ((4, 20), (24, 40)):
        s.crash(start)
        s.beat(start, end, 'house')
        s.bassline(start, end, 'anthem')
        s.stabs(start, end, stab, vel=0.55)
    s.melody(brass, 'mel', 4, A, 0.55)
    s.crash(12)
    s.melody(lead, 'mel', 12, B, 0.5)
    s.arp(12, 20, lo=67, vel=0.4, gain=0.6)
    s.melody(bell, 'mel', 20, 'D5:1 G5:1 B5:1.5 A5:.5 | A5:2 F#5:1 D5:1 | E5:1 G5:1 B5:1 E6:1 | E6:1.5 D6:.5 C6:1 G5:1', 0.5)
    s.roll(23, 0)
    s.melody(brass, 'mel', 24, A, 0.5)
    s.melody(lead, 'mel', 24, A, 0.35, 12)
    s.crash(32)
    s.melody(lead, 'mel', 32, B, 0.5)
    s.melody(brass, 'mel', 32, B, 0.35, -12)
    s.arp(32, 40, lo=67, vel=0.4, gain=0.6)
    return s


TRACKS = [kick_off, deadline_day, sunday_league, matchday]


# ---------------------------------------------------------------- IMA ADPCM
STEPS = [7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130,
         143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282,
         1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
         9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767]
INDEX = [-1, -1, -1, -1, 2, 4, 6, 8]
BLOCK = 512  # bytes a block: 4 header bytes + 1016 samples, + the header's own one


def ima_encode(pcm):
    """mono int16 -> (blocks, samples a block); also the decoded samples (to check)"""
    per = (BLOCK - 4) * 2 + 1
    pcm = [int(v) for v in pcm] + [0] * (-len(pcm) % per)
    out, dec, idx = bytearray(), [], 0
    for b in range(0, len(pcm), per):
        pred = pcm[b]
        out += struct.pack('<hBB', pred, idx, 0)
        dec.append(pred)
        nib = []
        for s in pcm[b + 1:b + per]:
            step = STEPS[idx]
            diff = s - pred
            code = 0
            if diff < 0:
                code, diff = 8, -diff
            vp = step >> 3
            if diff >= step:
                code |= 4
                diff -= step
                vp += step
            if diff >= step >> 1:
                code |= 2
                diff -= step >> 1
                vp += step >> 1
            if diff >= step >> 2:
                code |= 1
                vp += step >> 2
            pred = max(-32768, min(32767, pred - vp if code & 8 else pred + vp))
            idx = max(0, min(88, idx + INDEX[code & 7]))
            nib.append(code)
            dec.append(pred)
        out += bytes(nib[i] | nib[i + 1] << 4 for i in range(0, len(nib), 2))
    return bytes(out), per, dec


def write_adpcm(path, pcm):
    data, per, dec = ima_encode(pcm)
    fmt = struct.pack('<HHIIHHHH', 0x11, 1, SR, SR * BLOCK // per, BLOCK, 4, 2, per)
    body = b'WAVE' + b'fmt ' + struct.pack('<I', len(fmt)) + fmt + b'fact' + struct.pack('<II', 4, len(pcm)) + b'data' + struct.pack('<I', len(data)) + data
    with open(path, 'wb') as f:
        f.write(b'RIFF' + struct.pack('<I', len(body)) + body)
    return np.array(dec[:len(pcm)])


def write_pcm(path, pcm):
    data = np.asarray(pcm, '<i2').tobytes()
    fmt = struct.pack('<HHIIHH', 1, 1, SR, SR * 2, 2, 16)
    body = b'WAVE' + b'fmt ' + struct.pack('<I', len(fmt)) + fmt + b'data' + struct.pack('<I', len(data)) + data
    with open(path, 'wb') as f:
        f.write(b'RIFF' + struct.pack('<I', len(body)) + body)


def main():
    os.makedirs(OUT, exist_ok=True)
    for k, make in enumerate(TRACKS, 1):
        song = make()
        mix = song.render()
        pcm = np.round(mix * 32767).astype(np.int16)
        name = os.path.join(OUT, f'{k:02d} {song.name}.wav')
        dec = write_adpcm(name, pcm)
        err = np.sqrt(np.mean((dec - pcm) ** 2)) / np.sqrt(np.mean(pcm.astype(float) ** 2))
        # (the jump from the last sample to the first: no bigger than the track's own steps)
        steps = np.abs(np.diff(pcm.astype(float))) / 32767
        seam = abs(int(pcm[0]) - int(pcm[-1])) / 32767
        assert err < 0.1, f'{name}: ADPCM error {err:.3f}'
        assert seam <= np.percentile(steps, 99.9), f'{name}: loop seam jump {seam:.3f}'
        print(f'{name}: {len(pcm) / SR:.1f} s, {os.path.getsize(name) // 1024} KB, ADPCM error {err:.3f}, seam {seam:.4f}')
        if '--pcm' in sys.argv:
            os.makedirs(os.path.join(OUT, 'pcm'), exist_ok=True)
            write_pcm(os.path.join(OUT, 'pcm', f'{k:02d} {song.name}.wav'), np.concatenate([pcm, pcm]))


if __name__ == '__main__':
    main()
