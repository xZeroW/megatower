#!/usr/bin/env python3
"""Export only stat data and singleton groups; never instantiate .tscn files."""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"


def export():
    definitions = []
    for path in sorted((SOURCE / "resources/stats").glob("*.tres")):
        text = path.read_text()
        external = dict((key, value) for value, key in re.findall(
            r'\[ext_resource [^\n]*path="([^"]+)" id="([^"]+)"\]', text))
        # Limit fields to the root resource, not inline BBScript source text.
        fields = dict(re.findall(r'^(\w+) = (.+)$', text.split("\n[resource]\n")[1], re.M))

        def reference(key):
            value = fields.get(key, "null")
            if value == "null":
                return None
            match = re.fullmatch(r'ExtResource\("([^"]+)"\)', value)
            if match:
                return external[match[1]]
            if key == "bb_script" and value.startswith("SubResource("):
                return "res://" + path.relative_to(SOURCE).as_posix() + "::" + value
            raise ValueError(f"Unsupported {path}: {key} = {value}")

        origin = reference("origin_stat")
        color = fields.get("color", "Color(0, 0, 0, 1)")
        assert color.startswith("Color(") and color.endswith(")"), path
        definitions.append({
            "id": path.stem,
            "name": json.loads(fields.get("name", '""')),
            "icon": reference("icon"),
            "color": [float(x) for x in color[6:-1].split(",")],
            "is_attack_type": fields.get("is_attack_type", "false") == "true",
            "is_percentage": fields.get("is_percentage", "false") == "true",
            "max_amount": int(float(fields.get("max_amount", "-1"))),
            "origin_stat": Path(origin).stem if origin else None,
            "ignore_minimum_amount": fields.get("ignore_minimum_amount", "false") == "true",
            "minimum_amount": float(fields.get("minimum_amount", "0.0")),
            "stat_script": reference("stat_script"),
            "bb_script": reference("bb_script"),
        })

    singleton = (SOURCE / "singletons/stats.gd").read_text()
    ids = dict(re.findall(r'var (\w+): StatResource = load\("res://resources/stats/(\w+)\.tres"\)', singleton))
    groups = {}
    for name, body in re.findall(r'var (\w+): Array\[StatResource\] = \[([^\]]*)\]', singleton):
        groups[name.lower()] = [ids[name] for name in re.findall(r'\b[A-Z][A-Z_]+\b', body)]
    groups["list"] = [row["id"] for row in definitions]
    groups["transformed_stats"] = [row["id"] for row in definitions if row["origin_stat"]]
    return json.dumps({"definitions": definitions, "groups": groups}, indent=2) + "\n"


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    target = ROOT / "artifacts/stats.json"
    content = export()
    if args.check:
        if target.read_text() != content:
            raise SystemExit("Stat catalog is stale; run python3 tools/export_stats.py")
    else:
        target.write_text(content)
        print(f"Exported {len(json.loads(content)['definitions'])} stat definitions")
