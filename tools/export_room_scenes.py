#!/usr/bin/env python3
"""Export CSG room geometry from the recovered room .tscn files (text only).

Parent Node3D transforms are folded into each part's world matrix. MeshInstance3D
nodes (the monolith's custom ArrayMesh) and AnimatedLight/AnimationPlayer are not
exported.
"""
import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"
ROOMS = ["room_normal", "room_final", "chest_room", "mystic_trader", "monolith_room",
         "altar_room", "room_transition"]
IDENT = [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]


def mul(a, b):
    """Godot Transform3D text: three basis columns then origin."""
    def col(m, j):
        return m[3 * j:3 * j + 3]
    out = []
    for j in range(3):
        c = col(b, j)
        out += [sum(a[3 * k + i] * c[k] for k in range(3)) for i in range(3)]
    o = b[9:12]
    out += [sum(a[3 * k + i] * o[k] for k in range(3)) + a[9 + i] for i in range(3)]
    return out


def floats(text):
    return [float(x) for x in text.split(",")]


def export_room(name):
    text = (SOURCE / "scenes/rooms" / f"{name}.tscn").read_text()
    dims = {}
    for m in re.finditer(r'\[sub_resource type="ShaderMaterial" id="([^"]+)"\]\n(.*?)(?=\n\[|\Z)', text, re.S):
        d = re.search(r"shader_parameter/dim = ([0-9.]+)", m[2])
        dims[m[1]] = float(d[1]) if d else 1.0
    worlds, parts = {}, []
    for header, body in re.findall(r'^(\[node [^\n]*\])\n(.*?)(?=^\[|\Z)', text, re.M | re.S):
        node = re.match(r'\[node name="([^"]+)"(?: type="(\w+)")?(?: parent="([^"]*)")?', header)
        node_name, kind, parent = node[1], node[2], node[3]
        tr = re.search(r"^transform = Transform3D\(([^)]*)\)", body, re.M)
        local = floats(tr[1]) if tr else IDENT
        if parent is None:
            path, world = ".", local
        else:
            path = node_name if parent == "." else f"{parent}/{node_name}"
            world = mul(worlds[parent], local)
        worlds[path] = world
        mat = re.search(r'^material = SubResource\("([^"]+)"\)', body, re.M)
        dim = dims.get(mat[1], 1.0) if mat else 1.0
        if kind == "CSGBox3D":
            size = re.search(r"^size = Vector3\(([^)]*)\)", body, re.M)
            parts.append({"name": path, "shape": "box", "matrix": world,
                          "size": floats(size[1]) if size else [1, 1, 1], "dim": dim})
        elif kind == "CSGCylinder3D":
            r = re.search(r"^radius = ([0-9.e-]+)", body, re.M)
            h = re.search(r"^height = ([0-9.e-]+)", body, re.M)
            parts.append({"name": path, "shape": "cylinder", "matrix": world,
                          "size": [float(r[1]) if r else 0.5, float(h[1]) if h else 2.0, 0.0],
                          "dim": dim})
    return parts


def export():
    return {name: export_room(name) for name in ROOMS}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    out = ROOT / "artifacts/room-scenes.json"
    data = json.dumps(export(), indent=1) + "\n"
    if args.check:
        assert out.read_text() == data, "artifacts/room-scenes.json is stale"
    else:
        out.write_text(data)
        print({k: len(v) for k, v in json.loads(data).items()})
