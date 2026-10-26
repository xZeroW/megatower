
class StatusEffectTypes:
    static var BUFF=StatusEffectType.new()
    static var DEBUFF=StatusEffectType.new()
class RNGManager:
    static var gameplay_rand=RandomNumberGenerator.new()
class Trace:
    static var record: Callable
    static func push(action):record.call(action)
class Battle extends RefCounted:
    var enemies: Array[Character]=[]
    func get_enemy_characters_in_combat() -> Array[Character]:return enemies
class Memory extends RefCounted:
    var battle: Battle
    var local_player: Character
    func is_player(character):return character==local_player
    func get_character_reference(character):return character
class PopupLabelData extends RefCounted:
    var text: String
    func _init(value,_color):text=value
class BattleLogData extends RefCounted:
    var applied_status_effect: StatusEffect
    var status_effect_applier: Character
    var status_effect_target: Character
class Flash extends RefCounted:
    func show_as_stun():pass
    func flash():Trace.push(["stun_flash"])
class Room extends RefCounted:
    var screen_flash_effect=Flash.new()
    var confused: bool=false:
        set(value):
            confused=value
            if value:Trace.push(["confused"])
class Canvas extends RefCounted:
    var room_screen=Room.new()
    func create_popup_label(data):Trace.push(["buff_failed" if data.text=="BUFF FAILED" else "debuff_immune"])
class State extends RefCounted:
    func add_to_battle_log(_battle,data):Trace.push(["log",data.status_effect_target,data.status_effect_applier,data.applied_status_effect.resource,data.applied_status_effect.amount])
    func play_status_effect(resource,amount):Trace.push(["play",resource,amount])
class Manager extends RefCounted:
    func add_temp_stat(target,stat):Trace.push(["temp",target,stat.amount])
class UIManager:
    static func update_selected_player_health_info(target):Trace.push(["health",target])
    static func update_player_status_effects(target):Trace.push(["statuses",target])
class Processor extends RefCounted:
    var memory=Memory.new()
    var canvas_layer=Canvas.new()
    var room_screen=canvas_layer.room_screen
    var gameplay_state=State.new()
    var character_manager=Manager.new()
    func attack_character(_battle,target,attacker,_source):Trace.push(["attack",target,attacker])
    func try_to_damage_character(target,attacker,data):
        Trace.push(["damage",target,attacker,data.damage])
        data.free()
# PROCESSOR_METHODS

var stats: Dictionary={}
var statuses: Dictionary={}
var status_ids: Dictionary={}
var actors: Array[Character]=[]
var events: Array=[]
var callback_mode: int=0

func effects(values):return values.map(func(value):return [status_ids[value.resource],value.amount] if is_instance_valid(value) else null)
func snapshot():
    return actors.map(func(actor):return {"active":effects(actor.battle_profile.active_status_effects),"stun":actor.battle_profile.stun_turns_left,
        "turns":actor.battle_profile.turns.map(func(turn):return {"received":effects(turn.received_status_effects),"applied":effects(turn.applied_status_effects),"consumed":effects(turn.consumed_stacks),"abilities":turn.abilities.size(),"stopped":turn.stopped_attacking} if is_instance_valid(turn) else null)})
func record(action):
    var values=action.map(func(value):return actors.find(value) if value is Character else (status_ids[value] if value is StatusEffectResource else value))
    events.push_back({"action":values,"snapshot":snapshot(),"state":RNGManager.gameplay_rand.state})
func received(effect,actor,applier):
    record(["received",actor,applier,effect.resource,effect.amount])
    if callback_mode==20:
        actors[0].battle_profile.active_item_sets.push_back(ItemSets.CHROMALURE)
        RNGManager.gameplay_rand.randf()
    if callback_mode==27:actors[0].battle_profile.active_item_sets.clear()

func run_case(case):
    actors.clear();events.clear();callback_mode=int(case.mode)
    var processor=Processor.new()
    processor.memory.battle=Battle.new()
    for _index in 3:
        var actor=Character.new()
        actor.stats.assign([Stat.new([Stats.MAX_HEALTH,100]),Stat.new([Stats.HEALTH,50]),Stat.new([Stats.TENACITY,100 if case.mode==5 else (50 if case.mode==6 else 0)])])
        actors.push_back(actor)
    processor.memory.local_player=actors[0]
    processor.memory.battle.enemies.assign([actors[1],actors[2]])
    var target=actors[1]
    var applier=actors[0]
    var resource=statuses[case.resource] if case.resource!=null else null
    match int(case.mode):
        1:applier=null
        3:actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.FEAR,0))
        4:actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.DEBUFF_IMMUNITY,0))
        7:actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED,ItemSets.UNCHAINED])
        8:
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED,ItemSets.UNCHAINED])
            if resource!=null:actors[2].battle_profile.active_status_effects.push_back(StatusEffect.new(resource,resource.limit if resource.limit!=-1 else 1))
        9:processor.memory.battle=null
        10:actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.CLARITY,0))
        11:actors[1].battle_profile.turns[0].consumed_stacks.push_back(StatusEffect.new(StatusEffects.CLARITY,1))
        12:actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.EPHEMERAL_LOCK,1))
        13:
            actors[0].probe_specializations.assign([Specializations.MINDBREAKER,Specializations.THUNDERBORN])
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
        14:target=actors[0]
        15:
            applier=actors[2]
            applier.battle_profile.active_item_sets.assign([ItemSets.UNCHAINED,ItemSets.UNCHAINED])
        16:actors[1].battle_profile.turns.clear()
        17:actors[1].battle_profile.turns.assign([null])
        18:
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
            actors[2].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.DEBUFF_IMMUNITY,1))
            actors[2].stats.push_back(Stat.new([Stats.TENACITY,100]))
        19:actors[0].battle_profile.active_item_sets.assign([ItemSets.CHROMALURE])
        21:actors[0].battle_profile.active_item_sets.assign([ItemSets.THUNDER,ItemSets.THUNDER,ItemSets.THUNDER])
        22:
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
            actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(resource,resource.limit if resource.limit!=-1 else 1))
        23:
            actors[1].battle_profile.active_status_effects.assign([StatusEffect.new(StatusEffects.CLARITY,-1),null,StatusEffect.new(StatusEffects.CLARITY,1)])
            actors[1].battle_profile.turns[0].consumed_stacks.assign([null,StatusEffect.new(StatusEffects.CLARITY,-1)])
        24:actors[1].battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.EPHEMERAL_LOCK,0.4))
        25:
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
            actors[1].stats.push_back(Stat.new([Stats.TENACITY,100]))
        26:
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
            actors[2].battle_profile.active_status_effects.assign([null,StatusEffect.new(resource,0.5),StatusEffect.new(resource,1.5)])
        27:actors[0].battle_profile.active_item_sets.assign([ItemSets.CHROMALURE])
        28:
            actors[0].probe_specializations.assign([Specializations.MINDBREAKER,Specializations.THUNDERBORN])
            actors[0].battle_profile.active_item_sets.assign([ItemSets.UNCHAINED])
            target=actors[0]
    for actor in actors:
        actor.cache_stats()
        var ability=Ability.new()
        ability.resource=Abilities.TRIPLE_ATTACK
        if actor.battle_profile.turns.size() and is_instance_valid(actor.battle_profile.turns.back()):actor.battle_profile.turns.back().abilities.push_back(ability)
        actor.recieved_status_effect.connect(received.bind(actor,applier))
    RNGManager.gameplay_rand.seed=int(case.seed)
    Trace.record=record
    var result=await processor.try_to_apply_status_effect(target,applier,resource,case.amount)
    var output={"result":result,"events":events.duplicate(true),"snapshot":snapshot(),"state":RNGManager.gameplay_rand.state}
    for actor in actors:
        for connection in actor.recieved_status_effect.get_connections():actor.recieved_status_effect.disconnect(connection.callable)
    Trace.record=Callable()
    actors.clear()
    return output

func _initialize():run()
func run():
    var input=JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    for row in input.catalog.definitions:
        var resource=StatResource.new()
        resource.name=row.name
        resource.max_amount=row.max_amount
        resource.minimum_amount=row.minimum_amount
        resource.ignore_minimum_amount=row.ignore_minimum_amount
        stats[row.id]=resource
        Stats.LIST.push_back(resource)
# ASSIGN_STATS
# ASSIGN_STATUSES
    for row in input.policies:
        var resource=statuses[row.id]
        resource.name=row.name
        resource.limit=row.limit
        resource.affected_by_tenacity=row.affected_by_tenacity
        resource.type=StatusEffectTypes.BUFF if row.kind=="buff" else (StatusEffectTypes.DEBUFF if row.kind=="debuff" else StatusEffectType.new())
        status_ids[resource]=row.id
    var results: Array=[]
    for case in input.cases:results.push_back(await run_case(case))
    var file=FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify(results)+"\n")
    file.close()
    for stat in Stat._probe_instances:
        if is_instance_valid(stat):stat.free()
    Stat._probe_instances.clear()
    quit()
