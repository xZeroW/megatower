# Tower of Loot

## Run

```bash
cargo run -p megatower-client
```

Megatower is the base client, with its existing menus, resolved HUD layout,
pixel-art slots, tooltips, options, audio and room renderer. Its new runs use
`megatower-sim` for labyrinths, map-dependent exits, combat, equipment, gems, shops
and progression. The adapter lives in `crates/megatower-ui/src/labyrinth/`;
Megatower's original simulation does not run alongside it. Adventurer selection
currently chooses a portrait; the first skill gem determines the starting build.

No crates or binaries have been renamed. To run the original simulation for
reference/parity work: `MEGATOWER_LEGACY=1 cargo run -p megatower-client`.

### Controls

- Choose a first gem with 1–3 or a click. In combat, 1–4 use linked skills;
  Space passes and Left/Right/Tab changes target.
- Click a door or use W/A/D/S to move relative to your facing. Enter descends
  from resolved stairs. M shows the floor map; P shows the passive tree.
- The room fills the screen above a bottom bar (hero, skills, actions). Equipment and all 20 bag slots are in a drawer over the left side: open it with I, Tab or the bag button, close it with Esc. Right-click gear to
  equip/unequip. Select a gem, then click equipment to socket it; linked gems
  appear in the original bottom-left slots (right-click to unsocket).
- Crafting orbs (shown under the level as `Rf Au As Rl Rg`) drop from fights, elites, treasure
  and bosses. Select a bag or worn item, then press 1 Reforge (reroll affixes), 2 Augment
  (add an affix), 3 Ascend (normal → magic → rare) or 4 Relink (reroll links). On the passive
  tree (P) select an allocated node and press R to spend a Regret orb and refund its point.
- Merchants also sell two crafting orbs (buttons over the room view, or keys 1 and 2). Right-clicking
  an allocated tree node to refund it now costs an Orb of Regret.
- At a merchant, click stock to buy, right-click stock to lock it, and
  right-click bag gems to sell. At an altar, select a bag gem and upgrade it,
  or click a socketed gem below the equipment. Escape leaves room choices or
  opens the original pause/options menu.

### Gameplay checks

```bash
cargo test -p megatower-client -p megatower-ui -p megatower-sim
```

### Combos

Skills like Shatter, Detonate, Rupture, Toxic Burst and Overload cash in ailments: they hit
hard (and burst the ailment's damage) on a target that is frozen (Shatter), ignited, bleeding, poisoned or
shocked, and use it up. Set a target up with one skill, then pay it off with another. The skill
tooltip shows PRIMED when the selected target is ready.

### Danger

Wandering a floor too long raises Danger (shown under the life bar): enemies you meet grow
stronger until you take the stairs.

### Saving

The run is saved automatically on the map between rooms (never mid-fight or in a shop, shrine,
event or altar: those reopen from the last map state). With a save, Enter the Tower offers
Continue or New run. Dying, or starting a new run, deletes it. The save is `run.ron` beside the
profile.

### Meta progression

Dying pays Legacy points (1, plus 3 per floor beaten and 5 per boss). On the main menu's
Legacy screen, spend them on unlocks: Arc, Venom Dart and Frost Nova join the offered starter gems, and Purse,
Hoarder, Alchemist and Scholar give a head start (gold, orbs, a passive point). Legacy is saved
to `legacy.json` beside the profile.

### Settings

Settings are saved to `~/.config/io.github.xzerow.megatower/settings.toml`
(`$XDG_CONFIG_HOME` is honoured); the profile goes to `~/.local/share/megaloot/profile.json`
(`MEGALOOT_USER_DIR` overrides it).

## QA

```bash
cargo run -p megatower-client --features dev
```

enables the Bevy Remote Protocol with `bevy_brp_extras` on port 15702 (screenshots,
key/mouse injection), e.g.

```bash
curl -s -X POST http://127.0.0.1:15702 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/send_keys","params":{"keys":["Space"]}}'
```

`GAME_AUTOSTART=1` skips the menus to the new run's starter selection;
`GAME_SEED` and `GAME_FLOOR` make the labyrinth reproducible.
`GAME_MOVES` / `GAME_LATE_MOVES` play scripted commands in that run (pick a gem,
walk, fight, shops, rooms, tree; the list is on `scripted_moves` in
`crates/megatower-ui/src/labyrinth/mod.rs`), and `tools/lab-shot.sh OUT.png
[VAR=value ...]` runs the new rules once and saves a screenshot.
The `MEGALOOT_*` screenshot and pointer-injection tooling is shared.
`tools/qa-shot.sh OUT.png [VAR=value ...]` explicitly runs the **legacy reference**
with its original scripted simulation (`MEGALOOT_CAPTURE_FRAMES=n` also writes
`artifacts/frames/`). It is not a screenshot of the new game's rules.

## Architecture

`crates/megatower-client` is the thin binary: `DefaultPlugins`, every plugin
below, and the env-driven QA tooling (`src/dev`). The libraries sit under
`crates/`, and dependencies only point down (enforced by
`crates/megatower-client/tests/architecture.rs`).

| Crate | Role |
| --- | --- |
| `megatower-sim` | target game's labyrinth, combat, equipment, gems, room interactions and progression |
| `megatower-stats` | headless attribute graph (a port of `bevy_gauge`'s core): stats as expressions over other stats, tagged modifiers; used by the sim's `derived` rules |
| `megatower-ui` | original presentation and the `labyrinth` adapter from `megatower-sim` into its HUD/timeline/input |
| `megatower-runtime` | Bevy capability plugins: `audio` (kira buses driven by `MusicCue`/`SfxQueue`/`AudioVolumes`), `input` (`GameAction`, bindings), `render` (off-screen room, HUD layout; `RoomSceneRequest` picks the scene) |
| `megatower-assets` | layouts, room geometry, palette |
| `megatower-core` | simulation (run, combat, items, enemies), settings and profile data, paths, translations |

Lower crates own the resources they consume; the UI writes them.

## Rooms

Room scenes were baked from the original game into one `.glb` per room in
`crates/megatower-runtime/src/render/rooms/`, with the per-mesh materials in
`assets/data/rooms/room-materials.ron` and the lights in `assets/data/rooms/room-scenes.ron`.
The the original game project and its export scripts are gone, so edit those files directly.

**Hot reload.** In dev (debug) builds, edit a room while the game runs: when a `.glb` in
`crates/megatower-runtime/src/render/rooms/` or `assets/data/rooms/room-materials.ron` /
`room-scenes.json` changes on disk, the room is respawned from the new files within half a
second, with no rebuild. Works from any editor that writes those files (Blender, the original game export).
Release builds use the embedded copies; `MEGALOOT_HOT_ROOMS=0` turns it off in dev builds.

## Data files

Everything under `assets/data/` (`layouts/`, `content/`, `rooms/`) is RON, compiled into the binary and parsed with `megatower_core::ron_data::from_str`.

## Runtime assets

The game loads fonts, textures, and audio from the workspace `assets/` folder,
not from the original game project, which is no longer in the repo.
