extends SceneTree

## Offline diagnostic: render the recovered room scene using Godot itself.
## Run after `tools/resolve_godot_ui.py` has staged the script-free project:
## godot --headless --path /tmp/opencode/megaloot-layout-probe --script tools/probe_room.gd

func _initialize() -> void:
	call_deferred("render_room")

func render_room() -> void:
	var viewport := SubViewport.new()
	viewport.size = Vector2i(352, 304)
	viewport.own_world_3d = true
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(viewport)

	var environment := Environment.new()
	environment.background_mode = Environment.BG_COLOR
	environment.background_color = Color(0.0431373, 0.0588235, 0.0901961, 1.0)
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.ambient_light_color = Color.WHITE
	var world_environment := WorldEnvironment.new()
	world_environment.environment = environment
	viewport.add_child(world_environment)

	var room = load("res://scenes/rooms/room_normal.tscn").instantiate()
	viewport.add_child(room)
	var camera := Camera3D.new()
	camera.fov = 45.0
	camera.position = Vector3(0.0, 1.357, 2.75)
	camera.look_at_from_position(
		camera.position,
		camera.position + Vector3(0.0, -0.182236, -0.983255),
		Vector3.UP
	)
	viewport.add_child(camera)
	camera.current = true

	for _frame in 8:
		await process_frame
	var image := viewport.get_texture().get_image()
	var err := image.save_png("/tmp/opencode/godot-room-normal.png")
	print("Godot room probe saved; error=", err, " size=", image.get_size())
	quit(0 if err == OK else 1)
