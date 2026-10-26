#!/usr/bin/env python3
"""Execute recovered `Format` helpers in Godot 4.4.1."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered/classes/utils/format.gd"
STAGING = Path("/tmp/opencode/megaloot-format-probe")


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        expected = json.loads((ROOT / "artifacts/format-source-hashes.json").read_text())
        actual = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        if expected.get("classes/utils/format.gd") != actual:
            raise SystemExit("format oracle is stale; run python3 tools/probe_format.py")
        raise SystemExit(0)
    if len(sys.argv) != 1:
        raise SystemExit("usage: probe_format.py [--check]")
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / "project.godot").write_text(
        'config_version=5\n[application]\nconfig/name="Format oracle"\n'
    )
    (STAGING / "format.gd").write_bytes(SOURCE.read_bytes())
    (STAGING / "probe.gd").write_text(
        """extends SceneTree
const Format = preload("res://format.gd")

func _init() -> void:
    var amounts := [-1234567.8, -1000.0, -999.99, -100.01, -99.99, -1.05, -0.01, 0.0, 0.01, 1.05, 9.999, 10.0, 99.99, 100.0, 999.99, 1000.0, 1000.1, 9999.9, 999999.0, 1000000.0, 1234567.0, 999999999.0, 1000000000.0, 1e63, 1.0000000000000001e66, 1e69]
    var numbers := []
    for amount in amounts:
        for mask in range(32):
            var rules: Array[Format.Rules] = []
            if mask & 1:
                rules.push_back(Format.Rules.USE_PREFIX)
            if mask & 2:
                rules.push_back(Format.Rules.USE_SUFFIX)
            if mask & 4:
                rules.push_back(Format.Rules.PERCENTAGE)
            if mask & 8:
                rules.push_back(Format.Rules.IGNORE_PLUS)
            if mask & 16:
                rules.push_back(Format.Rules.IS_WHOLE)
            numbers.push_back([amount, mask, Format.number(amount, rules)])
    var times := []
    for value in [-1.25, 0.0, 0.001, 0.9999, 1.234, 59.999, 60.0, 3599.999, 3600.0, 3661.789]:
        times.push_back([value, Format.time(value)])
    var seconds := []
    for value in [-1, 0, 1, 59, 60, 3599, 3600, 3661]:
        seconds.push_back([value, Format.to_seconds(value)])
    var file := FileAccess.open("res://golden.json", FileAccess.WRITE)
    file.store_string(JSON.stringify({"numbers": numbers, "times": times, "seconds": seconds}))
    quit()
"""
    )
    result = subprocess.run(
        ["godot", "--headless", "--path", str(STAGING), "--script", "probe.gd"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    print(result.stdout, end="")
    print(result.stderr, end="")
    if result.returncode or "ERROR:" in result.stderr or "SCRIPT ERROR" in result.stdout:
        raise SystemExit("Format oracle failed; artifacts unchanged")
    golden = json.loads((STAGING / "golden.json").read_text())
    assert len(golden["numbers"]) == 832
    assert len(golden["times"]) == 10
    assert len(golden["seconds"]) == 8
    (ROOT / "artifacts/format-golden.json").write_text(
        json.dumps(golden, separators=(",", ":")) + "\n"
    )
    (ROOT / "artifacts/format-source-hashes.json").write_text(
        json.dumps(
            {"classes/utils/format.gd": hashlib.sha256(SOURCE.read_bytes()).hexdigest()},
            indent=2,
        )
        + "\n"
    )
    print("Exported", len(golden["numbers"]), "number,", len(golden["times"]), "time and", len(golden["seconds"]), "to_seconds cases")
