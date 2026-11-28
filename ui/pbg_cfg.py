"""Merge the player lines ([STAFF]) of the player background pack's pics.cfg into ours.
usage: python pbg_cfg.py <our pics.cfg> <pack pics.cfg> <pack.zip> <out pics.cfg>
Only [STAFF] lines whose picture is in the zip; the pack's other sections point at stadium /
award / training pictures it does not ship, so ours stay. Bytes kept as cp1252 / CRLF."""
import re, sys, zipfile

ours, pack, zpath, out = sys.argv[1:5]
have = {n.split("/")[-1].lower() for n in zipfile.ZipFile(zpath).namelist()}
lines, sec, add, missing = open(pack, encoding="latin-1").read().splitlines(), None, [], 0
for l in lines:
    m = re.match(r"\[(\w+)\]", l)
    if m:
        sec = m.group(1)
    elif sec == "STAFF" and ":" in l and not l.startswith("#"):
        if l.rsplit(":", 1)[1].strip().lower() in have:
            add.append(l)
        else:
            missing += 1
add = list(dict.fromkeys(add))
text = open(ours, encoding="latin-1").read().splitlines()
i = text.index("[AWARDS]")
while text[i - 1].strip() == "":
    i -= 1
text[i:i] = [""] + add
open(out, "w", encoding="latin-1", newline="\r\n").write("\n".join(text) + "\n")
print(len(add), "player lines added,", missing, "skipped (picture not in zip)")
assert open(out, encoding="latin-1").read().count("[STAFF]") == 1
