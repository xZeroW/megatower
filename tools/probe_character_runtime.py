#!/usr/bin/env python3
"""Execute recovered cache/stat-mutation bodies and Berserker with empty items.

Stat metadata is recovered. Empty item/manager shells are explicit fixtures;
equipment aggregation and real content loading have separate source oracles.
"""
import hashlib
import json
import subprocess
from pathlib import Path
from probe_items import ROOT,SOURCE,nest
from probe_character_stats import methods,SETS,PASSIVES,SPECS,STATUSES
from export_stats import export

STAGING=Path('/tmp/opencode/megaloot-character-runtime-probe')

def build(extra_methods=()):
    catalog=json.loads(export())
    text=(ROOT/'tools/probe_character_runtime.gd').read_text()
    for marker,names in [('# STAT_IDS',[row['id'] for row in catalog['definitions']]),('# SET_IDS',SETS+['berserker','generic']),('# PASSIVE_IDS',PASSIVES),('# SPEC_IDS',SPECS),('# STATUS_IDS',STATUSES)]:
        kind={'# STAT_IDS':'StatResource','# SET_IDS':'ItemSetResource','# PASSIVE_IDS':'Passive','# SPEC_IDS':'Specialization'}.get(marker,'Resource')
        text=text.replace(marker,'\n'.join(f'    static var {name.upper()} = {kind}.new()' for name in names))
    text=text.replace('# ASSIGN_STATS','\n'.join(f'    Stats.{row["id"].upper()} = stats["{row["id"]}"]' for row in catalog['definitions']))
    char_methods=['get_item_set_count','get_equipment_items','get_stats_from_items','get_active_item_sets','get_passives','get_all_stat_arr','cache_stat','cache_stats','recalculate_cached_stats','get_stat_amount','get_health','get_max_health','get_health_percent','change_stat_amount','set_stat_amount','add_stat']
    char_methods+=list(extra_methods)
    for marker,path,names in [('# CHARACTER_METHODS','classes/objects/character.gd',char_methods),('# BERSERKER','resources/item_set_scripts/berserker.gd',['get_bonus_stats']),('# STAT_UTILS','classes/utils/stat_utils.gd',['modify','try_to_add_stat','set_stat_amount','change_stat_amount']),('# PERCENTAGE','classes/utils/math.gd',['get_percentage']),('# BONUS_INIT','classes/resource/bonus_stat.gd',['_init'])]:
        text=text.replace(marker,methods(path,names))
    text=text.replace('# STAT_CLASS',nest('classes/static/stat.gd','Stat'))
    # Lifetime bookkeeping only; free Object Stat leftovers after all snapshots.
    text=text.replace('class Stat extends Object:\n','class Stat extends Object:\n    static var _probe_instances: Array = []\n')
    text=text.replace('    func _init(args: Array = []) -> void :\n','    func _init(args: Array = []) -> void :\n        _probe_instances.push_back(self)\n')
    return text,catalog

if __name__=='__main__':
    text,catalog=build()
    cases=[dict(health=health,mode=mode,variant=variant) for health in [24.4,24.5,25,49.5,50,74.5,75,100] for mode in range(4) for variant in range(2)]
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Character stat runtime oracle"\n')
    (STAGING/'probe.gd').write_text(text.expandtabs(4))
    (STAGING/'input.json').write_text(json.dumps(dict(catalog=catalog,cases=cases)))
    result=subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='');print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr:raise SystemExit('Character runtime oracle failed; artifacts unchanged')
    data=json.loads((STAGING/'golden.json').read_text())
    assert len(data)==64
    (ROOT/'artifacts/character-runtime-golden.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
    deps=['classes/objects/character.gd','classes/static/stat.gd','classes/resource/bonus_stat.gd','classes/utils/stat_utils.gd','classes/utils/math.gd','classes/nodes/item_set_scripts.gd']
    deps += [path.relative_to(SOURCE).as_posix() for path in sorted((SOURCE/'resources/item_set_scripts').glob('*.gd'))]
    deps += [path.relative_to(SOURCE).as_posix() for path in sorted((SOURCE/'resources/stats').glob('*.tres'))]
    (ROOT/'artifacts/character-runtime-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print('Exported 64 character mutation sequences / 896 snapshots and synchronous signal views')
