extends SceneTree

# Bakes each recovered room scene (CSG shapes become plain meshes) into a .glb
# and writes the surface material parameters of every mesh node next to it.

var out_dir: String
var import_paths: Dictionary
var counter := 0
const ROOMS := ["room_normal", "room_final", "chest_room", "mystic_trader", "monolith_room",
	"altar_room", "room_transition", "room_forest"]

func _initialize() -> void:
	out_dir = OS.get_cmdline_user_args()[0]
	import_paths = JSON.parse_string(FileAccess.get_file_as_string("res://import_paths.json"))
	call_deferred("run")

func run() -> void:
	var materials := {}
	for room in ROOMS:
		var path := "res://scenes/rooms/%s.tscn" % room
		if not ResourceLoader.exists(path):
			continue
		var scene: Node = load(path).instantiate()
		root.add_child(scene)
		await process_frame
		await process_frame
		counter = 0
		materials[room] = bake(scene)
		var doc := GLTFDocument.new()
		var state := GLTFState.new()
		var err := doc.append_from_scene(scene, state)
		if err == OK:
			err = doc.write_to_filesystem(state, "%s/%s.glb" % [out_dir, room])
		print("%s: %s" % [room, error_string(err)])
		root.remove_child(scene)
		scene.free()
	var file := FileAccess.open("%s/room-materials.json" % out_dir, FileAccess.WRITE)
	file.store_string(JSON.stringify(materials, "  "))
	file.close()
	quit()

# Replaces CSG shapes by MeshInstance3D, drops lights/animation, and returns
# {node path: material description}.
func bake(scene: Node) -> Dictionary:
	var described := {}
	var drop: Array = []
	var csgs: Array = []
	collect(scene, csgs, drop)
	for csg in csgs:
		var mesh: Mesh = csg.bake_static_mesh()
		var instance := MeshInstance3D.new()
		instance.mesh = mesh
		instance.transform = csg.transform
		instance.material_override = csg.material
		var parent: Node = csg.get_parent()
		var index: int = csg.get_index()
		instance.name = csg.name
		parent.remove_child(csg)
		csg.free()
		parent.add_child(instance)
		parent.move_child(instance, index)
	for node in drop:
		node.get_parent().remove_child(node)
		node.free()
	describe(scene, scene, described)
	return described

func collect(node: Node, csgs: Array, drop: Array) -> void:
	for child in node.get_children():
		if child is CSGShape3D:
			csgs.append(child)
		elif child is AnimationPlayer or child.scene_file_path.ends_with("animated_light.tscn"):
			drop.append(child)
		else:
			collect(child, csgs, drop)

func describe(scene: Node, node: Node, described: Dictionary) -> void:
	for child in node.get_children():
		if child is MeshInstance3D:
			var key := str(scene.get_path_to(child))
			var material: Material = child.material_override
			if material == null and child.mesh != null and child.mesh.get_surface_count() > 0:
				material = child.get_surface_override_material(0)
			if material == null and child.mesh != null and child.mesh.get_surface_count() > 0:
				material = child.mesh.surface_get_material(0)
			counter += 1
			var info := material_info(material)
			info["node"] = key
			child.name = "m%d" % counter
			described[child.name] = info
		describe(scene, child, described)

func material_info(material: Material) -> Dictionary:
	if material is ShaderMaterial and material.shader != null:
		var params := {}
		for uniform in material.shader.get_shader_uniform_list():
			var value = material.get_shader_parameter(uniform.name)
			if value is CompressedTexture2D:
				value = import_paths.get(value.load_path, value.load_path)
			elif value is Texture2D:
				value = value.resource_path
			elif value is Color:
				value = [value.r, value.g, value.b, value.a]
			elif value is Vector3:
				value = [value.x, value.y, value.z]
			params[uniform.name] = value
		return {"shader": material.shader.resource_path, "params": params}
	if material is StandardMaterial3D:
		var albedo = ""
		if material.albedo_texture is CompressedTexture2D:
			albedo = import_paths.get(material.albedo_texture.load_path, material.albedo_texture.load_path)
		return {"standard": true, "albedo": albedo}
	return {}
