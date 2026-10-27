#!/usr/bin/env python3
"""Widen items that became cross-crate API: repeatedly run `cargo check`, and
for each E0603 "is private" error widen the definition site named in the note
(pub(crate)/pub(super)/private -> pub)."""
import json, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
for round_ in range(40):
    out = subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True,
    ).stdout
    fixes = set()
    for line in out.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        m = msg.get("message")
        if not m or m.get("level") != "error":
            continue
        if (m.get("code") or {}).get("code") not in ("E0603", "E0616", "E0624", "E0451"):
            continue
        # The definition site is the "defined here"/"private ..." note span.
        for child in m.get("children", []):
            for span in child.get("spans", []):
                fixes.add((span["file_name"], span["line_start"]))
        for span in m.get("spans", []):
            if "private" in (span.get("label") or "") and span.get("is_primary") is False:
                fixes.add((span["file_name"], span["line_start"]))
    if not fixes:
        print("no more visibility errors")
        break
    changed = 0
    for fname, ln in sorted(fixes):
        p = ROOT / fname
        if not p.exists():
            continue
        lines = p.read_text().split("\n")
        l = lines[ln - 1]
        new = re.sub(r"\bpub\((?:crate|super|in [^)]*)\)\s+", "pub ", l)
        if new == l and not l.lstrip().startswith("pub "):
            new = re.sub(r"^(\s*)(fn|struct|enum|const|static|type|mod|trait|use)\b", r"\1pub \2", l)
            if new == l and re.match(r"^\s+[a-z_][a-z0-9_]*\s*:", l):  # struct field
                new = re.sub(r"^(\s*)", r"\1pub ", l, 1)
        if new != l:
            lines[ln - 1] = new
            p.write_text("\n".join(lines))
            changed += 1
    print(f"round {round_}: widened {changed}")
    if not changed:
        break
