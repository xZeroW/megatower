extends SceneTree

var output: String
var import_paths: Dictionary
var nodes: Array = []

func _initialize() -> void:
	output = OS.get_cmdline_user_args()[0]
	import_paths = JSON.parse_string(FileAccess.get_file_as_string("res://import_paths.json"))
	call_deferred("export_container")

func export_container() -> void:
	var packed = load("res://scenes/ui/enemy_container/enemy_container.tscn")
	if packed == null:
		quit(1)
		return
	var holder = Control.new()
	holder.size = Vector2(352, 304)
	root.add_child(holder)
	var container = packed.instantiate()
	holder.add_child(container)
	await process_frame
	await process_frame
	container.position = Vector2.ZERO
	await process_frame
	walk(container, container)
	var file = FileAccess.open(output.path_join("artifacts/enemy-container-layout.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify({"size": [container.size.x, container.size.y], "nodes": nodes}, "\t"))
	print("Resolved ", nodes.size(), " enemy container nodes")
	quit()

func walk(node: Node, base: Control) -> void:
	if node is Control:
		var rect = node.get_global_rect()
		var entry = {"path": str(base.get_path_to(node)), "kind": node.get_class(), "visible": node.visible, "rect": [rect.position.x - base.global_position.x, rect.position.y - base.global_position.y, rect.size.x, rect.size.y]}
		if node.material is ShaderMaterial:
			var t = node.material.get_shader_parameter("type")
			if t != null:
				entry["palette"] = t
		if node is TextureRect or node is NinePatchRect or node is TextureProgressBar:
			var texture = node.texture if not node is TextureProgressBar else node.texture_progress
			if texture:
				entry["texture"] = texture.resource_path
				if texture is CompressedTexture2D and "::" in texture.resource_path:
					entry["texture"] = import_paths.get(texture.load_path, texture.resource_path)
				if texture is AtlasTexture:
					entry["texture"] = texture.atlas.resource_path if texture.atlas else ""
					entry["region"] = [texture.region.position.x, texture.region.position.y, texture.region.size.x, texture.region.size.y]
				if texture.get_class() == "CompressedTexture2D" or texture is AtlasTexture:
					var size = texture.get_size()
					entry["texture_size"] = [size.x, size.y]
		if node is NinePatchRect:
			entry["margins"] = [node.patch_margin_left, node.patch_margin_top, node.patch_margin_right, node.patch_margin_bottom]
		if node is Label:
			entry["text"] = node.text
			entry["font_size"] = node.get_theme_font_size("font_size")
		if node is ProgressBar:
			entry["value"] = node.value
			entry["maximum"] = node.max_value
		if node is TextureRect:
			entry["pivot"] = [node.pivot_offset.x, node.pivot_offset.y]
		nodes.append(entry)
	for child in node.get_children():
		walk(child, base)
