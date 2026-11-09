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
- [ ] **Flasks**, as promised in GAME-DESIGN.md (refilled at Rest rooms).
- [ ] **Utility gems**: 3-4 non-damage actives (block, stun, cleanse, buff).
- [ ] **Early combat**: start with 2 skills, 2-enemy fights from floor 1, and
      make mana bind (Heavy Strike costs 4 against 6 regen today).

## Phase 2: loot choices

- [ ] Pick 1 of 3 rewards after elites and bosses instead of auto-loot.
- [ ] Sell or salvage gear (into gold or orbs). Today gear can only be
      discarded.
- [ ] Merchants sell gear, not only gems and orbs.
- [ ] Prompt when the 20-slot bag is full instead of silently dropping loot.

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
- Re-run `cargo run -p megatower-sim --example bot --release -- 150 12` after
  each phase.
