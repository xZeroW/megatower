#!/usr/bin/env python3
"""ResourceLoader-check raw items and execute Items startup on explicit orders.

Uses resource-only staging, inert BB/logic scripts and synthetic overlap fixtures
in separate cases. Never loads scenes or starts the real game.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
import probe_character_content as base
from export_character_content import ROOT,SOURCE,ResourceFile,sections
from probe_character_stats import methods

STAGING = Path('/tmp/opencode/megaloot-item-catalog-probe')
SCHEMAS = ['classes/resource/item_resource.gd','classes/resource/socket_type.gd',
           'classes/resource/activation_effect.gd','classes/resource/consumption_effect.gd',
           'classes/resource/item_unlock_requirement.gd']

if __name__=='__main__':
    base.STAGING = STAGING
    deps = base.stage()
    base.write('project.godot','config_version=5\n[application]\nconfig/name="Item catalog oracle"\n')
    bonus_path = 'classes/resource/bonus_stat.gd'
    # This data schema already comes from the base probe. Add the unchanged
    # equality method to check its null-removal mutation on real null entries.
    # methods() is class-indented; top-level methods need one less indent.
    bonus = (SOURCE/bonus_path).read_text().replace('@tool\n','').split('\nfunc ')[0].replace('Array[ItemSetResource]','Array[Resource]')+'\n'+'\n'.join(line[4:] if line.startswith('    ') else line for line in methods(bonus_path,['is_same_as']).splitlines())+'\n'
    base.write(bonus_path,bonus)
    for script in SCHEMAS:
        source = (SOURCE/script).read_text().replace('@tool\n','')
        if script.endswith('item_resource.gd'): source = source.split('\nfunc get_translated_name')[0]
        elif script.endswith('activation_effect.gd'): source = source.split('\nfunc ')[0]
        elif script.endswith('consumption_effect.gd'): source = source.split('\nfunc get_bb_container_data')[0]
        elif script.endswith('item_unlock_requirement.gd'): source = source.replace('item_resource: ItemResource','item_resource: Resource')
        base.write(script,source)
    rarity_path = 'classes/item_related/item_rarity.gd'
    rarity = (SOURCE/rarity_path).read_text()
    enum = re.search(r'enum Type\{.*?\}',rarity,re.S)[0]
    base.write(rarity_path,'class_name ItemRarity extends RefCounted\n'+enum+'\n')
    item_paths = []
    for path in sorted((SOURCE/'resources/items').glob('*.tres')):
        item_paths.append('res://'+path.relative_to(SOURCE).as_posix())
        resource = ResourceFile(path)
        output = ['[gd_resource type="Resource" format=3]\n']
        for kind,header,body in sections(path.read_text()):
            header = re.sub(r' uid="[^"]+"','',header)
            if kind=='ext_resource':
                if 'type="Texture2D"' in header: continue
                if 'type="Script"' in header:
                    script = re.search(r'path="res://([^"]+)"',header)[1]
                    if script not in SCHEMAS and script not in base.SCHEMAS: base.write(script,'extends RefCounted\n')
            else:
                body = re.sub(r'^texture = .*$','texture = null',body,flags=re.M)
                body = re.sub(r'^icon = .*$','icon = null',body,flags=re.M)
                if kind=='resource': body+='\nmetadata/probe_texture = '+json.dumps(resource.ref(resource.fields.get('texture','null')))+'\n'
            output.append('['+kind+header+']\n'+body+'\n')
        base.write(path.relative_to(SOURCE),''.join(output))
    for path in (SOURCE/'resources/socket_types').glob('*.tres'):
        fields = ResourceFile(path).fields
        base.write(path.relative_to(SOURCE),'[gd_resource type="Resource" format=3]\n[ext_resource type="Script" path="res://classes/resource/socket_type.gd" id="1"]\n[resource]\nscript = ExtResource("1")\nname = '+fields.get('name','&""')+'\n')
    singleton = (SOURCE/'singletons/items.gd').read_text().split('\nfunc _ready')[0]
    singleton = 'class Items extends Node:\n'+'\n'.join('    '+line for line in singleton.splitlines()[1:])+'\n'
    singleton += methods('singletons/items.gd',['_ready','sort_item_resources','get_most_expensive_item_price','get_resources'])
    probe = (ROOT/'tools/probe_item_catalog.gd').read_text().replace('# ITEMS_SOURCE',singleton)
    base.write('probe.gd',probe)
    base.write('input.json',json.dumps(item_paths))
    editor = subprocess.run(['godot','--headless','--editor','--path',str(STAGING),'--quit'],capture_output=True,text=True,timeout=60)
    editor.check_returncode()
    if 'SCRIPT ERROR:' in editor.stderr: raise SystemExit(editor.stderr[:6000])
    cases=[]
    for synthetic in [False,True]:
        for reverse in [False,True]:
            for demo in [False,True]:
                result = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd','--',str(int(synthetic)),str(int(reverse)),str(int(demo))],capture_output=True,text=True,timeout=60)
                result.check_returncode()
                if 'ERROR:' in result.stderr: raise SystemExit(result.stderr[:6000])
                case = json.loads((STAGING/'golden.json').read_text())
                case.update(synthetic=synthetic,reverse=reverse,demo=demo)
                cases.append(case)
    assert len(cases[0]['raw'])==199
    (ROOT/'artifacts/item-catalog-golden.json').write_text(json.dumps(cases,separators=(',',':'))+'\n')
    deps += [path.removeprefix('res://') for path in item_paths]+SCHEMAS+base.SCHEMAS+[rarity_path,'singletons/items.gd']
    (ROOT/'artifacts/item-catalog-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print('Exported 199 raw ItemResource records and eight startup/order/demo cases (four synthetic)')
