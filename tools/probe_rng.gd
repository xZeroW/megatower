extends SceneTree

# Executed standalone under Godot 4.4.1; manager points at a staged copy of the
# recovered rng_manager.gd, unchanged. No recovered project/addon/scene is loaded.
func _initialize() -> void:
	var manager = load("res://rng_manager.gd").new()
	var rows: Array = []
	for base_seed in [0, 1, 42, 123456789, -1, -9223372036854775808]:
		for operation in ["randi", "randf", "range", "float_range", "weighted", "pick", "shuffle"]:
			var rng = RandomNumberGenerator.new()
			rng.seed = base_seed
			var values: Array = []
			var states: Array = [str(rng.state)]
			for i in 128:
				match operation:
					"randi": values.append(rng.randi())
					"randf": values.append(PackedFloat32Array([rng.randf()]).to_byte_array().decode_u32(0))
					"range":
						var bounds: Array = [[0, 0], [0, 1], [7, -9], [-10000, 20000], [0, 2147483646], [0, 1000000000]]
						var pair: Array = bounds[i % bounds.size()]
						values.append(rng.randi_range(pair[0], pair[1]))
					"float_range": values.append(PackedFloat32Array([rng.randf_range(-3.25, 8.5)]).to_byte_array().decode_u32(0))
					"weighted":
						var pools: Array = [[1.0, 1.0, 1.0], [0.0, 2.5, 0.125, 10.0], [0.0, 0.0], [1.0], [1e-30, 1e-20, 1e20]]
						values.append(rng.rand_weighted(PackedFloat32Array(pools[i % pools.size()])))
					"pick": values.append(manager.pick_random(rng, 7))
					"shuffle":
						var array: Array = range(i % 20)
						manager.shuffle_array(rng, array)
						values.append(array)
				states.append(str(rng.state))
			rows.append({"seed": str(base_seed), "operation": operation, "values": values, "states": states})
		for operation in ["global", "string", "skew"]:
			seed(base_seed)
			var values: Array = []
			for i in 128:
				match operation:
					"global": values.append(str(PackedFloat64Array([randf()]).to_byte_array().decode_u64(0)))
					"string":
						var lengths: Array = [0, 1, 16, 62, 100, -1, -62]
						values.append(manager.generate_random_string(lengths[i % lengths.size()]))
					"skew": values.append(manager.random_skewed_number(1000))
			rows.append({"seed": str(base_seed), "operation": operation, "values": values, "states": []})
	var file = FileAccess.open("res://golden.json", FileAccess.WRITE)
	file.store_string(JSON.stringify(rows) + "\n")
	file.close()
	manager.free()
	quit()
