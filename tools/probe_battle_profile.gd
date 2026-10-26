
var resources: Dictionary = {}
var ability_a = AbilityResource.new()
var ability_b = AbilityResource.new()

func effect(id: String, amount: float) -> StatusEffect:
	return StatusEffect.new(resources[id], amount)

func turn_snapshot(turn: BattleTurn):
	if not is_instance_valid(turn): return null
	return {"dodges":turn.dodges, "a":turn.is_used_ability(ability_a), "b":turn.is_used_ability(ability_b),
		"consumed":turn.get_consumed_stacks().map(func(value):return [value.resource.name,value.amount])}

func snapshot(profile: BattleProfile) -> Dictionary:
	var flags = [profile.has_timeout(), profile.is_stunned(), profile.is_confused(), profile.is_cursed(), profile.is_silenced(), profile.is_slowed(), profile.is_marked_by_arthur()]
	return {"effects":profile.active_status_effects.map(func(value):return [value.resource.name,value.amount] if is_instance_valid(value) else null),
		"turns":profile.turns.map(turn_snapshot), "previous":profile.get_previous_turns().map(turn_snapshot),
		"last":turn_snapshot(profile.get_last_turn()), "dodges":profile.get_total_dodges(),
		"ability_counts":[profile.get_used_ability_count(ability_a),profile.get_used_ability_count(ability_b)],
		"amounts":[profile.get_status_effect_amount(resources.poison),profile.get_status_effect_amount(resources.grace)],
		"total":profile.get_total_status_effect_amount(),"flags":flags,"stun":profile.stun_turns_left,
		"removed":profile.removed_status_effects.size()}

func run_case(history: int, stun: int, config: int) -> Dictionary:
	var profile = BattleProfile.new()
	profile.turns.clear()
	for index in history:
		if index == 1 or (history == 4 and index == 3):
			profile.turns.push_back(null)
			continue
		var turn = BattleTurn.new()
		turn.dodges = index + 2
		var ability = Ability.new()
		ability.resource = ability_a if index % 2 == 0 else ability_b
		turn.abilities.assign([ability, null, ability])
		turn.consumed_stacks.assign([null, effect("grace", 0.5)])
		profile.turns.push_back(turn)
	profile.stun_turns_left = stun
	profile.active_status_effects.assign([effect("poison", 2.5 + config), null, effect("grace", 1.5), effect("poison", 0.5),
		effect("stun", 1),effect("timeout", 2),effect("confusion", 2),effect("black_curse", 0),
		effect("silence", 1),effect("slowness", 2),effect("arthurs_mark", 2),null])
	if config % 2 == 1: profile.active_status_effects.push_front(effect("grace", -0.5))
	var steps: Array = []
	var record = func(op: String, result = null): steps.append({"op":op,"result":result,"snapshot":snapshot(profile)})
	record.call("initial")
	record.call("limit_poison",profile.can_receive_status_effect(resources.poison))
	record.call("limit_grace",profile.can_receive_status_effect(resources.grace))
	record.call("debuffs",profile.get_active_debuffs().map(func(value): return value.name))
	profile.consume_stack(resources.poison, -1)
	record.call("consume_poison_all")
	profile.consume_stack(resources.grace, 0.5)
	record.call("consume_grace_half")
	profile.consume_stack(resources.grace, 0.5)
	record.call("consume_grace_half")
	profile.waken()
	record.call("waken")
	profile.remove_buffs()
	record.call("remove_buffs")
	profile.remove_matching_status_effects(resources.poison, -0.5)
	record.call("negative_remove")
	profile.get_curr_turn().dodges += 7
	record.call("mutate_current")
	profile.process_stun()
	record.call("process_stun")
	profile.waken()
	record.call("waken")
	profile.remove_matching_status_effects(resources.poison)
	record.call("remove_poison")
	profile.consume_stack(resources.poison)
	record.call("consume_missing")
	profile.cleanup()
	profile.free()
	return {"history":history,"stun":stun,"config":config,"steps":steps}

func _initialize() -> void:
	var kinds = {}
	for row in policy_rows:
		if not kinds.has(row.kind): kinds[row.kind] = StatusEffectType.new()
		var resource = StatusEffectResource.new()
		resource.name = row.name
		resource.type = kinds[row.kind]
		resource.is_temporary = row.is_temporary
		resource.limit = row.limit
		resources[row.id] = resource
	StatusEffectTypes.BUFF = kinds.buff
	StatusEffectTypes.DEBUFF = kinds.debuff
	StatusEffects.TIMEOUT = resources.timeout
	StatusEffects.STUN = resources.stun
	StatusEffects.CONFUSION = resources.confusion
	StatusEffects.BLACK_CURSE = resources.black_curse
	StatusEffects.CURSE = resources.curse
	StatusEffects.SILENCE = resources.silence
	StatusEffects.SLOWNESS = resources.slowness
	StatusEffects.ARTHURS_MARK = resources.arthurs_mark
	# Exercise suspicious typed-array methods separately. The Python runner
	# records the actual engine diagnostics, not a guessed correction.
	var arguments = OS.get_cmdline_user_args()
	if arguments.size():
		var profile = BattleProfile.new()
		if arguments[0] == "random_buff":
			profile.active_status_effects.assign([effect("armor_shield", 2)])
			profile.remove_random_buff()
		elif arguments[0] == "used_abilities":
			var ability = Ability.new()
			ability.resource = ability_a
			profile.turns[0].abilities.assign([ability])
			profile.get_used_abilities()
		profile.cleanup()
		profile.free()
		quit()
		return
	var sequences: Array = []
	for history in 6:
		for stun in [0,1,3]:
			for config in 10: sequences.append(run_case(history,stun,config))
	var policies: Array = []
	for row in policy_rows:
		for amount in [-1.5,0.0,0.5,1.0,1.5,5.0,100.0]:
			var profile = BattleProfile.new()
			profile.stun_turns_left = 1
			profile.active_status_effects.assign([effect(row.id, amount),null,effect(row.id,0.5),null])
			var can_receive = profile.can_receive_status_effect(resources[row.id])
			var after_limit = snapshot(profile)
			profile.waken()
			var after_waken = snapshot(profile)
			profile.remove_buffs()
			policies.append({"id":row.id,"amount":amount,"can_receive":can_receive,"after_limit":after_limit,
				"after_waken":after_waken,"after_buffs":snapshot(profile)})
			profile.cleanup()
			profile.free()
	var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
	file.store_string(JSON.stringify({"sequences":sequences,"policies":policies}) + "\n")
	file.close()
	quit()
