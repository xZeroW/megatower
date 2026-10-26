#!/usr/bin/env python3
"""Execute recovered container/slot code in isolation; no scenes or addons."""
import hashlib
import json
from pathlib import Path
import subprocess
from probe_items import FIXTURES, ROOT, SOURCE, nest

STAGING = Path("/tmp/opencode/megaloot-inventory-probe")


def build():
    fixtures = FIXTURES.replace('class Items:\n', 'class Items:\n    static var LIST: Array[ItemResource] = []\n')
    fixtures = fixtures.replace('class Empty:\n', 'class Empty:\n    static var item_container: ItemContainer = null\n')
    fixtures += '''
class Character extends RefCounted:
    pass
class Memory extends RefCounted:
    pass
class SocketTypes:
    static var HELMET = SocketType.new()
    static var CHESTPLATE = SocketType.new()
    static var LEGGINGS = SocketType.new()
    static var BOOTS = SocketType.new()
    static var NECKLACE = SocketType.new()
    static var WEAPON = SocketType.new()
    static var RING = SocketType.new()
    static var REPLICATED_WEAPON = SocketType.new()
class ItemContainerResources:
    static var EQUIPMENT = ItemContainerResource.new()
    static var INVENTORY = ItemContainerResource.new()
'''
    paths = {
        'classes/objects/item.gd': ('Item', ('_init', 'get_texture')),
        'classes/resource/item_resource.gd': ('ItemResource', ()),
        'classes/static/stat.gd': ('Stat', ()),
        'classes/resource/bonus_stat.gd': ('BonusStat', ()),
        'classes/utils/stat_utils.gd': ('StatUtils', ()),
        'classes/utils/item_utils.gd': ('ItemUtils', ()),
        'classes/objects/item_container.gd': ('ItemContainer', ()),
        'classes/resource/item_container_resource.gd': ('ItemContainerResource', ()),
        'classes/objects/slot.gd': ('Slot', ()),
    }
    text = fixtures + '\n' + '\n'.join(nest(path, name, omitted) for path, (name, omitted) in paths.items())
    text += (ROOT / 'tools/probe_inventory.gd').read_text()
    return text.expandtabs(4), paths


if __name__ == '__main__':
    text, paths = build()
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Inventory oracle"\n')
    (STAGING / 'probe.gd').write_text(text)
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=60)
    print(result.stdout, end='')
    print(result.stderr[:6000], end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('Inventory oracle failed; artifacts unchanged')
    content = (STAGING / 'golden.json').read_text()
    data = json.loads(content)
    assert len(data['sequences']) == 36
    assert len(data['sorts']) == 35
    (ROOT / 'artifacts/inventory-golden.json').write_text(content)
    hashes = {path: hashlib.sha256((SOURCE / path).read_bytes()).hexdigest() for path in paths}
    (ROOT / 'artifacts/inventory-source-hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print('Exported 36 container operation sequences and 35 sorting cases')
