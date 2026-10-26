extends SceneTree

class StatResource extends Resource:
    var max_amount: int = -1
    var ignore_minimum_amount: bool = false
    var minimum_amount: float = 0
class Stat extends RefCounted:
    var resource: StatResource
    var base_amount: float
    var modifier_amount: float
    var negative_amount: float
class BonusStat extends RefCounted:
    var resource: StatResource
    var is_modifier: bool
    var amount: float
class Specialization extends Resource:
    pass
class Passive extends Resource:
    pass
class ItemSetResource extends Resource:
    var game_logic_script: GDScript
class GameLogicScript extends Object:
    var character: Character
    func get_bonus_stats() -> Array[BonusStat]: return character.script_bonuses
class FixtureLogic extends GameLogicScript:
    pass
class BattleTurn extends RefCounted:
    var triple_attack: bool = false
    func is_used_ability(_resource) -> bool: return triple_attack
class BattleProfile extends RefCounted:
    var statuses: Array = []
    var turn = BattleTurn.new()
    func get_curr_turn() -> BattleTurn: return turn
    func has_active_status_effect_resource(resource) -> bool: return statuses.has(resource)
    func is_slowed() -> bool: return statuses.has(StatusEffects.SLOWNESS)
class Abilities:
    static var TRIPLE_ATTACK = Resource.new()
class Stats:
# STAT_IDS
    static var LIST: Array[StatResource] = []
class ItemSets:
# SET_IDS
class Passives:
# PASSIVE_IDS
class Specializations:
# SPEC_IDS
class StatusEffects:
# STATUS_IDS
class StatUtils:
# STAT_MODIFY
class Math:
# PERCENTAGE
class Character extends RefCounted:
    var cached_stats: Dictionary = {}
    var battle_profile = BattleProfile.new()
    var doubled_stat: StatResource
    var adapted_stats: Array = []
    var transformed_stats: Array = []
    var floor_number: int = 0
    var sets: Array[ItemSetResource] = []
    var passives: Array[Passive] = []
    var specializations: Array[Specialization] = []
    var script_bonuses: Array[BonusStat] = []
    func get_active_item_sets() -> Array[ItemSetResource]: return sets
    func get_passives() -> Array[Passive]: return passives
    func get_active_specializations() -> Array[Specialization]: return specializations
# CHARACTER_METHODS

func _initialize() -> void:
    var input = JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    var stats = {}
    for row in input.catalog.definitions:
        var resource = StatResource.new()
        resource.max_amount = row.max_amount
        resource.ignore_minimum_amount = row.ignore_minimum_amount
        resource.minimum_amount = row.minimum_amount
        stats[row.id] = resource
# ASSIGN_STATS
    var resources = {}
# ASSIGN_RESOURCES
# STATUS_LOOKUP
    var results: Array = []
    for case in input.cases:
        Stats.LIST.clear()
        var character = Character.new()
        var context = case.context
        for id in case.order:
            Stats.LIST.push_back(stats[id])
            var channels = case.channels[id]
            character.cached_stats[stats[id]] = {"base_amount":0.0,"modifier_amount":0.0,"negative_amount":0.0,"final_amount":0.0}
            var stat = Stat.new()
            stat.resource = stats[id]
            stat.base_amount = channels.base_amount
            stat.modifier_amount = channels.modifier_amount
            stat.negative_amount = channels.negative_amount
            character.cache_stat(stat)
        for id in context.item_sets: character.sets.push_back(resources[id])
        for id in context.passives: character.passives.push_back(resources[id])
        for id in context.specializations: character.specializations.push_back(resources[id])
        for id in context.status_effects: character.battle_profile.statuses.push_back(statuses[id])
        for id in context.adapted_stats: character.adapted_stats.push_back(stats[id])
        for id in context.transformed_stats: character.transformed_stats.push_back(stats[id])
        if context.doubled_stat != null: character.doubled_stat = stats[context.doubled_stat]
        character.floor_number = context.floor_number
        character.battle_profile.turn.triple_attack = context.triple_attack_used
        for row in case.script_bonuses:
            var bonus = BonusStat.new()
            bonus.resource = stats[row.resource]
            bonus.amount = row.amount
            bonus.is_modifier = row.is_modifier
            character.script_bonuses.push_back(bonus)
        # Synthetic script hook only: method body still executes actual
        # game_logic_script.new/get_bonus_stats/free calls.
        var script_set = ItemSetResource.new()
        script_set.game_logic_script = FixtureLogic
        character.sets.push_back(script_set)
        for _repeat in int(case.repeats): character.recalculate_cached_stats()
        var result = {}
        for id in case.order: result[id] = character.cached_stats[stats[id]]
        results.push_back(result)
    var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify(results)+"\n")
    file.close()
    quit()
