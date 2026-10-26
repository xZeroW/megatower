#!/usr/bin/env python3
"""Execute recovered GameModeScript/Challenge/Practice bodies in Godot 4.4.1.

Fixtures: Rooms identities, a memory shell exposing is_endless, GameMode fields
(base_room_count default 4, recovered last_floor values) and a stat-reading
Character. The script text, StatUtils.modify and ceil are unchanged source.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from probe_character_stats import methods
from probe_items import ROOT, SOURCE

STAGING = Path('/tmp/opencode/megaloot-game-mode-probe')
ROOMS = ['ENTRANCE', 'FINAL', 'MERCHANT', 'BATTLE', 'CHEST', 'ENDLESS_MONOLITH', 'MYSTIC_TRADER', 'REFORGE_TRANSFER_ROOM', 'ENEMY_UPGRADE', 'DISMANTLE', 'FOREST_ROOM']


def inner(path, name, parent=None):
    lines = (SOURCE / path).read_text().splitlines()
    body = []
    for line in lines:
        if line.startswith('class_name ') or line.startswith('extends '):
            continue
        body.append(line.expandtabs(4))
    text = '\n'.join(body)
    # The recovered _init reads the StateManager autoload; instances are populated by the harness.
    text = re.sub(r'^func _init\(\) -> void :.*?(?=^func |\Z)', '', text, flags=re.M | re.S)
    return f'class {name} extends {parent}:\n' + '    pass\n' + '\n'.join('    ' + line if line else line for line in text.splitlines()) + '\n'


def build():
    rooms = 'class RoomResource:\n    var name := ""\n' + 'class Rooms:\n' + '\n'.join(f'    static var {name} := RoomResource.new()' for name in ROOMS) + '\n    static func name_of(room) -> String:\n' + ''.join(f'        if room == {name}:\n            return "{name}"\n' for name in ROOMS) + '        return "?"\n'
    fixtures = '''
class Stats:
    static var GOLD_ON_KILL = "gold_on_kill"
    static var GREED = "greed"
class Memory:
    var is_endless := false
class GameMode:
    var base_room_count := 4
    var last_floor := -1
class GameplayComponent:
    var memory = Memory.new()
class Character:
    var values := {}
    func get_stat_amount(stat) -> Array[float]:
        return [float(values.get(stat, 0.0)), 0.0, 0.0]
class StatUtils:
''' + methods('classes/utils/stat_utils.gd', ['modify']).replace('    func modify', '    static func modify')
    scripts = inner('classes/objects/game_mode_script.gd', 'GameModeScript', 'GameplayComponent')
    scripts += inner('resources/game_mode_scripts/challenge.gd', 'Challenge', 'GameModeScript')
    scripts += inner('resources/game_mode_scripts/practice.gd', 'Practice', 'GameModeScript')
    return rooms + fixtures + scripts


RUNNER = '''
func Rooms_name(room) -> String:
    return Rooms.name_of(room)

func _init():
    var out := []
    var floors := []
    var players := [null, {"gold_on_kill": 0.0, "greed": 0.0}, {"gold_on_kill": 7.0, "greed": 33.0}, {"gold_on_kill": 120.0, "greed": -20.0}]
    for mode_id in ["challenge", "practice"]:
        for endless in [false, true]:
            var script = Challenge.new() if mode_id == "challenge" else Practice.new()
            script.game_mode = GameMode.new()
            script.game_mode.last_floor = 100 if mode_id == "challenge" else -1
            script.memory.is_endless = endless
            for floor_number in range(0, 151):
                floors.push_back([mode_id, endless, floor_number, Rooms_name(script.get_last_room(floor_number)), script.get_battles_this_floor(floor_number),
                    script.get_rooms_up_to_floor(floor_number, 0).count(Rooms.BATTLE), script.get_rooms_up_to_floor(floor_number, 3).count(Rooms.BATTLE)])
                for room_idx in range(-1, 9):
                    var gold := []
                    for p in players:
                        var character = null
                        if p != null:
                            character = Character.new()
                            character.values = {Stats.GOLD_ON_KILL: p.gold_on_kill, Stats.GREED: p.greed}
                        gold.push_back(script.get_gold_on_kill(character, floor_number))
                    out.push_back([mode_id, endless, floor_number, room_idx, Rooms_name(script.get_room_type(floor_number, room_idx)), script.get_room_count(floor_number),
                        script.get_max_enemies(floor_number, room_idx), script.get_gold_reward(floor_number), script.get_battle_count(floor_number, room_idx),
                        script.is_last_room(floor_number, room_idx), gold])
    var file := FileAccess.open("res://golden.json", FileAccess.WRITE)
    file.store_string(JSON.stringify({"rows": out, "floors": floors}))
    quit()
'''

if __name__ == '__main__':
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Game mode oracle"\n')
    (STAGING / 'probe.gd').write_text('extends SceneTree\nconst ROOM_NAMES = ' + json.dumps(ROOMS) + '\n' + build() + RUNNER)
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=600)
    print(result.stdout[-800:], result.stderr[-2500:])
    if result.returncode or 'ERROR:' in result.stderr or 'SCRIPT ERROR' in result.stdout:
        raise SystemExit('Game mode oracle failed; artifacts unchanged')
    golden = json.loads((STAGING / 'golden.json').read_text())
    (ROOT / 'artifacts/game-mode-golden.json').write_text(json.dumps(golden, separators=(',', ':')) + '\n')
    hashes = {p: hashlib.sha256((SOURCE / p).read_bytes()).hexdigest() for p in
              ['classes/objects/game_mode_script.gd', 'resources/game_mode_scripts/challenge.gd', 'resources/game_mode_scripts/practice.gd', 'classes/utils/stat_utils.gd']}
    (ROOT / 'artifacts/game-mode-source-hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print('Exported', len(golden['rows']), 'room rows and', len(golden['floors']), 'floor rows')
