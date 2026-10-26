
var pool: Array[Item] = []

func bonus(resource: StatResource, amount: float, modifier: bool, sets: Array = []) -> BonusStat:
	var value = BonusStat.new(resource, amount, modifier)
	value.boosting_sets.assign(sets)
	return value

func make_item(index: int, rarity: int, config: int) -> Item:
	var resource = ItemResource.new()
	resource.name = "fixture_" + str(index)
	resource.socket_type = SocketTypes.WEAPON if index == 0 else SocketTypes.HELMET
	resource.set_resources.assign([[ItemSets.SET_A,ItemSets.SET_A],[ItemSets.SET_B,ItemSets.GENERIC],
		[ItemSets.GENERIC],[ItemSets.SET_A],[ItemSets.SET_B],[ItemSets.SET_B]][index])
	match index:
		0: resource.bonus_stats.assign([bonus(Stats.PHYSICAL_ATTACK,2.5,false,[ItemSets.SET_A,ItemSets.SET_B,ItemSets.SET_A])])
		1: resource.bonus_stats.assign([bonus(Stats.PHYSICAL_ATTACK,1.5,false),bonus(Stats.PHYSICAL_ATTACK,-0.5,false),bonus(Stats.ARMOR,12.5,true,[ItemSets.SET_A])])
		2: resource.bonus_stats.assign([bonus(Stats.ARMOR,3,false,[ItemSets.SET_A])])
		3:
			resource.is_special = true
			resource.bonus_stats.assign([bonus(Stats.ARMOR,-2,false),bonus(Stats.ARMOR,-12.5,true)])
		4: resource.bonus_stats.assign([bonus(Stats.GOLD,100,false)])
		5: resource.bonus_stats.assign([bonus(Stats.PHYSICAL_ATTACK,1,true,[ItemSets.GENERIC])])
	var item = Item.new()
	item.resource = resource
	item.rarity = rarity as ItemRarity.Type
	if index == 0 or index == 1:
		item.reforged_passive = Passive.new()
		item.reforged_passive.name = "passive_" + str(index)
		item.reforged_passive.bonus_stats.assign([bonus(Stats.GOLD,2,false)] if index == 0 else [bonus(Stats.ARMOR,0.5,true)])
	if index == 0 and config & 1:
		item.reforged_stats.push_back(Stat.new([bonus(Stats.PHYSICAL_ATTACK,4,false,[ItemSets.GENERIC])]))
	if index == 1 and config & 2: item.is_phantom = true
	if index == 2 and config & 4: item.is_reforge = true
	return item

func labels(items: Array[Item]) -> Array:
	return items.map(func(item):return pool.find(item))

func names(sets: Array[ItemSetResource]) -> Array:
	return sets.map(func(value):return value.name)

func stat_snapshot(stats: Array[Stat]) -> Array:
	return stats.map(func(stat):return [stat.resource.name,stat.base_amount,stat.modifier_amount,stat.negative_amount,names(stat.base_boosting_sets),names(stat.modifier_boosting_sets)])

func snapshot(character: Character) -> Dictionary:
	var first = stat_snapshot(character.get_stats_from_items())
	# The source clears/frees the old equipment_stats before each rebuild.
	var second = stat_snapshot(character.get_stats_from_items())
	character.cache_stats()
	var cache = Stats.LIST.map(func(resource):
		var stat = character.cached_stats[resource]
		return [resource.name,stat.base_amount,stat.modifier_amount,stat.negative_amount,float(stat.final_amount)])
	return {"stats":first,"rebuilt":second,"equipment":labels(character.get_equipment_items()),
		"cache":cache,
		"without_burnout":labels(character.get_equipment_items(false)),"active":names(character.get_active_item_sets()),
		"limited":names(character.get_limited_active_item_sets()),
		"counts":[Character.get_item_set_count(character,ItemSets.SET_A),Character.get_item_set_count(character,ItemSets.SET_B),Character.get_item_set_count(character,ItemSets.GENERIC),Character.get_item_set_count(character,ItemSets.PHANTOM)],
		"passives":character.get_passives().map(func(value):return value.name)}

func run_case(rarity: int, config: int, replica: bool, reverse: bool) -> Dictionary:
	var character = Character.new()
	character.inventory = ItemContainer.new(ItemContainerResources.INVENTORY,3)
	character.equipment = ItemContainer.new(ItemContainerResources.EQUIPMENT,5)
	character.base_passive = Passive.new()
	character.base_passive.name = "base"
	character.base_passive.bonus_stats.assign([bonus(Stats.PHYSICAL_ATTACK,1,false)])
	character.stats.assign([Stat.new([Stats.PHYSICAL_ATTACK,3]),Stat.new([Stats.ARMOR,-2])])
	character.battle_profile.external_stats.assign([Stat.new([bonus(Stats.ARMOR,10,true)])])
	character.battle_profile.stats.assign([Stat.new([Stats.GOLD,7])])
	character.battle_profile.turn.stats.assign([Stat.new([Stats.ARMOR,5])])
	var ability = Ability.new()
	ability.resource = AbilityResource.new()
	ability.resource.bonus_stats_while_active.assign([bonus(Stats.ARMOR,3,false)])
	character.battle_profile.turn.abilities.assign([ability,null,ability])
	pool.clear()
	for index in 6: pool.push_back(make_item(index,rarity,config))
	character.inventory.items.assign([pool[3],pool[4],null])
	character.equipment.items.assign([pool[0],pool[1],pool[2],pool[5],null])
	character.equipment.sockets.assign([SocketTypes.WEAPON,SocketTypes.HELMET,SocketTypes.HELMET,SocketTypes.HELMET,SocketTypes.REPLICATED_WEAPON if replica else null])
	if reverse:
		character.equipment.modification_order.reverse()
		character.equipment.items.assign([pool[5],pool[2],pool[1],pool[0],null])
	if config & 1: character.battle_profile.active_item_sets.assign([ItemSets.SET_B,ItemSets.SET_B,ItemSets.GENERIC])
	if config & 2: character.battle_profile.active_item_sets.push_back(ItemSets.SET_A)
	var states: Array = [snapshot(character)]
	pool[0].burnout = 1
	pool[3].burnout = 2
	states.push_back(snapshot(character))
	pool[0].is_phantom = true
	pool[1].is_reforge = true
	states.push_back(snapshot(character))
	character.equipment.items[1] = null
	character.inventory.items[0] = null
	states.push_back(snapshot(character))
	for item in pool:
		item.cleanup()
		item.free()
	for stat in character.equipment_stats: stat.free()
	character.inventory.free()
	character.equipment.free()
	return {"rarity":rarity,"config":config,"replica":replica,"reverse":reverse,"snapshots":states}

func _initialize() -> void:
	Stats.LIST.assign([Stats.PHYSICAL_ATTACK,Stats.ARMOR,Stats.GOLD])
	var a0 = BonusStatsArray.new()
	a0.bonus_stats.assign([bonus(Stats.ARMOR,2,false)])
	var a2 = BonusStatsArray.new()
	a2.bonus_stats.assign([bonus(Stats.PHYSICAL_ATTACK,3,true)])
	ItemSets.SET_A.bonus_stats.assign([a0,null,a2])
	var b0 = BonusStatsArray.new()
	b0.bonus_stats.assign([bonus(Stats.GOLD,1,false)])
	var b1 = BonusStatsArray.new()
	b1.bonus_stats.assign([bonus(Stats.ARMOR,-1,false)])
	ItemSets.SET_B.bonus_stats.assign([b0,b1])
	var rows: Array = []
	for rarity in 9:
		for config in 8:
			for replica in [false,true]:
				for reverse in [false,true]: rows.push_back(run_case(rarity,config,replica,reverse))
	var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
	file.store_string(JSON.stringify(rows)+"\n")
	file.close()
	for stat in Stat._probe_instances:
		if is_instance_valid(stat): stat.free()
	Stat._probe_instances.clear()
	quit()
