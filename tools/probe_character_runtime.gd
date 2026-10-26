extends SceneTree

class StatResource extends Resource:
    var name: String
    var max_amount: int = -1
    var ignore_minimum_amount: bool = false
    var minimum_amount: float = 0
class BonusStat extends RefCounted:
    var resource: StatResource
    var amount: float = 1
    var is_modifier: bool = false
    var boosting_sets: Array[ItemSetResource] = []
# BONUS_INIT
# STAT_CLASS
class Stats:
# STAT_IDS
    static var LIST: Array[StatResource] = []
    static func get_amount_from_rarity(amount: float,rarity: int):return amount*(rarity+1)
class StatUtils:
# STAT_UTILS
class Math:
# PERCENTAGE
class BonusStatsArray extends Resource:
    var bonus_stats: Array[BonusStat] = []
class ItemSetResource extends Resource:
    var bonus_stats: Array[BonusStatsArray] = []
    var game_logic_script: GDScript
class ItemResource extends Resource:
    var is_special: bool = false
    var bonus_stats: Array[BonusStat] = []
class Item extends RefCounted:
    var resource: ItemResource
    var reforged_passive: Passive
    func get_bonus_stats() -> Array[BonusStat]:return []
    func get_set_resources() -> Array[ItemSetResource]:return []
    func has_burnout():return false
class ItemContainer extends RefCounted:
    func get_items() -> Array[Item]:return []
    func get_active_item_sets() -> Array[ItemSetResource]:return []
class ItemUtils:
    static func get_item_set_count(_items,_set):return 0
class ItemSets:
# SET_IDS
class Passive extends Resource:
    var bonus_stats: Array[BonusStat] = []
class Passives:
# PASSIVE_IDS
class Specialization extends Resource:
    pass
class Specializations:
# SPEC_IDS
class AbilityResource extends Resource:
    var bonus_stats_while_active: Array[BonusStat] = []
class Ability extends RefCounted:
    var resource: AbilityResource
class Abilities:
    static var TRIPLE_ATTACK = AbilityResource.new()
class StatusEffects:
# STATUS_IDS
class BattleTurn extends RefCounted:
    var stats: Array[Stat] = []
    var abilities: Array[Ability] = []
    func is_used_ability(resource):return abilities.any(func(ability):return is_instance_valid(ability) and ability.resource==resource)
class BattleProfile extends RefCounted:
    var stats: Array[Stat] = []
    var external_stats: Array[Stat] = []
    var turn = BattleTurn.new()
    var active_item_sets: Array[ItemSetResource] = []
    var statuses: Array = []
    func get_curr_turn() -> BattleTurn:return turn
    func has_active_status_effect_resource(resource):return statuses.has(resource)
    func is_slowed():return statuses.has(StatusEffects.SLOWNESS)
class Empty:
    static var passive = Passive.new()
class GameLogicScript extends Object:
    var character: Character
    func get_bonus_stats() -> Array[BonusStat]:return []
class Berserker extends GameLogicScript:
# BERSERKER
class Character extends RefCounted:
    signal stat_changed(resource: StatResource,old_amount: float)
    var inventory = ItemContainer.new()
    var equipment = ItemContainer.new()
    var stats: Array[Stat] = []
    var equipment_stats: Array[Stat] = []
    var cached_stats: Dictionary = {}
    var base_passive: Passive
    var battle_profile = BattleProfile.new()
    var transformed_stats: Array[StatResource] = []
    var adapted_stats: Array[StatResource] = []
    var doubled_stat: StatResource
    var floor_number: int = 0
    var stat_change_process_requested: bool = false
    func get_active_specializations() -> Array[Specialization]:return []
# CHARACTER_METHODS

var stats: Dictionary = {}
var stat_ids: Dictionary = {}

func stat_array(values: Array[Stat]):
    return values.map(func(value):return [stat_ids[value.resource],value.base_amount,value.modifier_amount,value.negative_amount])
func snapshot(character: Character):
    var cache = {}
    for id in stats:
        var value = character.cached_stats[stats[id]]
        cache[id] = [float(value.final_amount),float(value.base_amount),float(value.modifier_amount),float(value.negative_amount)]
    return {"base":stat_array(character.stats),"external":stat_array(character.battle_profile.external_stats),"battle":stat_array(character.battle_profile.stats),
        "turn":stat_array(character.battle_profile.turn.stats),"cache":cache,"requested":character.stat_change_process_requested}

func run_case(case):
    var character = Character.new()
    character.stats.assign([Stat.new([Stats.MAX_HEALTH,100.0]),Stat.new([Stats.HEALTH,case.health]),Stat.new([Stats.ARMOR,6.0]),
        Stat.new([Stats.ACTIVE_ARMOR,4.0]),Stat.new([Stats.MANA,1.0]),Stat.new([Stats.MAX_MANA,5.0]),Stat.new([Stats.ACCURACY,100.0]),Stat.new([Stats.TOTAL_ATTACKS,1.0])])
    match int(case.mode):
        1:character.battle_profile.active_item_sets.assign([ItemSets.BERSERKER])
        2:character.battle_profile.active_item_sets.assign([ItemSets.BERSERKER,ItemSets.BERSERKER])
        3:character.battle_profile.active_item_sets.assign([ItemSets.BERSERKER,ItemSets.GENERIC,ItemSets.SWIFTNESS])
    if case.variant==1:
        character.base_passive = Passives.MANAFLOW
        character.doubled_stat = Stats.MAX_HEALTH
        var ability = Ability.new()
        ability.resource=Abilities.TRIPLE_ATTACK
        character.battle_profile.turn.abilities.push_back(ability)
    var events: Array = []
    character.stat_changed.connect(func(resource,old_amount):events.push_back({"resource":stat_ids[resource],"old":old_amount,"snapshot":snapshot(character)}))
    var steps: Array = []
    var record = func(op):
        steps.push_back({"op":op,"snapshot":snapshot(character),"events":events.duplicate(true)})
        events.clear()
    character.cache_stats()
    record.call("rebuild")
    character.set_stat_amount(Stats.HEALTH,25)
    record.call("set_health")
    character.cache_stats()
    record.call("rebuild")
    character.add_stat(character.battle_profile.stats,Stat.new([Stats.ARMOR,5]))
    record.call("add_armor")
    character.change_stat_amount(character.stats,Stat.new([Stats.ARMOR,-100]))
    record.call("remove_armor")
    character.add_stat(character.stats,Stat.new([Stats.MAX_HEALTH,10]))
    record.call("add_max_health")
    character.change_stat_amount(character.battle_profile.turn.stats,Stat.new([Stats.HEALTH,-500]))
    record.call("remove_health")
    character.add_stat(character.battle_profile.external_stats,Stat.new([Stats.MAX_MANA,2]))
    record.call("add_max_mana")
    character.change_stat_amount(character.battle_profile.turn.stats,Stat.new([Stats.TOTAL_ATTACKS,1]))
    record.call("change_attacks")
    character.set_stat_amount(Stats.MAX_HEALTH,50)
    record.call("set_max_health")
    character.change_stat_amount(character.battle_profile.stats,Stat.new([Stats.ACCURACY,-10]))
    record.call("change_accuracy")
    character.add_stat(character.stats,Stat.new([Stats.ARMOR,-2]))
    record.call("subtract_armor")
    character.cache_stats()
    record.call("rebuild")
    # Repeated setters show stale cache and unchanged notification flag semantics.
    character.set_stat_amount(Stats.MANA,99)
    record.call("set_mana")
    for connection in character.stat_changed.get_connections():character.stat_changed.disconnect(connection.callable)
    return {"health":case.health,"mode":case.mode,"variant":case.variant,"steps":steps}

func _initialize():
    var input=JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    for row in input.catalog.definitions:
        var resource=StatResource.new()
        resource.name=row.name
        resource.max_amount=row.max_amount
        resource.minimum_amount=row.minimum_amount
        resource.ignore_minimum_amount=row.ignore_minimum_amount
        stats[row.id]=resource
        stat_ids[resource]=row.id
        Stats.LIST.push_back(resource)
# ASSIGN_STATS
    ItemSets.BERSERKER=ItemSetResource.new()
    ItemSets.BERSERKER.game_logic_script=Berserker
    ItemSets.GENERIC=ItemSetResource.new()
    ItemSets.SWIFTNESS=ItemSetResource.new()
    ItemSets.SWIFTNESS.game_logic_script=GameLogicScript
    Passives.MANAFLOW=Passive.new()
    var result: Array = []
    for case in input.cases:result.push_back(run_case(case))
    var file=FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify(result)+"\n")
    file.close()
    for stat in Stat._probe_instances:
        if is_instance_valid(stat):stat.free()
    Stat._probe_instances.clear()
    ItemSets.BERSERKER.game_logic_script=null
    ItemSets.SWIFTNESS.game_logic_script=null
    quit()
