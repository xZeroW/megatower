#!/usr/bin/env python3
"""Build a script-free layout probe; never change the recovered project.

Godot, rather than a guessed Rust approximation, resolves PackedScene overrides,
minimum sizes, anchors, margins, atlas regions and Container geometry.
"""
import pathlib
import re
import shutil
import subprocess
import json

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"
PROBE = pathlib.Path("/tmp/opencode/megaloot-layout-probe")
PROBE.mkdir(parents=True, exist_ok=True)
for name in ("assets", "resources"):
    dest = PROBE / name
    if not dest.exists():
        dest.symlink_to(SOURCE / name, target_is_directory=True)
(PROBE / ".godot").mkdir(exist_ok=True)
cache = PROBE / ".godot/imported"
if not cache.exists():
    cache.symlink_to(SOURCE / ".godot/imported", target_is_directory=True)

# Only engine-native visual properties are kept. Exported gameplay properties
# cannot be assigned without the original scripts and their native SDKs.
properties = {
    "visible", "clip_contents", "custom_minimum_size", "layout_mode",
    "anchors_preset", "grow_horizontal", "grow_vertical", "mouse_filter",
    "size_flags_horizontal", "size_flags_vertical", "size_flags_stretch_ratio",
    "alignment", "columns", "text", "horizontal_alignment", "vertical_alignment",
    "texture", "region_rect", "expand_mode", "stretch_mode", "draw_center",
    "patch_margin_left", "patch_margin_top", "patch_margin_right", "patch_margin_bottom",
    "value", "max_value", "min_value", "show_percentage", "current_tab",
    "tabs_visible", "fit_content", "bbcode_enabled", "scroll_active", "material",
}
for scene in SOURCE.rglob("*.tscn"):
    if ".godot" in scene.parts:
        continue
    lines, section, skip = [], "", False
    in_dict = False
    drop_dicts = scene.name in ("enemy_container.tscn", "intro_state.tscn", "main_menu_state.tscn", "library_state.tscn", "profile_state.tscn", "adventurer_profile_popup.tscn", "adventurer_overview_container.tscn")
    for line in scene.read_text().splitlines():
        # Multi-line dictionaries (AnimationPlayer `libraries = {...}`) are not
        # kept; dropping only the opening line would orphan the body.
        # Only scenes that are laid out standalone get this: in the gameplay
        # canvas the orphaned body currently hides SubViewport children whose
        # default 512x512 size would otherwise distort the resolved layout.
        if drop_dicts and in_dict:
            in_dict = line.strip() != "}"
            continue
        if drop_dicts and line.startswith("libraries = {"):
            in_dict = True
            continue
        if line.startswith("["):
            section = line.split(" ", 1)[0]
            skip = section == "[connection" or (section == "[ext_resource" and any(f'type="{kind}"' in line for kind in ("Script", "Resource"))) or (section == "[sub_resource" and 'type="GDScript"' in line)
            if section == "[node":
                line = re.sub(r' node_paths=PackedStringArray\([^)]*\)', '', line)
            if not skip:
                lines.append(line)
            continue
        if skip:
            continue
        if line.startswith("script = "):
            continue
        if section == "[node" and " = " in line:
            key = line.split(" = ", 1)[0]
            if key in ("icon", "icon_texture"):
                lines.append(line.replace(key + " =", "metadata/source_icon =", 1))
                continue
            if key == "text":
                lines.append(line.replace("text =", "metadata/source_text =", 1))
            if key not in properties and not key.startswith(("anchor_", "offset_", "theme_override_")):
                continue
        lines.append(line)
    output = PROBE / scene.relative_to(SOURCE)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n")

# DeathScreen renders inside a SubViewport (640x360). The headless probe cannot
# lay out SubViewport children, so stage it as plain full-rect Controls: the
# viewport has the container's size, so the geometry is identical.
death = PROBE / "scenes/ui/death_screen/death_screen.tscn"
text = death.read_text()
text = text.replace('type="SubViewportContainer"', 'type="Control"')
# The staging filter drops `libraries = {` but keeps the dictionary body, which
# breaks parsing of every later node; remove those orphaned lines.
text = re.sub(r'(\[node name="AnimationPlayer"[^\n]*\]\n)&"": SubResource\([^\n]*\)\n\}\n', r'\1', text)
text = text.replace('[node name="SubViewport" type="SubViewport" parent="."]', '[node name="SubViewport" type="Control" parent="."]\nlayout_mode = 1\nanchors_preset = 15\nanchor_right = 1.0\nanchor_bottom = 1.0\ngrow_horizontal = 2\ngrow_vertical = 2')
death.write_text(text)

# Options tabs (GenericTabContainer.current_tab): 1 video, 2 audio, 3 misc.
options = PROBE / "scenes/ui/options_screen/options_screen.tscn"
for tab, suffix in ((1, "video"), (2, "audio"), (3, "misc")):
    (options.parent / f"options_screen_{suffix}.tscn").write_text(
        options.read_text().replace("current_tab = 0", f"current_tab = {tab}", 1))

# Adventurer profile popup: the file selects the Records tab (`current_tab = 1`);
# stage the Overview tab too.
popup = PROBE / "scenes/popup_containers/adventurer_profile_popup/adventurer_profile_popup.tscn"
(popup.parent / "adventurer_profile_popup_overview.tscn").write_text(
    popup.read_text().replace("current_tab = 1", "current_tab = 0", 1))

# Library tabs: `GenericTabContainer` shows `current_tab`; stage the Bestiary tab.
library = PROBE / "scenes/states/library_state/library_state.tscn"
for tab, suffix in ((1, "bestiary"), (2, "synergies"), (3, "passives")):
    (library.parent / f"library_state_{suffix}.tscn").write_text(
        library.read_text().replace("current_tab = 0", f"current_tab = {tab}", 1))

for shader in SOURCE.rglob("*.gdshader"):
    if ".godot" in shader.parts:
        continue
    target = PROBE / shader.relative_to(SOURCE)
    target.parent.mkdir(parents=True, exist_ok=True)
    if not (target.exists() and target.samefile(shader)):
        shutil.copyfile(shader, target)

(PROBE / "project.godot").write_text('''config_version=5
[application]
config/name="Megaloot layout probe"
[display]
window/size/viewport_width=640
window/size/viewport_height=360
[gui]
theme/custom_font="res://assets/fonts/monogram-extended.ttf"
[rendering]
renderer/rendering_method="gl_compatibility"
''')
shutil.copyfile(ROOT / "tools/export_layout.gd", PROBE / "export_layout.gd")
shutil.copyfile(ROOT / "tools/export_enemy_container.gd", PROBE / "export_enemy_container.gd")
import_paths = {}
for metadata in (SOURCE / "assets").rglob("*.import"):
    source_path = "res://" + str(metadata.relative_to(SOURCE))[:-7]
    for cached in re.findall(r'res://\.godot/imported/[^"\s]+', metadata.read_text()):
        import_paths[cached] = source_path
(PROBE / "import_paths.json").write_text(json.dumps(import_paths))
subprocess.run(["godot", "--headless", "--path", str(PROBE), "--script", "export_layout.gd", "--", str(ROOT)], check=True, timeout=600)
subprocess.run(["godot", "--headless", "--path", str(PROBE), "--script", "export_enemy_container.gd", "--", str(ROOT)], check=True, timeout=300)

# Extra standalone screens (esc menu, options, ...), resolved the same way.
SCREENS = {
    "esc-menu-layout": ("res://scenes/ui/esc_menu_screen/esc_menu_screen.tscn", "640x360", "0x0"),
    "options-layout": ("res://scenes/ui/options_screen/options_screen.tscn", "640x360", "0x0"),
    "intro-layout": ("res://scenes/states/intro_state/intro_state.tscn", "640x360", "0x0"),
    "main-menu-layout": ("res://scenes/states/main_menu_state/main_menu_state.tscn", "640x360", "0x0"),
    "intro-layout": ("res://scenes/states/intro_state/intro_state.tscn", "640x360", "0x0"),
    "main-menu-layout": ("res://scenes/states/main_menu_state/main_menu_state.tscn", "640x360", "0x0"),
    "intro-layout": ("res://scenes/states/intro_state/intro_state.tscn", "640x360", "0x0"),
    "main-menu-layout": ("res://scenes/states/main_menu_state/main_menu_state.tscn", "640x360", "0x0"),
    "library-layout": ("res://scenes/states/library_state/library_state.tscn", "640x360", "0x0"),
    "library-bestiary-layout": ("res://scenes/states/library_state/library_state_bestiary.tscn", "640x360", "0x0"),
    "library-synergies-layout": ("res://scenes/states/library_state/library_state_synergies.tscn", "640x360", "0x0"),
    "library-passives-layout": ("res://scenes/states/library_state/library_state_passives.tscn", "640x360", "0x0"),
    "options-video-layout": ("res://scenes/ui/options_screen/options_screen_video.tscn", "640x360", "0x0"),
    "options-audio-layout": ("res://scenes/ui/options_screen/options_screen_audio.tscn", "640x360", "0x0"),
    "options-misc-layout": ("res://scenes/ui/options_screen/options_screen_misc.tscn", "640x360", "0x0"),
    "profile-layout": ("res://scenes/states/profile_state/profile_state.tscn", "640x360", "0x0"),
    "adventurer-profile-layout": ("res://scenes/popup_containers/adventurer_profile_popup/adventurer_profile_popup.tscn", "640x360", "0x0"),
    "adventurer-overview-layout": ("res://scenes/popup_containers/adventurer_profile_popup/adventurer_profile_popup_overview.tscn", "640x360", "0x0"),
    "death-screen-layout": ("res://scenes/ui/death_screen/death_screen.tscn", "640x360", "0x0"),
    # Sits in the Inventory's PlayerContainer: 308x268 below the 26px tab row.
    "battle-log-layout": ("res://scenes/ui/battle_log_tab_container/battle_log_tab_container.tscn", "308x268", "0x26"),
}
for name, (scene, size, position) in SCREENS.items():
    subprocess.run(["godot", "--headless", "--path", str(PROBE), "--script", "export_layout.gd", "--", str(ROOT), scene, name, size, position], check=True, timeout=300)


def patch_instanced_button_texts(layout_name, scene_res):
    """Instanced GenericButtons keep their `text` only on the button node, which
    the probe cannot forward to the NameLabel; copy it in from the source."""
    # Staged variants (library_state_bestiary) take their texts from the original.
    scene = SOURCE / scene_res.removeprefix("res://").replace("library_state_bestiary", "library_state").replace("library_state_passives", "library_state").replace("library_state_synergies", "library_state").replace("adventurer_profile_popup_overview", "adventurer_profile_popup").replace("options_screen_video", "options_screen").replace("options_screen_audio", "options_screen").replace("options_screen_misc", "options_screen")
    texts, current = {}, None
    for line in scene.read_text().splitlines():
        if line.startswith("[node "):
            m = re.match(r'\[node name="([^"]+)"(?: parent="([^"]*)")?.*instance=', line)
            current = (m.group(2) + "/" + m.group(1)) if m and m.group(2) else None
        elif current and line.startswith('text = "'):
            texts[current] = line[len('text = "'):-1]
            current = None
    target = ROOT / "artifacts" / f"{layout_name}.json"
    data = json.loads(target.read_text())
    for visual in data["visuals"]:
        path = visual["path"]
        if visual["kind"] != "Label" or not path.endswith("/MarginContainer/HBoxContainer/NameLabel"):
            continue
        owner = path[: -len("/MarginContainer/HBoxContainer/NameLabel")]
        for key, text in texts.items():
            if owner.endswith("/" + key):
                visual["text"] = text
    target.write_text(json.dumps(data, indent="\t"))


for name, (scene, _size, _position) in SCREENS.items():
    patch_instanced_button_texts(name, scene)
