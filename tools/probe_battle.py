#!/usr/bin/env python3
"""Execute the unchanged recovered Battle queries/selection with fixture Enemy objects.

Fixtures: Enemy/Character shells (out_of_combat, can_battle, an Arthur's-mark
profile flag) and StatusEffects.ARTHURS_MARK identity. battle.gd, EnemyUtils.is_valid
and RNGManager.pick_random bodies are source text; no scenes/addons are loaded.
"""
import hashlib
import json
import random
import subprocess
from pathlib import Path
from probe_character_stats import methods
from probe_items import ROOT, SOURCE

STAGING = Path('/tmp/opencode/megaloot-battle-probe')


def scenarios():
    rng = random.Random(1234)
    result = []
    for _ in range(400):
        count = rng.choice([0, 1, 2, 3, 3, 3])
        slots = [None if rng.random() < 0.2 else dict(ooc=rng.random() < 0.4, can=rng.random() < 0.7, mark=rng.random() < 0.25) for _ in range(count)]
        nxt = [rng.random() < 0.3 for _ in range(3)]
        result.append(dict(slots=slots, next=nxt, initial=rng.choice([-1, 0, 1, 2, 3, 4, 7, -2]), selected=rng.choice([-1, 0, 1, 2, 3, 4, 7, -2]),
                           seed=rng.randrange(1, 10**9), remove=rng.choice([-1, 0, 1, 2]), turn=rng.randrange(0, 120), kind=rng.randrange(2)))
    return result


def build():
    battle = (SOURCE / 'classes/objects/battle.gd').read_text()
    lines = battle.splitlines()
    assert lines[0].startswith('class_name Battle extends Object')
    body = 'class Battle extends Object:\n' + '\n'.join('    ' + line.expandtabs(4) for line in lines[1:])
    utils = 'class EnemyUtils:\n' + methods('classes/utils/enemy_utils.gd', ['is_valid']).replace('    static func', '    static func')
    pick = 'class RNGManager:\n    static var gameplay_rand := RandomNumberGenerator.new()\n' + methods('singletons/rng_manager.gd', ['pick_random']).replace('    func pick_random', '    static func pick_random')
    fixtures = '''
class Character extends Object:
    pass
class FakeProfile:
    var marked := false
    func has_active_status_effect_resource(_r) -> bool:
        return marked
class Enemy extends Character:
    var out_of_combat := false
    var can := true
    var battle_profile := FakeProfile.new()
    func can_battle() -> bool:
        return can
    func cleanup(): pass
class Player extends Character:
    pass
class StatusEffects:
    static var ARTHURS_MARK = 1
'''
    return fixtures + utils + pick + body


RUNNER = '''
func label(e, enemies):
    return enemies.find(e) if is_instance_valid(e) else -1

func labels(list, enemies):
    return list.map(func(e): return label(e, enemies))

func snapshot(b, enemies):
    return {"slots": b.enemies_to_battle.map(func(e): return label(e, enemies)), "next": b.next_enemies.map(func(e): return label(e, enemies)),
            "initial": b.initial_selected_enemy_idx, "selected": b.selected_enemy_idx}

func queries(b, enemies):
    var adj := []
    for e in enemies:
        adj.push_back(labels(b.get_adjacent_enemy_characters_in_combat(e), enemies) if is_instance_valid(e) else null)
    var idxs := []
    for e in enemies:
        idxs.push_back(b.get_enemy_idx_from_character(e) if is_instance_valid(e) else null)
    var found := []
    for e in enemies:
        found.push_back(label(b.get_enemy_from_character(e), enemies) if is_instance_valid(e) else null)
    var unknown := Enemy.new()
    var unknown_adj = labels(b.get_adjacent_enemy_characters_in_combat(unknown), enemies)
    var unknown_idx = b.get_enemy_idx_from_character(unknown)
    unknown.free()
    return {"has_in_combat": b.has_enemy_in_combat(), "in_combat": labels(b.get_enemies_in_combat(), enemies),
            "in_combat_idx": Array(b.get_enemies_in_combat_idx()), "aggressive": Array(b.get_aggresive_enemies_idx()),
            "has_to_battle": b.has_enemies_to_battle(), "arthur": b.arthurs_mark_present(), "adjacent": adj, "idx": idxs, "found": found,
            "unknown_adjacent": unknown_adj, "unknown_idx": unknown_idx, "next_type": b.get_next_turn_type(), "reduction": b.get_partner_damage_reduction()}

func _init():
    var cases = JSON.parse_string(FileAccess.get_file_as_string("res://cases.json"))
    var out := []
    for c in cases:
        var b := Battle.new()
        var enemies := []
        for s in c.slots:
            var e := Enemy.new()
            if s == null:
                enemies.push_back(e)
                continue
            e.out_of_combat = s.ooc
            e.can = s.can
            e.battle_profile.marked = s.mark
            enemies.push_back(e)
        for i in c.slots.size():
            b.enemies_to_battle.push_back(enemies[i])
            if c.slots[i] == null:
                enemies[i].free()
        for i in 3:
            if c.next[i]:
                var n := Enemy.new()
                b.next_enemies[i] = n
                enemies.push_back(n)
        b.initial_selected_enemy_idx = int(c.initial)
        b.selected_enemy_idx = int(c.selected)
        b.current_turn = int(c.turn)
        b.curr_turn_type = int(c.kind)
        var rec := {"before": queries(b, enemies)}
        var first = b.get_selected_enemy()
        rec["selected1"] = [label(first, enemies), snapshot(b, enemies)]
        var second = b.get_selected_enemy()
        rec["selected2"] = [label(second, enemies), snapshot(b, enemies)]
        RNGManager.gameplay_rand.seed = int(c.seed)
        var random_enemy = b.get_random_enemy_in_combat()
        rec["random"] = [label(random_enemy, enemies), RNGManager.gameplay_rand.state]
        b.clear_invalid_enemies()
        rec["cleared"] = snapshot(b, enemies)
        rec["after_clear"] = queries(b, enemies)
        if int(c.remove) >= 0 and int(c.remove) < b.enemies_to_battle.size():
            b.remove_enemy(int(c.remove))
            rec["removed"] = snapshot(b, enemies)
        out.push_back(rec)
    var file := FileAccess.open("res://golden.json", FileAccess.WRITE)
    file.store_string(JSON.stringify(out))
    quit()
'''

if __name__ == '__main__':
    STAGING.mkdir(parents=True, exist_ok=True)
    (STAGING / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="Battle oracle"\n')
    cases = scenarios()
    (STAGING / 'cases.json').write_text(json.dumps(cases))
    (STAGING / 'probe.gd').write_text('extends SceneTree\n' + build() + RUNNER)
    result = subprocess.run(['godot', '--headless', '--path', str(STAGING), '--script', 'probe.gd'], capture_output=True, text=True, timeout=300)
    print(result.stdout[-800:], result.stderr[-2500:])
    if result.returncode or 'ERROR:' in result.stderr or 'SCRIPT ERROR' in result.stdout:
        raise SystemExit('Battle oracle failed; artifacts unchanged')
    golden = json.loads((STAGING / 'golden.json').read_text())
    (ROOT / 'artifacts/battle-golden.json').write_text(json.dumps(dict(cases=cases, results=golden), separators=(',', ':')) + '\n')
    hashes = {p: hashlib.sha256((SOURCE / p).read_bytes()).hexdigest() for p in ['classes/objects/battle.gd', 'classes/utils/enemy_utils.gd', 'singletons/rng_manager.gd']}
    (ROOT / 'artifacts/battle-source-hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print('Exported', len(golden), 'source-executed Battle scenarios')
