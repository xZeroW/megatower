#!/usr/bin/env python3
"""Export raw ItemResource definitions; does NOT apply Items._ready mutations."""
import argparse
import json
from pathlib import Path
from export_character_content import ROOT,SOURCE,ResourceFile


def export():
    rows = []
    for path in sorted((SOURCE/'resources/items').glob('*.tres')):
        resource = ResourceFile(path)
        fields = resource.fields
        def ref(key): return resource.ref(fields.get(key,'null'))
        socket = ref('socket_type')
        adapted = ref('stat_to_adapt')
        # None of the 199 recovered root items declares these effects. Do not
        # manufacture potions/consumables from stale names or old game versions.
        assert fields.get('activation_effect','null')=='null',path
        assert fields.get('consumption_effect','null')=='null',path
        row = dict(id='res://'+path.relative_to(SOURCE).as_posix(),name=json.loads(fields.get('name','&""').removeprefix('&').replace("\\'","'")),
                   texture=ref('texture'),bb_script=ref('bb_script'),socket_type=Path(socket).stem if socket else None,
                   set_resources=[resource.ref(token) for token in resource.array(fields['set_resources'])] if 'set_resources' in fields else ['res://resources/item_sets/generic.tres'],
                   consumption_effect=None,activation_effect=None,ability_to_learn=ref('ability_to_learn'),
                   stat_to_adapt=Path(adapted).stem if adapted else None,passive=ref('passive'),bonus_stats=resource.bonuses(fields.get('bonus_stats','[]')),
                   unlock_requirements=[],extra_price=float(fields.get('extra_price','0.0')),spawn_floor=int(fields.get('spawn_floor','1')),
                   buy_limit=int(fields.get('buy_limit','-1')),
                   custom_rarity=['common','uncommon','rare','legendary','mythic','eternal','abyssal','cosmic','divine'][int(fields.get('custom_rarity','0'))])
        for flag,default in [('hide_level','false'),('use_custom_rarity','false'),('reforgable','true'),('is_artifact','false'),('is_special','false')]:
            row[flag] = resource.boolean(fields.get(flag,default))
        for token in resource.array(fields.get('unlock_requirements','[]')):
            requirement = resource.sub(token)
            row['unlock_requirements'].append(dict(item_resource=resource.ref(requirement.get('item_resource','null')),completed_battles=int(requirement.get('completed_battles','0'))))
        rows.append(row)
    assert len(rows)==199
    return json.dumps(rows,indent=2)+'\n'


if __name__=='__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    args = parser.parse_args()
    target = ROOT/'artifacts/items.json'
    content = export()
    if args.check:
        if target.read_text()!=content: raise SystemExit('Raw item catalog stale; run python3 tools/export_items.py')
    else:
        target.write_text(content)
        print('Exported 199 raw recovered item definitions (before Items._ready)')
