"""Mean volume per section of a rendered track (skill step 5 check). usage: python arc.py NN"""
import json, re, subprocess, sys, glob

n = sys.argv[1]
meta = json.load(open(f"out/{n}.json"))
mp3 = glob.glob(f"out/{n} *.mp3")[0]
row = []
for start, name, _ in meta["sections"]:
    r = subprocess.run(["ffmpeg", "-ss", str(start + 1), "-t", "6", "-i", mp3, "-af", "volumedetect", "-f", "null", "-"],
                       capture_output=True, text=True).stderr
    mean = re.search(r"mean_volume: (-?[\d.]+)", r).group(1)
    row.append(f"{name} {mean}")
r = subprocess.run(["ffmpeg", "-i", mp3, "-af", "volumedetect", "-f", "null", "-"], capture_output=True, text=True).stderr
print(meta["title"], meta["length"], "s |", " | ".join(row), "| max", re.search(r"max_volume: (-?[\d.]+)", r).group(1))
