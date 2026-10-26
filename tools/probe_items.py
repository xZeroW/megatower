#!/usr/bin/env python3
"""Execute recovered item methods with explicit dependency fixtures, no scenes.

Item, ItemResource, Stat, BonusStat and StatUtils method bodies are unchanged.
Only Item's SaveSystem copy constructor and texture loader are omitted: neither
is covered by this oracle. The default generic-set preload becomes fixture
identity. Resource definitions below are synthetic branch fixtures, NOT a
replacement for the as-yet-unported recovered item catalog.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Megaloot-GDRE-Recovered"
STAGING = Path("/tmp/opencode/megaloot-item-probe")

FIXTURES = '''extends SceneTree

class ItemRarity:
    enum Type {COMMON, UNCOMMON, RARE, LEGENDARY, MYTHIC, ETERNAL, ABYSSAL, COSMIC, DIVINE}
class SocketType extends Resource:
    var name: String = "helmet"
class ItemSetResource extends Resource:
    var name: String
    func _init(value: String = "") -> void: name = value
class Passive extends Resource:
    var name: String = ""
class AbilityResource extends Resource:
    pass
class ItemUnlockRequirement extends Resource:
    pass
class ActivationEffect extends Resource:
    var use_limit: int = 3
class ConsumptionEffect extends Resource:
    var base_price: float = 4.0
    func get_value(floor_number: int) -> float: return base_price + floor_number
class StatResource extends Resource:
    var name: String
    var max_amount: int = -1
    var is_percentage: bool = false
    func _init(value: String = "", cap: int = -1) -> void:
        name = value
        max_amount = cap
class ItemSets:
    static var GENERIC = ItemSetResource.new("generic")
    static var REFORGE = ItemSetResource.new("reforge")
    static var PHANTOM = ItemSetResource.new("phantom")
    static var SET_A = ItemSetResource.new("set_a")
    static var SET_B = ItemSetResource.new("set_b")
class Stats:
    static var DIAMOND = StatResource.new("Diamond")
    static var GOLD = StatResource.new("Gold")
    static var PHYSICAL_ATTACK = StatResource.new("Physical Attack")
    static var ARMOR = StatResource.new("Armor")
    static var CRIT_CHANCE = StatResource.new("Crit Chance", 100)
    static var MAGIC_DAMAGE = StatResource.new()
    static var MAGIC_ATTACK = StatResource.new()
    static var FREEZE_DAMAGE = StatResource.new()
    static var FREEZE_ATTACK = StatResource.new()
    static var ARMOR_DAMAGE = StatResource.new()
    static var ARMORED_ATTACK = StatResource.new()
    static var PHYSICAL_DAMAGE = StatResource.new()
    static func get_amount_from_rarity(amount: float, rarity: int) -> float: return amount * (rarity + 1)
    static func get_bb_container_data(_stat): return BBContainerData.new()
class Empty:
    static var item_resource = ItemResource.new()
    static var passive = Passive.new()
class Items:
    static var INSIGHTUS: ItemResource = null
class Price extends RefCounted:
    var type: StatResource
    var amount: float
    func _init(kind: StatResource, value: float) -> void:
        type = kind
        amount = value
class Format:
    enum Rules {USE_PREFIX, PERCENTAGE}
    static func number(value, _rules = []) -> String: return str(value)
class BBContainerData extends RefCounted:
    func _init(_text = "") -> void: pass
class T:
    static func get_translated_string(value: String, _context: String = "") -> String: return value
class Math:
    static func rand_success(chance: float) -> bool: return randf() * 100.0 < chance
'''


def nest(path, name, omitted=()):
    text = (SOURCE / path).read_text()
    text = re.sub(r'^@tool\s*\n', '', text)
    text = re.sub(r'^class_name ' + name, 'class ' + name, text)
    for method in omitted:
        text = re.sub(r'^func ' + method + r'\(.*?(?=^(?:static )?func |\Z)', '', text, flags=re.M | re.S)
    text = text.replace('preload("res://resources/item_sets/generic.tres")', 'ItemSets.GENERIC')
    lines = text.splitlines()
    return lines[0] + ':\n' + '\n'.join('    ' + line.expandtabs(4) for line in lines[1:]) + '\n'


def main():
    paths = {
        'classes/objects/item.gd': ('Item', ('_init', 'get_texture')),
        'classes/resource/item_resource.gd': ('ItemResource', ()),
        'classes/static/stat.gd': ('Stat', ()),
        'classes/resource/bonus_stat.gd': ('BonusStat', ()),
        'classes/utils/stat_utils.gd': ('StatUtils', ()),
    }
    text = FIXTURES + '\n' + '\n'.join(nest(path, name, omitted) for path, (name, omitted) in paths.items())
    text += (ROOT / 'tools/probe_items.gd').read_text()
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Item oracle"\n')
    (STAGING / 'probe.gd').write_text(text.expandtabs(4))
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=60)
    print(result.stdout, end='')
    print(result.stderr[:6000], end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr:
        raise SystemExit('Item oracle failed; existing artifacts unchanged')
    content = (STAGING / 'golden.json').read_text()
    data = json.loads(content)
    assert len(data['items']) == 243
    assert len(data['rolls']) == 18
    (ROOT / 'artifacts/item-golden.json').write_text(content)
    hashes = {path: hashlib.sha256((SOURCE / path).read_bytes()).hexdigest() for path in paths}
    (ROOT / 'artifacts/item-source-hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print('Exported 243 item mutation sequences and 18 seeded reforge sequences')


if __name__ == '__main__': main()
