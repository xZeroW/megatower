#!/usr/bin/env python3
"""Execute character attack math/gates with recovered bodies and Godot RNG.

Uses explicit cached-value inputs, real status policies and fixture identities;
no target damage resolution, combat callbacks, scenes or turn execution.
"""
import hashlib
import json
import subprocess
from pathlib import Path
from probe_character_vitals import build
from probe_character_stats import methods
from probe_items import ROOT,SOURCE,nest

STAGING=Path('/tmp/opencode/megaloot-character-attack-probe')


def cases():
    result=[]
    for attack in range(4):
        for scenario in range(8):
            for history in range(4):
                for override in range(3):
                    values=dict(physical_attack=0.0,magic_attack=0.0,freeze_attack=0.0,armored_attack=0.0,max_health=101.0,
                        active_armor=20.0,armor=10.0,malice=2.0,elemental_power=12.5,adaptive_attack=1.25,
                        power=[0,25,-25,-100,-150,100,0,33.3][scenario],combat=2.25,wisdom=3.5,
                        physical_damage=12.5,magic_damage=-25.0,freeze_damage=50.0,electricity=[0,0.1,8,-4,0,12,0,1.5][scenario],
                        toxicity=[0,2,3.5,0,-2,0,1,0][scenario],dazzle=scenario%3-1,armor_shields=scenario%3-1,
                        crit_chance=[-5,0,50.9,100,150,25,100,50][scenario],agility=[-1,0,25,100,-1,100,25,100][scenario])
                    values[['physical_attack','magic_attack','freeze_attack','armored_attack'][attack]]=[5.5,5.5,5.25,25.0][attack]
                    consumed={0:[],1:[['vimblow',2],['omni_blitz',1]],2:[['omni_blitz',1],['vimblow',2],['magic_blitz',2],['multi_attack_charge',0]],
                        3:[['magic_blitz',2],None,['omni_blitz',-1]],4:[None],5:[['vimblow',0]],6:[],7:[['omni_blitz',0.5],['vimblow',1]]}[scenario]
                    active={0:[],1:[],2:[['fury',2]],3:[['weakness',1],['enervation',1]],
                        4:[['weakness',1.5],['fury',1.5],['magic_shield',0.4],['combat_insight',4.5],['combat_insight',1]],
                        5:[['weakness',4],['enervation',2],['combat_insight',6]],
                        6:[['armor_shield',0],['malice_shield',0],['arthurs_mark',0],['timeout',0],['elusive',0]],
                        7:[['weakness',-1],['enervation',-2],['magic_shield',1],['fury',-1],['combat_insight',6]]}[scenario]
                    abilities={1:['enchanted_attack'],2:['toxic_attack','stun_attack','headbutt','confusion_strike'],3:['enchanted_attack'],
                        6:['toxic_attack','stun_strike','confusion_strike','repulse'],7:['repulse','stun_strike']}.get(scenario,[])
                    sets=['zephyron','arcanum'] if scenario%2 else []
                    if scenario in [1,4,7]:sets.append('scout')
                    result.append(dict(values=values,consumed=consumed,active=active,abilities=abilities,sets=sets,
                        history=history,override=override,omni=scenario in [3,7],ironclad=scenario%2==1,stun=1 if scenario==5 else 0,seed=42))
    # Selection ties use identical inputs across type candidates.
    for values in [[5,5,5,25],[0,5,5,25],[0,0,5,25],[0,0,0,0],[-2,-2,-2,-10]]:
        case=json.loads(json.dumps(result[0]))
        case['values'].update(zip(['physical_attack','magic_attack','freeze_attack','armored_attack'],values))
        case['values']['elemental_power']=0
        result.append(case)
    return result


if __name__=='__main__':
    names=['get_attack_type','get_attack_damage_type','get_status_effects_on_hit','get_adjacent_status_effects_on_hit','get_starting_status_effects',
        'get_crit_chance','get_turn_size','has_magic_shield','get_physical_attack','get_magic_attack','get_freeze_attack','get_armored_attack',
        'get_attack_damage','get_attack_damage_data','get_physical_damage','get_magic_damage','get_freeze_damage','apply_damage_output_boosters',
        'try_to_crit','try_to_dodge','try_to_hit','try_to_block','try_to_parry','try_to_multi_attack','try_to_reduce_damage']
    text,catalog,policies=build(names)
    text=text.replace('class Math:\n','class Math:\n    enum Operation {ROUND_NEAREST,ROUND_DOWN,ROUND_UP}\n'+methods('classes/utils/math.gd',['rand_success']))
    text=text.replace('class Empty:\n','class Empty:\n    static var stat_resource=StatResource.new()\n')
    text=text.replace('class BattleTurn extends RefCounted:\n','class BattleTurn extends RefCounted:\n    var attack_type: StatResource=Empty.stat_resource\n    var consumed_stacks: Array[StatusEffect]=[]\n'+methods('classes/objects/battle_turn.gd',['get_consumed_stacks']))
    text=text.replace('class BattleProfile extends RefCounted:\n','class BattleProfile extends RefCounted:\n    var stun_turns_left: int=0\n'+methods('classes/objects/battle_profile.gd',['is_stunned','has_timeout','is_marked_by_arthur','consume_stack']))
    text=text.replace('class ItemSets:\n','class ItemSets:\n'+ '\n'.join('    static var '+name.upper()+'=ItemSetResource.new()' for name in ['zephyron','scout','arcanum'])+'\n')
    text=text.replace('class Passives:\n','class Passives:\n    static var IRONCLAD=Passive.new()\n')
    ability_names=['enchanted_attack','toxic_attack','stun_attack','stun_strike','headbutt','confusion_strike','repulse']
    text=text.replace('class Abilities:\n','class Abilities:\n'+'\n'.join('    static var '+name.upper()+'=AbilityResource.new()' for name in ability_names)+'\n')
    # DamageData keeps its actual constructor/multiplier, without uncalled
    # boost callbacks. Referenced Character type is the fixture above.
    source=(SOURCE/'classes/objects/damage_data.gd').read_text().split('\nfunc _init')[0]
    source=source.replace('class_name DamageData extends Object','class DamageData extends Object:')
    lines=source.splitlines()
    text+='\n'+lines[0]+'\n'+'\n'.join('    '+line.expandtabs(4) for line in lines[1:])+'\n'+methods('classes/objects/damage_data.gd',['_init','apply_multiplier'])
    text+='\nclass RNGManager:\n    static var gameplay_rand=RandomNumberGenerator.new()\n'
    text+='\n'+(ROOT/'tools/probe_character_attack.gd').read_text()
    text=text.replace('# ASSIGN_STATS','\n'.join(f'    Stats.{row["id"].upper()}=stats["{row["id"]}"]' for row in catalog['definitions']))
    text=text.replace('# ASSIGN_STATUSES','\n'.join('    statuses["'+row['id']+'"] = StatusEffects.'+row['id'].upper() for row in policies))
    text=text.replace('# ABILITY_LOOKUP','    var abilities={'+','.join('"'+name+'":Abilities.'+name.upper() for name in ability_names)+'}')
    text=text.replace('# SET_LOOKUP','    var sets={'+','.join('"'+name+'":ItemSets.'+name.upper() for name in ['zephyron','arcanum','scout'])+'}')
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Character attack oracle"\n')
    (STAGING/'probe.gd').write_text(text.expandtabs(4))
    inputs=cases()
    (STAGING/'input.json').write_text(json.dumps(dict(catalog=catalog,policies=policies,cases=inputs)))
    result=subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='');print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr:raise SystemExit('Attack oracle failed; artifacts unchanged')
    outcomes=json.loads((STAGING/'golden.json').read_text())
    assert len(outcomes)==len(inputs)==389
    for case,outcome in zip(inputs,outcomes):case['expected']=outcome
    (ROOT/'artifacts/character-attack-golden.json').write_text(json.dumps(inputs,separators=(',',':'))+'\n')
    deps=['classes/objects/character.gd','classes/objects/battle_profile.gd','classes/objects/battle_turn.gd','classes/objects/status_effect.gd','classes/objects/damage_data.gd','classes/utils/stat_utils.gd','classes/utils/math.gd','singletons/status_effects.gd']
    deps+=['classes/resource/stat_resource.gd','classes/resource/status_effect_resource.gd']
    deps+=[path.relative_to(SOURCE).as_posix() for group in ['stats','status_effects'] for path in sorted((SOURCE/'resources'/group).glob('*.tres'))]
    (ROOT/'artifacts/character-attack-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print('Exported 389 attack math/gate cases with per-operation RNG state and shield/parry mutation views')
