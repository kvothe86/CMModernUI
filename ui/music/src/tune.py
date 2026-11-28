"""Tuning check (step 3): every pitched instrument, every note in range: f0 by autocorrelation."""
import numpy as np
from sfz import SR
from inst import load

def f0(y):
    x = y[int(0.3 * SR):int(0.9 * SR)].mean(axis=1)
    x = x - x.mean()
    if len(x) < 1000 or np.abs(x).max() < 1e-5:
        return None
    n = len(x)
    c = np.fft.irfft(np.abs(np.fft.rfft(x, 2 * n)) ** 2)[:n]
    lo, hi = int(SR / 2200), int(SR / 25)
    k = lo + int(np.argmax(c[lo:hi]))
    if 0 < k < n - 1:
        a, b, d = c[k - 1], c[k], c[k + 1]
        k = k + 0.5 * (a - d) / (a - 2 * b + d + 1e-12)
    return SR / k

bad, none = [], []
for name, ins in load().items():
    if name in ("timp", "drums", "cym"):
        continue
    for m in range(ins.lo, ins.hi + 1):
        y = ins.render(m, 0.6, 2.0)
        if y is None:
            none.append((name, m)); continue
        f = f0(y)
        if f is None:
            none.append((name, m, "silent")); continue
        cents = 1200 * np.log2(f / (440 * 2 ** ((m - 69) / 12)))
        cents = (cents + 600) % 1200 - 600
        if abs(cents) >= 30:
            bad.append((name, m, round(cents)))
    print(name, "checked", flush=True)
print("NONE:", none)
print("BAD:", bad)
