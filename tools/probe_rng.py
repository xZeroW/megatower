#!/usr/bin/env python3
"""Execute RNG source/engine oracle without loading .tscn or addons."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
STAGING = Path("/tmp/opencode/megaloot-rng-probe")
SOURCE = ROOT / "Megaloot-GDRE-Recovered/singletons/rng_manager.gd"

if __name__ == "__main__":
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / "project.godot").write_text('config_version=5\n[application]\nconfig/name="RNG oracle"\n')
    (STAGING / "rng_manager.gd").write_text(SOURCE.read_text())
    (STAGING / "probe.gd").write_text((ROOT / "tools/probe_rng.gd").read_text())
    result = subprocess.run(["godot", "--headless", "--path", str(STAGING), "--script", "probe.gd"], check=True, capture_output=True, text=True, timeout=60)
    print(result.stdout, end="")
    print(result.stderr, end="")
    if "ERROR:" in result.stderr:
        raise SystemExit("RNG oracle failed; existing artifacts unchanged")
    data = json.loads((STAGING / "golden.json").read_text())
    assert len(data) == 60
    assert all(len(row["values"]) == 128 for row in data)
    (ROOT / "artifacts/rng-golden.json").write_text(json.dumps(data, separators=(",", ":")) + "\n")
    hashes = {"singletons/rng_manager.gd": hashlib.sha256(SOURCE.read_bytes()).hexdigest()}
    (ROOT / "artifacts/rng-source-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print("Exported 7680 source/engine-executed RNG operations")
