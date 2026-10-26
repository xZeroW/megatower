#!/usr/bin/env python3
"""Execute unchanged Enemy.get_new_* scaling bodies and Balance constants in Godot 4.4.1."""
import hashlib
import json
import subprocess
from pathlib import Path
from probe_character_stats import methods
from probe_items import ROOT, SOURCE

STAGING = Path('/tmp/opencode/megaloot-enemy-scaling-probe')
NAMES = ['get_new_stat_amount', 'get_new_max_health', 'get_new_armor', 'get_new_toughness', 'get_new_recovery',
         'get_new_physical_attack', 'get_new_magical_attack', 'get_new_freeze_attack', 'get_new_toxicity',
         'get_new_electricity', 'get_new_malice', 'get_new_agility', 'get_new_life_steal', 'get_new_cinder_damage',
         'get_new_dazzle', 'get_new_faith']
HARNESS = '''
class FakeResource:
    var floor_number: int = 0

class Balance:
    static var ENEMY_SCALE_RATE: float = 1.0175
    static var SCALE_RATE: float = 1.25

class Subject extends RefCounted:
    var resource := FakeResource.new()
    var player_count: int
    var battle_count: int
    var spawn_floor: int
    var level: int
''' 

def build():
    balance = (SOURCE / 'classes/static/balance.gd').read_text()
    assert 'ENEMY_SCALE_RATE: float = 1.0175' in balance and 'SCALE_RATE: float = 1.25' in balance
    text = HARNESS + methods('classes/objects/enemy.gd', NAMES)
    text += '''
const NAMES = %s
func _init():
''' % json.dumps(NAMES) + '''
    var out := []
    for spawn_floor in [1, 2, 5, 10, 30, 100]:
        for level in range(0, 8):
            for battle_count in [0, 1, 5, 40]:
                for player_count in [0, 1, 3]:
                    for resource_floor in [0, 3, 9]:
                        for base in [0.0, 1.0, 4.0, 10.0, 37.5, 120.0]:
                            var s := Subject.new()
                            s.spawn_floor = spawn_floor
                            s.level = level
                            s.battle_count = battle_count
                            s.player_count = player_count
                            s.resource.floor_number = resource_floor
                            var row := [spawn_floor, level, battle_count, player_count, resource_floor, base]
                            for name in NAMES:
                                row.push_back(s.call(name, base))
                            out.push_back(row)
    var file := FileAccess.open("res://golden.json", FileAccess.WRITE)
    file.store_string(JSON.stringify(out))
    quit()
'''
    return text


if __name__ == '__main__':
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Enemy scaling oracle"\n')
    # Subject is the only class using the methods; _init lives on a SceneTree script.
    body = build()
    head, tail = body.split('\nconst NAMES', 1)
    script = ('extends SceneTree\n' + head.replace('class Subject extends RefCounted:', 'class Subject extends RefCounted:').strip('\n')
              + '\nconst NAMES' + tail)
    (STAGING / 'probe.gd').write_text(script)
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=300)
    print(result.stdout[-500:], result.stderr[-1500:])
    if result.returncode or 'ERROR:' in result.stderr or 'SCRIPT ERROR' in result.stdout:
        raise SystemExit('Enemy scaling oracle failed; artifacts unchanged')
    data = json.loads((STAGING / 'golden.json').read_text())
    (ROOT / 'artifacts/enemy-scaling-golden.json').write_text(json.dumps({'names': NAMES, 'rows': data}, separators=(',', ':')) + '\n')
    (ROOT / 'artifacts/enemy-scaling-source-hashes.json').write_text(json.dumps({
        p: hashlib.sha256((SOURCE / p).read_bytes()).hexdigest() for p in ['classes/objects/enemy.gd', 'classes/static/balance.gd']}, indent=2) + '\n')
    print('Exported', len(data), 'source-executed enemy scaling rows')
