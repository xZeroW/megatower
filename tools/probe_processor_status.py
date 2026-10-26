#!/usr/bin/env python3
"""Source processor methods with synchronous logging dependencies, no scenes.

Character/cache/profile/status bodies are recovered. Manager/attack/temp-stat/UI
boundaries only record requests. Synthetic specialization identities are explicit.
"""
import hashlib
import json
import subprocess
from pathlib import Path
from probe_character_vitals import build
from probe_character_stats import methods
from probe_items import ROOT, SOURCE

STAGING = Path('/tmp/opencode/megaloot-processor-status-probe')
SETS = ['unchained', 'thunder', 'chromalure']


def cases(policies):
    result = []
    for row in policies:
        for mode in range(10):
            result.append(dict(resource=row['id'], mode=mode, amount=[1.9, -1.2, 0.9, 5, 2, 1, 2, 3, 1, 1][mode], seed=42))
    # Focused branches, including sticky success and original-target reads.
    for resource in ['stun', 'confusion', 'silence', 'poison', 'ephemeral_armor']:
        for mode in range(10, 29):
            result.append(dict(resource=resource, mode=mode, amount=2.5, seed=42))
    result += [dict(resource=None, mode=0, amount=2, seed=42)]
    return result


if __name__ == '__main__':
    text, catalog, policies = build()
    text = text.replace('class Math:\n', 'class Math:\n'+methods('classes/utils/math.gd', ['rand_success']))
    text = text.replace('class StatusEffectResource extends Resource:\n', 'class StatusEffectResource extends Resource:\n    var affected_by_tenacity: bool=false\n    var color: Color\n')
    text = text.replace('class BattleTurn extends RefCounted:\n', 'class BattleTurn extends RefCounted:\n    var consumed_stacks: Array[StatusEffect]=[]\n')
    text = text.replace('class BattleProfile extends RefCounted:\n', 'class BattleProfile extends RefCounted:\n    var stun_turns_left: int=0\n'+methods('classes/objects/battle_profile.gd', ['consume_stack']))
    text = text.replace('class ItemSets:\n', 'class ItemSets:\n'+''.join(f'    static var {name.upper()}=ItemSetResource.new()\n' for name in SETS))
    text = text.replace('class Specializations:\n', 'class Specializations:\n    static var THUNDERBORN=Specialization.new()\n    static var MINDBREAKER=Specialization.new()\n')
    text = text.replace('class Character extends RefCounted:\n', '''class Character extends RefCounted:
    signal recieved_status_effect(effect: StatusEffect)
    var probe_specializations: Array[Specialization]=[]
''')
    text = text.replace('func get_active_specializations() -> Array[Specialization]:return []', 'func get_active_specializations() -> Array[Specialization]:return probe_specializations')
    # Actual DamageData fields/constructor. Resolution is deliberately a log.
    source = (SOURCE/'classes/objects/damage_data.gd').read_text().split('\nfunc _init')[0].splitlines()
    text += '\nclass DamageData extends Object:\n'+'\n'.join('    '+line.expandtabs(4) for line in source[1:])+'\n'+methods('classes/objects/damage_data.gd', ['_init'])
    text += '\nclass Await:\n'+methods('classes/utils/await.gd', ['emit'])
    text += '\n'+(ROOT/'tools/probe_processor_status.gd').read_text()
    text = text.replace('# PROCESSOR_METHODS', methods('classes/nodes/battle_processor.gd', ['try_to_apply_status_effect', 'process_applied_status_effect']))
    text = text.replace('# ASSIGN_STATS', '\n'.join(f'    Stats.{row["id"].upper()}=stats["{row["id"]}"]' for row in catalog['definitions']))
    text = text.replace('# ASSIGN_STATUSES', '\n'.join(f'    statuses["{row["id"]}"]=StatusEffects.{row["id"].upper()}' for row in policies))
    inputs = cases(policies)
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Processor status oracle"\n')
    (STAGING/'probe.gd').write_text(text.expandtabs(4))
    (STAGING/'input.json').write_text(json.dumps(dict(catalog=catalog, policies=policies, cases=inputs)))
    run = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'], capture_output=True, text=True, timeout=60)
    print(run.stdout, end=''); print(run.stderr[:6000], end='')
    run.check_returncode()
    if 'ERROR:' in run.stderr: raise SystemExit('Processor oracle failed; artifacts unchanged')
    outcomes = json.loads((STAGING/'golden.json').read_text())
    assert len(outcomes) == len(inputs)
    for case, expected in zip(inputs, outcomes): case['expected'] = expected
    (ROOT/'artifacts/processor-status-golden.json').write_text(json.dumps(inputs, separators=(',',':'))+'\n')
    deps = ['classes/nodes/battle_processor.gd','classes/objects/character.gd','classes/objects/battle_profile.gd','classes/objects/battle_turn.gd','classes/objects/status_effect.gd','classes/objects/damage_data.gd','classes/utils/await.gd','classes/utils/math.gd','classes/utils/stat_utils.gd','classes/static/stat.gd','classes/resource/bonus_stat.gd','classes/resource/status_effect_resource.gd','singletons/status_effects.gd']
    deps += [path.relative_to(SOURCE).as_posix() for group in ['stats','status_effects'] for path in sorted((SOURCE/'resources'/group).glob('*.tres'))]
    (ROOT/'artifacts/processor-status-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps}, indent=2)+'\n')
    print(f'Exported {len(inputs)} processor status cases with ordered boundary snapshots and RNG states')
