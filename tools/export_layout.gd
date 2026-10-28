extends SceneTree

var output: String
var visuals: Array = []
var import_paths: Dictionary

func _initialize() -> void:
	output = OS.get_cmdline_user_args()[0]
	import_paths = JSON.parse_string(FileAccess.get_file_as_string("res://import_paths.json"))
	call_deferred("export_layout")

func export_layout() -> void:
	var args = OS.get_cmdline_user_args()
	if args.size() >= 3:
		await export_screen(args[1], args[2], args[3] if args.size() > 3 else "640x360", args[4] if args.size() > 4 else "0x0")
		return
	var packed = load("res://scenes/ui/gameplay_canvas_layer/gameplay_canvas_layer.tscn")
	if packed == null:
		quit(1)
		return
	var scene = packed.instantiate()
	root.add_child(scene)
	var room = scene.get_node("MainScreen/Screens/RoomScreen")
	var actions = load("res://scenes/ui/room_action_containers/battle_room_action_container/battle_room_action_container.tscn").instantiate()
	room.get_node("VBoxContainer/BottomMarginContainer/RoomActionContainerHolder").add_child(actions)
	bind_static_icons(scene)
	await process_frame
	await process_frame
	await process_frame
	var main = scene.get_node("MainScreen")
	main.size = Vector2(640, 360)
	await process_frame
	await process_frame
	walk(main)
	var file = FileAccess.open(output.path_join("assets/data/layouts/godot-layout.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify({"viewport": [640, 360], "visuals": visuals}, "\t"))
	print("Resolved ", visuals.size(), " visible controls using Godot containers")
	quit()

func bind_static_icons(node: Node, keep_label_overrides: bool = false) -> void:
	if node.has_meta("source_text"):
		var label = node.get_node_or_null("NameLabel")
		# Standalone screens keep NameLabel text overrides; the button's own default
		# "BUTTON" must not replace them.
		if label is Label and not (keep_label_overrides and node.get_meta("source_text") == "BUTTON"):
			label.text = node.get_meta("source_text")
	if node.has_meta("source_icon"):
		var icon = node.get_node_or_null("IconTextureRect")
		if icon is TextureRect:
			icon.texture = node.get_meta("source_icon")
	for child in node.get_children():
		bind_static_icons(child, keep_label_overrides)

func walk(node: Node) -> void:
	if node is CanvasItem and not node.is_visible_in_tree():
		return
	if node is Control:
		var rect = node.get_global_rect()
		var entry = {"path": str(node.get_path()), "kind": node.get_class(), "rect": [rect.position.x, rect.position.y, rect.size.x, rect.size.y]}
		if node.material is ShaderMaterial:
			var palette_type = node.material.get_shader_parameter("type")
			if palette_type != null:
				entry["palette"] = palette_type
		if node is TextureRect or node is NinePatchRect:
			var texture: Texture2D = node.texture
			if texture:
				entry["texture"] = texture.resource_path
				if texture is CompressedTexture2D and "::" in texture.resource_path:
					entry["texture"] = import_paths.get(texture.load_path, texture.resource_path)
				if texture is AtlasTexture:
					entry["texture"] = texture.atlas.resource_path
					entry["region"] = [texture.region.position.x, texture.region.position.y, texture.region.size.x, texture.region.size.y]
			if node is NinePatchRect:
				entry["margins"] = [node.patch_margin_left, node.patch_margin_top, node.patch_margin_right, node.patch_margin_bottom]
				if node.region_rect.size.x > 0 and node.region_rect.size.y > 0:
					entry["region"] = [node.region_rect.position.x, node.region_rect.position.y, node.region_rect.size.x, node.region_rect.size.y]
			elif texture and node.stretch_mode in [2, 3, 4, 5]:
				var drawn = texture.get_size()
				if node.stretch_mode in [4, 5]:
					drawn *= min(rect.size.x / drawn.x, rect.size.y / drawn.y)
				if node.stretch_mode in [3, 5]:
					rect.position += (rect.size - drawn) * 0.5
				entry["rect"] = [rect.position.x, rect.position.y, drawn.x, drawn.y]
		if node is Label:
			entry["text"] = node.text
			entry["font_size"] = node.get_theme_font_size("font_size")
		if node is ProgressBar:
			entry["value"] = node.value
			entry["maximum"] = node.max_value
		visuals.append(entry)
	for child in node.get_children():
		walk(child)

## Resolve one screen scene at 640x360 and write artifacts/<name>.json. The
## scene's root Control is stretched over the viewport like the game does.
func export_screen(scene_path: String, name: String, size_text: String, position_text: String) -> void:
	var packed = load(scene_path)
	if packed == null:
		quit(1)
		return
	var scene = packed.instantiate()
	# CanvasLayer children anchor to the window; the headless default is 64x64.
	root.size = Vector2i(int(size_text.split("x")[0]), int(size_text.split("x")[1]))
	root.add_child(scene)
	bind_static_icons(scene, true)
	await process_frame
	if scene is Control:
		var size_parts = size_text.split("x")
		var position_parts = position_text.split("x")
		scene.set_anchors_preset(Control.PRESET_FULL_RECT)
		scene.size = Vector2(float(size_parts[0]), float(size_parts[1]))
		scene.position = Vector2(float(position_parts[0]), float(position_parts[1]))
	var viewport_size = Vector2i(int(size_text.split("x")[0]), int(size_text.split("x")[1]))
	for sub in scene.find_children("*", "SubViewport", true, false):
		sub.size = viewport_size
	for _i in range(6):
		await process_frame
	walk(scene)
	var file = FileAccess.open(output.path_join("assets/data/layouts/" + name + ".json"), FileAccess.WRITE)
	file.store_string(JSON.stringify({"viewport": [640, 360], "visuals": visuals}, "\t"))
	print("Resolved ", visuals.size(), " controls for ", name)
	quit()
