#!/usr/bin/env python3
"""Run recovered character item-source methods with real item/container code.

Synthetic dependency definitions, not the real item/set/passive catalog. No scenes.
"""
import hashlib
import json
import subprocess
from pathlib import Path
from probe_inventory import build
from probe_items import ROOT, SOURCE
from probe_character_stats import methods

STAGING = Path('/tmp/opencode/megaloot-character-items-probe')

if __name__ == '__main__':
    text, paths = build()
    text = text.split('\nvar pool:')[0]
    character = '''class Character extends RefCounted:
    var inventory: ItemContainer
    var equipment: ItemContainer
    var equipment_stats: Array[Stat] = []
    var stats: Array[Stat] = []
    var cached_stats: Dictionary = {}
    var base_passive: Passive = null
    var battle_profile = BattleProfile.new()
    func get_active_specializations() -> Array[Specialization]: return []
    func recalculate_cached_stats() -> void: pass
'''
    names = ['get_equipment_items','get_stats_from_items','get_active_item_sets','get_limited_active_item_sets','get_item_set_count','get_passives','get_all_stat_arr','cache_stat','cache_stats']
    character += methods('classes/objects/character.gd',names)
    text = text.replace('class Character extends RefCounted:\n    pass\n',character)
    text = text.replace('class AbilityResource extends Resource:\n    pass', 'class AbilityResource extends Resource:\n    var bonus_stats_while_active: Array[BonusStat] = []')
    text = text.replace('class Passive extends Resource:\n', 'class Passive extends Resource:\n    var bonus_stats: Array[BonusStat] = []\n')
    text = text.replace('class ItemSetResource extends Resource:\n', 'class ItemSetResource extends Resource:\n    var bonus_stats: Array[BonusStatsArray] = []\n')
    text = text.replace('class Stats:\n', 'class Stats:\n    static var LIST: Array[StatResource] = []\n')
    # Instrument lifetime tracking only: source cache_stats leaves temporary
    # Object Stats alive. Free probe leftovers after observations, not in-game.
    text = text.replace('class Stat extends Object:\n', 'class Stat extends Object:\n    static var _probe_instances: Array = []\n')
    text = text.replace('    func _init(args: Array = []) -> void :\n', '    func _init(args: Array = []) -> void :\n        _probe_instances.push_back(self)\n')
    text += '''
class Specialization extends Resource:
    pass
class BattleProfile extends RefCounted:
    var active_item_sets: Array[ItemSetResource] = []
    var external_stats: Array[Stat] = []
    var stats: Array[Stat] = []
    var turn = BattleTurn.new()
    func get_curr_turn() -> BattleTurn: return turn
class BattleTurn extends RefCounted:
    var stats: Array[Stat] = []
    var abilities: Array[Ability] = []
class Ability extends RefCounted:
    var resource: AbilityResource
class BonusStatsArray extends Resource:
    var bonus_stats: Array[BonusStat] = []
'''
    text += (ROOT/'tools/probe_character_items.gd').read_text()
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Character items oracle"\n')
    (STAGING/'probe.gd').write_text(text.expandtabs(4))
    result = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='')
    print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('Character item oracle failed; artifacts unchanged')
    content = (STAGING/'golden.json').read_text()
    data = json.loads(content)
    assert len(data) == 288
    (ROOT/'artifacts/character-items-golden.json').write_text(content)
    deps = [*paths,'classes/objects/character.gd']
    (ROOT/'artifacts/character-items-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print('Exported 288 character item-source cases / 1152 snapshots')
