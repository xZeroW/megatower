#!/usr/bin/env python3
"""Export battle-policy fields from recovered status resources, without scenes."""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'Megaloot-GDRE-Recovered'


def export():
    rows = []
    for path in sorted((SOURCE / 'resources/status_effects').glob('*.tres')):
        text = path.read_text()
        refs = dict(re.findall(r'path="([^"]+)" id="([^"]+)"', text))
        refs = {value: key for key, value in refs.items()}
        fields = dict(re.findall(r'^(\w+) = (.+)$', text.split('\n[resource]\n')[1], re.M))
        kind = 'debuff'
        if 'type' in fields:
            ref = re.fullmatch(r'ExtResource\("([^"]+)"\)', fields['type'])
            assert ref, path
            kind = Path(refs[ref[1]]).stem
        assert kind in ('buff', 'debuff', 'penalty', 'neutral', 'blessing', 'rule'), (path, kind)
        def ref(key):
            match = re.fullmatch(r'ExtResource\("([^"]+)"\)', fields.get(key, 'null'))
            return refs[match[1]] if match else None
        color = [float(v) for v in fields.get('color', 'Color(0, 0, 0, 1)')[6:-1].split(',')]
        rows.append(dict(id=path.stem, name=json.loads(fields['name']), kind=kind,
                         color=color, icon=ref('icon'), application_message=json.loads(fields.get('application_message', '""')),
                         application_sound=ref('application_sound'), is_percent=fields.get('is_percent', 'false') == 'true',
                         affected_by_tenacity=fields.get('affected_by_tenacity','false')=='true',
                         is_temporary=fields.get('is_temporary', 'false') == 'true',
                         limit=int(fields.get('limit', '1'))))
    assert len(rows) == 50
    return json.dumps(rows, indent=2) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    target = ROOT / 'assets/data/content/status-effect-policies.json'
    content = export()
    if args.check:
        if target.read_text() != content:
            raise SystemExit('Status policy catalog stale; run python3 tools/export_status_effects.py')
    else:
        target.write_text(content)
        print('Exported 50 status-effect battle policies')
