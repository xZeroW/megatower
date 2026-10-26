#!/usr/bin/env python3
"""Execute the recovered market-refresh quote path in Godot 4.4.1."""

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MEMORY = ROOT / "Megaloot-GDRE-Recovered/classes/static/memory.gd"
MATH = ROOT / "Megaloot-GDRE-Recovered/classes/utils/math.gd"
MANAGER = ROOT / "Megaloot-GDRE-Recovered/scenes/managers/market_manager/market_manager.gd"
STAGING = Path("/tmp/opencode/megaloot-market-quote-probe")


def method(path: Path, name: str) -> str:
    text = path.read_text()
    match = re.search(rf"^(?:static )?func {name}\([^\n]*\).*?(?=^(?:static )?func |\Z)", text, re.M | re.S)
    if not match:
        raise RuntimeError(f"missing {name} in {path}")
    return match.group().expandtabs(4)


if __name__ == "__main__":
    hashes = {
        "classes/static/memory.gd": hashlib.sha256(MEMORY.read_bytes()).hexdigest(),
        "classes/utils/math.gd": hashlib.sha256(MATH.read_bytes()).hexdigest(),
        "scenes/managers/market_manager/market_manager.gd": hashlib.sha256(MANAGER.read_bytes()).hexdigest(),
    }
    if sys.argv[1:] == ["--check"]:
        expected = json.loads((ROOT / "artifacts/market-quote-source-hashes.json").read_text())
        if expected != hashes:
            raise SystemExit("market quote oracle is stale; run python3 tools/probe_market_quote.py")
        raise SystemExit(0)
    if len(sys.argv) != 1:
        raise SystemExit("usage: probe_market_quote.py [--check]")
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / "project.godot").write_text('config_version=5\n[application]\nconfig/name="Market quote oracle"\n')
    quote = method(MEMORY, "get_market_refresh_price")
    rounding = re.sub(r"^static func", "static func", method(MATH, "big_round"))
    refresh = method(MANAGER, "get_refresh_price")
    (STAGING / "probe.gd").write_text(
        """extends SceneTree
class Math:
"""
        + "\n".join("    " + line if line else line for line in rounding.splitlines())
        + """
class Price:
    var amount: float
    func _init(_currency, value: float) -> void:
        amount = value
class Stats:
    static var GOLD = null
class Player:
    var refresh_count := 0
class Memory:
    var floor_number := 0
    var room_idx := 0
    var base := 0.0
    var local_player := Player.new()
    func get_base_gold_on_kill(_floor_number: int) -> float:
        return base
"""
        + "\n".join("    " + line if line else line for line in quote.splitlines())
        + """
class MarketManager:
    var memory: Memory
"""
        + "\n".join("    " + line if line else line for line in refresh.splitlines())
        + """
func _init() -> void:
    var rows := []
    for base in [0.0, 1.0, 2.5, 10.0, 15.0, 100.0, 1000.0, 1000000.0]:
        for floor in [-2, 0, 1, 3, 4, 17, 80]:
            for room in [-1, 0, 5]:
                for refresh_count in [-2, 0, 1, 3, 4, 9, 30]:
                    var manager := MarketManager.new()
                    manager.memory = Memory.new()
                    manager.memory.base = base
                    manager.memory.floor_number = floor
                    manager.memory.room_idx = room
                    manager.memory.local_player.refresh_count = refresh_count
                    rows.push_back([base, floor, room, refresh_count, manager.get_refresh_price().amount])
    var file := FileAccess.open("res://golden.json", FileAccess.WRITE)
    file.store_string(JSON.stringify(rows))
    quit()
"""
    )
    result = subprocess.run(["godot", "--headless", "--path", str(STAGING), "--script", "probe.gd"], capture_output=True, text=True, timeout=60)
    print(result.stdout, end="")
    print(result.stderr, end="")
    if result.returncode or "ERROR:" in result.stderr or "SCRIPT ERROR" in result.stdout:
        raise SystemExit("Market quote oracle failed; artifacts unchanged")
    rows = json.loads((STAGING / "golden.json").read_text())
    assert len(rows) == 1176
    (ROOT / "artifacts/market-quote-golden.json").write_text(json.dumps(rows, separators=(",", ":")) + "\n")
    (ROOT / "artifacts/market-quote-source-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print("Exported", len(rows), "market refresh quote cases")
