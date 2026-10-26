
var stats: Dictionary={}
var stat_ids: Dictionary={}
var statuses: Dictionary={}
var status_ids: Dictionary={}

func effects(values):return values.map(func(value):return [status_ids[value.resource],value.amount] if is_instance_valid(value) else null)
func snapshot(character):
    return {"active":effects(character.battle_profile.active_status_effects),"consumed":character.battle_profile.turns.map(func(turn):return effects(turn.consumed_stacks) if is_instance_valid(turn) else null)}
func packet(data):
    return {"type":stat_ids[data.type] if is_instance_valid(data.type) else null,"damage":data.damage,"source":data.source,"accuracy":data.accuracy,"penetration":data.penetration,
        "crit":data.is_crit,"lucky":data.is_lucky,"pure":data.pure,"on_hit":effects(data.status_effects_on_hit)}

func run_case(case,abilities,sets):
    var character=Character.new()
    for id in stats:
        var value=float(case.values.get(id,0))
        character.cached_stats[stats[id]]={"final_amount":value,"base_amount":value,"modifier_amount":0.0,"negative_amount":0.0}
    for id in case.sets:character.battle_profile.active_item_sets.push_back(sets[id])
    for row in case.active:character.battle_profile.active_status_effects.push_back(StatusEffect.new(statuses[row[0]],row[1]))
    character.battle_profile.stun_turns_left=case.stun
    if case.omni:character.transformed_stats.assign([Stats.OMNI_CRIT_CHANCE])
    if case.ironclad:character.base_passive=Passives.IRONCLAD
    match int(case.history):
        0:character.battle_profile.turns.clear()
        2:character.battle_profile.turns.assign([BattleTurn.new(),null])
        3:character.battle_profile.turns.assign([BattleTurn.new(),null,BattleTurn.new()])
    if character.battle_profile.turns.size() and is_instance_valid(character.battle_profile.turns.back()):
        var turn=character.battle_profile.turns.back()
        for id in case.abilities:
            var ability=Ability.new()
            ability.resource=abilities[id]
            turn.abilities.push_back(ability)
        for row in case.consumed:turn.consumed_stacks.push_back(StatusEffect.new(statuses[row[0]],row[1]) if row!=null else null)
        if case.override==1:turn.attack_type=Stats.TRUE_DAMAGE
        if case.override==2:turn.attack_type=null
    var result={"attacks":[character.get_physical_attack(),character.get_magic_attack(),character.get_freeze_attack(),character.get_armored_attack()],
        "type":stat_ids[character.get_attack_type()],"damage_type":stat_ids[character.get_attack_damage_type()],"attack_damage":character.get_attack_damage(),
        "on_hit":effects(character.get_status_effects_on_hit()),"adjacent":effects(character.get_adjacent_status_effects_on_hit()),"starting":effects(character.get_starting_status_effects())}
    var data=character.get_attack_damage_data(DamageData.Source.ATTACK)
    result.packet=packet(data)
    character.apply_damage_output_boosters(data)
    result.boosted=packet(data)
    data.free()
    result.output=[character.get_physical_damage(0),character.get_magic_damage(0),character.get_freeze_damage(0),character.get_physical_damage(-2.5),character.get_magic_damage(-2.5),character.get_freeze_damage(-2.5)]
    var rng=RandomNumberGenerator.new()
    rng.seed=int(case.seed)
    RNGManager.gameplay_rand=rng
    var gates: Array=[]
    for type in [Stats.PHYSICAL_DAMAGE,Stats.MAGIC_DAMAGE,Stats.FREEZE_DAMAGE,null]:gates.push_back({"result":character.try_to_crit(rng,type),"state":rng.state})
    gates.push_back({"result":character.try_to_dodge(rng),"state":rng.state})
    for accuracy in [-1,0,50,100,200]:gates.push_back({"result":character.try_to_hit(character,accuracy),"state":rng.state})
    result.rng=gates
    result.block=character.try_to_block()
    result.multi=character.try_to_multi_attack()
    result.parries=[]
    for _index in 2:result.parries.push_back({"result":character.try_to_parry(),"snapshot":snapshot(character)})
    result.reductions=[]
    for initial in [0,0.000001,-3,1,100]:result.reductions.push_back({"result":character.try_to_reduce_damage(initial),"snapshot":snapshot(character)})
    return result

func _initialize():
    var input=JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    for row in input.catalog.definitions:
        var resource=StatResource.new()
        resource.name=row.name
        stats[row.id]=resource
        stat_ids[resource]=row.id
# ASSIGN_STATS
# ASSIGN_STATUSES
    for row in input.policies:
        statuses[row.id].name=row.name
        statuses[row.id].limit=row.limit
        status_ids[statuses[row.id]]=row.id
# ABILITY_LOOKUP
# SET_LOOKUP
    var result: Array=[]
    for case in input.cases:result.push_back(run_case(case,abilities,sets))
    var file=FileAccess.open("res://golden.json",FileAccess.WRITE)
    file.store_string(JSON.stringify(result)+"\n")
    file.close()
    Stat._probe_instances.clear()
    quit()
