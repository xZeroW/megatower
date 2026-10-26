#!/usr/bin/env python3
"""Probe selected real ItemManager methods on the inventory dependency fixtures."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
from probe_inventory import build
from probe_items import ROOT, SOURCE, nest

STAGING = Path('/tmp/opencode/megaloot-inventory-interaction-probe')

if __name__ == '__main__':
    text, paths = build()
    text = text.split('\nfunc _initialize()')[0]
    text = text.replace('class Math:\n    static func rand_success(chance: float) -> bool: return randf() * 100.0 < chance\n', nest('classes/utils/math.gd','Math'))
    paths['classes/utils/math.gd'] = ('Math', ())
    text = text.replace('class Empty:\n', 'class Empty:\n    static var slot: Slot = Slot.new()\n')
    text = text.replace('class Stats:\n', 'class Stats:\n    static var TRANSFORMED_STATS: Array[StatResource] = []\n')
    text = text.replace('    static var INVENTORY = ItemContainerResource.new()\n', '''    static var INVENTORY = ItemContainerResource.new()
    static var LIBRARY = ItemContainerResource.new()
    static var LOOT_STASH = ItemContainerResource.new()
    static var MARKET = ItemContainerResource.new()
    static var MERCHANT = ItemContainerResource.new()
''')
    text += '\n' + nest('classes/objects/swap_result.gd','ItemPressResult')
    source = (SOURCE / 'singletons/item_manager.gd').read_text()
    text += '\nclass ItemManager extends RefCounted:\n    signal item_created(item: Item)\n'
    for name in ['can_swap','can_merge','matches_socket','get_equip_slot','sort_by_rarity','get_rand_weight','create_item','create_insightus','get_rand_item_from_gold']:
        body = re.search(r'^func ' + name + r'\(.*?(?=^func |\Z)',source,re.M|re.S)
        assert body, name
        text += '\n'.join('    ' + line.expandtabs(4) for line in body[0].splitlines()) + '\n'
    text += (ROOT / 'tools/probe_inventory_interactions.gd').read_text()
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Inventory interactions"\n')
    (STAGING / 'probe.gd').write_text(text.expandtabs(4))
    try:
        result = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    except subprocess.TimeoutExpired as error:
        print((error.stderr or b'')[:6000].decode(errors='replace'))
        raise SystemExit('ItemManager probe timed out; artifacts unchanged')
    print(result.stdout,end='')
    print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('ItemManager probe failed; artifacts unchanged')
    content = (STAGING / 'golden.json').read_text()
    data = json.loads(content)
    assert len(data['pairs']) == 200
    assert len(data['creations']) == 72
    assert len(data['gold']) == 5
    (ROOT / 'artifacts/inventory-interactions-golden.json').write_text(content)
    hashes = {path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in [*paths,'classes/objects/swap_result.gd','singletons/item_manager.gd']}
    (ROOT / 'artifacts/inventory-interactions-source-hashes.json').write_text(json.dumps(hashes,indent=2)+'\n')
    print('Exported 200 slot-gate cases, 72 item creations, gold and Insightus sequences')
