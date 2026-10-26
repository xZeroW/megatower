#!/usr/bin/env python3
"""Execute recovered healing/vital mutations and character status application.

Reuses the empty-equipment character/cache fixture; stat and status policies are
real recovered metadata. No scenes, processor gates or gameplay manager execution.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from probe_character_runtime import build as runtime_build
from probe_items import ROOT,SOURCE,nest
from probe_character_stats import methods
from export_status_effects import export

STAGING=Path('/tmp/opencode/megaloot-character-vitals-probe')

def build(extra_methods=()):
    extra=['get_mana','get_max_mana','get_missing_health','get_missing_mana','get_missing_armor','get_guard_reduction','is_overhealed','is_full_health',
           'set_health','set_active_armor','reset_health','reset_active_armor','change_health','change_active_armor','change_mana','heal','try_to_add_status_effect','set_status_effect_amount']
    text,catalog=runtime_build(extra+list(extra_methods))
    text=text.split('\nvar stats:')[0]
    policies=json.loads(export())
    text=text.replace('class BattleTurn extends RefCounted:\n','''class BattleTurn extends RefCounted:
    var stopped_attacking: bool = false
    var received_status_effects: Array[StatusEffect] = []
    var applied_status_effects: Array[StatusEffect] = []
    var health_recovered: float = 0
    var percent_health_recovered: float = 0
''')
    profile='''class BattleProfile extends RefCounted:
    var stats: Array[Stat] = []
    var external_stats: Array[Stat] = []
    var turns: Array[BattleTurn] = [BattleTurn.new()]
    var active_item_sets: Array[ItemSetResource] = []
    var active_status_effects: Array[StatusEffect] = []
    var removed_status_effects: Array[StatusEffectResource] = []
'''+methods('classes/objects/battle_profile.gd',['get_valid_turns','get_curr_turn','is_slowed','is_cursed','has_active_status_effect_resource','get_active_status_effects','remove_matching_status_effects','get_status_effect_amount'])
    text=re.sub(r'^class BattleProfile .*?(?=^class Empty)',profile,text,flags=re.M|re.S)
    status='''class StatusEffectType extends Resource:
    pass
class StatusEffectResource extends Resource:
    var name: String = ""
    var limit: int = 1
    var type: StatusEffectType
class StatusEffects:
'''
    status+='\n'.join('    static var '+row['id'].upper()+' = StatusEffectResource.new()' for row in policies)+'\n'
    status+=methods('singletons/status_effects.gd',['has_active_status_effect_resource']).replace('    func ','    static func ',1)
    status+=nest('classes/objects/status_effect.gd','StatusEffect')
    text=re.sub(r'^class StatusEffects:.*?(?=^class BattleTurn)',status,text,flags=re.M|re.S)
    # Additional set/passive identities referenced by the actual helper bodies.
    text=text.replace('class ItemSets:\n','class ItemSets:\n    static var FLESH = ItemSetResource.new()\n')
    return text,catalog,policies

if __name__=='__main__':
    text,catalog,policies=build()
    text+='\n'+(ROOT/'tools/probe_character_vitals.gd').read_text()
    text=text.replace('# ASSIGN_STATS','\n'.join(f'    Stats.{row["id"].upper()} = stats["{row["id"]}"]' for row in catalog['definitions']))
    text=text.replace('# ASSIGN_STATUSES','\n'.join('    statuses["'+row['id']+'"] = StatusEffects.'+row['id'].upper() for row in policies))
    vital_cases=[dict(health=health,flags=flags,variant=variant) for health in [0,99.5,100,150] for flags in range(8) for variant in range(2)]
    status_cases=[dict(resource=row['id'],mode=mode) for row in policies for mode in range(4)]
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Character vitals/status oracle"\n')
    (STAGING/'probe.gd').write_text(text.expandtabs(4))
    (STAGING/'input.json').write_text(json.dumps(dict(catalog=catalog,policies=policies,vitals=vital_cases,statuses=status_cases)))
    result=subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='');print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr:raise SystemExit('Vitals/status oracle failed; artifacts unchanged')
    data=json.loads((STAGING/'golden.json').read_text())
    assert len(data['vitals'])==64 and len(data['statuses'])==200
    (ROOT/'artifacts/character-vitals-golden.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
    deps=['classes/objects/character.gd','classes/objects/battle_profile.gd','classes/objects/status_effect.gd','singletons/status_effects.gd','classes/static/stat.gd','classes/resource/bonus_stat.gd','classes/utils/stat_utils.gd','classes/utils/math.gd']
    deps += [path.relative_to(SOURCE).as_posix() for group in ['stats','status_effects','item_set_scripts'] for path in sorted((SOURCE/'resources'/group).glob('*')) if path.suffix in ['.gd','.tres']]
    (ROOT/'artifacts/character-vitals-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print('Exported 64 vital/healing sequences and 200 status-application sequences')
