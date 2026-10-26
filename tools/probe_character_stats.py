#!/usr/bin/env python3
"""Probe the unchanged character recalculation body, with resolved dependency fixtures.

No character startup, cache_stats aggregation, item scripts or scenes are executed.
All 58 stat policies are real exported resource data. Three explicit Stats.LIST
orders expose order dependence, without claiming the game's filesystem order.
"""
import hashlib
import json
import random
import re
import subprocess
from pathlib import Path
from probe_items import ROOT, SOURCE
from export_stats import export

STAGING = Path('/tmp/opencode/megaloot-character-stat-probe')
SETS = ['demonic','poverty','hunter','magma','cataclysm','swiftness']
PASSIVES = ['freedom_drive','manaflow','liberation','empty_bastion']
SPECS = ['chromaspite','deserter']
STATUSES = ['invulnerability','blindness','confusion','slowness','exhaustion']


def methods(path, names):
    source = (SOURCE / path).read_text()
    result = ''
    for name in names:
        match = re.search(r'^(?:static )?func ' + name + r'\(.*?(?=^(?:static )?func |\Z)',source,re.M|re.S)
        assert match, (path,name)
        result += '\n'.join('    '+line.expandtabs(4) for line in match[0].splitlines())+'\n'
    return result


def build_cases(definitions):
    ids = [row['id'] for row in definitions]
    shuffled = ids.copy()
    random.Random(41).shuffle(shuffled)
    cases = []
    rng = random.Random(71)
    # Each named branch alone, all branches together, then mixed combinations.
    modes = [dict()] + [{group:[name]} for group,names in [('item_sets',SETS),('passives',PASSIVES),('specializations',SPECS),('status_effects',STATUSES)] for name in names]
    modes += [dict(item_sets=SETS,passives=PASSIVES,specializations=SPECS,status_effects=STATUSES)]
    modes += [dict(item_sets=[name for name in SETS if rng.randrange(2)],passives=[name for name in PASSIVES if rng.randrange(2)],
                   specializations=[name for name in SPECS if rng.randrange(2)],status_effects=[name for name in STATUSES if rng.randrange(2)]) for _ in range(24)]
    for order in [ids,list(reversed(ids)),shuffled]:
        # Same cases in every order, so order dependence is observable rather
        # than confounded by different numeric inputs.
        rng = random.Random(718)
        for mode_index,mode in enumerate(modes):
            for variant in range(6):
                channels = {id:dict(base_amount=rng.choice([-5.5,0,0.5,1,3.25,27.5,125.5]),modifier_amount=rng.choice([-150,-25.5,0,12.5,101]),negative_amount=rng.choice([0,0.5,2.5,9])) for id in ids}
                channels['max_health']['base_amount'] = [100,100,100,101,500,1][variant]
                channels['max_health']['modifier_amount'] = 0
                channels['max_health']['negative_amount'] = 0
                channels['health']['base_amount'] = [23.9,24.9,25,26,120,0.5][variant]
                channels['health']['modifier_amount'] = 0
                channels['health']['negative_amount'] = 0
                channels['crit_chance']['base_amount'] = [99,100,101,175.5,300,-5][variant]
                context = dict(item_sets=[],passives=[],specializations=[],status_effects=[],adapted_stats=[],transformed_stats=[],doubled_stat=None,floor_number=[0,8,9,19,99,-1][variant],triple_attack_used=variant%2==1)
                context.update(mode)
                if variant in [2,3]: context['adapted_stats'] = ['physical_attack','armor','crit_chance','max_health','agility']
                if variant == 4: context['adapted_stats'] = ids
                if variant > 0: context['transformed_stats'] = ['eldership']
                context['doubled_stat'] = [None,'max_health','elemental_power','crit_chance','physical_attack','armor'][variant]
                bonuses = [dict(resource='max_health',amount=-10.5,is_modifier=False),dict(resource='physical_attack',amount=13.5,is_modifier=True),dict(resource='elemental_power',amount=45,is_modifier=False)] if variant%2 else []
                for group in ['item_sets','passives','specializations']:
                    context[group] = [f'res://resources/{group}/{name}.tres' for name in context[group]]
                cases.append(dict(order=order,channels=channels,context=context,script_bonuses=bonuses,repeats=2 if variant==5 else 1))
    return cases


if __name__ == '__main__':
    catalog = json.loads(export())
    cases = build_cases(catalog['definitions'])
    fixtures = (ROOT/'tools/probe_character_stats.gd').read_text()
    # Insert extracted methods into the corresponding fixture classes.
    for marker,path,names in [('# STAT_MODIFY','classes/utils/stat_utils.gd',['modify']),
                              ('# PERCENTAGE','classes/utils/math.gd',['get_percentage']),
                              ('# CHARACTER_METHODS','classes/objects/character.gd',['cache_stat','recalculate_cached_stats','get_stat_amount','get_health','get_max_health','get_health_percent'])]:
        fixtures = fixtures.replace(marker,methods(path,names).rstrip())
    # Source identities, with no texture/script loads or singleton startup.
    ids = [row['id'] for row in catalog['definitions']]
    for marker,names in [('# STAT_IDS',ids),('# SET_IDS',SETS),('# PASSIVE_IDS',PASSIVES),('# SPEC_IDS',SPECS),('# STATUS_IDS',STATUSES)]:
        fixtures = fixtures.replace(marker,'\n'.join(f'    static var {name.upper()} = Resource.new()' for name in names))
    fixtures = fixtures.replace('# ASSIGN_STATS','\n'.join(f'    Stats.{id.upper()} = stats["{id}"]' for id in ids))
    setup = []
    for singleton,group,kind,names in [('ItemSets','item_sets','ItemSetResource',SETS),('Passives','passives','Passive',PASSIVES),('Specializations','specializations','Specialization',SPECS)]:
        for name in names:
            setup += [f'    {singleton}.{name.upper()} = {kind}.new()',f'    resources["res://resources/{group}/{name}.tres"] = {singleton}.{name.upper()}']
    fixtures = fixtures.replace('# ASSIGN_RESOURCES','\n'.join(setup))
    fixtures = fixtures.replace('# STATUS_LOOKUP','    var statuses = {' + ','.join(f'"{name}":StatusEffects.{name.upper()}' for name in STATUSES) + '}')
    STAGING.mkdir(parents=True,exist_ok=True)
    (STAGING/'project.godot').write_text('config_version=5\n[application]\nconfig/name="Character stat oracle"\n')
    (STAGING/'probe.gd').write_text(fixtures.expandtabs(4))
    (STAGING/'input.json').write_text(json.dumps(dict(catalog=catalog,cases=cases)))
    result = subprocess.run(['godot','--headless','--path',str(STAGING),'--script','probe.gd'],capture_output=True,text=True,timeout=60)
    print(result.stdout,end='')
    print(result.stderr[:6000],end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('Character oracle failed; artifacts unchanged')
    results = json.loads((STAGING/'golden.json').read_text())
    assert len(results) == len(cases)
    for case,result in zip(cases,results): case['expected'] = result
    (ROOT/'artifacts/character-stats-golden.json').write_text(json.dumps(cases,separators=(',',':'))+'\n')
    deps = ['classes/objects/character.gd','classes/utils/stat_utils.gd','classes/utils/math.gd','classes/resource/stat_resource.gd',
            *[path.relative_to(SOURCE).as_posix() for path in sorted((SOURCE/'resources/stats').glob('*.tres'))]]
    (ROOT/'artifacts/character-stats-source-hashes.json').write_text(json.dumps({path:hashlib.sha256((SOURCE/path).read_bytes()).hexdigest() for path in deps},indent=2)+'\n')
    print(f'Exported {len(cases)} character recalculation cases across 58 stats and three explicit orders')
