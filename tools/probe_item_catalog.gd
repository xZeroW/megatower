extends SceneTree

class File:
    static var order: Array = []
    static func get_file_paths(_path): return order
class System:
    static var demo: bool = false
    static func is_demo(): return demo
class ItemSets:
    static var ESSENTIAL = load("res://resources/item_sets/essential.tres")
    static var TOME = load("res://resources/item_sets/tome.tres")
    static var CONSUMABLE = load("res://resources/item_sets/consumable.tres")
# ITEMS_SOURCE

func path(resource):return resource.resource_path if is_instance_valid(resource) else null
func paths(values):return values.map(path)
func bonus(value):
    if not is_instance_valid(value):return null
    return {"resource":value.resource.resource_path.get_file().trim_suffix(".tres"),"amount":value.amount,"is_modifier":value.is_modifier,"boosting_sets":paths(value.boosting_sets)}
func definition(resource: ItemResource):
    var activation = null
    if is_instance_valid(resource.activation_effect):
        var effect = resource.activation_effect
        activation = {"ability":path(effect.ability),"status_effect":[effect.status_effect.resource.resource_path.get_file().trim_suffix(".tres"),effect.status_effect.amount] if is_instance_valid(effect.status_effect) else null,
            "temp_stats":effect.temp_stats.map(bonus),"use_limit":effect.use_limit,"activates_during_skip":effect.activates_during_skip,
            "armor_percent_to_restore":effect.armor_percent_to_restore,"health_percent_to_restore":effect.health_percent_to_restore,"mana_to_regenerate":effect.mana_to_regenerate}
    return {"id":path(resource),"name":String(resource.name),"texture":resource.get_meta("probe_texture") if resource.has_meta("probe_texture") else null,
        "bb_script":path(resource.bb_script),"socket_type":resource.socket_type.resource_path.get_file().trim_suffix(".tres") if is_instance_valid(resource.socket_type) else null,
        "set_resources":paths(resource.set_resources),"consumption_effect":{"stat_to_gain":bonus(resource.consumption_effect.stat_to_gain),"base_price":resource.consumption_effect.base_price} if is_instance_valid(resource.consumption_effect) else null,
        "activation_effect":activation,"ability_to_learn":path(resource.ability_to_learn),"stat_to_adapt":resource.stat_to_adapt.resource_path.get_file().trim_suffix(".tres") if is_instance_valid(resource.stat_to_adapt) else null,
        "passive":path(resource.passive),"bonus_stats":resource.bonus_stats.map(bonus),
        "unlock_requirements":resource.unlock_requirements.map(func(value):return {"item_resource":path(value.item_resource),"completed_battles":value.completed_battles}),
        "extra_price":resource.extra_price,"spawn_floor":resource.spawn_floor,"hide_level":resource.hide_level,"buy_limit":resource.buy_limit,
        "use_custom_rarity":resource.use_custom_rarity,"custom_rarity":["common","uncommon","rare","legendary","mythic","eternal","abyssal","cosmic","divine"][resource.custom_rarity],
        "reforgable":resource.reforgable,"is_artifact":resource.is_artifact,"is_special":resource.is_special}

func synthetic_items():
    var values: Array = []
    for index in 12:
        var resource = ItemResource.new()
        resource.name = "synthetic_"+str(index)
        resource.spawn_floor = [1,2,18][index%3]
        resource.extra_price = index%4
        resource.is_artifact = index%2==0
        resource.is_special = index==0 or index==9
        if index in [1,5,7,9]:resource.activation_effect = ActivationEffect.new()
        if index in [2,5,8,9]:resource.ability_to_learn = load("res://resources/abilities/triple_attack.tres")
        if index in [3,6,8,9]:resource.stat_to_adapt = load("res://resources/stats/armor.tres")
        if index in [4,6,7,9]:resource.consumption_effect = ConsumptionEffect.new()
        var id = "res://resources/items/synthetic_"+str(index)+".tres"
        ResourceSaver.save(resource,id)
        values.push_back(id)
    return values

func snapshot(items: Items):
    var runtime = {}
    for resource in items.LIST:runtime[path(resource)] = definition(resource)
    return {"list":paths(items.LIST),"market":paths(items.MARKET),"essentials":paths(items.ESSENTIALS),"tomes":paths(items.TOMES),
        "consumables":paths(items.CONSUMABLES),"stat_adapters":paths(items.STAT_ADAPTERS),"artifacts":paths(items.ARTIFACTS),"special":paths(items.SPECIAL),
        "highest_price_per_floor":items.HIGHEST_PRICE_PER_FLOOR,"sets_per_floor":items.SETS_PER_FLOOR.map(paths),"passives_per_floor":items.PASSIVES_PER_FLOOR.map(paths),"definitions":runtime}

func _initialize():
    var args = OS.get_cmdline_user_args()
    var all_paths = synthetic_items() if args[0]=="1" else JSON.parse_string(FileAccess.get_file_as_string("res://input.json"))
    if args[1]=="1":all_paths.reverse()
    System.demo=args[2]=="1"
    File.order=all_paths.map(func(value):return value.get_file())
    var raw = []
    # Keep resources alive to observe the same resource aliases at startup.
    var loaded = all_paths.map(func(value):return load(value))
    for resource in loaded:raw.push_back(definition(resource))
    var items = Items.new()
    items._ready()
    var probes: Array = []
    for floor_number in [-3,-1,0,1,5,15,16,100]:
        probes.push_back({"floor":floor_number,"price":items.get_most_expensive_item_price(floor_number),"resources":paths(items.get_resources(items.LIST,floor_number))})
    var file = FileAccess.open("res://golden.json",FileAccess.WRITE)
    var a = BonusStat.new()
    var b = BonusStat.new()
    a.resource=load("res://resources/stats/armor.tres")
    b.resource=a.resource
    a.boosting_sets.assign([null,null,load("res://resources/item_sets/generic.tres")])
    b.boosting_sets.assign([null,load("res://resources/item_sets/generic.tres")])
    var equalities: Array = []
    for _index in 2:equalities.push_back({"result":a.is_same_as(b),"sets":paths(a.boosting_sets)})
    file.store_string(JSON.stringify({"raw":raw,"order":all_paths,"expected":snapshot(items),"queries":probes,"bonus_equality":equalities})+"\n")
    file.close()
    items.free()
    File.order.clear()
    quit()
