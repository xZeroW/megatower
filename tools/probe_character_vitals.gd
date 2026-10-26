
var stats: Dictionary = {}
var stat_ids: Dictionary = {}
var statuses: Dictionary = {}
var status_ids: Dictionary = {}

func stat_array(values: Array[Stat]):
    return values.map(func(value):return [stat_ids[value.resource],value.base_amount,value.modifier_amount,value.negative_amount])
func effects(values):
    return values.map(func(value):return [status_ids[value.resource],value.amount] if is_instance_valid(value) else null)
func turn_snapshot(turn):
    if not is_instance_valid(turn):return null
    return {"stopped":turn.stopped_attacking,"received":effects(turn.received_status_effects),"applied":effects(turn.applied_status_effects),"healed":turn.health_recovered,"percent":turn.percent_health_recovered}
func snapshot(character: Character):
    var cache = {}
    for id in stats:
        var value=character.cached_stats[stats[id]]
        cache[id]=[float(value.final_amount),float(value.base_amount),float(value.modifier_amount),float(value.negative_amount)]
    var profile=character.battle_profile
    var turn=profile.turns.back() if profile.turns.size() and is_instance_valid(profile.turns.back()) else BattleTurn.new()
    return {"base":stat_array(character.stats),"external":stat_array(profile.external_stats),"battle":stat_array(profile.stats),"turn":stat_array(turn.stats),"cache":cache,"requested":character.stat_change_process_requested,
        "vitals":[character.get_health(),character.get_max_health(),character.get_health_percent(),character.get_mana(),character.get_max_mana(),character.get_missing_health(),character.get_missing_mana(),character.get_missing_armor(),character.get_guard_reduction()],
        "full":character.is_full_health(),"over":character.is_overhealed(),"history":profile.turns.map(turn_snapshot),"effects":effects(profile.active_status_effects),"removed":profile.removed_status_effects.map(func(value):return status_ids[value])}

func make_character(health: float,variant: int = 0):
    var character=Character.new()
    character.stats.assign([Stat.new([Stats.MAX_HEALTH,100.0]),Stat.new([Stats.HEALTH,health]),Stat.new([Stats.ARMOR,10.0]),
        Stat.new([Stats.ACTIVE_ARMOR,20.0 if variant else 2.0]),Stat.new([Stats.MANA,9.0 if variant else -2.0]),Stat.new([Stats.MAX_MANA,5.0]),Stat.new([Stats.ACCURACY,100.0])])
    return character

func vital_case(case):
    var character=make_character(case.health,int(case.variant))
    var flags=int(case.flags)
    if flags&1:character.battle_profile.active_item_sets.assign([ItemSets.FLESH])
    if flags&2:character.battle_profile.active_status_effects.push_back(StatusEffect.new(StatusEffects.CURSE,1))
    if flags&4:character.base_passive=Passives.EMPTY_BASTION
    var events: Array = []
    character.stat_changed.connect(func(resource,old_amount):events.push_back({"resource":stat_ids[resource],"old":old_amount,"snapshot":snapshot(character)}))
    var steps: Array = []
    var record=func(op,result=null):
        steps.push_back({"op":op,"result":result,"snapshot":snapshot(character),"events":events.duplicate(true)})
        events.clear()
    character.cache_stats()
    record.call("rebuild")
    record.call("heal",character.heal(20))
    character.change_mana(10)
    record.call("change_mana")
    character.cache_stats()
    record.call("rebuild")
    character.change_active_armor(3)
    record.call("change_armor")
    character.change_health(-200)
    record.call("change_health")
    character.reset_health()
    record.call("reset_health")
    character.cache_stats()
    record.call("rebuild")
    character.set_health(-2.5)
    record.call("set_health")
    character.set_active_armor(15.5)
    record.call("set_armor")
    character.cache_stats()
    record.call("rebuild")
    character.change_health(0.25)
    record.call("fraction_health")
    character.reset_active_armor()
    record.call("reset_armor")
    character.cache_stats()
    record.call("rebuild")
    for connection in character.stat_changed.get_connections():character.stat_changed.disconnect(connection.callable)
    return {"health":case.health,"variant":case.variant,"flags":case.flags,"steps":steps}

func status_case(case):
    var character=make_character(50)
    var applier=make_character(90)
    var mode=int(case.mode)
    var resource=statuses[case.resource]
    match mode:
        1:
            character.battle_profile.active_status_effects.assign([null,StatusEffect.new(resource,1)])
            character.battle_profile.turns.clear()
        2:
            character.battle_profile.active_status_effects.assign([StatusEffect.new(resource,0.5),StatusEffect.new(resource,2.5),null])
            character.battle_profile.removed_status_effects.assign([resource])
            character.battle_profile.turns.assign([null])
    character.cache_stats()
    applier.cache_stats()
    var steps: Array = []
    var record=func(op,result=null):steps.push_back({"op":op,"result":result,"snapshot":snapshot(character),"applier":snapshot(applier)})
    var actor=applier if mode==0 else (character if mode in [1,3] else null)
    record.call("initial")
    character.set_status_effect_amount(resource,2)
    record.call("set_two")
    record.call("apply_positive",character.try_to_add_status_effect(actor,resource,3.5))
    character.set_status_effect_amount(resource,0)
    record.call("set_zero")
    record.call("apply_negative",character.try_to_add_status_effect(actor,resource,-2))
    character.set_status_effect_amount(resource,5)
    record.call("set_five")
    record.call("apply_zero",character.try_to_add_status_effect(actor,resource,0))
    character.set_status_effect_amount(resource,1)
    record.call("set_one")
    record.call("apply_minus_one",character.try_to_add_status_effect(actor,resource,-1))
    character.cache_stats()
    record.call("rebuild")
    return {"resource":case.resource,"mode":case.mode,"steps":steps}

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
# ASSIGN_STATUSES
    for row in input.policies:
        statuses[row.id].name=row.name
        statuses[row.id].limit=row.limit
        status_ids[statuses[row.id]]=row.id
    var vitals: Array = []
    for case in input.vitals:vitals.push_back(vital_case(case))
    var status_results: Array = []
    for case in input.statuses:status_results.push_back(status_case(case))
    var file=FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify({"vitals":vitals,"statuses":status_results})+"\n")
    file.close()
    for stat in Stat._probe_instances:
        if is_instance_valid(stat):stat.free()
    Stat._probe_instances.clear()
    quit()
