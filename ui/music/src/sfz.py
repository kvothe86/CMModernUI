"""Tiny SFZ sampler for offline rendering (enough for Sonatina Symphonic Orchestra)."""
import os, re, subprocess, numpy as np

SR = 44100
NOTE = {"c":0,"d":2,"e":4,"f":5,"g":7,"a":9,"b":11}

def note_num(v):
    if isinstance(v, int): return v
    v = v.strip().lower()
    if re.fullmatch(r"-?\d+", v): return int(v)
    m = re.fullmatch(r"([a-g])([#b]?)(-?\d+)", v)
    if not m: raise ValueError(v)
    n = NOTE[m.group(1)] + (1 if m.group(2) == "#" else -1 if m.group(2) == "b" else 0)
    return n + (int(m.group(3)) + 1) * 12

def _read(path, root, defines):
    out = []
    for line in open(path, encoding="utf-8", errors="ignore"):
        line = line.split("//")[0]
        if "#define" not in line:
            for k, v in sorted(defines.items(), key=lambda kv: -len(kv[0])):
                line = line.replace(k, v)
        m = re.match(r'\s*#define\s+(\$\w+)\s+(.+)', line)
        if m: defines[m.group(1)] = m.group(2).strip(); continue
        m = re.match(r'\s*#include\s+"(.+)"', line)
        if m:
            out += _read(os.path.join(root, m.group(1)), root, defines); continue
        out.append(line)
    return out

def parse(path):
    root = os.path.dirname(path)
    text = "\n".join(_read(path, root, {}))
    tokens = re.split(r"(<\w+>)", text)
    control, group, master, regions = {}, {}, {}, []
    kind = None
    def opcodes(s):
        res = {}
        for m in re.finditer(r"(\w+)=(.*?)(?=\s+\w+=|$)", s.replace("\n", " ")):
            res[m.group(1)] = m.group(2).strip()
        return res
    for tok in tokens:
        if re.fullmatch(r"<\w+>", tok):
            kind = tok[1:-1]
            if kind == "group": group = {}
            if kind == "master": master = {}; group = {}
            continue
        ops = opcodes(tok)
        if kind == "control": control.update(ops)
        elif kind == "master": master.update(ops)
        elif kind == "group": group.update(ops)
        elif kind == "region":
            r = {**master, **group, **ops}
            if "sample" in r: r["_path"] = os.path.normpath(os.path.join(root, control.get("default_path", ""), r["sample"].replace("\\", "/")))
            regions.append(r)
    for r in regions:
        if "key" in r:
            k = note_num(r["key"]); r.setdefault("lokey", k); r.setdefault("hikey", k); r.setdefault("pitch_keycenter", k)
        r["_lo"] = note_num(r.get("lokey", 0)); r["_hi"] = note_num(r.get("hikey", 127))
        r["_center"] = note_num(r.get("pitch_keycenter", 60))   # SFZ default is 60
    return [r for r in regions if "_path" in r and r.get("trigger", "attack") == "attack"]

_audio = {}
def audio(path):
    if path not in _audio:
        p = path
        if not os.path.exists(p):
            for ext in (".flac", ".wav"):
                if os.path.exists(os.path.splitext(path)[0] + ext): p = os.path.splitext(path)[0] + ext
        raw = subprocess.run(["ffmpeg","-v","quiet","-i",p,"-f","f32le","-ac","2","-ar",str(SR),"-"],
                             capture_output=True, check=True).stdout
        _audio[path] = np.frombuffer(raw, dtype=np.float32).reshape(-1, 2).copy()
    return _audio[path]

def _xf(r, pre, val):
    w = 1.0
    lo, hi = r.get(f"xfin_lo{pre}"), r.get(f"xfin_hi{pre}")
    if lo is not None:
        lo, hi = float(lo), float(hi)
        w *= 1.0 if val >= hi else 0.0 if val <= lo else (val - lo) / (hi - lo)
    lo, hi = r.get(f"xfout_lo{pre}"), r.get(f"xfout_hi{pre}")
    if lo is not None:
        lo, hi = float(lo), float(hi)
        w *= 1.0 if val <= lo else 0.0 if val >= hi else (hi - val) / (hi - lo)
    return w

class Instrument:
    def __init__(self, path, fallback_attack=0.0, max_release=1.5):
        self.regions = parse(path)
        self.rr = {}
        self.fallback_attack = fallback_attack
        self.max_release = max_release
        if not self.regions: raise ValueError("no regions in " + path)

    def render(self, midi, vel, dur, release=None):
        """vel 0..1, dur seconds (held length). Returns stereo float32 array or None."""
        v127 = max(1, min(127, int(round(vel * 127))))
        cc = v127
        cands = []
        for r in self.regions:
            if not (r["_lo"] <= midi <= r["_hi"]): continue
            if not (int(r.get("lovel", 0)) <= v127 <= int(r.get("hivel", 127))): continue
            ok = True
            for key in r:
                m = re.fullmatch(r"(lo|hi)cc(\d+)", key)
                if m:
                    val = cc if m.group(2) == "1" else 0
                    lim = float(r[key])
                    if (m.group(1) == "lo" and val < lim) or (m.group(1) == "hi" and val > lim): ok = False
            if not ok: continue
            w = _xf(r, "vel", v127) * _xf(r, "cc1", cc)
            if w <= 0.001: continue
            cands.append((r, w))
        if not cands: return None
        L = max(int(r.get("seq_length", 1)) for r, _ in cands)
        if L > 1:
            k = self.rr.get(midi, 0); self.rr[midi] = k + 1
            pos = k % L + 1
            cands = [(r, w) for r, w in cands if int(r.get("seq_length", 1)) == 1 or int(r.get("seq_position", 1)) == pos] or cands
        out = None
        for r, w in cands:
            y = self._region(r, midi, v127, cc, dur, release) * w
            if out is None: out = y
            else:
                n = max(len(out), len(y)); z = np.zeros((n, 2), np.float32)
                z[:len(out)] += out; z[:len(y)] += y; out = z
        return out

    def _region(self, r, midi, v127, cc, dur, rel_override=None):
        s = audio(r["_path"])
        semis = midi - r["_center"] + float(r.get("tune", 0)) / 100 + float(r.get("transpose", 0))
        ratio = 2 ** (semis / 12)
        one_shot = r.get("loop_mode") == "one_shot"
        attack = float(r.get("ampeg_attack", self.fallback_attack))
        release = min(float(r.get("ampeg_release", 0.3)), self.max_release)
        if rel_override is not None: release = rel_override
        offset = int(r.get("offset", 0))
        ls, le = r.get("loop_start"), r.get("loop_end")
        n = int((len(s) - offset) / ratio) if one_shot else int((dur + release) * SR)
        idx = offset + np.arange(n) * ratio
        if not one_shot and idx[-1] >= len(s) - 2:
            if ls is not None and le is not None and int(le) > int(ls) + 100:
                ls, le = int(ls), int(le)
                over = idx >= le
                idx[over] = ls + np.mod(idx[over] - ls, le - ls)
            else:   # no loop points: loop the middle of the sample
                a, b = int(len(s) * 0.35), int(len(s) * 0.92)
                over = idx >= b
                idx[over] = a + np.mod(idx[over] - a, b - a)
        idx = np.clip(idx, 0, len(s) - 2)
        i0 = idx.astype(np.int64); fr = (idx - i0)[:, None].astype(np.float32)
        y = s[i0] * (1 - fr) + s[i0 + 1] * fr
        env = np.ones(n, np.float32)
        na = int(attack * SR)
        if na > 1: env[:min(na, n)] = np.linspace(0, 1, min(na, n))
        if not one_shot:
            nh = int(dur * SR)
            if nh < n: env[nh:] *= np.linspace(1, 0, n - nh) ** 2
        db = float(r.get("volume", 0)) + float(r.get("group_volume", 0)) + float(r.get("gain_cc1", 0)) * cc / 127
        g = 10 ** (db / 20)
        if float(r.get("amp_veltrack", 100)) > 0 and "gain_cc1" not in r:
            g *= (v127 / 127) ** 1.6
        pan = float(r.get("pan", 0)) / 100
        y = y * env[:, None] * g
        if pan:
            y[:, 0] *= np.sqrt((1 - pan) / 2) * 1.414; y[:, 1] *= np.sqrt((1 + pan) / 2) * 1.414
        return y.astype(np.float32)
