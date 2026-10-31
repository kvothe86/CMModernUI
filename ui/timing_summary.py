"""timing_summary.py [log]: per "mark NAME" section of a cmui.log written with cmui.timing on, the
frames and the worst / mean milliseconds of cmui's hooks (texts, flip pass, panels, glyphs, search
results), and any slow texts. Default log: E:\\cm25\\cmui.log."""
import re, sys
from collections import defaultdict

log = sys.argv[1] if len(sys.argv) > 1 else r'E:\cm25\cmui.log'
mark, rows, slow = 'start', defaultdict(list), defaultdict(int)
for line in open(log, encoding='utf-8', errors='replace'):
    m = re.match(r'script: mark (\S+)$', line.strip())
    if m:
        mark = m.group(1)
        continue
    if 'slow text' in line:
        slow[mark] += 1
        continue
    m = re.match(r'timing: (\d+) texts ([\d.]+) ms(?:, flip ([\d.]+) ms)?.*?panels ([\d.]+), glyphs ([\d.]+), results ([\d.]+)', line)
    if m:
        rows[mark].append([float(m.group(i) or 0) for i in (2, 3, 4, 5, 6)])
print(f'{"screen":14} frames  worst/mean ms: texts  flip  panels  glyphs  results  slow texts')
for name, r in rows.items():
    worst = [max(c) for c in zip(*r)]
    mean = [sum(c) / len(c) for c in zip(*r)]
    cells = '  '.join(f'{w:5.1f}/{a:4.1f}' for w, a in zip(worst, mean))
    print(f'{name:14} {len(r):6}  {cells}  {slow[name]}')
