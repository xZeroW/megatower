#!/usr/bin/env python3
"""Export CSG room geometry, materials and lights from the recovered room
.tscn files (text only).

Parent Node3D transforms are folded into each part's world matrix. Each part
carries its material's wall texture, UV scale and `dim`; `AnimatedLight`
instances become `lights` (world position, range, attenuation). MeshInstance3D
nodes (the monolith's custom meshes) and AnimationPlayer are not exported; the
lights' tiny range flicker (0.015) is not modelled.
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


def find_texture(name):
    """`dungeon_wall.png-<hash>.ctex` -> its source PNG under assets/."""
    for found in (SOURCE / "assets").rglob(name):
        return found.relative_to(SOURCE).as_posix()
    raise SystemExit(f"texture {name} not found")


# `animated_light.tscn` defaults.
LIGHT_RANGE, LIGHT_ATTENUATION = 14.763, 0.05


def export_room(name):
    text = (SOURCE / "scenes/rooms" / f"{name}.tscn").read_text()
    ext_paths = dict((i, p) for p, i in re.findall(r'\[ext_resource [^\n]*path="res://([^"]+)" id="([^"]+)"\]', text))
    ctex = {}
    for m in re.finditer(r'\[sub_resource type="CompressedTexture2D" id="([^"]+)"\]\nload_path = "[^"]*/([^"/]+?\.png)-[0-9a-f]+\.ctex"', text):
        ctex[m[1]] = find_texture(m[2])
    materials = {}
    for m in re.finditer(r'\[sub_resource type="ShaderMaterial" id="([^"]+)"\]\n(.*?)(?=\n\[|\Z)', text, re.S):
        body = m[2]
        d = re.search(r"shader_parameter/dim = ([0-9.]+)", body)
        t = re.search(r'shader_parameter/texture_albedo = (Sub|Ext)Resource\("([^"]+)"\)', body)
        u = re.search(r"shader_parameter/uv1_scale = Vector3\(([0-9.]+),", body)
        texture = None
        if t:
            texture = ctex.get(t[2]) if t[1] == "Sub" else ext_paths.get(t[2])
        materials[m[1]] = {"dim": float(d[1]) if d else 1.0, "texture": texture,
                           "uv_scale": float(u[1]) if u else 1.0}
    dims = {k: v["dim"] for k, v in materials.items()}
    lights = []
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
        material = materials.get(mat[1], {}) if mat else {}
        texture = material.get("texture") or "assets/textures/3d/dungeon_wall.png"
        uv_scale = material.get("uv_scale", 1.75)
        if node_name.startswith("AnimatedLight"):
            rng = re.search(r"^omni_range = ([0-9.]+)", body, re.M)
            att = re.search(r"^omni_attenuation = ([0-9.]+)", body, re.M)
            lights.append({"name": path, "position": world[9:12],
                           "range": float(rng[1]) if rng else LIGHT_RANGE,
                           "attenuation": float(att[1]) if att else LIGHT_ATTENUATION})
            continue
        if kind == "CSGBox3D":
            size = re.search(r"^size = Vector3\(([^)]*)\)", body, re.M)
            parts.append({"name": path, "shape": "box", "matrix": world,
                          "size": floats(size[1]) if size else [1, 1, 1], "dim": dim,
                          "texture": texture, "uv_scale": uv_scale})
        elif kind == "CSGCylinder3D":
            r = re.search(r"^radius = ([0-9.e-]+)", body, re.M)
            h = re.search(r"^height = ([0-9.e-]+)", body, re.M)
            parts.append({"name": path, "shape": "cylinder", "matrix": world,
                          "size": [float(r[1]) if r else 0.5, float(h[1]) if h else 2.0, 0.0],
                          "dim": dim, "texture": texture, "uv_scale": uv_scale})
    return {"parts": parts, "lights": lights}


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
        print({k: (len(v["parts"]), len(v["lights"])) for k, v in json.loads(data).items()})
