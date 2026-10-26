
var pool: Array[Item] = []
var events: Array = []

func label(item) -> int:
	return pool.find(item) if is_instance_valid(item) else -1

func labels(items: Array[Item]) -> Array:
	return items.map(func(item): return label(item))

func set_names(values: Array[ItemSetResource]) -> Array:
	return values.map(func(value): return value.name)

func fixture(index: int) -> ItemResource:
	var resource = ItemResource.new()
	resource.name = str(index)
	resource.set_resources.assign([ItemSets.SET_A if index % 2 == 0 else ItemSets.SET_B])
	resource.socket_type = [SocketTypes.HELMET, SocketTypes.HELMET, SocketTypes.WEAPON, null, null, null, SocketTypes.RING, SocketTypes.CHESTPLATE][index]
	if index == 3:
		resource.activation_effect = ActivationEffect.new()
	if index == 4:
		resource.consumption_effect = ConsumptionEffect.new()
	if index in [6, 7]:
		var bonus = BonusStat.new(Stats.PHYSICAL_ATTACK, 2.5)
		bonus.boosting_sets.assign([ItemSets.SET_B if index == 6 else ItemSets.SET_A])
		resource.bonus_stats.assign([bonus])
	return resource

func create_pool(config: int) -> void:
	pool.clear()
	Items.LIST.clear()
	for i in 9:
		var item = Item.new()
		if i < 8:
			item.resource = pool[0].resource if i == 1 else fixture(i)
			if i != 1: Items.LIST.push_back(item.resource)
		item.rarity = [0, 2, 1, 0, 0, 0, 3, 1, 0][i] as ItemRarity.Type
		pool.push_back(item)
	if config == 1:
		pool[0].burnout = 1
		pool[6].is_phantom = true
		pool[7].is_reforge = true
	if config == 2:
		pool[1].burnout = 3
		pool[2].is_banish = true

func observe_container(container: ItemContainer) -> void:
	container.item_added.connect(func(item: Item, idx: int): record(container, "added", label(item), idx, 0))
	container.item_updated.connect(func(item: Item, idx: int): record(container, "updated", label(item), idx, 0))
	container.item_removing.connect(func(item: Item): record(container, "removing", label(item), -1, 0))
	container.item_removed.connect(func(idx: int, cause: int): record(container, "removed", -1, idx, cause))

func record(container: ItemContainer, kind: String, item: int, index: int, cause: int) -> void:
	events.append({"kind": kind, "item": item, "index": index, "cause": cause, "raw": labels(container.items), "order": container.modification_order.duplicate()})

func snapshot(container: ItemContainer) -> Dictionary:
	var get_values: Array = [label(container.get_item(0)), label(container.get_item(-1))]
	if container.items.size() >= 2: get_values.push_back(label(container.get_item(-2)))
	var states: Array = []
	for item in pool:
		states.append([item.rarity, item.reforge_level, item.is_reforge] if is_instance_valid(item) else null)
	return {"raw": labels(container.items), "virtual": labels(container.get_items_with_null()), "items": labels(container.get_items()),
		"count": container.get_item_count(), "full": container.is_full(), "last": container.get_last_item_idx(),
		"active": set_names(container.get_active_item_sets()), "filtered_a": labels(container.get_filtered_from_item_set(ItemSets.SET_A)),
		"filtered_b": labels(container.get_filtered_from_item_set(ItemSets.SET_B)), "duplicates": container.has_duplicates(),
		"synergy": set_names(container.get_synergy_sets(pool[0])) if is_instance_valid(pool[0]) else [],
		"order": container.modification_order.duplicate(), "get": get_values, "states": states}

func run_sequence(size: int, equipment: bool, config: int) -> Dictionary:
	create_pool(config)
	var container = ItemContainer.new(ItemContainerResources.EQUIPMENT if equipment else ItemContainerResources.INVENTORY, size)
	var base_slots: Array[SocketType] = [SocketTypes.HELMET, SocketTypes.WEAPON, SocketTypes.REPLICATED_WEAPON, SocketTypes.RING]
	for index in container.items.size():
		container.sockets.push_back(base_slots[index % 4])
	observe_container(container)
	events.clear()
	var steps: Array = []
	var add_step = func(op: String, args: Array, result = null):
		var state: Dictionary
		if op == "cleanup":
			# Source normally destroys the container after cleanup. Calling its
			# typed ItemUtils helpers on freed Object refs is not a valid query.
			state = {"raw": labels(container.items), "order": container.modification_order.duplicate(), "states": pool.map(func(item): return [item.rarity, item.reforge_level, item.is_reforge] if is_instance_valid(item) else null)}
		else:
			state = snapshot(container)
		steps.append({"op": op, "args": args, "result": result, "snapshot": state, "events": events.duplicate(true)})
		events.clear()
	add_step.call("initial", [])
	for index in 9:
		var result = container.try_to_add_item(pool[index])
		add_step.call("try_add", [index], result)
	var accepted = container.try_to_add_item_at(base_slots, 1, pool[2])
	add_step.call("try_at", [1, 2], accepted)
	accepted = container.try_to_add_item_at(base_slots, 0, pool[2])
	add_step.call("try_at", [0, 2], accepted)
	container.remove_item_at(0, ItemContainer.ItemRemoveCause.BANISHED)
	add_step.call("remove", [0, 1])
	container.remove_item_at(0)
	add_step.call("remove", [0, 0])
	container.add_item_at(0, pool[0])
	add_step.call("add_at", [0, 0])
	if container.items.size() > 0:
		container.add_item_at(-1, pool[6])
		add_step.call("add_at", [-1, 6])
		container.merge_items(-1, pool[1], null)
		add_step.call("merge", [-1, 1])
		container.remove_reforge(-1)
		add_step.call("remove_reforge", [-1])
	container.process_modification(99)
	add_step.call("modify", [99])
	if container.items.size() >= 5:
		container.sort_items()
		add_step.call("sort", [])
	var slot = Slot.new(container, 0)
	slot.remove_item(ItemContainer.ItemRemoveCause.INSIGHTUS_ACTIVATED)
	add_step.call("slot_remove", [0, 4])
	accepted = slot.try_to_add_item(base_slots, pool[0])
	add_step.call("slot_add", [0, 0], accepted)
	container.clear()
	add_step.call("clear", [])
	var first_empty = container.get_first_empty_idx()
	add_step.call("first_empty", [], first_empty)
	accepted = container.try_to_add_item(pool[2])
	add_step.call("try_add", [2], accepted)
	container.cleanup()
	add_step.call("cleanup", [])
	for item in pool:
		if is_instance_valid(item):
			item.cleanup()
			item.free()
	container.free()
	return {"size": size, "equipment": equipment, "config": config, "steps": steps}

func run_sort(size: int, pattern: int) -> Dictionary:
	pool.clear()
	Items.LIST.clear()
	var resources: Array[ItemResource] = []
	for i in 8: resources.push_back(fixture(i))
	Items.LIST.assign(resources)
	# Missing resource ranks (-1) deliberately share a source sorting group.
	if pattern == 4: Items.LIST.clear()
	var container = ItemContainer.new(ItemContainerResources.INVENTORY, size)
	for i in size:
		var item = Item.new()
		item.resource = resources[0 if pattern < 2 else (i * 7 + 3) % 8]
		item.rarity = (0 if pattern == 0 else (size - i) % 9) as ItemRarity.Type
		pool.push_back(item)
		container.items[i] = item
	var original = labels(container.items)
	var notification = {"count": 0, "old_visible": true}
	container.item_updated.connect(func(_item: Item, _index: int):
		notification.count += 1
		notification.old_visible = notification.old_visible and labels(container.items) == original)
	container.sort_items()
	var result = {"size": size, "pattern": pattern, "raw": labels(container.items), "notification": notification}
	for item in pool:
		item.cleanup()
		item.free()
	container.free()
	return result

func _initialize() -> void:
	var sequences: Array = []
	for size in [0, 1, 3, 8, 20, -1]:
		for equipment in [false, true]:
			for config in 3:
				sequences.append(run_sequence(size, equipment, config))
	var sorts: Array = []
	for size in [5, 8, 20, 35, 64, 128, 512]:
		for pattern in 5: sorts.append(run_sort(size, pattern))
	var file = FileAccess.open("res://golden.json", FileAccess.WRITE)
	file.store_string(JSON.stringify({"sequences": sequences, "sorts": sorts}) + "\n")
	file.close()
	quit()
