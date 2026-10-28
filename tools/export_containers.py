#!/usr/bin/env python3
"""Export all recovered item-container resources; never loads a .tscn."""
import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"


def export():
    rows = []
    for path in sorted((SOURCE / "resources/item_containers").glob("*.tres")):
        fields = dict(re.findall(r'^(\w+) = (.+)$', path.read_text().split("\n[resource]\n")[1], re.M))
        row = {"id": path.stem, "source_path": "res://" + path.relative_to(SOURCE).as_posix()}
        for flag in ["move_to_inventory_on_alt_press", "merge_on_buy", "is_shop"]:
            value = fields.get(flag, "false")
            assert value in ("true", "false"), (path, flag)
            row[flag] = value == "true"
        row["size"] = int(fields.get("size", "0"))
        rows.append(row)
    assert len(rows) == 14
    return json.dumps(rows, indent=2) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = export()
    target = ROOT / "assets/data/content/item-containers.json"
    if args.check:
        if target.read_text() != content:
            raise SystemExit("Container catalog is stale; run python3 tools/export_containers.py")
    else:
        target.write_text(content)
        print("Exported 14 recovered container definitions")
