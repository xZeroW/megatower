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

The `MEGALOOT_*` variables in `crates/megatower-dev/src/script.rs` drive scripted runs and screenshots.

## Architecture

A thin app crate composes capability crates; dependencies only point down
(enforced by `crates/megatower-game/tests/architecture.rs`).

| Crate | Role |
| --- | --- |
| `megatower-game` | the `megatower` binary: `DefaultPlugins` plus every plugin below |
| `megatower-frontend` | Bevy front end over a run: screens, HUD composition, input handling, options persistence glue |
| `megatower-dev` | QA tooling: env-driven script driver and screenshots |
| `megatower-render` | off-screen 3D room, Godot-resolved HUD layout (`RoomSceneRequest` selects the scene) |
| `megatower-audio` | kira music/SFX buses, driven by `MusicCue`, `SfxQueue`, `AudioVolumes` |
| `megatower-input` | `GameAction`, key bindings, leafwing input plugin (`ActiveBindings`) |
| `megatower-settings` | `Options` data and persistence group |
| `megatower-profile` | adventurer profile and records |
| `megatower-layout` | Godot-resolved layouts, room geometry, palette |
| `megatower-gameplay` | the simulation (run, combat, items, enemies) |
| `megatower-ui` | tooltip/info content builders and BBCode parsing |
| `megatower-core` | paths, translations, shared utilities |

Lower crates own the resources they consume; the front end writes them.
