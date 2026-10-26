extends SceneTree

class File:
    static var order: Array = []
    static func get_file_paths(_path): return order
class Specializations:
    static var LIST = {}
class Loader extends RefCounted:
    const DIR = "res://resources/specializations/"
    class SpecializationArray:
        var arr: Array[Specialization] = []
        func _init(arg_arr: Array[Specialization]): arr = arg_arr
    var LIST: Dictionary[ItemSetResource, SpecializationArray] = {}
# GROUPING
class StatusEffects:
    static var SILENCE = Resource.new()
class BattleProfile extends RefCounted:
    var uses: int = 0
    var free_ability_uses: int = 0
    var silenced: bool = false
    func has_active_status_effect_resource(_resource): return silenced
    func get_used_ability_count(_resource): return uses
class Character extends RefCounted:
    enum UseAbilityResult {SUCCESS,FAIL,MANA}
    var sets: Array[ItemSetResource] = []
    var battle_profile = BattleProfile.new()
    var mana: int = 0
    func get_mana(): return mana
    func get_active_item_sets() -> Array[ItemSetResource]: return sets
# SELECTOR

func path(resource):
    return resource.resource_path if is_instance_valid(resource) else null

func color(value: Color): return [value.r,value.g,value.b,value.a]

func bonuses(values):
    return values.map(func(value):return {"resource":value.resource.resource_path.get_file().trim_suffix(".tres"),"amount":value.amount,"is_modifier":value.is_modifier,"boosting_sets":value.boosting_sets.map(path)})

func _initialize() -> void:
    var input = JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    var records = {}
    for source in input.paths:
        var resource = load(source)
        var row = {"id":source,"name":resource.name}
        if resource is Specialization:
            row.merge({"color":color(resource.color),"original_item_set":path(resource.original_item_set),"synergy_item_set":path(resource.synergy_item_set)})
        else:
            row.merge({"game_logic_script":path(resource.ability_script if resource is AbilityResource else resource.game_logic_script),"bb_script":path(resource.bb_script)})
            if resource is ItemSetResource or resource is AbilityResource: row.icon = resource.get_meta("probe_icon") if resource.has_meta("probe_icon") else null
            if resource is ItemSetResource:
                row.color = color(resource.color)
                row.bonus_stats = resource.bonus_stats.map(func(tier):return bonuses(tier.bonus_stats) if is_instance_valid(tier) else null)
            else:
                row.bonus_stats = bonuses(resource.bonus_stats_while_active if resource is AbilityResource else resource.bonus_stats)
            if resource is AbilityResource:
                row.merge({"mana_cost":resource.mana_cost,"use_limit":resource.use_limit,"quick_cast":resource.quick_cast,"non_target":resource.non_target,"to_attack":resource.to_attack,
                    "status_effects_on_activation":resource.status_effects_on_activation.map(func(value):return {"resource":value.resource.resource_path.get_file().trim_suffix(".tres") if is_instance_valid(value.resource) else null,"amount":value.amount})})
        records[source] = row
    var selections: Array = []
    for case in input.selections:
        File.order = case.order.map(func(value):return value.get_file())
        var loader = Loader.new()
        loader._ready()
        Specializations.LIST = loader.LIST
        var character = Character.new()
        for source in case.active_sets: character.sets.push_back(load(source))
        selections.push_back(character.get_active_specializations().map(path))
    var gates: Array = []
    for case in input.gates:
        var character = Character.new()
        character.mana = case.mana
        character.battle_profile.uses = case.uses
        character.battle_profile.free_ability_uses = case.free
        character.battle_profile.silenced = case.silenced
        gates.push_back(character.can_use_ability(load(case.id)))
    var appearances = {}
    for source in input.paths:
        var resource = load(source)
        if resource is Specialization: appearances[source] = color(resource.get_color())
        if resource is AbilityResource: appearances[source] = {"preview":color(resource.get_preview_color()),"cast":color(resource.get_cast_color())}
    var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify({"records":records,"selections":selections,"gates":gates,"appearances":appearances})+"\n")
    file.close()
    Specializations.LIST.clear()
    File.order.clear()
    quit()
