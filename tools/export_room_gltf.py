#!/usr/bin/env python3
"""Export every recovered room scene to a baked .glb plus its material
parameters (`room-materials.json`), using Godot headless in a throwaway probe
project; the recovered project is never touched.

    tools/export_room_gltf.py [out_dir]   (default: crates/megatower-runtime/src/render/rooms)

The .glb files are embedded by `megatower-runtime`; `room-materials.json`
goes to `artifacts/` for `megatower-assets`.
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"
PROBE = pathlib.Path("/tmp/claude-1000/megatower-room-gltf-probe")
OUT = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "crates/megatower-runtime/src/render/rooms"

PROBE.mkdir(parents=True, exist_ok=True)
OUT.mkdir(parents=True, exist_ok=True)
for name in ("assets", "resources", "scenes"):
    dest = PROBE / name
    if not dest.exists():
        dest.symlink_to(SOURCE / name, target_is_directory=True)
(PROBE / ".godot").mkdir(exist_ok=True)
cache = PROBE / ".godot/imported"
if not cache.exists():
    cache.symlink_to(SOURCE / ".godot/imported", target_is_directory=True)
(PROBE / "project.godot").write_text('config_version=5\n[application]\nconfig/name="Megaloot room glTF probe"\n'
                                     '[rendering]\nrenderer/rendering_method="gl_compatibility"\n')
import_paths = {}
for metadata in (SOURCE / "assets").rglob("*.import"):
    source_path = "res://" + str(metadata.relative_to(SOURCE))[:-7]
    for cached in re.findall(r'res://\.godot/imported/[^"\s]+', metadata.read_text()):
        import_paths[cached] = source_path
(PROBE / "import_paths.json").write_text(json.dumps(import_paths))
(PROBE / "export_room_gltf.gd").write_text((ROOT / "tools/export_room_gltf.gd").read_text())
subprocess.run(["godot", "--headless", "--path", str(PROBE), "--script", "export_room_gltf.gd", "--", str(OUT)],
               check=True, timeout=600)

(ROOT / "artifacts/room-materials.json").write_text((OUT / "room-materials.json").read_text())
(OUT / "room-materials.json").unlink()
