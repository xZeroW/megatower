#!/usr/bin/env python3
"""Export EnemyResource definitions (enemies + special_enemies); never loads scenes/scripts."""
import argparse
import json
from pathlib import Path
from export_character_content import ROOT, SOURCE, ResourceFile


def enemy(path):
    r = ResourceFile(path)
    f = r.fields
    def stem(ref): return Path(ref).stem if ref else None
    starting = []
    for token in r.array(f.get('starting_status_effects', '[]')):
        if token == 'null':
            starting.append(None); continue
        group = r.sub(token)
        effects = []
        for t in r.array(group.get('status_effects', '[]')):
            b = r.sub(t)
            effects.append(dict(resource=stem(r.ref(b.get('resource', 'null'))), amount=int(b.get('amount', '0'))))
        starting.append(effects)
    levels = []
    for token in r.array(f.get('stats_per_level', '[]')):
        # Source would dereference null.bonus_stats; recovered data has none.
        assert token != 'null', path
        levels.append(r.bonuses(r.sub(token).get('bonus_stats', '[]')))
    return dict(
        id='res://' + path.relative_to(SOURCE).as_posix(),
        name=json.loads(f.get('name', '""')),
        texture=r.ref(f.get('texture', 'null')),
        floor_number=int(f.get('floor_number', '0')),
        flying=r.boolean(f.get('flying', 'false')),
        starting_status_effects=starting,
        active_item_sets=[r.ref(t) for t in r.array(f.get('active_item_sets', '[]'))],
        stats_per_level=levels,
        base_stats=r.bonuses(f.get('base_stats', '[]')),
        abilities=[r.ref(t) for t in r.array(f.get('abilities', '[]'))],
        passives=[r.ref(t) for t in r.array(f.get('passives', '[]'))],
        is_unique=r.boolean(f.get('is_unique', 'false')),
        hide_stats=r.boolean(f.get('hide_stats', 'false')))


def export():
    result = {}
    for group in ('enemies', 'special_enemies'):
        result[group] = [enemy(p) for p in sorted((SOURCE / 'resources' / group).glob('*.tres'))]
    assert (len(result['enemies']), len(result['special_enemies'])) == (57, 7)
    return json.dumps(result, indent=1) + '\n'


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    target = ROOT / 'assets/data/content/enemies.json'
    content = export()
    if a.check:
        if target.read_text() != content: raise SystemExit('Enemies stale; run python3 tools/export_enemies.py')
    else:
        target.write_text(content); print('Exported 57 enemies + 7 special')
