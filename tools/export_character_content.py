#!/usr/bin/env python3
"""Export declared character-stat content only; never loads scenes/gameplay scripts."""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'Megaloot-GDRE-Recovered'
GROUPS = ['item_sets','passives','abilities','specializations']


def sections(text):
    return [(match[1],match[2],match[3]) for match in re.finditer(
        r'^\[(ext_resource|sub_resource|resource)([^\n]*)\]\n(.*?)(?=^\[|\Z)',text,re.M|re.S)]


class ResourceFile:
    def __init__(self,path):
        self.path = path
        self.refs = {}
        self.subs = {}
        for kind,header,body in sections(path.read_text()):
            fields = dict(re.findall(r'^(\w+) = (.+)$',body,re.M))
            if kind == 'ext_resource':
                self.refs[re.search(r'\bid="([^"]+)"',header)[1]] = re.search(r'path="([^"]+)"',header)[1]
            elif kind == 'sub_resource':
                self.subs[re.search(r'\bid="([^"]+)"',header)[1]] = fields
            else:
                self.fields = fields

    def ref(self,value):
        if value == 'null': return None
        embedded = re.fullmatch(r'SubResource\("([^"]+)"\)',value)
        if embedded:
            assert embedded[1] in self.subs, (self.path,value)
            return 'res://'+self.path.relative_to(SOURCE).as_posix()+'::'+embedded[1]
        match = re.fullmatch(r'ExtResource\("([^"]+)"\)',value)
        assert match, (self.path,value)
        return self.refs[match[1]]

    def array(self,value):
        if value.startswith('Array['):
            match = re.fullmatch(r'Array\[.*\]\((\[.*\])\)',value)
            assert match, (self.path,value)
            value = match[1]
        assert value.startswith('[') and value.endswith(']'), (self.path,value)
        tokens = re.findall(r'(?:ExtResource|SubResource)\("[^"]+"\)|null',value)
        assert re.sub(r'(?:ExtResource|SubResource)\("[^"]+"\)|null|[\[\],\s]','',value) == '', (self.path,value)
        return tokens

    def sub(self,value):
        match = re.fullmatch(r'SubResource\("([^"]+)"\)',value)
        assert match, (self.path,value)
        return self.subs[match[1]]

    def bonuses(self,value):
        result = []
        for token in self.array(value):
            fields = self.sub(token)
            stat = self.ref(fields['resource'])
            assert stat.startswith('res://resources/stats/') and stat.endswith('.tres'), stat
            result.append(dict(resource=Path(stat).stem,amount=float(fields.get('amount','1.0')),
                               is_modifier=self.boolean(fields.get('is_modifier','false')),
                               boosting_sets=[self.ref(token) for token in self.array(fields.get('boosting_sets','[]'))]))
        return result

    @staticmethod
    def boolean(value):
        assert value in ('true','false'), value
        return value == 'true'


def export():
    result = {}
    for group in GROUPS:
        rows = []
        for path in sorted((SOURCE/'resources'/group).glob('*.tres')):
            resource = ResourceFile(path)
            fields = resource.fields
            row = dict(id='res://'+path.relative_to(SOURCE).as_posix(),name=json.loads(fields.get('name','""')))
            if group == 'specializations':
                row.update(original_item_set=resource.ref(fields.get('original_item_set','null')),
                           synergy_item_set=resource.ref(fields.get('synergy_item_set','null')),
                           color=[float(value) for value in fields.get('color','Color(0, 0, 0, 1)')[6:-1].split(',')])
            else:
                row.update(game_logic_script=resource.ref(fields.get('ability_script' if group=='abilities' else 'game_logic_script','null')),
                           bb_script=resource.ref(fields.get('bb_script','null')))
                if group in ('item_sets','abilities'): row['icon'] = resource.ref(fields.get('icon','null'))
                if group == 'item_sets':
                    row['color'] = [float(value) for value in fields.get('color','Color(0, 0, 0, 1)')[6:-1].split(',')]
                    row['bonus_stats'] = [None if token=='null' else resource.bonuses(resource.sub(token).get('bonus_stats','[]')) for token in resource.array(fields.get('bonus_stats','[]'))]
                else:
                    row['bonus_stats'] = resource.bonuses(fields.get('bonus_stats_while_active' if group=='abilities' else 'bonus_stats','[]'))
                if group == 'abilities':
                    row.update(mana_cost=int(fields.get('mana_cost','1')),use_limit=int(fields.get('use_limit','0')),
                               quick_cast=resource.boolean(fields.get('quick_cast','false')),non_target=resource.boolean(fields.get('non_target','true')),
                               to_attack=resource.boolean(fields.get('to_attack','false')))
                    row['status_effects_on_activation'] = []
                    for token in resource.array(fields.get('status_effects_on_activation','[]')):
                        bonus = resource.sub(token)
                        target = resource.ref(bonus.get('resource','null'))
                        row['status_effects_on_activation'].append(dict(resource=Path(target).stem if target else None,amount=int(bonus.get('amount','0'))))
            rows.append(row)
        result[group] = rows
    assert {key:len(value) for key,value in result.items()} == dict(item_sets=38,passives=63,abilities=45,specializations=31)
    return json.dumps(result,indent=2)+'\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    args = parser.parse_args()
    content = export()
    path = ROOT/'assets/data/content/character-content.json'
    if args.check:
        if path.read_text() != content: raise SystemExit('Character content stale; run python3 tools/export_character_content.py')
    else:
        path.write_text(content)
        print('Exported 38 item sets, 63 passives, 45 abilities and 31 specializations')
