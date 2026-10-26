#!/usr/bin/env python3
"""Run recovered upgrade function bodies in Godot 4.4, without scenes/addons.

Only the method is extracted. A tiny Item/SocketTypes fixture supplies socket
identity; missing items, ordinary items and replicated weapons are separate cases.
Faith/Luck runtime signal handlers are deliberately outside this probe.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"
STAGING = Path("/tmp/opencode/megaloot-stat-probe")


def build_probe():
    math = (SOURCE / "classes/utils/math.gd").read_text()
    text = '''extends SceneTree

class SocketTypes:
    const WEAPON = 1

class Item extends RefCounted:
    var resource = {"socket_type": 0}

class StatScript extends RefCounted:
    func get_amount_to_upgrade(_item: Item, _modifier: bool, _floor: int) -> float:
        return 0.0

class Math:
'''
    text += "\n".join("\t" + line for line in math.splitlines()[1:]) + "\n"
    paths = sorted((SOURCE / "resources/stat_scripts").glob("*.gd"))
    hashes = {"classes/utils/math.gd": hashlib.sha256(math.encode()).hexdigest()}
    for index, path in enumerate(paths):
        source = path.read_text()
        hashes[path.relative_to(SOURCE).as_posix()] = hashlib.sha256(source.encode()).hexdigest()
        match = re.search(r'^func get_amount_to_upgrade\(.*?(?=^func |\Z)', source, re.M | re.S)
        text += f"\nclass Rule{index} extends StatScript:\n"
        if match:
            text += "\n".join("\t" + line for line in match[0].splitlines()) + "\n"
        else:
            text += "\tpass\n"
    text += '\nfunc _initialize() -> void:\n\tvar rows: Array = []\n'
    for index, path in enumerate(paths):
        text += f'''\tfor floor_number in [-1, 0, 1, 2, 9, 24, 25, 39, 99, 100, 999, 10000]:
\t\tfor modifier in [false, true]:
\t\t\tfor socket in ["missing", "other", "weapon", "replicated_weapon"]:
\t\t\t\tvar item: Item = null
\t\t\t\tif socket != "missing":
\t\t\t\t\titem = Item.new()
\t\t\t\t\titem.resource.socket_type = 1 if socket == "weapon" else 2
\t\t\t\trows.append({{"rule": "{path.stem}", "floor": floor_number, "modifier": modifier, "socket": socket, "amount": Rule{index}.new().get_amount_to_upgrade(item, modifier, floor_number)}})
'''
    output = STAGING / "golden.json"
    text += f'\tvar file = FileAccess.open({json.dumps(str(output))}, FileAccess.WRITE)\n'
    text += '\tfile.store_string(JSON.stringify(rows) + "\\n")\n\tfile.close()\n\tquit()\n'
    return text.expandtabs(4), hashes


if __name__ == "__main__":
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / "project.godot").write_text('config_version=5\n[application]\nconfig/name="Stat oracle"\n')
    text, hashes = build_probe()
    (STAGING / "probe.gd").write_text(text)
    result = subprocess.run(["godot", "--headless", "--path", str(STAGING), "--script", "probe.gd"], check=True, timeout=60, capture_output=True, text=True)
    print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="")
    if "SCRIPT ERROR" in result.stderr or "ERROR:" in result.stderr:
        raise SystemExit("Godot oracle failed; existing artifacts were not overwritten")
    content = (STAGING / "golden.json").read_text()
    rows = json.loads(content)
    assert len(rows) == 34 * 12 * 2 * 4, len(rows)
    (ROOT / "artifacts/stat-upgrade-golden.json").write_text(content)
    (ROOT / "artifacts/stat-upgrade-source-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(f"Exported {len(rows)} source-executed upgrade cases")
