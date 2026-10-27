# Tower of Loot

## Run

```bash
cargo run -p megatower-game
```

Settings are saved to `~/.config/io.github.xzerow.megatower/settings.toml`
(`$XDG_CONFIG_HOME` is honoured); the profile goes to `~/.local/share/megaloot/profile.json`
(`MEGALOOT_USER_DIR` overrides it).

## QA

```bash
cargo run -p megatower-game --features dev
```

enables the Bevy Remote Protocol with `bevy_brp_extras` on port 15702 (screenshots,
key/mouse injection), e.g.

```bash
curl -s -X POST http://127.0.0.1:15702 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/send_keys","params":{"keys":["Space"]}}'
```

The `MEGALOOT_*` variables in `apps/megatower-game/src/dev/script.rs` drive scripted runs and screenshots.
`tools/qa-shot.sh OUT.png [VAR=value ...]` builds, runs the game once with them, saves the screenshot and exits (`MEGALOOT_CAPTURE_FRAMES=n` also writes `artifacts/frames/`).

## Architecture

`apps/megatower-game` is the thin binary: `DefaultPlugins`, every plugin
below, and the env-driven QA tooling (`src/dev`). The libraries sit under
`crates/`, and dependencies only point down (enforced by
`apps/megatower-game/tests/architecture.rs`).

| Crate | Role |
| --- | --- |
| `megatower-ui` | screens, HUD composition, input handling, tooltip content and BBCode, options and profile systems |
| `megatower-runtime` | Bevy capability plugins: `audio` (kira buses driven by `MusicCue`/`SfxQueue`/`AudioVolumes`), `input` (`GameAction`, bindings), `render` (off-screen room, HUD layout; `RoomSceneRequest` picks the scene) |
| `megatower-assets` | Godot-resolved layouts, room geometry, palette |
| `megatower-core` | simulation (run, combat, items, enemies), settings and profile data, paths, translations |

Lower crates own the resources they consume; the UI writes them.

## Rooms

Room scenes are baked by Godot: `python3 tools/export_room_gltf.py` (needs `godot`
4.4.1 on `PATH`) writes one `.glb` per room to `crates/megatower-runtime/src/render/rooms/`
and the per-mesh materials to `artifacts/room-materials.json`. Lights still come from
`tools/export_room_scenes.py` (`artifacts/room-scenes.json`).
