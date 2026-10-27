#!/usr/bin/env python3
"""tools/rename_crate.py <old> <new>: rename crates/<old> to crates/<new> (dir,
package, lib ident, dependency paths, imports, docs). Names without prefix."""
import os, re, subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
old, new = sys.argv[1:3]
subprocess.check_call(["git", "mv", f"crates/megatower-{old}", f"crates/megatower-{new}"], cwd=ROOT)
pats = [
    (re.compile(rf"megatower-{old}\b"), f"megatower-{new}"),
    (re.compile(rf"megatower_{old}\b"), f"megatower_{new}"),
]
for dp, dn, fs in os.walk(ROOT):
    dn[:] = [d for d in dn if d not in ("target", ".git", "Megaloot-GDRE-Recovered", "__pycache__")]
    for f in fs:
        if f.endswith((".toml", ".rs", ".md", ".py", ".sh")) and f != "rename_crate.py":
            p = Path(dp) / f
            s = p.read_text()
            n = s
            for r, t in pats:
                n = r.sub(t, n)
            if n != s:
                p.write_text(n)
