#!/usr/bin/env python3
"""Count lines of Rust source code in the synapcore project."""

from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent
EXCLUDE_DIRS = {"target", ".git"}

stats = defaultdict(lambda: {"files": 0, "code": 0, "blank": 0, "comment": 0})
total = {"files": 0, "code": 0, "blank": 0, "comment": 0}

for rs in ROOT.rglob("*.rs"):
    if any(excl in rs.parts for excl in EXCLUDE_DIRS):
        continue

    crate = rs.relative_to(ROOT).parts[0]
    code = blank = comment = 0

    with open(rs) as f:
        for line in f:
            stripped = line.strip()
            if not stripped:
                blank += 1
            elif stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                comment += 1
            else:
                code += 1

    stats[crate]["files"] += 1
    stats[crate]["code"] += code
    stats[crate]["blank"] += blank
    stats[crate]["comment"] += comment
    total["files"] += 1
    total["code"] += code
    total["blank"] += blank
    total["comment"] += comment

print(f"{'Crate':<16} {'Files':>6} {'Code':>6} {'Blank':>6} {'Comment':>6} {'Total':>6}")
print("-" * 52)
for crate in sorted(stats):
    s = stats[crate]
    t = s["code"] + s["blank"] + s["comment"]
    print(f"{crate:<16} {s['files']:>6} {s['code']:>6} {s['blank']:>6} {s['comment']:>6} {t:>6}")
print("-" * 52)
t = total["code"] + total["blank"] + total["comment"]
print(f"{'TOTAL':<16} {total['files']:>6} {total['code']:>6} {total['blank']:>6} {total['comment']:>6} {t:>6}")