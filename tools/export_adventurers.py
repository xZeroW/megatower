#!/usr/bin/env python3
"""Export Adventurer resources (stats, sockets, ability, passive, item lists); never loads scenes."""
import argparse
import json
from pathlib import Path
from export_character_content import ROOT, SOURCE, ResourceFile

LIST = ['fren', 'pyry', 'sid', 'elys', 'fael', 'tyr', 'viran', 'kinno', 'riel', 'amon', 'jace']  # Adventurers.LIST
STARTING = ['fren', 'pyry', 'sid']
DEFAULT_SOCKETS = ['helmet', 'chestplate', 'leggings', 'boots', 'necklace', 'weapon', 'ring', 'ring']


def stem(ref):
    return Path(ref).stem if ref else None


def adventurer(name):
    path = SOURCE / f'resources/adventurers/{name}.tres'
    r = ResourceFile(path)
    f = r.fields
    return dict(
        id='res://' + path.relative_to(SOURCE).as_posix(),
        name=json.loads(f['name'].removeprefix('&')),
        portrait=r.ref(f.get('portrait', 'null')), blink=r.ref(f.get('blink', 'null')),
        starting_inventory_items=[r.ref(t) for t in r.array(f.get('starting_inventory_items', '[]'))],
        challenge_market_items=[r.ref(t) for t in r.array(f.get('challenge_market_items', '[]'))],
        starting_market_items=[r.ref(t) for t in r.array(f.get('starting_market_items', '[]'))],
        doubled_stat=stem(r.ref(f.get('doubled_stat', 'null'))),
        ability=r.ref(f.get('ability', 'null')),
        abilities_to_learn_size=int(f.get('abilities_to_learn_size', '1')),
        passive=r.ref(f.get('passive', 'null')),
        bonus_stats=r.bonuses(f.get('bonus_stats', '[]')),
        sockets=[stem(r.ref(t)) for t in r.array(f['sockets'])] if 'sockets' in f else DEFAULT_SOCKETS)


def export():
    all_names = sorted(p.stem for p in (SOURCE / 'resources/adventurers').glob('*.tres'))
    assert sorted(LIST) == all_names, all_names
    result = dict(list=[adventurer(n) for n in LIST], starting=[f'res://resources/adventurers/{n}.tres' for n in STARTING])
    for a in result['list']:
        assert len(a['sockets']) == 8 and a['ability'], a['name']
    return json.dumps(result, indent=1) + '\n'


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    target = ROOT / 'assets/data/content/adventurers.json'
    content = export()
    if a.check:
        if target.read_text() != content: raise SystemExit('Adventurers stale; run python3 tools/export_adventurers.py')
    else:
        target.write_text(content); print('Exported 11 adventurers')
