#!/usr/bin/env python3
"""Export BattleAction popup/color data (16 resources, no scenes)."""
import argparse
import json
from pathlib import Path
from export_character_content import ROOT, SOURCE, ResourceFile


def color(value, default='Color(0, 0, 0, 1)'):
    return [float(v) for v in (value or default)[6:-1].split(',')]


def export():
    rows = []
    for path in sorted((SOURCE / 'resources/battle_actions').glob('*.tres')):
        r = ResourceFile(path)
        f = r.fields
        rows.append(dict(id=path.stem, name=json.loads(f['name']), icon=r.ref(f.get('icon', 'null')), color=color(f.get('color')),
                         use_limit=int(f.get('use_limit', '0')), popup_color=color(f.get('popup_color')),
                         popup_text=json.loads(f.get('popup_text', '""')), popup_delay=float(f.get('popup_delay', '0.0')),
                         screen_shake=r.boolean(f.get('screen_shake', 'true'))))
    assert len(rows) == 16
    return json.dumps(rows, indent=1) + '\n'


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    target = ROOT / 'artifacts/battle-actions.json'
    content = export()
    if a.check:
        if target.read_text() != content: raise SystemExit('Battle actions stale; run python3 tools/export_battle_actions.py')
    else:
        target.write_text(content); print('Exported 16 battle actions')
