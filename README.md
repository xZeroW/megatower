# Tower of Loot

## Run

```bash
cargo run -p megatower-client
```

Megatower is the base client, with its existing menus, resolved HUD layout,
pixel-art slots, tooltips, options, audio and room renderer. Its new runs use
`megatower-sim` for labyrinths, map-dependent exits, combat, equipment, gems, shops
and progression. The adapter lives in `crates/megatower-ui/src/labyrinth/`;
Megatower's original simulation does not run alongside it. The adventurer you pick
brings a passive and a starting kit (shown under the grid when you hover one); the
first skill gem determines the starting build and brings a partner skill with it, so
you start with two.

No crates or binaries have been renamed. To run the original simulation for
reference/parity work: `MEGATOWER_LEGACY=1 cargo run -p megatower-client`.

### Controls

- Click an adventurer to select them and read their passive and kit, then press Start
  (or Enter) to begin; Back returns to the menu.
- Choose a first gem with 1–3 or a click. In combat, 1–4 use linked skills;
  Space guards (no skill, enemy hits deal half damage that turn), F drinks a
  flask (35% of your life, takes the turn; 2 charges, refilled at Rest rooms,
  one back per new floor) and Left/Right/Tab changes target.
- Click a door or use W/A/D/S to move relative to your facing. Enter descends
  from resolved stairs. M shows the floor map; P shows the passive tree.
  Hover a door arrow, or open the map, to see what the next room holds. A Rest room asks
  before it heals you (key 1, or Esc to move on): it stays usable until you do, once per floor.
  Some rooms are empty halls; about half of a floor's rooms are plain fights.
- To cross a floor you have already cleared, click a room on the map (M) or press G (or the
  >> button beside the door arrows) to go to the stairs: you walk through cleared rooms only, in
  one go, and enter the last room as if you had walked in (a shop opens, the boss waits at the
  stairs). Every door still counts for Danger.
- The room fills the screen above a bottom bar (hero, skills, actions). Equipment and all 20 bag slots are in a drawer over the left side: open it with I, Tab or the bag button, close it with Esc. Right-click gear to
  equip/unequip. Right-clicking a bag gem sockets it into the socketed item you have selected
  (the weapon if none is). Select a gem, then click equipment to socket it; linked gems
  appear in the original bottom-left slots (right-click to unsocket).
- Crafting orbs (shown under the level as `Rf Au As Rl Rg`) drop from fights, elites, treasure
  and bosses. Select a bag or worn item, then press 1 Reforge (reroll affixes), 2 Augment
  (add an affix), 3 Ascend (normal → magic → rare) or 4 Relink (reroll links). On the passive
  tree (P) select an allocated node and press R to spend a Regret orb and refund its point.
- Merchants also sell two crafting orbs (buttons over the room view, or keys 1 and 2). Right-clicking
  an allocated tree node to refund it now costs an Orb of Regret.
- Elites and bosses end in a choice: three rewards appear in the bottom bar and you keep one
  (click it or press 1-3; Escape skips). Loot that does not fit in the bag waits the same way:
  free a slot and take it, or leave and it is salvaged for gold.
- Select a bag slot and press Delete to salvage it for gold; a merchant pays twice as much.
  The tooltip of a bag slot shows what it would pay.
- At a merchant, click stock to buy (gems on the left, three pieces of gear on the right),
  right-click a gem in stock to lock it, and right-click bag gems or gear to sell. At an altar, select a bag gem and upgrade it,
  or click a socketed gem below the equipment. Escape leaves room choices or
  opens the original pause/options menu.

### Gameplay checks

```bash
cargo test -p megatower-client -p megatower-ui -p megatower-sim
```

### Support gems

A support gem's tooltip lists everything it does (extra targets, damage, mana cost) and ends
with the tags a skill needs for it to work, e.g. Ricochet: "Hits 2 more enemies, 15% less damage,
25% more mana cost, Tags: Projectile or Chain". A skill gem lists its own tags the same way, and
a support works when the skill has any of its tags. A support linked to a skill it does not work
with is greyed out with a red x in the socket row, says "NO EFFECT on <skill>" when hovered and is
noted in the log; the skill's own tooltip lists the supports that do apply.

### Combos

Skills like Shatter, Detonate, Rupture, Toxic Burst and Overload cash in ailments: they hit
hard (and burst the ailment's damage) on a target that is frozen (Shatter), ignited, bleeding, poisoned or
shocked, and use it up. Set a target up with one skill, then pay it off with another. The skill
tooltip shows PRIMED when the selected target is ready.

Enemies use ailments too: their hits can poison, ignite, bleed, chill, shock or freeze you
(listed under your bars). Resistances shrink them and Second Wind removes them. Watch the
marker under each enemy: "winds up" means a heavy hit comes next turn (guard it, or stun
the enemy to cancel it), an orange "+40%" is a rally that makes every enemy hit harder, and
a purple "call" brings another enemy in. Hover an enemy to read its next move.

### Danger

Wandering a floor too long raises Danger (shown under the life bar): enemies you meet grow
stronger until you take the stairs.

Floors 4 and 7, and every floor from 10, have a modifier, named in purple on the top line
(hover it): Burning and Venomous enemies ignite or poison, Barren floors have no Rest room,
Hunted floors twice the elites, Crowded floors bigger fights. The tower has no last floor:
past floor 9 enemies grow faster with every floor.

### Saving

The run is saved automatically on the map between rooms (never mid-fight or in a shop, shrine,
event or altar: those reopen from the last map state). With a save, Enter the Tower offers
Continue or New run. Dying, or starting a new run, deletes it. The save is `run.ron` beside the
profile.

### Meta progression

Dying pays Legacy points (1, plus 3 per floor beaten and 5 per boss). On the main menu's
Legacy screen, spend them on unlocks: Arc, Venom Dart and Frost Nova join the offered starter gems, and Purse,
Hoarder, Alchemist and Scholar give a head start (gold, orbs, a passive point). Ten more add
content to every later run: two events, two support gems, three boons and three uniques, most
of them rule changes ("your first skill in each fight repeats") rather than stats. Until bought
they never drop or appear. Hover an unlock to read what it does. Reaching floor
4, 7, 10, 13, 16 or 20 for the first time pays a bonus (5 to 50 points). The profile keeps your
best floor with each starter gem, shown when you pick one and when you die. Legacy is saved
to `legacy.json` beside the profile.

### Log file

Everything the console prints (and any panic, with a backtrace) is also written to
`%APPDATA%\megatower\logs\megatower.log` on Windows (`~/.local/share/megatower/logs/` elsewhere,
honouring `XDG_DATA_HOME`). The previous run is kept as `megatower.previous.log`. It starts with the
OS, CPU and GPU, and every window mode, resolution or vsync change is logged before it is applied, so
the last line says what was being done at a crash. `RUST_LOG=info,bevy_winit=debug,wgpu=warn` writes
more.

On Windows the Fullscreen option is shown as borderless fullscreen: exclusive fullscreen crashed the
game there (set `MEGATOWER_EXCLUSIVE_FULLSCREEN=1` to try it again).

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
`GAME_SEED` and `GAME_FLOOR` make the labyrinth reproducible; `GAME_ADVENTURER=Gorix`
picks the adventurer (Kestrel otherwise).
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
For release downloads, extract the whole archive and keep `assets/` next to the
game executable; copying the executable alone will not include these files.
