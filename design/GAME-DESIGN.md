# New game — design draft

Working title TBD. Turn-based, one hero, build-driven (deep but simplified ARPG-style
builds), exploring hidden labyrinth floors.

## Run loop

1. Start a run: pick a starting class archetype (Str / Dex / Int start point on the tree)
   and **one starter active gem** from several options. No support gems at the start.
2. Each floor is a hidden labyrinth. Enter a room → it is revealed and resolved
   (fight, loot, event…).
3. After a room is resolved the **map** opens: move through any open door. Cleared and
   revealed rooms can be revisited freely.
4. Find the **Stairs** room (guarded by a boss on every 3rd floor) to descend. Deeper
   floors are bigger and roll higher item levels.
5. Death ends the run; meta progression (unlocks) is out of scope for the first slice.

## Labyrinth

- Floor = grid (5×5 on floor 1, growing to ~8×8). Generated as a random spanning tree
  (every room reachable) plus a few extra doors to create loops.
- Visibility per room: `Hidden` → `Seen` (adjacent to a visited room: you see the door
  and a `?`, not the type) → `Visited` (type and contents known).
- Room types: Fight, Elite, Treasure, Merchant (sells gems, see below), Shrine (pick 1 of 3 boons), Rest
  (heal + refill flasks), Event, Gem Altar, Stairs/Boss. Start room is always empty.
- Placement rules: Stairs far from start (BFS distance), at least one Rest and one
  Treasure per floor, Elites off the main path.
- Current numbers (`game-sim/src/labyrinth`): side = 5 + (floor − 1) / 2, max 8; one
  extra door per 8 rooms; elites = 1 + floor / 3; Rest, Treasure, Merchant and Gem Altar
  once per floor; Shrine on half the floors; 1–2 Events; the rest are Fights. You can only
  leave a room once it is resolved.
- Later options: scouting items that reveal room types, a danger clock that makes fights
  harder the longer you wander.

## Combat

- Hero vs. 1–4 enemies. Each turn the hero uses one skill or a flask; enemies then act.
- Enemies telegraph their next action (intent) so builds/defences can be planned.
- Resources: Life, Mana (skill costs, regenerates per turn), optional Energy Shield.
- Turn-based reading of ARPG mechanics: "cast speed" → chance to act twice / repeat,
  durations measured in turns, damage over time ticks at turn end.
- Current implementation (`game-sim/src/combat`, data in `assets/data/game/`):
  - Hero starts at 100 life, 40 mana, 6 mana regen per turn. Enemies repeat their
    intents (Attack / Guard / Heal) in order; guard lasts until their next action.
  - Fights hold 1 enemy on floor 1, up to 3 deeper; elite rooms add an Elite (×2 life,
    30% more damage) and maybe a normal; boss stairs hold the boss. Enemies get 30%
    more life and 20% more damage per floor.
  - Evasion dodges attacks only, capped at 75%. Damage over time ignores mitigation.
  - Ailment numbers are in `game-sim/src/ailment.rs`.
  - `cargo test -p game-sim --test balance -- --ignored --nocapture` prints how each
    unsupported starter gem does per room type. Baseline: floor-1 fights are won, the
    floor-3 boss is not, so progression (supports, items, passives) must carry it.

## Builds

### Damage types
Physical, Fire, Cold, Lightning, Chaos. Enemy and hero resistances (cap 75%), armour vs.
physical, evasion as dodge chance.

Ailments: Ignite (fire DoT), Chill (enemy acts later) / Freeze (skips a turn),
Shock (takes increased damage), Bleed (physical DoT), Poison (stacking chaos DoT).

Damage pipeline (one function, fully unit-tested):
`base → added flat → conversion (Phys → Lightning → Cold → Fire → Chaos, capped 100%)
→ "increased" (summed) → "more" (multiplied) → crit → mitigation (armour / resist)`.
Converted damage keeps the modifiers of the types it passed through.

### Skill gems + supports
- Only weapon, body and helm have sockets (all linked within the piece): weapon & body up
  to 4, helm up to 2. Gloves, boots, rings and amulet are pure stat items. That caps a
  build at 3 skills, one per socketed piece.
- An active gem in a linked group is a skill; supports in the same group modify it.
- Skills and supports carry tags (Attack, Spell, Melee, Projectile, Area, Chain,
  Duration, element tags); a support only applies if its tags match.
- Gems gain XP and level; attribute requirements (Str/Dex/Int) gate them.
- **Starter gem:** the run starts with one active gem picked from several options
  (e.g. 3–4 offered, the class's start region biasing which ones); it starts in a
  linked weapon socket with no supports. Supports must be found or bought.
- **Gem rarity:** Common → Uncommon → Rare → Epic. Rarity is rolled when a gem drops or
  appears in the shop (deeper floors roll higher). Higher rarity raises the gem's base
  values and adds bonus properties (e.g. lower cost, extra ailment chance, an extra
  target). Duplicates do **not** combine: buying or finding copies never raises rarity.
  The only way to upgrade rarity is the Gem Altar room.
- **Gem sources:** combat/elite/treasure drops and the Merchant. Boss and Treasure rooms
  have better odds of supports.
- Starter set: ~6 actives (e.g. heavy strike, split arrow, fireball, frost nova,
  spark-chain, poison dart) and ~8 supports (extra targets, added element, chain,
  elemental focus, leech, echo, ailment chance, increased area).

### Passive tree
- One shared tree of ~50 nodes; Str/Dex/Int starting regions.
- Small nodes (+stats), notables (bigger clusters), ~4 keystones with real trade-offs
  (e.g. skills cost life instead of mana; can't crit but deal more damage).
- 1 point per level; respec costs a currency item.
- Current implementation (`game-sim/src/passives.rs`, data in `assets/data/game/tree.ron`
  with hand-placed node positions): 45 nodes, 3 starts. The start comes from the
  starter gem (spells: Intelligence, melee: Strength, other attacks: Dexterity).
  Keystones: Glass Cannon (40% more damage, 30% less life), Blood Pact (skills cost life,
  15% more life), Steady Hand (no crits, 30% more damage), Spellweaver (spells 25% repeat
  chance, 40% more mana cost). Levels: XP to next = 10·L + 5·L²; an enemy gives
  6 + 4·floor XP (elites ×3, bosses ×8); each level adds 1 point, +5 life, +2 mana.
  Refunds exist in the sim but no screen or currency uses them yet.

### Gem Altar
A special room that upgrades a gem's rarity by one tier for free. Each altar has
limited uses (1 use, maybe 2 on deeper floors), then it is spent; revisiting a spent
altar does nothing. About one altar per floor, rarer than Rest/Treasure, so choosing
which gem to upgrade is a real build decision.

### Merchant
Reuses the existing Merchant room (`Room::Merchant`, `refresh_merchant` in
`megatower-core/src/run/market.rs`): a 5-slot shop with paid refreshes whose price
grows with each refresh, and lockable slots. Instead of spells it stocks **gems**:
a mix of actives and supports from a floor-gated pool, gem level scaling with depth.
**Selling:** gems can be sold back to the merchant, for a fraction of their value
(starting point: 30% of the merchant's price for that gem, level and rarity; tunable). Gems in
sockets must be removed first. Inventory is limited (a fixed grid of slots for unsocketed
gems and items), so selling is the main way to free room and get gold from extra gems.

### Loot
- Rarity: Normal (0 affixes), Magic (1–2), Rare (3–6), Unique (fixed mods).
- Affixes are prefixes/suffixes with tiers gated by item level (= floor depth).
- 4 crafting orbs: reroll affixes, add an affix, upgrade rarity, reroll sockets/links.
- Current implementation (`game-sim/src/items.rs`, `character.rs`, `loot.rs`, data in
  `assets/data/game/items.ron`): 19 bases, 20 affixes with level-gated tiers, 2 uniques.
  Magic items hold at most 1 prefix + 1 suffix, rares 3–6 affixes (3 + 3 max). New items
  roll 1 link, each extra link at 45%. Bag holds 20 items/gems; gems stay socketed when
  gear is swapped. Drops: fights 40% item / 30% gem, elites 1 magic+ item / 60% gem,
  bosses and treasure 2 items + 1 gem; 70% of gem drops are supports, gem level up to
  1 + floor / 2. Runs start with a 2-link Worn Blade (attack starters) or Gnarled Wand
  (spells) and a Leather Vest. Crafting orbs are not in yet.

## Architecture

- New pure-Rust sim crate (no Bevy): `stats` (modifier engine), `damage`, `gems`,
  `items`, `passives`, `labyrinth`, `combat`, `run`. Deterministic seeded RNG,
  unit tests per module. Content in RON under `assets/data/game/`.
- Keep the existing infra: client binary, audio/input/render plugins, settings,
  atomic profile/save writes, QA tooling (`tools/qa-shot.sh`, BRP).
- New screens: Map, Combat, Inventory & Sockets, Passive Tree. Placeholder art of our
  own (the recovered textures/audio belong to the original game and must be replaced
  before anything is shared).
- The ported sim and Godot-parity code are retired once the new loop is playable.

## Phases

1. **Sim foundations** — modifier/stat engine, damage pipeline, tests.
2. **Labyrinth** — generator, visibility, movement; minimal map screen.
3. **Combat** — turn loop, skills from linked gems, enemies with intents; minimal
   combat screen. First playable: walk the map, fight, descend.
4. **Items** — affixes, rarity, drops, inventory and socketing UI.
5. **Passive tree** — leveling, tree data, tree UI.
6. **Content & cleanup** — merchants/shrines/events, balance pass, retire old code.

Vertical-slice target after phase 3–5: 3 floors, ~5 enemy types + 1 boss,
6 actives, 8 supports, a 20-node tree, magic/rare drops.
