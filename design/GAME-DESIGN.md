# New game — design draft

Working title TBD. Turn-based, one hero, build-driven (deep but simplified ARPG-style
builds), exploring hidden labyrinth floors.

> **Presentation status:** the screens described below (from the removed `game-client` prototype)
> are superseded by megatower's HUD driven by `megatower-sim`
> (`crates/megatower-ui/src/labyrinth/`, see the README). The rules and numbers here
> still hold; room view, doors, enemy drawing, bars and combat text now come from
> megatower's own presentation code.

## Run loop

1. Start a run: pick a starting class archetype (Str / Dex / Int start point on the tree)
   and **one starter active gem** from several options. No support gems at the start.
2. Each floor is a hidden labyrinth. Enter a room → it is revealed and resolved
   (fight, loot, event…).
3. After a room is resolved the **map** opens: move through any open door. Cleared and
   revealed rooms can be revisited freely.
4. Find the **Stairs** room (guarded by a boss on every 3rd floor) to descend. Deeper
   floors are bigger and roll higher item levels.
5. The run autosaves on the map (`Run::save`/`load`, `run.ron`) and resumes from Enter the Tower.
   Death ends the run and pays Legacy points (`megatower-sim/src/legacy.rs`): 1 + 3 per floor
   beaten + 5 per boss. They buy permanent unlocks on the Legacy screen (main menu): three more starter
   gems (Arc, Venom Dart, Frost Nova; the first run offers Heavy Strike, Split Shot and
   Fireball) and head starts (+40 / +80 gold, an Ascend and Augment orb, a passive point).
   The profile also keeps the deepest floor reached with each starter gem (shown where the
   starter is picked, on the death screen and on the Legacy screen) and pays milestones once:
   reaching floor 4 / 7 / 10 / 13 / 16 / 20 for the first time gives 5 / 10 / 15 / 20 / 30 /
   50 Legacy on top.

## Labyrinth

- Floor = grid (5×5 on floor 1, one more per side every two floors, 7×7 from floor 5 on:
  deeper floors get harder, not longer). Generated as a random spanning tree
  (every room reachable) plus a few extra doors to create loops.
- Floor modifiers: floors 4 and 7, then every floor from 10, roll one twist, named in the
  status line (hover it for the rule). Burning: enemy hits add 2-4 fire (scaled with depth)
  and ignite 40% of the time. Venomous: 1-3 chaos and 50% poison. Barren: no Rest room.
  Hunted: twice the elites. Crowded: one more enemy in every plain fight.
- Visibility per room: `Hidden` → `Seen` (adjacent to a visited room: you see the door
  and the room's type) → `Visited`.
- Room types: Fight, Elite, Treasure, Merchant (sells gems, see below), Shrine (pick 1 of 3 boons), Rest
  (heal + refill flasks), Event, Gem Altar, Stairs/Boss. Start room is always empty.
- Placement rules: Stairs far from start (BFS distance), at least one Rest and one
  Treasure per floor, Elites off the main path.
- Current numbers (`megatower-sim/src/labyrinth`): side = 5 + (floor − 1) / 2, max 8; one
  extra door per 8 rooms; elites = 1 + floor / 3; Rest, Treasure, Merchant and Gem Altar
  once per floor; Shrine on half the floors; 1–2 Events; the rest are Fights. You can only
  leave a room once it is resolved.
- The room view (`game-client/src/ (removed) room_view.rs`): the hero stands in the 3D room from
  `megatower-runtime` and faces the way they last walked (`Floor::facing`). Doors are
  built in code where the floor has exits: ahead in the baked far doorway (bricked up
  when there is no exit ahead), left and right on the side walls between z -8.0 and
  -5.45 (the middle pillars of the trader / altar / chest / monolith scenes are hidden
  so those doors show). The way back is the "Turn back" button. Click a door (it
  lights up under the cursor) or press W / A / D / S; a short dolly or turn plays,
  then the hero moves. The corner minimap and the full map (M) are for looking only.
  Scenes: treasure `chest_room`, merchant `mystic_trader`, Gem Altar `altar_room`,
  shrine `monolith_room`, everything else `room_normal`.
- Fights happen in the same room view, drawn the way megatower draws enemies
  (`game-client/src/ (removed) fighters.rs`, ported from `megatower-ui` `compose.rs`
  `enemy_widget`): each enemy's sprite (`enemies.ron` `sprite`) over the room with its
  oval shadow, the stats panel from `enemy-container-layout.ron` (pill health bar and
  value label; guard shows as the armor bar), ailment icons above, the selection arrow
  under the target and the next intent beneath. Hits squash and shake the sprite,
  enemy actions lunge it, dodges sidestep it, with megatower's keyframes. Clicking a
  sprite targets it; the target's name, the hero, skills and the log sit in the side
  panel.
  Life bars animate like megatower's (`game-client/src/ (removed) bars.rs`, from `resource_bar.rs`):
  a hit drops the main fill at once while an orange trail keeps the old value, waits
  0.25s, then drains at 2.5x the maximum per second; the hero's life bar does the same.
- **Danger clock** (`Floor::danger`): every door walked on a floor counts. Past 1 door per room,
  each 5 more doors add a level of danger (max 10); enemies met after that have +6% life and
  +5% more damage per level. It shows as "Danger N" in the HUD, resets on each new floor and is
  saved with the run. A bot that clears every room hits level 1+ on 99% of runs, which costs it
  2–3 points of floor-9 clear rate; players who skip rooms avoid it.
- Later option: scouting items that reveal room types.

## Combat

- Hero vs. up to 5 enemies on the field at once, with more waiting in a queue (see below). Each turn
  the hero uses one skill or a flask; enemies then act.
- Enemies telegraph their next action (intent) so builds/defences can be planned.
- Resources: Life, Mana (skill costs, regenerates per turn), optional Energy Shield.
- Turn-based reading of ARPG mechanics: "cast speed" → chance to act twice / repeat,
  durations measured in turns, damage over time ticks at turn end.
- Current implementation (`megatower-sim/src/combat`, data in `assets/data/game/`):
  - Hero starts at 100 life, 24 mana, 3 mana regen per turn, with two skills: the
    starter gem in the weapon and its `partner` (gems.ron) in the vest. A turn is a
    skill, Guard (enemy hits deal half damage that turn) or a flask (35% of
    maximum life; 2 charges, refilled at Rest rooms, one back per new floor,
    `FLASK_CHARGES` / `FLASK_HEAL`). Utility skill gems carry an `effect`
    (gems.ron) in place of, or on top of, damage: Ward (Iron Wall), Stun
    (Concussive Blow), Cleanse (Second Wind), Empower (Battle Cry). Enemies repeat their
    intents in order: Attack, Guard (lasts until their next action), Heal, Rally (every
    enemy deals more damage for a turn or two), Summon (calls another enemy in while the
    field has room; summoned enemies cannot summon and pay no XP or gold) and Charge (a
    wind-up turn before a heavy Attack; a stun during the wind-up cancels the hit too).
  - Enemies inflict ailments on the hero (`ailments:` in enemies.ron, a chance per hit). The
    ailment is built from the damage that got through, so resistances and armour shrink it,
    and Second Wind removes it. A frozen hero loses the turn.
  - Fights hold 1 or 2 enemies on floor 1 and grow with depth: 1 to 1 + ceil(floor/3) on the field (at most 5,
    `MAX_FIELD`) plus up to floor/6 more waiting behind them (at most 3). An elite room holds an
    Elite (×2 life, 30% more damage) and up to 1 + floor/6 normal companions; boss stairs hold the
    boss, with up to floor/6 helpers. Enemies get 60% more life and 35% more damage per floor. Past floor 9 each floor counts
    for more (floor 9 + n adds 1 + n/2 floors' worth, `scaling_depth`), so floor 12 scales
    like floor 15 used to and floor 16 like floor 30: an endless run ends.
    Winning a fight restores 15% of life.
  - Evasion dodges attacks only, capped at 75%. Damage over time ignores mitigation.
  - Ailment numbers are in `megatower-sim/src/ailment.rs`.
  - `cargo test -p megatower-sim --test balance -- --ignored --nocapture` prints how each
    unsupported level-1 starter gem does in single fights.

## Balance

The run logic lives in `megatower-sim/src/run.rs`, so `megatower-sim/src/bot.rs` can play whole
runs through the same commands as the screens: it explores every room, hits the weakest
enemy with its best affordable skill, wears whatever scores higher, sockets fitting
supports, buys supports, takes the best-looking non-keystone passives, the first shrine
boon and the first event choice.

```bash
cargo run -p megatower-sim --example bot --release -- 300 9
```

prints, per starter gem over 300 seeds, how often the bot dies on floor 1, beats the
floor 3 / 6 / 9 bosses, and its average depth and level. Current numbers, after orbs and the content pass
(600 runs, stop after floor 9; the report also lists deaths per floor and the enemies
standing when the hero died):

| Starter | dies f1 | f3 boss | f6 boss | f9 boss | avg level |
| --- | --- | --- | --- | --- | --- |
| Heavy Strike | 0% | 97% | 91% | 86% | 21.5 |
| Split Shot | 2% | 90% | 83% | 80% | 20.4 |
| Fireball | 0% | 94% | 86% | 84% | 21.0 |
| Frost Nova | 1% | 87% | 82% | 80% | 20.2 |
| Arc | 1% | 87% | 83% | 81% | 20.3 |
| Venom Dart | 0% | 92% | 83% | 80% | 20.6 |

The bot full-clears every floor, so it out-levels a player who skips rooms; most of its
deaths come at the floor-3 boss. Changes in this pass: 15% life back after a win (floor-1
deaths were up to 50%), enemy scaling from +30% life / +20% damage per floor to +60% /
+35% (almost nothing died after floor 3), Split Shot, Frost Nova, Arc and Venom Dart
buffed and Heavy Strike trimmed to bring the starters within ~10 points, and shrines
with no blessings left now resolve instead of soft-locking.

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

### Crowds and the queue
Up to five enemies fight at once; the rest of a fight waits in a queue (`+N waiting` in the room
view). At the end of each turn, waiting enemies step up into free places and act from the next turn
(`CombatEvent::Arrive`). Indices never change: a fallen enemy keeps its place in the list and
newcomers are added at the end. The fight is won when the field is empty and the queue is too.
In the room, one to three enemies stand in a row; four are packed closer and five stand in a
zig-zag, every other one a step in front (`enemy_place`). Each waiting enemy shows as a dark
silhouette behind a fighter (the original's next-enemy layer), so ten enemies read as five in front and
five shadows behind. Crowds favour area and chain skills, so
single-target starters got a nudge (Venom Dart's chaos hit 5–8 → 6–9). The bot's floor-9 clear is
75–80% for every starter.

### Combos: setup and payoff
Ailments are the setup; **payoff skills** cash them in. A payoff gem (`payoff:` in `gems.ron`)
hits a target that has one of its `needs` for more damage (`more_damage`), may deal the
ailment's remaining damage at once (`burst`) and uses the ailment up (`consume`). Against an
unprimed target it is a weak plain hit, so you pair it with a setup skill in another socket group
(only weapon, body and helm carry skills, so a combo build is two or three groups):

| Payoff | Needs | Cashes in |
| --- | --- | --- |
| Shatter | Chill or Freeze | +120% damage, uses both up |
| Detonate | Ignite | +50% damage and 150% of the burn left |
| Rupture | Bleed | +25% damage and 200% of the bleed left |
| Toxic Burst | Poison | 130% of every poison stack left |
| Overload | Shock | +100% damage, uses the shock up |

The skill tooltip says what a payoff needs and shows PRIMED when the selected target will pay;
a cash-in pops its name over the enemy and is logged. Payoff gems drop and sell like other
actives; they are never starters. The bot cashes in when primed and sets up otherwise
(80–86% floor-9 clears).

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
- Current implementation (`megatower-sim/src/passives.rs`, data in `assets/data/game/tree.ron`
  with hand-placed node positions): 45 nodes, 3 starts. The start comes from the
  starter gem (spells: Intelligence, melee: Strength, other attacks: Dexterity).
  Keystones: Glass Cannon (40% more damage, 30% less life), Blood Pact (skills cost life,
  15% more life), Steady Hand (no crits, 30% more damage), Spellweaver (spells 25% repeat
  chance, 40% more mana cost). Levels: XP to next = 10·L + 5·L²; an enemy gives
  6 + 4·floor XP (elites ×3, bosses ×8); each level adds 1 point, +5 life, +2 mana.
  Refunds cost an Orb of Regret (R on the tree screen).

### Gem Altar
A special room that upgrades a gem's rarity by one tier for free. Each altar has
limited uses (1 use, maybe 2 on deeper floors), then it is spent; revisiting a spent
altar does nothing. About one altar per floor, rarer than Rest/Treasure, so choosing
which gem to upgrade is a real build decision.

### Content
- Enemies (`enemies.ron`): 13 normal enemies with a `min_floor`, so tougher ones (Blood Ghoul,
  Snow Wraith, Shadow Ghost, Iron and Magma Golem) only show from floor 3–6 on. Bosses: the
  Warden (any boss floor), the Ruined King (from floor 3) and the Cinder Drake and Stormwing
  (from floor 6, resistant to fire and lightning). Elites and fights pick among what has unlocked.
  Each boss has a signature: the Warden winds up Judgement (guard it or stun the wind-up), the
  Ruined King raises Bone Archers and poisons with every hit, the Cinder Drake ignites and
  deals 50% more damage under half life (`enrage:`), Stormwing charges a Thunderclap that
  shocks and then dives into the shocked hero.
  Among normal enemies: rats and ghouls bleed, slimes and spiders poison, cultists and Magma
  Golems ignite, Snow Wraiths chill and sometimes freeze, Cave Bats call more bats, Goblin
  Raiders rally, Shadow Ghosts and Iron Golems wind up heavy hits.
- Events: 9 scenes. Choices may cost gold (`cost`); the Peddler sells orbs and gems, and
  some results give orbs (`Orb`).
- Boons: 12 (adds Ferocity, Bulwark, Thirst, Echoes). Uniques: 7 (Gravewatch, Rimeheart,
  Warden's Plate, Windrunners and Leechtooth join Emberheart and Tempest Loop).
- With these in and the second balance pass, the bot's floor-9 clear rate is 80–86% across the starters.
  Second pass: Magma Golem fire resistance 75 → 55 and life 55 → 50 (it killed more than any
  other enemy), Warden life 160 → 150, Cinder Drake life 175 → 190 and breath 16–23 → 18–26
  (it was the easiest boss), Arc 3–21 → 3–19.

### Merchant
Reuses the existing Merchant room (`Room::Merchant`, `refresh_merchant` in
`megatower-core/src/run/market.rs`): a 5-slot shop with paid refreshes whose price
grows with each refresh, and lockable slots. Instead of spells it stocks **gems**:
a mix of actives and supports from a floor-gated pool, gem level scaling with depth.
**Selling:** gems can be sold back to the merchant, for a fraction of their value
(starting point: 30% of the merchant's price for that gem, level and rarity; tunable). Gems in
sockets must be removed first. Inventory is limited (a fixed grid of slots for unsocketed
gems and items), so selling is the main way to free room and get gold from extra gems.

Implemented (`megatower-sim/src/merchant.rs`): each merchant also stocks 3 pieces of gear,
magic or better (60% / 35% / 5% magic / rare / unique), bought once and untouched by
refreshes. Gear costs 30 / 60 / 130 / 300 gold for normal / magic / rare / unique, +20% per
item level. A merchant buys bag gems and gear for 30% of the price (gear counts the gems
socketed in it). Anywhere else on the map a bag slot can be salvaged for half of that.

### Current room implementation
- Gold: 3 + 2·floor (+0–2) per enemy (elites ×3, bosses ×10); treasure chests hold
  15 + 8·floor plus their loot. Merchant prices: 30 (skills) / 40 (supports) gold, +35%
  per gem level, ×1.6 / ×2.5 / ×4 for Uncommon / Rare / Epic; refresh costs
  (4 + 2·floor) × (refreshes + 1). Stock stays per merchant room while on the floor.
- Shrines offer 3 of 8 boons (`boons.ron`), or can be left without one.
- Events (`events.ron`): 4 scenes with 2–3 choices and weighted outcomes (heal, hurt,
  gold, gem, item, boon, XP).
- Rest, Treasure and plain Stairs resolve on entering; Merchant and Gem Altar resolve on
  entering but can be reopened by walking back in (the altar while it has uses).

Each merchant also stocks 2 crafting orbs (25–60 gold, +15% per floor, Ascension from floor 2;
bought once, not affected by refreshes).

### Loot
- Rarity: Normal (0 affixes), Magic (1–2), Rare (3–6), Unique (fixed mods).
- Affixes are prefixes/suffixes with tiers gated by item level (= floor depth).
  - **Prefixes** are what the hero is and hits with: life, mana, armour, evasion, added damage
    and increased damage (`AffixKind::of_stat`; a test keeps `items.ron` to this rule).
  - **Suffixes** are how it behaves: resistances, crit chance and multiplier, mana regen, leech,
    conversion, penetration, ailments and attributes.
  - Implicits come with the base and are separate from affixes (as in PoE; the tooltip lists them
    first, above a rule): a Bone Amulet's implicit life still allows a Sturdy (life) prefix.
    Among its affixes an item holds at most one modifier of each type (stat, kind and tag condition).
  - Max life and mana are whole numbers (55, not 55.2).
- 4 crafting orbs: reroll affixes, add an affix, upgrade rarity, reroll sockets/links.
- Current implementation (`megatower-sim/src/items.rs`, `character.rs`, `loot.rs`, data in
  `assets/data/game/items.ron`): 19 bases, 20 affixes with level-gated tiers, 2 uniques.
  Magic items hold at most 1 prefix + 1 suffix, rares 3–6 affixes (3 + 3 max). New items
  roll 1 link, each extra link at 45%. Bag holds 20 items/gems; gems stay socketed when
  gear is swapped. Drops: fights 40% item / 30% gem, treasure 2 items + 1 gem.
  Elites and bosses offer a pick instead: 2 pieces of gear (elites 60 / 35 / 5% magic / rare /
  unique, bosses 15 / 65 / 20%) and 1 gem, of which the player keeps one (`Mode::Reward`,
  `Run::take_reward`; skipping pays nothing). Loot that does not fit in the bag waits in the
  same screen: every piece can be taken once a slot is free, and what is left is salvaged
  for gold. 70% of gem drops are supports, gem level up to
  1 + floor / 2. Runs start with a 2-link Worn Blade (attack starters) or Gnarled Wand
  (spells) and a Leather Vest. Crafting orbs (`megatower-sim/src/orbs.rs`) are
  in: Reforge, Augment, Ascend, Relink and Regret (the respec currency). They sit in a stash
  beside the bag, drop from fights (7%), elites (35%), treasure (1) and bosses (2), and work
  between fights on bag or worn items. The bot spends Ascend/Augment on worn gear and Relink
  on 1-link pieces; with that, floor-9 clears moved from ~70–81% to ~77–90%.

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
