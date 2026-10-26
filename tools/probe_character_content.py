#!/usr/bin/env python3
"""Validate exported data with Godot ResourceLoader and execute recovered selectors.

Stages resource-only copies with original data schemas, inert gameplay/BB scripts,
and no textures/audio/scenes/addons. Original resources are never changed.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from export_character_content import ROOT,SOURCE,GROUPS,ResourceFile,sections
from probe_character_stats import methods

STAGING = Path('/tmp/opencode/megaloot-character-content-probe')
SCHEMAS = ['classes/item_related/item_set_resource.gd','classes/resource/passive.gd',
           'classes/resource/specialization.gd','classes/resource/ability_resource.gd',
           'classes/resource/bonus_stats_array.gd','classes/resource/bonus_stat.gd',
           'classes/resource/bonus_status_effect.gd','classes/resource/stat_resource.gd']


def write(path,text):
    target = STAGING/path
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(text)


def stage():
    paths = []
    for group in GROUPS:
        for path in sorted((SOURCE/'resources'/group).glob('*.tres')):
            paths.append(path.relative_to(SOURCE).as_posix())
            resource = ResourceFile(path)
            output = ['[gd_resource type="Resource" format=3]\n']
            for kind,header,body in sections(path.read_text()):
                header = re.sub(r' uid="[^"]+"','',header)
                if kind == 'ext_resource':
                    if 'type="Texture2D"' in header or 'type="AudioStream' in header: continue
                    if 'type="Script"' in header:
                        script = re.search(r'path="res://([^"]+)"',header)[1]
                        if script not in SCHEMAS: write(script,'extends RefCounted\n')
                else:
                    # Keep undeclared old fields: Godot itself must ignore them.
                    body = re.sub(r'^icon = .*$', 'icon = null',body,flags=re.M)
                    if kind=='resource':
                        body += '\nmetadata/probe_icon = '+json.dumps(resource.ref(resource.fields.get('icon','null')))+'\n' if resource.fields.get('icon','null')!='null' else ''
                output.append('['+kind+header+']\n'+body+'\n')
            write(path.relative_to(SOURCE),''.join(output))
    # Stat/status reference identities are real paths. Their rendering/logic
    # fields are irrelevant to this data/selector probe and are not loaded.
    for group,script in [('stats','classes/resource/stat_resource.gd'),('status_effects','classes/resource/status_effect_resource.gd')]:
        for path in sorted((SOURCE/'resources'/group).glob('*.tres')):
            fields = ResourceFile(path).fields
            write(path.relative_to(SOURCE),'[gd_resource type="Resource" format=3]\n[ext_resource type="Script" path="res://'+script+'" id="1"]\n[resource]\nscript = ExtResource("1")\nname = '+fields.get('name','""')+'\n')
    for path in SCHEMAS:
        source = (SOURCE/path).read_text().replace('@tool\n','')
        if path.endswith('bonus_stat.gd'):
            source = source.split('\nfunc ')[0]
            # Break a Godot 4.4 cyclic script-type reference at probe shutdown.
            # All stored values are still actual ItemSetResource instances.
            source = source.replace('Array[ItemSetResource]','Array[Resource]')
        write(path,source)
    write('classes/resource/status_effect_resource.gd','class_name StatusEffectResource extends Resource\n@export var name: String\n')
    return paths


if __name__=='__main__':
    paths = stage()
    write('project.godot','config_version=5\n[application]\nconfig/name="Character content oracle"\n')
    specs = [path for path in paths if '/specializations/' in path]
    sets = ['res://'+path for path in paths if '/item_sets/' in path]
    selections = [dict(order=['res://'+path for path in order],active_sets=active)
                  for order in [specs,list(reversed(specs))]
                  for active in [[],sets,sets+sets,*[[a,b] for a in sets for b in sets]]]
    gates = [dict(id='res://'+path,uses=uses,free=free,mana=mana,silenced=silenced)
             for path in paths if '/abilities/' in path for uses in [0,1,3] for free in [-1,0,1]
             for mana in [0,1,3,99] for silenced in [False,True]]
    probe = (ROOT/'tools/probe_character_content.gd').read_text()
    for marker,path,names in [('# SELECTOR','classes/objects/character.gd',['get_active_specializations','can_use_ability']),
                              ('# GROUPING','singletons/specializations.gd',['_ready'])]:
        probe = probe.replace(marker,methods(path,names))
    write('probe.gd',probe)
    write('input.json',json.dumps(dict(paths=['res://'+path for path in paths],selections=selections,gates=gates)))
    editor = subprocess.run(['godot','--headless','--editor','--path',str(STAGING),'--quit'],capture_output=True,text=True,timeout=60)
    editor.check_returncode()
    if 'SCRIPT ERROR:' in editor.stderr: raise SystemExit(editor.stderr[:6000])
    result = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='');print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('Content oracle failed; artifacts unchanged')
    data = json.loads((STAGING/'golden.json').read_text())
    assert len(data['records'])==len(paths)
    assert len(data['selections'])==len(selections)
    assert len(data['gates'])==len(gates)
    for case,expected in zip(selections,data.pop('selections')): case['expected']=expected
    for case,expected in zip(gates,data.pop('gates')): case['expected']=expected
    data.update(selections=selections,gates=gates)
    (ROOT/'artifacts/character-content-golden.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
    deps = [*paths,*SCHEMAS,'classes/objects/character.gd','singletons/specializations.gd']
    (ROOT/'artifacts/character-content-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print(f'Exported {len(paths)} ResourceLoader records, {len(selections)} specialization selections and {len(gates)} ability gates')
