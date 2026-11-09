# Gameplay plan

Outcome of the gameplay review of November 2026, benchmarked against Megaloot
and the Dungeonbound preview. The build engine (gems, links, affixes, orbs,
passive tree) is the game's strength. The work below is about giving the player
more decisions around it, not about adding raw content.

**Structure decision:** the game is endless. Floors keep getting harder until
the hero dies. There is no final floor and no win screen.

Evidence the plan is based on: a heuristic bot (hits the weakest enemy, takes
the first boon, never picks a keystone) reaches floor 9 in about 72-80% of runs
and floor 12 in 53-75%. Deaths cluster on boss floors, floor 6 most of all.
AoE starters (Frost Nova, Arc) outscale single-target ones (Heavy Strike).

## Phase 1: decisions per minute

- [x] **Reveal adjacent room types.** Doors currently lead to "?" rooms, while
      the danger clock punishes extra doors. Showing what is behind each door
      turns the clock into a route trade-off.
- [x] **Guard action.** The hero sees enemy intents but can only attack or
      pass. Add a defensive action that answers a telegraphed hit. Done as
      Guard: no skill this turn, enemy hits deal half damage. It replaces
      Pass in the UI.
- [x] **Flasks**, as promised in GAME-DESIGN.md (refilled at Rest rooms).
      Done: the hero carries 2 charges; drinking one in a fight restores 35%
      of maximum life and takes the turn (F). A Rest room refills both, each
      new floor gives one back. Flasks cannot be drunk outside a fight.
- [x] **Utility gems**: 3-4 non-damage actives (block, stun, cleanse, buff).
      Done as an `effect` on skill gems (gems.ron), found as drops: Iron Wall
      (enemy hits deal 80% less this turn), Concussive Blow (small hit, the
      target loses its next action and cannot be stunned again until it has
      acted), Second Wind (removes ailments, restores 15% life) and Battle
      Cry (50% more damage for 3 turns).
      Open: Second Wind's cleanse does nothing until enemies inflict ailments
      (Phase 3); bosses can be stunned like anyone else.
- [x] **Early combat**: start with 2 skills, 2-enemy fights from floor 1, and
      make mana bind. Done: every starter gem has a `partner` gem that starts
      in the vest (its payoff skill; Split Shot gets Venom Dart), fights hold
      1 to 1 + ceil(floor/3) enemies, and the hero has 24 mana with 3 regen
      (was 40 and 6). Shatter now also cashes in Chill, so Frost Nova's
      partner is usable.
      Open: Arc's partner Overload needs Shock, which Arc applies only 20% of
      the time.

## Phase 2: loot choices

- [x] Pick 1 of 3 rewards after elites and bosses instead of auto-loot.
      Done: 2 pieces of gear and 1 gem are offered, one is kept (keys 1-3,
      Escape skips for nothing). Bosses roll better gear than elites (20%
      unique). Gold, XP and orbs are still paid automatically.
- [x] Sell or salvage gear (into gold or orbs). Done: merchants buy gear
      and gems for 30% of their price; Delete salvages a bag slot anywhere
      else for half of that. There is no free discard any more.
- [x] Merchants sell gear, not only gems and orbs. Done: 3 pieces, magic
      or better, 30-300 gold by rarity and +20% per item level.
- [x] Prompt when the 20-slot bag is full instead of silently dropping loot.
      Done: the loot waits in the reward screen; free a slot to take it,
      or leave and it is salvaged.
      Open: a boss used to drop 2 items and a gem and now gives 1 of 3, so
      gear comes in slower; merchant gear and sale gold are the counterweight.
      The reward cells are not dimmed when the bag is full (only the tooltip
      and key label say so).

## Phase 3: enemies that ask questions

- [ ] Enemies inflict ailments on the hero (makes resistances matter).
- [ ] New intent kinds: buff, summon, charge-up attack.
- [ ] One signature mechanic per boss; bosses are only bigger intent loops
      today.

## Phase 4: endless support

- [ ] Best-floor record per starter gem, shown on the title and death screens.
- [ ] Milestones ("first time past floor 6") that pay Legacy or unlock content.
- [ ] Floor modifiers every few floors ("enemies ignite", "no Rest room",
      "elites doubled"). The last new content appears at floor 6 today.
- [ ] Cap the grid size so later floors do not get slower; let scaling do the
      work.
- [ ] Accelerate scaling after floor 9. Target: a good player dies around
      floors 12-20 in 30-45 minutes.

## Phase 5: variety and meta

- [ ] Adventurers get a passive and a starting kit (the pick is a portrait
      today).
- [ ] Wider Legacy: unlock gems, uniques and events (7 unlocks for 124 points
      today).
- [ ] Rule-changing boons and uniques ("first skill each fight repeats",
      "poison can crit") in place of flat stat bumps.

## Cut or simplify

- [ ] Filler fights: cut plain fights to about half of the rooms.
- [ ] Rest-on-entry: make Rest a room the player chooses to use (it is wasted
      at full life today).
- [ ] Dead design: Energy Shield has no source or use; gem XP and attribute
      requirements are in GAME-DESIGN.md but not in the game. Implement or
      delete.
- [ ] Small "+10%" tree nodes: fewer, bigger nodes.
- [ ] Regret orb: fold the respec into the merchant.

## Balance watch

- Floor 6 is the wall (Cinder Drake / Stormwing, iron and magma golems).
- AoE starters reach floor 12 far more often than single-target ones.
- After the early-combat change (150 runs per starter, floor 12 cleared):
  Heavy Strike 53%, Split Shot 69%, Fireball 59%, Frost Nova 81%, Arc 77%,
  Venom Dart 51%. Floor-1 deaths stay at 0-1%. The bot barely notices the
  smaller mana pool because it falls back to its cheaper skill.
- After flasks (same bot, drinks below 40% life): Heavy Strike 65%, Split
  Shot 81%, Fireball 75%, Frost Nova 90%, Arc 85%, Venom Dart 69%. Flasks
  add 8-18 points everywhere, so the Phase 4 scaling has more to push
  against.
- After utility gems (the bot uses them only in plain cases): Heavy Strike
  59%, Split Shot 83%, Fireball 73%, Frost Nova 91%, Arc 91%, Venom Dart
  69%. Within noise of the flask numbers, Arc aside.
- After loot choices (the bot takes the best-scoring gear offer and sells
  spare gear): Heavy Strike 64%, Split Shot 81%, Fireball 79%, Frost Nova
  92%, Arc 91%, Venom Dart 70%. Unchanged within noise.
- Re-run `cargo run -p megatower-sim --example bot --release -- 150 12` after
  each phase.
