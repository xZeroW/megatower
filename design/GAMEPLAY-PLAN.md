# Gameplay plan

Outcome of the gameplay review of November 2026, benchmarked against Megaloot
and the Dungeonbound preview. Underkeep (turn-based grid crawler, four-person
party, hand-built dungeons with secrets, traps and puzzles) is a third
reference: not a roguelike, but its fights reward planning around what the
enemy is about to do, and its dungeons hide things worth a detour. Phase 3
takes the first point; hidden rooms and traps are candidates for Phase 4's
floor variety. The build engine (gems, links, affixes, orbs,
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

- [x] Enemies inflict ailments on the hero (makes resistances matter).
      Done: `ailments:` in enemies.ron gives a chance per hit; the ailment is
      built from the damage that got through, so resistances shrink it. Rats
      and ghouls bleed, slimes and spiders poison, cultists and Magma Golems
      ignite, Snow Wraiths chill (35%) and freeze (8%). The hero's ailments
      are listed under the bars. Second Wind's cleanse now has a use.
- [x] New intent kinds: buff, summon, charge-up attack. Done as Rally (all
      enemies deal more damage; Goblin Raider), Summon (Cave Bat calls bats;
      summoned enemies cannot summon and pay nothing) and Charge (a wind-up
      turn, then a heavy hit; a stun during the wind-up cancels the hit;
      Shadow Ghost, Iron Golem). Each has its own marker and a tooltip line.
- [x] One signature mechanic per boss. Warden: winds up Judgement (26-34).
      Ruined King: raises Bone Archers, every hit poisons. Cinder Drake:
      ignites, 50% more damage under half life. Stormwing: charged
      Thunderclap that shocks, then a dive into the shocked hero.
      Open: poison from floor-1 enemies ticks for under 1 damage; freeze on
      the hero is a lost turn with no counter but cold resistance; the
      enemy's wind-up is only readable from the marker and tooltip, there
      is no animation. The AoE gap that summons opened was closed by
      making single-target starters hit harder (see Balance watch).

## Phase 4: endless support

- [x] Best-floor record per starter gem, shown on the title and death screens.
      Done: the profile keeps the deepest floor per starter. It is shown
      under each offered gem where the run starts ("Best: floor 9" / "Not
      played yet"), on the death screen ("New record with Arc! (was floor
      5)") and on the Legacy screen.
- [x] Milestones ("first time past floor 6") that pay Legacy or unlock content.
      Done: reaching floor 4 / 7 / 10 / 13 / 16 / 20 for the first time pays
      5 / 10 / 15 / 20 / 30 / 50 Legacy, once per profile, listed on the death
      screen. The next one is named where the starter is picked.
      Open: milestones only pay points; none unlocks content yet (Phase 5).
- [x] Floor modifiers every few floors ("enemies ignite", "no Rest room",
      "elites doubled"). Done: floors 4 and 7, then every floor from 10, roll
      one of Burning (hits add fire and ignite), Venomous (chaos and poison),
      Barren (no Rest room), Hunted (twice the elites) and Crowded (one more
      enemy per plain fight). The status line names it; hover for the rule.
      Open: every modifier is a penalty; none pays more for the risk, and
      the player cannot see the next floor's modifier before descending.
- [x] Cap the grid size so later floors do not get slower; let scaling do the
      work. Done: 7x7 from floor 5 on (was 8x8 from floor 7).
- [x] Accelerate scaling after floor 9. Target: a good player dies around
      floors 12-20 in 30-45 minutes. Done: floor 9 + n counts as 1 + n/2
      more floors of enemy life and damage. The bot's deaths moved from
      "spread up to floor 20 and beyond" to floors 10-17 (see Balance watch).
      Open: run length in minutes is not measured; the bot is weaker than a
      good player, so the real wall may sit a few floors deeper.

## Phase 5: variety and meta

- [x] Adventurers get a passive and a starting kit (the pick was a portrait).
      Done: `assets/data/game/adventurers.ron` gives each of the 11 a named
      passive and a kit, handed over with the starter gem. Kestrel crit and
      evasion; Umbrath 20% more damage for 15% less life; Rethan 25% more
      damage at full life; Elaris leech; Branric guarding empowers the next
      turn; Gorix life and armour; Thalara mana per kill; Mirelle 12% repeat
      chance; Pellim first skill of a fight repeats; Orvian flasks also
      guard; Nivor crits always inflict ailments. Kits are gold, orbs or a
      gem (Iron Wall, Concussive Blow, ...). Shown under the grid where the
      adventurer is picked and above the starter gems. A click selects an
      adventurer; the run begins with the Start button (or Enter).
      Open: the passive is not shown once the run is on (no line in the
      stats panel); adventurers have no Legacy record of their own.
- [x] Wider Legacy: unlock gems, uniques and events (was 7 unlocks for 124
      points). Done: 17 unlocks for 279 points. The 10 new ones add content
      to the pools: 2 events (Gambler's Table, Sealed Vault), 2 support gems
      (Siphon, Cruelty), 3 boons (The Opening, Harvest, Poise) and 3 uniques
      (Herald's Charm, Duelist's Grips, Brewer's Cap). A locked piece never
      drops, is never sold or offered. The Legacy screen has two columns
      and says what the hovered unlock does.
      Open: milestones still pay points only; a fresh profile sees none of
      the rule-changers except through its adventurer.
- [x] Rule-changing boons and uniques ("first skill each fight repeats",
      "poison can crit") in place of flat stat bumps. Done as five rules any
      modifier source can carry: first skill of a fight repeats, mana per
      enemy a skill kills, guarding gives more damage next turn, crits always
      inflict the skill's ailments, drinking a flask also guards. They are on
      adventurers, 3 boons, 3 uniques and 2 support gems.
      Open: the older boons and uniques are still stat bumps; "poison can
      crit" was not built.

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
- After Phase 3 (the bot guards or stuns a wind-up, cleanses when ailing,
  buys no resistances on purpose): Heavy Strike 41%, Split Shot 68%,
  Fireball 63%, Frost Nova 91%, Arc 87%, Venom Dart 55%. Single-target
  starters lost 13-23 points, AoE ones 1-4. Magma Golem, Iron Golem, Snow
  Wraith and Shadow Ghost are what stands at most deaths.
- After the single-target pass: Heavy Strike 9-15 -> 14-21, Fireball 9-15
  -> 13-20, Venom Dart 9-14 -> 13-20 in total, and the three use 150 / 140 /
  140% of added damage (Frost Nova uses 70%), so one target pays more than
  hitting everyone. Heavy Strike 73%, Split Shot 87%, Fireball 85%, Frost
  Nova 95%, Arc 91%, Venom Dart 81%. The spread is 22 points (50 before the
  pass, 28 before Phase 3). Heavy Strike is still last. The run is now a
  little easier than before Phase 3, which Phase 4's scaling has to take
  back.
- After Phase 4 (7x7 cap, late scaling, floor modifiers), floor 12 cleared:
  Heavy Strike 37%, Split Shot 67%, Fireball 63%, Frost Nova 85%, Arc 83%,
  Venom Dart 53%. Run to floor 20 (`-- 150 20`), nobody clears it; average
  last floor 11.6 (Heavy Strike) to 14.5 (Frost Nova), and 85% of deaths fall
  on floors 10-17, most on 12-15. Before the phase 19-91% cleared floor 20.
  Floor 7 is a new small spike (43 of 900 runs; 13 of 600 before): its
  modifier lands on top of the first iron and magma golems. The spread
  between starters is back to 48 points at floor 12, since the late floors
  favour hitting everything.
- After Phase 5 the starter table is unchanged to the point (the bot has
  no adventurer and a fresh profile, so none of the new content is in its
  pools). Per adventurer (`-- 100 12 adventurers`, all starters pooled, the
  bot does not play around the passive), floor 12 cleared: none 63%, Kestrel
  70%, Umbrath 62%, Rethan 64%, Elaris 69%, Branric 66%, Gorix 69%, Thalara
  61%, Mirelle 69%, Pellim 77%, Orvian 68%, Nivor 70%. Pellim's sure repeat
  is the strongest; Umbrath and Thalara give the bot nothing. The unlockable
  content is not measured: the bot never owns it.
- Re-run `cargo run -p megatower-sim --example bot --release -- 150 12` after
  each phase.
