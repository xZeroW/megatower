
# This file is appended to the staged source classes by probe_items.py.
func definition(kind: int) -> ItemResource:
	var resource = ItemResource.new()
	resource.name = "fixture"
	resource.socket_type = SocketType.new()
	resource.set_resources.assign([ItemSets.SET_A])
	var physical = BonusStat.new(Stats.PHYSICAL_ATTACK, 2.5)
	physical.boosting_sets.assign([ItemSets.SET_B])
	resource.bonus_stats.assign([physical, BonusStat.new(Stats.PHYSICAL_ATTACK, -1.2), BonusStat.new(Stats.ARMOR, 20.0, true), BonusStat.new(Stats.CRIT_CHANCE, 1.0)])
	resource.spawn_floor = 2
	resource.extra_price = 3.5
	match kind:
		1: resource.activation_effect = ActivationEffect.new()
		2: resource.consumption_effect = ConsumptionEffect.new()
		3: resource.is_special = true
		4: resource.ability_to_learn = AbilityResource.new()
		5: resource.stat_to_adapt = Stats.ARMOR
		6:
			Items.INSIGHTUS = resource
			resource.is_special = true
		7: resource.use_custom_rarity = true
		8: resource.reforgable = false
	return resource

func make_item(kind: int, floor_number: int, rarity: int, level: int) -> Item:
	var item = Item.new()
	item.resource = definition(kind)
	item.spawn_floor = floor_number
	item.rarity = rarity as ItemRarity.Type
	item.reforge_level = level
	item.discount = 0.25
	item.uses = 2
	return item

func sets(values: Array[ItemSetResource]) -> Array:
	return values.map(func(value): return value.name)

func bonuses(item: Item) -> Array:
	var result: Array = []
	for bonus in item.get_bonus_stats():
		result.append([bonus.resource.name, bonus.amount, bonus.is_modifier, sets(bonus.boosting_sets)])
	return result

func snapshot(item: Item) -> Dictionary:
	var buy = item.get_buy_price()
	var full = item.get_buy_price(false)
	var reforged: Array = []
	for stat in item.reforged_stats:
		reforged.append([stat.resource.name, stat.base_amount, stat.modifier_amount, stat.negative_amount, sets(stat.base_boosting_sets), sets(stat.modifier_boosting_sets)])
	return {"rarity": item.rarity, "changed": item.changed_this_frame, "reforge_level": item.reforge_level, "kit": item.is_tinker_kit, "is_reforge": item.is_reforge,
		"definition": [item.resource.get_price(), item.resource.get_negative_value(), item.resource.get_rand_weight(), item.resource.get_max_rarity(item.spawn_floor), item.resource.calculate_price(item.rarity), item.resource.get_unique_stat_resources().map(func(value): return value.name)],
		"buy": [buy.type.name, buy.amount], "full": [full.type.name, full.amount], "tinker": item.get_tinker_price().amount,
		"remaining_uses": item.get_remaining_uses(), "uses": item.uses, "can_increase": item.can_increase_rarity(), "can_upgrade": item.can_upgrade(),
		"can_reforge": item.can_reforge(), "can_convert": item.can_convert_into_tinker_kit(), "infusable": item.is_infusable(),
		"sets": sets(item.get_set_resources()), "bonuses": bonuses(item), "reforged": reforged}

func _initialize() -> void:
	var rows: Array = []
	for kind in 9:
		for floor_number in [0, 1, 10]:
			for rarity in [0, 3, 8]:
				for level in [0, 1, 5]:
					var item = make_item(kind, floor_number, rarity, level)
					var snapshots: Array = [snapshot(item)]
					item.try_to_increase_rarity()
					item.decrease_rarity()
					item.refill()
					item.reforge()
					snapshots.append(snapshot(item))
					item.convert_to_tinker_kit()
					snapshots.append(snapshot(item))
					var receiver = make_item(0, floor_number, 0, 0)
					var compatible = Item.is_compatible(receiver, item)
					receiver.merge_with(item)
					snapshots.append(snapshot(receiver))
					rows.append({"kind": kind, "floor": floor_number, "rarity": rarity, "level": level, "compatible": compatible, "snapshots": snapshots})
					receiver.cleanup()
					receiver.free()
					# merge_with transfers Stat objects into receiver. Free only the
					# incoming Item itself, not those transferred stat references.
					item.free()
	var rolls: Array = []
	for base_seed in [0, 42, -1]:
		for floor_number in [0, 20, 1000]:
			for initial_reforges in [0, 1]:
				seed(base_seed)
				var item = make_item(0, floor_number, 0, 0)
				item.roll_reforge(floor_number, initial_reforges)
				rolls.append({"seed": str(base_seed), "floor": floor_number, "initial_reforges": initial_reforges, "snapshot": snapshot(item), "next_random": randi()})
				item.cleanup()
				item.free()
	var file = FileAccess.open("res://golden.json", FileAccess.WRITE)
	file.store_string(JSON.stringify({"items": rows, "rolls": rolls}) + "\n")
	file.close()
	quit()
