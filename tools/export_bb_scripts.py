#!/usr/bin/env python3
"""Export BBScript.get_bb_container_data bodies using a restricted grammar.

Supported: static push_back of BBContainerData.new(text[, color]), stat/status/
battle-action/keyword/item-set/passive references, counter hint/display, `+=`
status lists, and `match specialization:` blocks. Any script containing other
statements is listed under "unsupported" with the first offending line."""
import json, re, sys, hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIR = ROOT / "Megaloot-GDRE-Recovered/resources/bb_scripts"
OUT = ROOT / "assets/data/content/bb-scripts.json"

STR = r'"((?:[^"\\]|\\.)*)"'
COLORS = {"DARK_GRAY", "DIM_GRAY", "ORANGE_RED"}


def unescape(s):
    return s.replace("\\n", "\n").replace('\\"', '"').replace("\\\\", "\\")


def color(expr):
    if expr is None:
        return "DARK_GRAY"
    m = re.fullmatch(r"Color\.([A-Z_]+)", expr)
    if m and m.group(1) in COLORS:
        return m.group(1)
    m = re.fullmatch(r"Stats\.([A-Z_0-9]+)\.color", expr)
    if m:
        return "stat:" + m.group(1)
    raise ValueError(expr)


def push(expr):
    m = re.fullmatch(r"BBContainerData\.new\(" + STR + r" \+ str\((\d+)\) \+ " + STR + r"(?:, (.+))?\)", expr)
    if m:
        return {"op": "text", "text": unescape(m.group(1)) + m.group(2) + unescape(m.group(3)), "color": color(m.group(4))}
    m = re.fullmatch(r"BBContainerData\.new\(" + STR + r"(?:, (.+))?\)", expr)
    if m:
        return {"op": "text", "text": unescape(m.group(1)), "color": color(m.group(2))}
    m = re.fullmatch(r"Stats\.get_bb_container_data\(Stats\.([A-Z_0-9]+)\)", expr)
    if m:
        return {"op": "stat", "name": m.group(1)}
    m = re.fullmatch(r"Stats\.get_bb_container_data\(Stats\.([A-Z_0-9]+), Stats\.DisplayMode\.UNKNOWN\)", expr)
    if m:
        return {"op": "stat", "name": m.group(1), "mode": "UNKNOWN"}
    for cls, op in [("StatusEffects", "status"), ("BattleActions", "action"), ("Keywords", "keyword"), ("ItemSets", "item_set"), ("Passives", "passive"), ("Abilities", "ability")]:
        m = re.fullmatch(cls + r"\.get_bb_container_data\(" + cls + r"\.([A-Z_0-9]+)\)", expr)
        if m:
            return {"op": op, "name": m.group(1)}
    m = re.fullmatch(r"BBContainerData\.create_counter_(hint|display)\(ItemSets\.([A-Z_0-9]+), (\d+)\)", expr)
    if m:
        return {"op": "counter_" + m.group(1), "set": m.group(2), "amount": int(m.group(3))}
    raise ValueError(expr)


def parse_block(lines, i, indent):
    ops = []
    while i < len(lines):
        raw = lines[i]
        cur = len(raw) - len(raw.lstrip("\t"))
        line = raw.strip()
        if cur < indent:
            break
        if cur > indent:
            raise ValueError("unexpected indent: " + line)
        i += 1
        if line.startswith("bb_container_data.push_back("):
            ops.append(push(line[len("bb_container_data.push_back("):-1]))
        elif line.startswith("bb_container_data += "):
            ops.append(push(line[len("bb_container_data += "):]))
        elif line == "match specialization:":
            cases = []
            while i < len(lines):
                raw = lines[i]
                cur = len(raw) - len(raw.lstrip("\t"))
                if cur <= indent:
                    break
                inline = re.fullmatch(r"Specializations\.([A-Z_0-9]+): pass", raw.strip())
                if inline and cur == indent + 1:
                    i += 1
                    cases.append({"specialization": inline.group(1), "ops": []})
                    continue
                m = re.fullmatch(r"Specializations\.([A-Z_0-9]+):", raw.strip())
                if not m or cur != indent + 1:
                    raise ValueError("match case: " + raw.strip())
                i += 1
                body, i = parse_block(lines, i, indent + 2)
                cases.append({"specialization": m.group(1), "ops": body})
            ops.append({"op": "match_specialization", "cases": cases})
        elif line in ("pass", "var specialization: Specialization = args[0] as Specialization", "var bb_container_data: Array[BBContainerData] = []", "return bb_container_data"):
            pass
        else:
            raise ValueError(line)
    return ops, i


def strip_replacement_functions(lines):
    """Drop get_text_replacements/get_bb_replacements bodies (they are hand
    ported in src/bb_scripts.rs) and report which were present."""
    out, extra, skipping = [], [], False
    for l in lines:
        if l.startswith("func "):
            name = l.split("(")[0][5:]
            skipping = name in ("get_text_replacements", "get_bb_replacements")
            if skipping:
                extra.append(name)
        if not skipping:
            out.append(l)
    return out, extra


def export():
    scripts, unsupported = {}, {}
    for path in sorted(DIR.glob("*.gd")):
        text = path.read_text()
        lines = [l for l in text.split("\n") if l.strip() and not l.strip().startswith("#")]
        entry = {"sha256": hashlib.sha256(text.encode()).hexdigest()}
        try:
            lines, extra = strip_replacement_functions(lines)
            start = next(k for k, l in enumerate(lines) if l.startswith("func get_bb_container_data("))
            if any(l.startswith("func ") for l in lines[start + 1:]) or any(l.startswith("func ") and "get_bb_container_data" not in l for l in lines):
                raise ValueError("additional functions")
            if extra:
                entry["replacement_functions"] = extra
            ops, _ = parse_block(lines, start + 1, 1)
            entry["ops"] = ops
            entry["uses_specialization"] = "args[0]" in text
            scripts[path.stem] = entry
        except (ValueError, StopIteration) as e:
            unsupported[path.stem] = str(e)[:100]
    return {"scripts": scripts, "unsupported": unsupported}


if __name__ == "__main__":
    content = json.dumps(export(), indent=1, sort_keys=True) + "\n"
    if "--check" in sys.argv:
        if OUT.read_text() != content:
            sys.exit("bb-scripts.json stale; run python3 tools/export_bb_scripts.py")
    else:
        OUT.write_text(content)
    d = json.loads(content)
    print(len(d["scripts"]), "supported;", len(d["unsupported"]), "unsupported")
