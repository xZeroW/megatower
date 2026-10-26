
func manager_definition(kind: int) -> ItemResource:
	var resource = ItemResource.new()
	resource.name = str(kind)
	resource.spawn_floor = 2
	resource.extra_price = 10 + kind
	resource.socket_type = SocketTypes.HELMET
	match kind:
		1: resource.is_special = true
		2: resource.activation_effect = ActivationEffect.new()
		3: resource.consumption_effect = ConsumptionEffect.new()
		4: resource.ability_to_learn = AbilityResource.new()
		5: resource.stat_to_adapt = Stats.ARMOR
		6:
			resource.is_special = true
			Items.INSIGHTUS = resource
		7:
			resource.use_custom_rarity = true
			resource.custom_rarity = ItemRarity.Type.RARE
		8:
			resource.passive = Passive.new()
			resource.passive.name = "passive"
	return resource

func creation_snapshot(item: Item) -> Array:
	var price = item.get_buy_price()
	return [item.resource.name, item.spawn_floor, item.rarity, item.reforge_level, item.has_reforged_passive(), item.changed_this_frame, item.transform_stat.name, price.type.name, price.amount]

func pair_case(manager: ItemManager, a_kind: int, b_kind: int, config: int) -> Dictionary:
	create_pool(0)
	var resources = [ItemContainerResources.INVENTORY, ItemContainerResources.EQUIPMENT, ItemContainerResources.LOOT_STASH, ItemContainerResources.MARKET, ItemContainerResources.MERCHANT]
	var a = ItemContainer.new(resources[a_kind], 4)
	var b = ItemContainer.new(resources[b_kind], 4)
	a.sockets.assign([SocketTypes.HELMET, SocketTypes.HELMET, SocketTypes.WEAPON, null])
	b.sockets.assign(a.sockets)
	a.items[0] = pool[0]
	a.items[1] = pool[1]
	b.items[0] = pool[2]
	b.items[1] = pool[1]
	var slot_a = Slot.new(a, 0)
	var slot_b = Slot.new(b, 0)
	match config:
		1: pool[0].is_phantom = true
		2: pool[2].is_reforge = true
		3: slot_a.index = -1
		4: slot_b.index = 1
		5, 7:
			pool[1].rarity = ItemRarity.Type.COMMON
			b.items[0] = pool[1]
			if config == 7: pool[0].reforged_stats.push_back(Stat.new([Stats.PHYSICAL_ATTACK, 5.0]))
		6: b.items[0] = pool[6]
	var equip_index = manager.get_equip_slot(b,slot_a).index if is_instance_valid(slot_a.get_item()) else null
	var result = {"a_kind":a_kind,"b_kind":b_kind,"config":config,
		"can_swap":manager.can_swap(slot_a,slot_b),"can_merge":manager.can_merge(slot_a,slot_b),
		"matches_socket":manager.matches_socket(slot_a,slot_b),"equip_index":equip_index,
		"rarity_compare":manager.sort_by_rarity(slot_a.get_item(),slot_b.get_item())}
	for item in pool:
		item.cleanup()
		item.free()
	a.free()
	b.free()
	return result

func _initialize() -> void:
	var manager = ItemManager.new()
	ItemContainerResources.MARKET.is_shop = true
	ItemContainerResources.MARKET.merge_on_buy = true
	ItemContainerResources.MERCHANT.is_shop = true
	var pairs: Array = []
	for a_kind in 5:
		for b_kind in 5:
			for config in 8: pairs.append(pair_case(manager,a_kind,b_kind,config))
	var observed: Array = []
	manager.item_created.connect(func(item: Item): observed.append(creation_snapshot(item)))
	var creations: Array = []
	for kind in 9:
		for floor_number in [0,7]:
			for rarity in [0,3,8,20]:
				observed.clear()
				var resource = manager_definition(kind)
				var item = manager.create_item(resource,floor_number,rarity)
				creations.append({"kind":kind,"floor":floor_number,"rarity":rarity,"snapshot":creation_snapshot(item),"observed":observed.duplicate(true),"weight":manager.get_rand_weight(resource,floor_number)})
				item.cleanup()
				item.free()
	var gold: Array = []
	var content: Array[ItemResource] = [manager_definition(0),manager_definition(7)]
	for amount in [0,55,100,500,10000]:
		seed(42)
		observed.clear()
		var item = manager.get_rand_item_from_gold(content,amount)
		gold.append({"amount":amount,"snapshot":creation_snapshot(item) if is_instance_valid(item) else null,"observed":observed.duplicate(true),"next_random":randi()})
		if is_instance_valid(item): item.free()
	Stats.TRANSFORMED_STATS.assign([Stats.ARMOR,Stats.PHYSICAL_ATTACK])
	seed(42)
	observed.clear()
	manager_definition(6)
	var insightus = manager.create_insightus(17)
	var insightus_result = {"snapshot":creation_snapshot(insightus),"observed":observed.duplicate(true),"next_random":randi()}
	insightus.free()
	var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
	file.store_string(JSON.stringify({"pairs":pairs,"creations":creations,"gold":gold,"insightus":insightus_result})+"\n")
	file.close()
	manager = null
	quit()
