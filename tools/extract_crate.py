#!/usr/bin/env python3
"""Move top-level modules out of megatower-client into their own crate.

    tools/extract_crate.py <crate> <module>... [--dep name=spec ...] [--from crates/megatower-client]

For each module: `git mv` its file/directory into `crates/<crate>/src`, add
`pub mod` lines to the new crate's lib.rs, drop them from the source crate's
lib.rs, and rewrite every `crate::<module>` path in all other crates (including
grouped `use crate::{a, b::c}` imports) to `<crate_ident>::<module>`.
"""
import argparse
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def split_top_level(text):
    parts, depth, cur = [], 0, ""
    for ch in text:
        if ch in "{(<":
            depth += 1
        elif ch in "})>":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur.strip())
    return parts


def rewrite_groups(src, mods, ident):
    """Pull moved modules out of `use crate::{...};` groups."""
    out, pos = [], 0
    for m in re.finditer(r"(pub(?:\([^)]*\))?\s+)?use crate::\{", src):
        if m.start() < pos:
            continue
        start = m.end()
        depth, i = 1, start
        while depth:
            depth += {"{": 1, "}": -1}.get(src[i], 0)
            i += 1
        body, end = src[start : i - 1], i
        while src[end] in " \t":
            end += 1
        if src[end] != ";":
            continue
        end += 1
        vis = m.group(1) or ""
        keep, moved = [], []
        for item in split_top_level(body):
            head = re.match(r"[A-Za-z_][A-Za-z0-9_]*", item)
            (moved if head and head.group(0) in mods else keep).append(item)
        if not moved:
            continue
        out.append(src[pos : m.start()])
        if keep:
            out.append(f"{vis}use crate::{{{', '.join(keep)}}};\n")
        out.append("\n".join(f"{vis}use {ident}::{it};" for it in moved))
        pos = end
    out.append(src[pos:])
    return "".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("crate")
    ap.add_argument("modules", nargs="+")
    ap.add_argument("--from", dest="src", default="crates/megatower-client")
    ap.add_argument("--dep", action="append", default=[])
    args = ap.parse_args()

    ident = args.crate.replace("-", "_")
    mods = set(args.modules)
    src_crate = ROOT / args.src
    dst = ROOT / "crates" / args.crate
    (dst / "src").mkdir(parents=True, exist_ok=True)

    lib_new = dst / "src" / "lib.rs"
    existing = lib_new.read_text() if lib_new.exists() else ""
    decls = []
    for mod in args.modules:
        for cand in (src_crate / "src" / f"{mod}.rs", src_crate / "src" / mod):
            if cand.exists():
                subprocess.check_call(["git", "mv", str(cand), str(dst / "src" / cand.name)], cwd=ROOT)
        decls.append(f"pub mod {mod};")
    lib_new.write_text(existing + "\n".join(decls) + "\n")

    manifest = dst / "Cargo.toml"
    if not manifest.exists():
        deps = "\n".join(args.dep)
        manifest.write_text(
            f'[package]\nname = "{args.crate}"\nversion.workspace = true\nedition.workspace = true\n\n'
            f'[lib]\nname = "{ident}"\npath = "src/lib.rs"\n\n[dependencies]\n{deps}\n'
        )

    # Drop the declarations from the source crate's lib.rs.
    lib_old = src_crate / "src" / "lib.rs"
    text = lib_old.read_text()
    for mod in args.modules:
        text = re.sub(rf"^(?:pub )?mod {mod};\n", "", text, flags=re.M)
    lib_old.write_text(text)

    # Rewrite uses everywhere else.
    for crate_dir in [d for base in ("crates", "apps") if (ROOT / base).exists() for d in (ROOT / base).iterdir()]:
        for dp, _, fs in os.walk(crate_dir / "src"):
            for f in fs:
                if not f.endswith(".rs"):
                    continue
                p = Path(dp) / f
                if dst in p.parents:
                    continue
                s = p.read_text()
                n = rewrite_groups(s, mods, ident)
                for mod in mods:
                    n = re.sub(rf"\bcrate::{mod}\b", f"{ident}::{mod}", n)
                if n != s:
                    p.write_text(n)
    print(f"moved {sorted(mods)} -> crates/{args.crate}")


if __name__ == "__main__":
    main()
