#!/usr/bin/env python3
"""Execute recovered turn/profile/status methods with explicit dependency fixtures.

No scenes, addons, character executor or real Ability instances are loaded.
Status battle-policy metadata comes from all 50 recovered .tres definitions.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from probe_items import ROOT, SOURCE, nest
from export_status_effects import export

STAGING = Path('/tmp/opencode/megaloot-battle-profile-probe')
FIXTURES = '''extends SceneTree
class StatusEffectType extends Resource:
    pass
class StatusEffectTypes:
    static var BUFF = StatusEffectType.new()
    static var DEBUFF = StatusEffectType.new()
class StatusEffectResource extends Resource:
    var name: String = ""
    var type: StatusEffectType = StatusEffectTypes.DEBUFF
    var is_temporary: bool = false
    var limit: int = 1
class AbilityResource extends Resource:
    var name: String
class Ability extends RefCounted:
    var resource: AbilityResource
class StatResource extends Resource:
    pass
class Stat extends Object:
    pass
class HitData extends Object:
    pass
class BattleAction extends Resource:
    pass
class ItemSetResource extends Resource:
    pass
class ItemContainer extends Object:
    func _init(_resource = null, _size = 0): pass
    func cleanup(): pass
class ItemContainerResources:
    static var EQUIPMENT = Resource.new()
class Empty:
    static var stat_resource = StatResource.new()
class StatusEffects:
    static var TIMEOUT = StatusEffectResource.new()
    static var STUN = StatusEffectResource.new()
    static var CONFUSION = StatusEffectResource.new()
    static var BLACK_CURSE = StatusEffectResource.new()
    static var CURSE = StatusEffectResource.new()
    static var SILENCE = StatusEffectResource.new()
    static var SLOWNESS = StatusEffectResource.new()
    static var ARTHURS_MARK = StatusEffectResource.new()
'''


if __name__ == '__main__':
    paths = {'classes/objects/battle_profile.gd': 'BattleProfile',
             'classes/objects/battle_turn.gd': 'BattleTurn',
             'classes/objects/status_effect.gd': 'StatusEffect'}
    source = (SOURCE / 'singletons/status_effects.gd').read_text()
    method = re.search(r'^func has_active_status_effect_resource\(.*?(?=^func |\Z)', source, re.M | re.S)[0]
    text = FIXTURES + '\n'.join('    ' + line.expandtabs(4).replace('func has_active', 'static func has_active') for line in method.splitlines()) + '\n'
    text += '\n'.join(nest(path, name) for path, name in paths.items())
    text += '\nvar policy_rows = JSON.parse_string(' + json.dumps(export()) + ')\n'
    text += (ROOT / 'tools/probe_battle_profile.gd').read_text()
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Battle profile oracle"\n')
    (STAGING / 'probe.gd').write_text(text.expandtabs(4))
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=60)
    print(result.stdout, end='')
    print(result.stderr[:6000], end='')
    result.check_returncode()
    if 'ERROR:' in result.stderr: raise SystemExit('Battle oracle failed; artifacts unchanged')
    content = (STAGING / 'golden.json').read_text()
    data = json.loads(content)
    assert len(data['sequences']) == 180
    assert len(data['policies']) == 350
    diagnostics = {}
    for mode in ['random_buff', 'used_abilities']:
        broken = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd', '--', mode], capture_output=True, text=True, timeout=15)
        broken.check_returncode()
        assert 'ERROR:' in broken.stderr, (mode, broken.stdout, broken.stderr)
        diagnostics[mode] = broken.stderr
    (ROOT / 'artifacts/battle-profile-source-errors.json').write_text(json.dumps(diagnostics, indent=2) + '\n')
    (ROOT / 'artifacts/battle-profile-golden.json').write_text(content)
    deps = [*paths, 'singletons/status_effects.gd', 'classes/resource/status_effect_resource.gd', *[
        path.relative_to(SOURCE).as_posix() for path in sorted((SOURCE / 'resources/status_effects').glob('*.tres'))]]
    (ROOT / 'artifacts/battle-profile-source-hashes.json').write_text(json.dumps({path: hashlib.sha256((SOURCE / path).read_bytes()).hexdigest() for path in deps}, indent=2) + '\n')
    print('Exported 180 battle profile sequences, 350 policy cases and two source-error diagnostics')
