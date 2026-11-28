"""The orchestra: Sonatina instruments with their verified ranges (orchestra-music-maker step 2)."""
import os
from sfz import Instrument

ROOT = os.path.expanduser("~/libs/sso/Sonatina Symphonic Orchestra")
TABLE = {
    "vn1": ("Strings - Performance/1st Violins Sustain.sfz", 55, 96),
    "vn2": ("Strings - Performance/2nd Violins Sustain.sfz", 55, 93),
    "va": ("Strings - Performance/Violas Sustain.sfz", 48, 84),
    "vc": ("Strings - Performance/Celli Sustain.sfz", 36, 73),
    "cb": ("Strings - Performance/Basses Sustain.sfz", 28, 60),
    "trem": ("Strings - Performance/1st Violins Tremolo.sfz", 55, 96),
    "vc_st": ("Strings - Performance/Celli Staccato.sfz", 36, 72),
    "cb_st": ("Strings - Performance/Basses Staccato.sfz", 28, 55),
    "hn": ("Brass - Performance/Horns Sustain.sfz", 41, 77),
    "hn_st": ("Brass - Performance/Horns Staccato.sfz", 43, 77),
    "tpt": ("Brass - Performance/Trumpets Sustain (looped).sfz", 55, 84),
    "tbn": ("Brass - Performance/Trombones Sustain (looped).sfz", 40, 72),
    "tbn_st": ("Brass - Performance/Trombones Staccato.sfz", 40, 72),
    "tuba": ("Brass - Performance/Tuba Sustain (looped).sfz", 28, 53),
    "choir": ("Chorus - Performance/Large Chorus.sfz", 45, 81),
    "timp": ("Percussion/Timpani.sfz", 36, 48),
    "drums": ("Percussion/Bass Drum & Snare.sfz", 48, 49),
    "cym": ("Percussion/Cymbals & Tamtam.sfz", 48, 57),
    "harp": ("Concert Harp.sfz", 36, 96),
    "pno": ("Grand Piano/Grand Piano.sfz", 21, 108),
    "cel": ("Percussion/Celeste.sfz", 60, 98),
}
LONG = {"cb", "vc", "choir"}

def load(keys=None):
    out = {}
    for k, (f, lo, hi) in TABLE.items():
        if keys and k not in keys:
            continue
        out[k] = Instrument(os.path.join(ROOT, f), max_release=2.5 if k in LONG else 1.5)
        out[k].lo, out[k].hi = lo, hi
    return out
