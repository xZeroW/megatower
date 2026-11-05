//! Populate Megatower's existing 640x360 HUD, not a second UI layout.
use super::*;
use crate::compose::{self, Align, Draw, Seg, TipView, WHITE, hex, image, palette, text, tinted};
use megatower_sim::labyrinth::Dir;
use megatower_sim::passives::NodeKind;
use megatower_sim::{
    gems::{Gem, GemRarity},
    items::{Item, ItemRarity, Slot},
    labyrinth::Visibility,
};

struct Choice {
    rect: R,
    action: Action,
    label: String,
    icon: Option<&'static str>,
    tip: Vec<String>,
    enabled: bool,
}

fn choice(rect: R, action: Action, label: impl Into<String>, tip: impl Into<String>) -> Choice {
    Choice {
        rect,
        action,
        label: label.into(),
        icon: None,
        tip: vec![tip.into()],
        enabled: true,
    }
}

fn wrap(value: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

// Same equipment order and 20 bag cells as the recovered scene. Only the
// game's gloves replace Megatower's leggings slot.
const EQUIPMENT: [EquipSlot; 8] = [
    EquipSlot::Helm,
    EquipSlot::Body,
    EquipSlot::Gloves,
    EquipSlot::Boots,
    EquipSlot::Amulet,
    EquipSlot::Weapon,
    EquipSlot::Ring1,
    EquipSlot::Ring2,
];

fn art(item: &Item) -> &'static str {
    match item.base.as_str() {
        "gnarled_wand" | "crystal_staff" => "assets/textures/items/magic_wand.png",
        "war_axe" => "assets/textures/items/training_axe.png",
        "silk_robe" => "assets/textures/items/shamans_robe.png",
        _ => match item.slot {
            Slot::Weapon => "assets/textures/items/training_sword.png",
            Slot::Body => "assets/textures/items/leather_chestplate.png",
            Slot::Helm => "assets/textures/items/leather_helmet.png",
            Slot::Gloves => "assets/textures/items/unchained_gauntlet.png",
            Slot::Boots => "assets/textures/items/leather_boots.png",
            Slot::Ring => "assets/textures/items/silver_ring.png",
            Slot::Amulet => "assets/textures/items/amulet_of_life.png",
        },
    }
}
fn rarity(loot: &Loot) -> i32 {
    match loot {
        Loot::Item(i) => match i.rarity {
            ItemRarity::Normal => 0,
            ItemRarity::Magic => 2,
            ItemRarity::Rare => 4,
            ItemRarity::Unique => 3,
        },
        Loot::Gem(g) => match g.rarity {
            GemRarity::Common => 0,
            GemRarity::Uncommon => 1,
            GemRarity::Rare => 2,
            GemRarity::Epic => 3,
        },
    }
}
fn loot_draw(out: &mut Vec<Draw>, rect: R, loot: &Loot) {
    out.push(image(
        rect,
        megatower_core::items::rarity::ItemRarity::texture(rarity(loot)),
    ));
    let inner = [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0];
    match loot {
        Loot::Item(item) => out.push(image(inner, art(item))),
        Loot::Gem(_) => {
            // socket.png is a slot frame, not a gem icon. Drawing it here
            // nested a second outline over the rarity border.
            out.push(image(inner, "assets/textures/items/rune.png"));
        }
    }
}

fn selection_outline(rect: R) -> Draw {
    Draw::Image {
        rect: [rect[0] - 1.0, rect[1] - 1.0, 36.0, 36.0],
        path: "assets/textures/ui/item_slot_outline.png".into(),
        region: Some([0.0, 0.0, 36.0, 36.0]),
        tint: palette(8),
        slice: None,
        flip_x: false,
    }
}

/// What an enemy's next move does, in a few characters: "8-12" for an attack
/// (with its damage modifiers and chill applied), "+100" for a guard, "20%"
/// for a heal.
/// Width of the hero's mana bar next to the health bar (canvas pixels).
const MANA_BAR_WIDTH: f32 = 40.0;

/// Side of a passive node's square on the tree panel (canvas pixels).
fn tree_node_size(id: &str) -> f32 {
    match content::get().tree.node(id).map(|n| n.kind) {
        Some(NodeKind::Small) | None => 7.0,
        Some(NodeKind::Start) => 11.0,
        Some(NodeKind::Notable) => 10.0,
        Some(NodeKind::Keystone) => 18.0,
    }
}

/// Both ends allocated: the connection is part of the character's path.
fn progress_lit(game: &Run, a: &str, b: &str) -> bool {
    game.character.progress.is_allocated(a) && game.character.progress.is_allocated(b)
}

/// The picture a notable or keystone carries: its own keystone art, or the
/// icon of the first thing it changes.
fn tree_node_icon(node: &megatower_sim::passives::NodeDef) -> &'static str {
    use megatower_sim::ailment::Ailment as A;
    use megatower_sim::modifier::Stat;
    use megatower_sim::tags::Tag;
    const ICONS: &str = "assets/textures/icons/";
    let keystone = match node.id.as_str() {
        "glass_cannon" => Some("frailty_keystone"),
        "blood_pact" => Some("resentment_keystone"),
        "spellweaver" => Some("prodigality_keystone"),
        "steady_hand" => Some("preservation_keystone"),
        _ => None,
    };
    if let Some(name) = keystone {
        return match name {
            "frailty_keystone" => "assets/textures/icons/frailty_keystone.png",
            "resentment_keystone" => "assets/textures/icons/resentment_keystone.png",
            "prodigality_keystone" => "assets/textures/icons/prodigality_keystone.png",
            _ => "assets/textures/icons/preservation_keystone.png",
        };
    }
    let _ = ICONS;
    let Some(m) = node.mods.first() else {
        return "assets/textures/icons/wisdom_icon.png";
    };
    match m.stat {
        Stat::Life | Stat::EnergyShield => "assets/textures/icons/max_health_icon.png",
        Stat::Mana | Stat::ManaRegen | Stat::ManaCost => "assets/textures/icons/mana_icon.png",
        Stat::Armour => "assets/textures/icons/guard_icon.png",
        Stat::Evasion => "assets/textures/icons/elusive_icon.png",
        Stat::Damage if m.requires.has(Tag::Spell) => "assets/textures/icons/magic_damage_icon.png",
        Stat::Damage | Stat::AddedMin(_) | Stat::AddedMax(_) => {
            "assets/textures/icons/attack_icon.png"
        }
        Stat::Convert(..) => "assets/textures/icons/power_icon.png",
        Stat::CritChance | Stat::CritMultiplier => "assets/textures/icons/execution_icon.png",
        Stat::Resistance(_) | Stat::MaxResistance(_) => {
            "assets/textures/icons/magic_shield_icon.png"
        }
        Stat::Penetration(_) => "assets/textures/icons/magic_shield_break_icon.png",
        Stat::AilmentChance(A::Ignite) => "assets/textures/icons/magma_icon.png",
        Stat::AilmentChance(A::Bleed) => "assets/textures/icons/bleed.png",
        Stat::AilmentChance(A::Poison) => "assets/textures/icons/toxicity_icon.png",
        Stat::AilmentChance(A::Shock) => "assets/textures/icons/electro_charge.png",
        Stat::AilmentChance(A::Chill) => "assets/textures/icons/slowness_icon.png",
        Stat::AilmentChance(A::Freeze) => "assets/textures/icons/freeze_icon.png",
        Stat::LifeLeech => "assets/textures/icons/omni_vamp.png",
        _ => "assets/textures/icons/fury_icon.png",
    }
}

/// Size of the map arrow pointing north (width, height), in canvas pixels.
const ARROW_SIZE: (f32, f32) = (7.0, 10.0);

/// The rectangles of an arrow pointing `facing`, relative to its own top-left
/// corner. North is a four-row head over a stem; the others are the same
/// arrow turned, so the box is 7x10 or 10x7.
fn arrow_rects(facing: megatower_sim::labyrinth::Dir) -> Vec<R> {
    use megatower_sim::labyrinth::Dir;
    let (w, h) = ARROW_SIZE;
    // Head rows 1, 3, 5, 7 wide, then a 3-wide stem.
    let mut up: Vec<R> = (0..4)
        .map(|i| {
            let width = 1.0 + 2.0 * i as f32;
            [(w - width) / 2.0, i as f32, width, 1.0]
        })
        .collect();
    up.push([2.0, 4.0, 3.0, h - 4.0]);
    up.into_iter()
        .map(|[x, y, rw, rh]| match facing {
            Dir::North => [x, y, rw, rh],
            Dir::South => [x, h - y - rh, rw, rh],
            Dir::East => [h - y - rh, x, rh, rw],
            Dir::West => [y, w - x - rw, rh, rw],
        })
        .collect()
}

/// The experience bar under the portrait (canvas pixels).
const XP_BAR: R = [16.0, 87.0, 36.0, 5.0];

fn intent_amount(enemy: &megatower_sim::combat::Enemy) -> String {
    use megatower_sim::combat::IntentKind;
    match &enemy.intent().kind {
        IntentKind::Attack(spec) => {
            let scale = enemy
                .fighter
                .mods
                .more(megatower_sim::modifier::Stat::Damage, spec.tags)
                * enemy.fighter.afflictions.damage_dealt_multiplier();
            let min: f64 = spec.base.iter().map(|(_, r)| r.min).sum();
            let max: f64 = spec.base.iter().map(|(_, r)| r.max).sum();
            format!("{:.0}-{:.0}", min * scale, max * scale)
        }
        IntentKind::Guard { armour } => format!("+{armour:.0}"),
        IntentKind::Heal { percent } => format!("{percent:.0}%"),
    }
}

fn tip_lines(loot: &Loot) -> Vec<String> {
    match loot {
        Loot::Item(item) => {
            // The slot is told by the item's look, so only its level is listed.
            let mut lines = vec![item.name.clone(), format!("Level {}", item.level)];
            // The base's implicits first, then a rule, then affixes or unique mods.
            lines.extend(megatower_sim::describe::mod_lines(&item.implicit));
            let rest: Vec<_> = item
                .affixes
                .iter()
                .flat_map(|a| a.mods.iter())
                .chain(item.fixed.iter())
                .copied()
                .collect();
            if !item.implicit.is_empty() && !rest.is_empty() {
                lines.push("-----".into());
            }
            lines.extend(megatower_sim::describe::mod_lines(&rest));
            if let Some(s) = &item.sockets {
                lines.push(format!("{} linked sockets", s.links));
                lines.extend(s.gems.iter().map(|g| megatower_sim::run::gem_name(&g.id)));
            }
            lines
        }
        Loot::Gem(g) => {
            let mut lines = vec![
                megatower_sim::run::gem_name(&g.id),
                format!("Level {}", g.level),
            ];
            if let Some(def) = content::get().gems.active(&g.id) {
                lines.push(format!("{:.0} mana / {:?}", def.cost, def.targeting));
                if let Some(payoff) = &def.payoff {
                    lines.push(megatower_sim::describe::payoff_line(payoff));
                }
                lines.extend(
                    def.base
                        .iter()
                        .map(|(kind, r)| format!("{:.0}-{:.0} {kind:?}", r.min, r.max)),
                );
            } else if let Some(def) = content::get().gems.support(&g.id) {
                lines.extend(megatower_sim::describe::mod_lines(&def.mods));
            }
            lines
        }
    }
}
fn tooltip(out: &mut Vec<Draw>, rects: &Rects, cursor: [f32; 2], lines: Vec<String>) {
    compose::draw_tip(
        out,
        rects,
        cursor,
        TipView {
            rows: lines
                .into_iter()
                .enumerate()
                .map(|(i, s)| vec![Seg::Text(s, if i == 0 { palette(8) } else { WHITE })])
                .collect(),
            icons: vec![],
            price: None,
            hint: None,
        },
    );
}

/// Extra height of the action panel at the bottom right, taken from the room
/// frame. The panel is 63 px tall in the recovered layout, level with the left
/// one; a positive value makes it taller.
pub(super) const ACTION_GROWTH: f32 = 0.0;

/// The action panel, canvas pixels.
pub(super) const ACTION_AREA: R = [310.0, 295.0 - ACTION_GROWTH, 328.0, 63.0 + ACTION_GROWTH];

/// Buttons laid out for the original 63 px panel sit this much higher, to stay
/// centred in the taller one.
const ACTION_SHIFT: f32 = -ACTION_GROWTH / 2.0;

/// A rect of the original panel moved to the centre of the taller one.
fn in_action_area(r: R) -> R {
    [r[0], r[1] + ACTION_SHIFT, r[2], r[3]]
}

/// A button of the given size centred vertically in the action area.
fn centred(x: f32, w: f32, h: f32) -> R {
    [
        x,
        (ACTION_AREA[1] + (ACTION_AREA[3] - h) / 2.0).round(),
        w,
        h,
    ]
}

/// The movement keys as an inverted T, W above S: ahead on top, then left,
/// back and right.
fn walk_button(turn: Turn) -> (R, &'static str) {
    // Two rows fit the 63 px panel's 57 px of inside; 26 px leaves the 16 px
    // arrow exactly centred.
    const W: f32 = 40.0;
    const H: f32 = 26.0;
    const GAP_X: f32 = 4.0;
    const GAP_Y: f32 = 2.0;
    let centre = ACTION_AREA[0] + ACTION_AREA[2] / 2.0;
    let top = ACTION_AREA[1] + (ACTION_AREA[3] - (2.0 * H + GAP_Y)) / 2.0;
    let (col, row, icon) = match turn {
        Turn::Ahead => (0.0, 0.0, "assets/textures/ui/move_ahead.png"),
        Turn::Left => (-1.0, 1.0, "assets/textures/ui/move_left.png"),
        Turn::Back => (0.0, 1.0, "assets/textures/ui/move_back.png"),
        Turn::Right => (1.0, 1.0, "assets/textures/ui/move_right.png"),
    };
    (
        [
            (centre - W / 2.0 + col * (W + GAP_X)).round(),
            (top + row * (H + GAP_Y)).round(),
            W,
            H,
        ],
        icon,
    )
}

impl Labyrinth {
    fn choices(&self, rects: &Rects) -> Vec<Choice> {
        let game = self.game();
        let mut out = Vec::new();
        let icon = |rect, action, path, tip: &str| Choice {
            rect,
            action,
            label: String::new(),
            icon: Some(path),
            tip: vec![tip.into()],
            enabled: true,
        };
        match &game.mode {
            Mode::Starter(offers) => {
                for (i, id) in offers.iter().enumerate() {
                    out.push(choice(
                        [170.0 + i as f32 * 110.0, 138.0, 80.0, 70.0],
                        Action::Starter(i),
                        megatower_sim::run::gem_name(id),
                        "Choose this first skill gem",
                    ));
                }
            }
            Mode::Map => {
                for turn in [Turn::Ahead, Turn::Left, Turn::Back, Turn::Right] {
                    let exit = game.floor.turns().into_iter().find(|(t, _)| *t == turn);
                    let (rect, arrow) = walk_button(turn);
                    let mut c = choice(
                        rect,
                        Action::Walk(turn),
                        "",
                        exit.as_ref()
                            .and_then(|(_, e)| e.known)
                            .map_or("Unexplored room", megatower_sim::run::room_name),
                    );
                    c.icon = Some(arrow);
                    c.enabled = exit.is_some()
                        && game.floor.current_room().cleared
                        && matches!(self.view, View::Profile | View::Map);
                    out.push(c);
                }
                if matches!(game.floor.current_room().kind, RoomKind::Stairs { .. }) {
                    out.push(choice(
                        centred(554.0, 76.0, 34.0),
                        Action::Descend,
                        "Descend",
                        "Enter the next floor",
                    ));
                }
            }
            Mode::Combat => {
                out.push(icon(
                    centred(322.0, 44.0, 34.0),
                    Action::Pass,
                    "assets/textures/icons/timeout_icon.png",
                    "Pass turn [Space]",
                ));
                if let Some(battle) = &game.battle {
                    for (i, skill) in battle.skills.iter().enumerate() {
                        let cost = battle.skill_cost(i).unwrap_or(0.0);
                        let resource = if battle.pays_with_life() {
                            "life"
                        } else {
                            "mana"
                        };
                        let mut c = choice(
                            centred(380.0 + i as f32 * 86.0, 78.0, 34.0),
                            Action::Skill(i),
                            format!("{}", i + 1),
                            skill.name.clone(),
                        );
                        c.label.clear();
                        c.icon = Some(if skill.tags.has(megatower_sim::tags::Tag::Attack) {
                            "assets/textures/icons/attack_icon.png"
                        } else {
                            "assets/textures/icons/magic_damage_icon.png"
                        });
                        c.tip
                            .extend([format!("{cost:.0} {resource}"), skill.supports.join(", ")]);
                        if let Some(payoff) = &skill.payoff {
                            c.tip.push(megatower_sim::describe::payoff_line(payoff));
                            let primed = battle.enemies.get(self.target).is_some_and(|e| {
                                payoff.needs.iter().any(|&a| e.fighter.afflictions.has(a))
                            });
                            if primed {
                                c.tip.push("PRIMED: the target will pay".into());
                            }
                        }
                        c.enabled = if battle.pays_with_life() {
                            battle.hero.life > cost
                        } else {
                            battle.hero.mana >= cost
                        };
                        out.push(c);
                    }
                }
            }
            Mode::Shop => {
                out.push(icon(
                    in_action_area(rects.merchant_leave),
                    Action::Leave,
                    "assets/textures/ui/leave_icon.png",
                    "Leave merchant [Esc]",
                ));
                let mut refresh = icon(
                    in_action_area(rects.merchant_refresh),
                    Action::Refresh,
                    "assets/textures/icons/refresh_icon.png",
                    "Refresh unlocked gems",
                );
                if let Some(shop) = game.shop() {
                    refresh.tip.push(format!("{} gold", shop.refresh_cost()));
                    refresh.enabled = game.character.gold >= shop.refresh_cost();
                    for (i, (rect, gem)) in rects.merchant.iter().zip(&shop.stock).enumerate() {
                        if let Some(g) = gem {
                            let price = megatower_sim::merchant::gem_price(&content::get().gems, g);
                            let mut c = choice(
                                in_action_area(*rect),
                                Action::Buy(i),
                                "",
                                megatower_sim::run::gem_name(&g.id),
                            );
                            c.tip = tip_lines(&Loot::Gem(g.clone()));
                            c.tip.push(format!("{price} gold"));
                            c.enabled = game.character.gold >= price;
                            out.push(c);
                        }
                    }
                }
                out.push(refresh);
                if let Some(shop) = game.shop() {
                    for (i, orb) in shop.orbs.iter().enumerate() {
                        let Some(orb) = *orb else { continue };
                        let price = megatower_sim::merchant::orb_price(orb, shop.floor);
                        let mut c = choice(
                            [340.0 + i as f32 * 150.0, 30.0, 140.0, 29.0],
                            Action::BuyOrb(i),
                            format!("{} {price}g [{}]", orb.short(), i + 1),
                            orb.name(),
                        );
                        c.tip
                            .extend([orb.describe().to_string(), format!("{price} gold")]);
                        c.enabled = game.character.gold >= price;
                        out.push(c);
                    }
                }
            }
            Mode::Shrine(offers) => {
                out.push(icon(
                    in_action_area(rects.merchant_leave),
                    Action::Leave,
                    "assets/textures/ui/leave_icon.png",
                    "Leave shrine [Esc]",
                ));
                for (i, id) in offers.iter().enumerate() {
                    let name = content::get()
                        .boons
                        .iter()
                        .find(|b| b.id == *id)
                        .map_or(id.as_str(), |b| b.name.as_str());
                    out.push(choice(
                        in_action_area([380.0 + i as f32 * 76.0, 313.0, 68.0, 29.0]),
                        Action::Boon(i),
                        format!("{}", i + 1),
                        name,
                    ));
                }
            }
            Mode::Event { id, outcome } => {
                if outcome.is_some() {
                    out.push(choice(
                        in_action_area([425.0, 313.0, 100.0, 29.0]),
                        Action::Leave,
                        "Continue",
                        "Leave event",
                    ));
                } else if let Some(event) = content::get().events.iter().find(|e| e.id == *id) {
                    for (i, c) in event.choices.iter().enumerate() {
                        let mut button = choice(
                            in_action_area([365.0 + i as f32 * 80.0, 313.0, 72.0, 29.0]),
                            Action::Event(i),
                            format!("{}", i + 1),
                            c.label.clone(),
                        );
                        if c.cost > 0 {
                            button.tip.push(format!("{} gold", c.cost));
                            button.enabled = game.character.gold >= c.cost;
                        }
                        out.push(button);
                    }
                }
            }
            Mode::Altar => {
                out.push(icon(
                    in_action_area(rects.merchant_leave),
                    Action::Leave,
                    "assets/textures/ui/leave_icon.png",
                    "Leave gem altar [Esc]",
                ));
                // Select any bag gem, or a gem in the linked sockets below
                // the equipment. The altar uses the simulation's own rules.
                if let Some(Selection::Bag(i)) = self.selection
                    && matches!(game.character.inventory.get(i), Some(Loot::Gem(_)))
                {
                    out.push(choice(
                        in_action_area([420.0, 313.0, 125.0, 29.0]),
                        Action::Upgrade(GemSpot::Bag(i)),
                        "Upgrade gem",
                        "Spend one altar use",
                    ));
                }
            }
            Mode::Dead => {}
        }
        if game.mode == Mode::Map {
            out.push(icon(
                rects.market_button,
                Action::Tree,
                "assets/textures/icons/wisdom_icon.png",
                "Passive tree [P]",
            ));
            // At the left of the action area, where combat keeps its pass button.
            out.push(icon(
                centred(322.0, 44.0, 34.0),
                Action::Map,
                "assets/textures/icons/floor_icon.png",
                "Floor map [M]",
            ));
        }
        out
    }

    pub(super) fn pointer(&mut self, ui: &mut UiState, rects: &Rects, input: &FrameInput) {
        let Some([x, y]) = ui.cursor else { return };
        if self.game().mode == Mode::Dead {
            if contains(compose::new_run_rect(), x, y) {
                self.start(self.portrait.clone());
                ui.timeline = Timeline::default();
            }
            return;
        }
        if let Some(c) = self
            .choices(rects)
            .into_iter()
            .find(|c| contains(c.rect, x, y))
        {
            if input.alt_click
                && let Action::Buy(i) = c.action
            {
                let pos = self.game().floor.current;
                if let Some(shop) = self.game_mut().shops.get_mut(&pos) {
                    shop.locked[i] = !shop.locked[i];
                }
            } else if input.click && c.enabled {
                self.act(ui, c.action);
            }
            return;
        }
        if matches!(self.game().mode, Mode::Starter(_)) {
            return;
        }
        if input.click {
            if let Some(i) = rects.tabs.iter().position(|r| contains(*r, x, y)) {
                ui.tab = [Tab::Profile, Tab::Stats, Tab::Log][i];
                self.view = View::Profile;
                return;
            }
            let frame = ui
                .timeline
                .frame
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.snapshot());
            if self.game().mode == Mode::Combat
                && let Some(i) = compose::hit_enemy(&frame, x, y)
            {
                self.target = i;
                return;
            }
        }
        if self.view == View::Tree {
            if let Some(id) = self.tree_hit(x, y) {
                self.tree_selection = Some(id.clone());
                self.act(
                    ui,
                    if input.alt_click {
                        Action::Refund(id)
                    } else {
                        Action::Allocate(id)
                    },
                );
            }
            return;
        }
        if self.view != View::Profile || ui.tab != Tab::Profile {
            return;
        }
        if let Some(i) = rects.inventory.iter().position(|r| contains(*r, x, y)) {
            self.selection = Some(Selection::Bag(i));
            if input.alt_click {
                if self.game().mode == Mode::Shop {
                    self.game_mut().sell(i);
                } else if self.game().mode == Mode::Map {
                    if matches!(self.game().character.inventory.get(i), Some(Loot::Item(_))) {
                        let _ = self.game_mut().character.equip(i, None);
                        self.selection = None;
                    } else {
                        for slot in EquipSlot::SOCKETED {
                            if self
                                .game_mut()
                                .character
                                .socket(i, slot, &content::get().gems)
                                .is_ok()
                            {
                                self.selection = Some(Selection::Worn(slot));
                                break;
                            }
                        }
                    }
                    ui.audio.push(("assets/sfx/item_equip.wav".into(), 0.0));
                }
            }
            return;
        }
        if let Some(i) = rects.equipment.iter().position(|r| contains(*r, x, y)) {
            let slot = EQUIPMENT[i];
            if self.game().mode == Mode::Map {
                if input.alt_click {
                    let _ = self.game_mut().character.unequip(slot);
                } else if let Some(Selection::Bag(bag)) = self.selection {
                    let result =
                        if matches!(self.game().character.inventory.get(bag), Some(Loot::Gem(_))) {
                            self.game_mut()
                                .character
                                .socket(bag, slot, &content::get().gems)
                        } else {
                            self.game_mut().character.equip(bag, Some(slot)).map(|_| ())
                        };
                    if result.is_ok() {
                        ui.audio.push(("assets/sfx/ui/rune_select.wav".into(), 5.0));
                    }
                }
            }
            self.selection = Some(Selection::Worn(slot));
            return;
        }
        if let Some(i) = rects.market.iter().position(|r| contains(*r, x, y)) {
            let slot = self.socket_slot();
            if self.game().mode == Mode::Altar {
                self.act(ui, Action::Upgrade(GemSpot::Socket(slot, i)));
            } else if input.alt_click {
                self.act(ui, Action::Unsocket(slot, i));
            }
        }
    }

    fn socket_slot(&self) -> EquipSlot {
        match self.selection {
            Some(Selection::Worn(s)) if EquipSlot::SOCKETED.contains(&s) => s,
            _ => EquipSlot::Weapon,
        }
    }

    pub(super) fn draw(&self, ui: &UiState, rects: &Rects) -> Vec<Draw> {
        let game = self.game();
        let mut out = Vec::new();
        if let Mode::Starter(offers) = &game.mode {
            out.push(Draw::Fill {
                rect: [0.0, 0.0, 640.0, 360.0],
                color: palette(1),
            });
            out.push(text(
                [120.0, 70.0, 400.0, 24.0],
                "CHOOSE YOUR FIRST SKILL GEM",
                WHITE,
                Align::Center,
            ));
            for (i, id) in offers.iter().enumerate() {
                let x = 194.0 + i as f32 * 110.0;
                loot_draw(
                    &mut out,
                    [x, 140.0, 34.0, 34.0],
                    &Loot::Gem(Gem {
                        id: id.clone(),
                        level: 1,
                        rarity: GemRarity::Common,
                    }),
                );
                out.push(text(
                    [x - 38.0, 184.0, 110.0, 13.0],
                    megatower_sim::run::gem_name(id),
                    WHITE,
                    Align::Center,
                ));
                if let Some(def) = content::get().gems.active(id) {
                    let damage: Vec<String> = def
                        .base
                        .iter()
                        .map(|(kind, r)| {
                            format!(
                                "{:.0}-{:.0} {}",
                                r.min,
                                r.max,
                                format!("{kind:?}").to_lowercase()
                            )
                        })
                        .collect();
                    let targets = match def.targeting {
                        megatower_sim::gems::Targeting::Single => "one".to_owned(),
                        megatower_sim::gems::Targeting::All => "all".to_owned(),
                        megatower_sim::gems::Targeting::Chain(n) => format!("chain {n}"),
                    };
                    out.push(text(
                        [x - 38.0, 200.0, 110.0, 13.0],
                        damage.join(" + "),
                        palette(8),
                        Align::Center,
                    ));
                    out.push(text(
                        [x - 38.0, 213.0, 110.0, 13.0],
                        format!("{:.0} mana, {targets}", def.cost),
                        palette(4),
                        Align::Center,
                    ));
                }
                out.push(text(
                    [x, 236.0, 34.0, 13.0],
                    format!("[{}]", i + 1),
                    palette(8),
                    Align::Center,
                ));
            }
            return out;
        }
        compose::draw_tabs(ui, rects, &mut out);
        // Keep the original gear button and exact resolved control geometry.
        out.extend(screens::layout_visuals(
            &megatower_assets::ui_layout::Layout {
                viewport: [640.0, 360.0],
                visuals: rects
                    .buttons
                    .iter()
                    .filter(|v| v.path.contains("OptionsButton"))
                    .cloned()
                    .collect(),
            },
            &[],
        ));
        out.push(text(
            rects.floor_label,
            format!("{} | {}", game.floor.number, game.character.progress.level),
            palette(2),
            Align::Left,
        ));
        let remaining = game
            .floor
            .positions()
            .filter(|p| !game.floor.room(*p).cleared)
            .count();
        out.push(text(
            rects.rooms_label,
            remaining.to_string(),
            palette(2),
            Align::Left,
        ));
        let frame = if ui.timeline.busy() {
            ui.timeline.frame.clone().unwrap_or_else(|| self.snapshot())
        } else {
            self.snapshot()
        };
        match self.view {
            View::Map => self.draw_map(&mut out),
            View::Tree => self.draw_tree(&mut out),
            View::Profile if ui.tab == Tab::Profile => self.profile(&mut out, ui, rects, &frame),
            View::Profile => self.information(&mut out, ui, rects),
        }
        for (i, enemy) in frame
            .enemies
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
        {
            if enemy.out_of_combat {
                continue;
            }
            let mut enemy = enemy.clone();
            enemy.selected = i == self.target;
            let cx = crate::frame::enemy_cx(&frame, i);
            if let Some(policies) = &self.policies {
                compose::enemy_widget(rects, policies, &enemy, cx, ui, i, &mut out);
            }
        }
        if frame.turn > 0 {
            out.push(text(
                rects.turn_label,
                format!("Turn {}", frame.turn),
                palette(4),
                Align::Center,
            ));
        }
        if game.mode == Mode::Shop
            && let Some(shop) = game.shop()
        {
            for (i, (rect, gem)) in rects.merchant.iter().zip(&shop.stock).enumerate() {
                let rect = &in_action_area(*rect);
                if let Some(g) = gem {
                    loot_draw(&mut out, *rect, &Loot::Gem(g.clone()));
                    out.push(text(
                        [rect[0], rect[1] + 34.0, 34.0, 13.0],
                        megatower_sim::merchant::gem_price(&content::get().gems, g).to_string(),
                        palette(8),
                        Align::Center,
                    ));
                    if shop.locked[i] {
                        out.push(tinted(
                            [rect[0] + 23.0, rect[1] - 3.0, 14.0, 14.0],
                            "assets/textures/ui/lock_icon.png",
                            palette(8),
                        ));
                    }
                }
            }
        }
        let choices = self.choices(rects);
        for c in &choices {
            if matches!(c.action, Action::Buy(_)) {
                continue;
            }
            compose::button(
                rects,
                c.rect,
                &c.label,
                c.icon,
                c.enabled && !ui.timeline.busy() && self.walk.is_none(),
                4,
                &mut out,
            );
            if let Action::Skill(i) = c.action {
                // Just left of the centred icon, level with it.
                out.push(text(
                    [
                        c.rect[0] + c.rect[2] / 2.0 - 22.0,
                        // The glyph sits low in its 13px box: level its middle with
                        // the icon's.
                        c.rect[1] + ((c.rect[3] - 16.0) / 2.0).ceil() + 8.0 - 10.5,
                        14.0,
                        13.0,
                    ],
                    (i + 1).to_string(),
                    palette(1),
                    Align::Right,
                ));
            }
        }
        if let Mode::Event { id, outcome } = &game.mode
            && let Some(event) = content::get().events.iter().find(|e| e.id == *id)
        {
            let lines = outcome
                .clone()
                .unwrap_or_else(|| vec![event.name.clone(), event.text.clone()]);
            let lines: Vec<_> = lines.iter().flat_map(|s| wrap(s, 42)).collect();
            for (i, line) in lines.iter().take(8).enumerate() {
                out.push(text(
                    [335.0, 90.0 + i as f32 * 18.0, 270.0, 16.0],
                    line,
                    WHITE,
                    Align::Center,
                ));
            }
        }
        compose::popup_draws(ui, &mut out);
        if let Some(cursor) = ui.cursor {
            if let Some(c) = choices
                .iter()
                .find(|c| contains(c.rect, cursor[0], cursor[1]))
            {
                // The movement, pass and skill buttons explain themselves.
                if !matches!(c.action, Action::Walk(_) | Action::Pass | Action::Skill(_)) {
                    tooltip(&mut out, rects, cursor, c.tip.clone());
                }
            } else if let Some(i) = compose::hit_enemy(&frame, cursor[0], cursor[1])
                && let Some(e) = game.battle.as_ref().and_then(|b| b.enemies.get(i))
            {
                let mut lines = vec![
                    e.fighter.name.clone(),
                    format!("Next: {} ({})", e.intent().name, intent_amount(e)),
                ];
                lines.extend(
                    megatower_sim::ailment::Ailment::ALL
                        .into_iter()
                        .filter(|a| e.fighter.afflictions.has(*a))
                        .map(|a| format!("{a:?}")),
                );
                tooltip(&mut out, rects, cursor, lines);
            } else if self.view == View::Profile
                && ui.tab == Tab::Profile
                && contains(XP_BAR, cursor[0], cursor[1])
            {
                let progress = &game.character.progress;
                tooltip(
                    &mut out,
                    rects,
                    cursor,
                    vec![
                        format!("Level {}", progress.level),
                        format!(
                            "{} / {} XP",
                            progress.xp,
                            megatower_sim::passives::xp_to_next(progress.level)
                        ),
                        format!("{} passive points to spend", progress.points),
                    ],
                );
            } else if self.view == View::Tree
                && let Some(id) = self.tree_hit(cursor[0], cursor[1])
                && let Some(node) = content::get().tree.nodes.iter().find(|n| n.id == id)
            {
                let mut lines = vec![node.name.clone()];
                lines.extend(megatower_sim::describe::mod_lines(&node.mods));
                tooltip(&mut out, rects, cursor, lines);
            } else if self.view == View::Profile && ui.tab == Tab::Profile {
                let loot = rects
                    .inventory
                    .iter()
                    .position(|r| contains(*r, cursor[0], cursor[1]))
                    .and_then(|i| game.character.inventory.get(i).cloned())
                    .or_else(|| {
                        rects
                            .equipment
                            .iter()
                            .position(|r| contains(*r, cursor[0], cursor[1]))
                            .and_then(|i| game.character.equipped(EQUIPMENT[i]).cloned())
                            .map(Loot::Item)
                    });
                if let Some(loot) = loot {
                    tooltip(&mut out, rects, cursor, tip_lines(&loot));
                } else if let Some(i) = rects
                    .market
                    .iter()
                    .position(|r| contains(*r, cursor[0], cursor[1]))
                    && let Some(g) = game
                        .character
                        .equipped(self.socket_slot())
                        .and_then(|it| it.sockets.as_ref())
                        .and_then(|s| s.gems.get(i))
                {
                    let lines = tip_lines(&Loot::Gem(g.clone()));
                    tooltip(&mut out, rects, cursor, lines);
                }
            }
        }
        if game.mode == Mode::Dead && !ui.timeline.busy() {
            out.push(Draw::Fill {
                rect: [0.0, 0.0, 640.0, 360.0],
                color: palette(1),
            });
            out.extend(screens::death_draws(
                &megatower_assets::ui_layout::death_screen_layout(),
                &format!("Died on floor {}", game.floor.number),
                self.last_reward
                    .map(|n| format!("+{n} Legacy ({} total)", game.legacy.points))
                    .as_deref(),
            ));
        }
        out
    }

    fn profile(&self, out: &mut Vec<Draw>, ui: &UiState, rects: &Rects, frame: &Frame) {
        let game = self.game();
        out.push(image(
            rects.portrait,
            self.portrait
                .as_deref()
                .unwrap_or("assets/textures/portraits/kestrel.png"),
        ));
        out.push(image(
            rects.portrait_border,
            megatower_core::definitions::BorderType::from_floor(true, game.floor.number as i32 - 1)
                .texture(),
        ));
        out.push(tinted(
            rects.gold_icon,
            "assets/textures/icons/gold_coins_icon.png",
            hex(0xffaa00),
        ));
        out.push(text(
            rects.gold,
            game.character.gold.to_string(),
            hex(0xffaa00),
            Align::Left,
        ));
        out.push(tinted(
            rects.diamond_icon,
            "assets/textures/icons/trial_level_icon.png",
            palette(8),
        ));
        out.push(text(
            rects.diamonds,
            game.character.progress.level.to_string(),
            palette(8),
            Align::Left,
        ));
        if game.character.orbs.total() > 0 {
            let mut below = rects.diamonds;
            below[1] += below[3];
            below[2] *= 4.0;
            out.push(text(
                below,
                orb_summary(&game.character.orbs),
                palette(8),
                Align::Left,
            ));
        }
        if game.floor.danger() > 0 {
            // Right of the level counter: the floor has been wandered too long.
            let mut at = rects.diamonds;
            at[0] += 70.0;
            at[2] *= 3.0;
            out.push(text(
                at,
                format!("Danger {}", game.floor.danger()),
                hex(0xff6a4d),
                Align::Left,
            ));
        }
        let v = &frame.player;
        // Use the original animated bars, numerical panels and segmented mana
        // pip. Armour here is only megatower-sim's temporary guard, not a new rule.
        let mut health = rects.bar_second;
        if v.max_armor > 0.0 {
            compose::bar(
                rects.bar_top,
                compose::bar_fracs(ui, BarKey::PlayerArmor, v.armor, v.max_armor, hex(0x4a5462)),
                hex(0x4a5462),
                out,
            );
            compose::label_panel(
                rects,
                rects.bar_top[0] + rects.bar_top[2] / 2.0,
                rects.bar_top[1] - 9.0,
                &format!("{:.0} / {:.0}", v.armor, v.max_armor),
                hex(0x8b93af),
                out,
            );
        } else {
            health[1] = ((rects.bar_top[1] + rects.bar_second[1]) / 2.0).round();
        }
        health[2] = if v.max_mana > 0 {
            rects.bar_top[2] - MANA_BAR_WIDTH - 4.0
        } else {
            rects.bar_top[2]
        };
        compose::bar(
            health,
            compose::bar_fracs(
                ui,
                BarKey::PlayerHealth,
                v.health,
                v.max_health,
                hex(0xff8700),
            ),
            hex(0x990f35),
            out,
        );
        compose::label_panel(
            rects,
            health[0] + health[2] / 2.0,
            health[1] - 9.0,
            &format!("{:.0} / {:.0}", v.health, v.max_health),
            hex(0xff4d85),
            out,
        );
        if v.max_mana > 0 {
            // The mana pool is large here (costs of 4-7 out of ~50), so it is
            // a plain pill bar like health rather than megaloot's pips.
            let right = rects.mana[0] + rects.mana[2] + 2.0;
            let blue = hex(0x285cc4);
            let rect = [right - MANA_BAR_WIDTH, health[1], MANA_BAR_WIDTH, health[3]];
            compose::bar(
                rect,
                (v.mana as f32 / v.max_mana.max(1) as f32, None),
                blue,
                out,
            );
            compose::label_panel(
                rects,
                rect[0] + rect[2] / 2.0,
                rect[1] - 9.0,
                &format!("{} / {}", v.mana, v.max_mana),
                hex(0x5b8def),
                out,
            );
        }
        // Experience toward the next level: a thin bar under the portrait.
        let progress = &game.character.progress;
        let next = megatower_sim::passives::xp_to_next(progress.level).max(1);
        let frac = (progress.xp as f32 / next as f32).clamp(0.0, 1.0);
        out.push(Draw::Fill {
            rect: XP_BAR,
            color: palette(2),
        });
        out.push(Draw::Fill {
            rect: [
                XP_BAR[0] + 1.0,
                XP_BAR[1] + 1.0,
                XP_BAR[2] - 2.0,
                XP_BAR[3] - 2.0,
            ],
            color: palette(1),
        });
        let fill = ((XP_BAR[2] - 2.0) * frac).round();
        if fill > 0.0 {
            out.push(Draw::Fill {
                rect: [XP_BAR[0] + 1.0, XP_BAR[1] + 1.0, fill, XP_BAR[3] - 2.0],
                color: hex(0x4d9fff),
            });
        }
        out.push(tinted(
            [15.0, 98.0, 16.0, 16.0],
            "assets/textures/icons/wisdom_icon.png",
            palette(8),
        ));
        out.push(text(
            [34.0, 100.0, 80.0, 13.0],
            format!("{} points", game.character.progress.points),
            palette(8),
            Align::Left,
        ));
        if let Some(policies) = &self.policies {
            for (i, status) in v.statuses.iter().take(5).enumerate() {
                let policy = policies.get(status.resource);
                if let Some(icon) = &policy.icon {
                    out.push(tinted(
                        [120.0 + i as f32 * 24.0, 98.0, 16.0, 16.0],
                        icon,
                        policy.color,
                    ));
                }
            }
        }
        for (i, rect) in rects.equipment.iter().enumerate() {
            if let Some(item) = game.character.equipped(EQUIPMENT[i]) {
                loot_draw(out, *rect, &Loot::Item(item.clone()));
            } else {
                out.push(Draw::Image {
                    rect: [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0],
                    path: "assets/textures/ui/equipment_slots.png".into(),
                    // The final two equipment cells share the seventh atlas
                    // frame (rings); there is no eighth frame in this image.
                    region: Some([i.min(6) as f32 * 32.0, 0.0, 32.0, 32.0]),
                    tint: palette(4),
                    slice: None,
                    flip_x: false,
                });
            }
            if self.selection == Some(Selection::Worn(EQUIPMENT[i])) {
                out.push(selection_outline(*rect));
            }
        }
        for (i, rect) in rects.inventory.iter().enumerate() {
            if let Some(loot) = game.character.inventory.get(i) {
                loot_draw(out, *rect, loot);
            }
            if self.selection == Some(Selection::Bag(i)) {
                out.push(selection_outline(*rect));
            }
        }
        let slot = self.socket_slot();
        if let Some(sockets) = game
            .character
            .equipped(slot)
            .and_then(|i| i.sockets.as_ref())
        {
            out.push(Draw::Text {
                rect: [83.0, 296.0, 170.0, 12.0],
                text: format!("{slot:?}: {} linked sockets", sockets.links),
                size: 12.0,
                color: palette(4),
                align: Align::Left,
            });
            for (i, rect) in rects
                .market
                .iter()
                .take(usize::from(sockets.links))
                .enumerate()
            {
                if let Some(g) = sockets.gems.get(i) {
                    loot_draw(out, *rect, &Loot::Gem(g.clone()));
                } else if i < usize::from(sockets.links) {
                    out.push(tinted(
                        *rect,
                        "assets/textures/ui/item_slot.png",
                        palette(2),
                    ));
                    out.push(tinted(
                        [rect[0] + 9.0, rect[1] + 9.0, 16.0, 16.0],
                        "assets/textures/icons/diamonds_icon.png",
                        palette(2),
                    ));
                }
                if i + 1 < usize::from(sockets.links) {
                    out.push(Draw::Fill {
                        rect: [rect[0] + 34.0, rect[1] + 16.0, 5.0, 2.0],
                        color: palette(4),
                    });
                }
            }
        }
    }

    fn information(&self, out: &mut Vec<Draw>, ui: &UiState, _rects: &Rects) {
        if ui.tab != Tab::Log {
            super::stats::draw(&self.game().character, out);
            return;
        }
        out.push(Draw::Fill {
            rect: [7.0, 28.0, 295.0, 262.0],
            color: palette(1),
        });
        for (i, line) in self.log.iter().rev().take(14).enumerate() {
            out.push(text(
                [16.0, 38.0 + i as f32 * 17.0, 275.0, 16.0],
                line,
                WHITE,
                Align::Left,
            ));
        }
    }

    fn draw_map(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Fill {
            rect: [7.0, 28.0, 295.0, 262.0],
            color: palette(1),
        });
        out.push(text(
            [16.0, 34.0, 275.0, 16.0],
            "LABYRINTH  /  [M] CLOSE",
            palette(8),
            Align::Center,
        ));
        let floor = &self.game().floor;
        let step = (240.0 / f32::from(floor.width.max(floor.height))).min(32.0);
        let cell = step - 7.0;
        let ox = 155.0 - f32::from(floor.width) * step / 2.0;
        let oy = 66.0;
        for p in floor.positions() {
            let room = floor.room(p);
            if room.visibility == Visibility::Hidden {
                continue;
            }
            let x = ox + f32::from(p.x) * step;
            let y = oy + f32::from(p.y) * step;
            for dir in [
                megatower_sim::labyrinth::Dir::East,
                megatower_sim::labyrinth::Dir::South,
            ] {
                let known = floor.neighbor(p, dir).is_some_and(|n| {
                    room.visibility == Visibility::Visited
                        || floor.room(n).visibility == Visibility::Visited
                });
                if room.has_door(dir) && known {
                    out.push(Draw::Fill {
                        rect: if dir == megatower_sim::labyrinth::Dir::East {
                            [x + cell, y + cell / 2.0, 7.0, 2.0]
                        } else {
                            [x + cell / 2.0, y + cell, 2.0, 7.0]
                        },
                        color: palette(4),
                    });
                }
            }
            out.push(Draw::Fill {
                rect: [x, y, cell, cell],
                color: if p == floor.current {
                    palette(8)
                } else {
                    palette(2)
                },
            });
            let mark = room.known_kind().map_or("?", |k| match k {
                RoomKind::Merchant => "$",
                RoomKind::GemAltar { .. } => "G",
                RoomKind::Stairs { .. } => ">",
                RoomKind::Shrine => "+",
                _ => "",
            });
            if p == floor.current {
                // An arrow in the room you stand in, pointing the way you face.
                let (w, h) = ARROW_SIZE;
                let (w, h) = if matches!(floor.facing, Dir::East | Dir::West) {
                    (h, w)
                } else {
                    (w, h)
                };
                let origin = [
                    (x + (cell - w) / 2.0).round(),
                    (y + (cell - h) / 2.0).round(),
                ];
                for r in arrow_rects(floor.facing) {
                    out.push(Draw::Fill {
                        rect: [origin[0] + r[0], origin[1] + r[1], r[2], r[3]],
                        color: palette(1),
                    });
                }
            } else {
                out.push(text([x, y + 2.0, cell, 16.0], mark, WHITE, Align::Center));
            }
        }
    }

    fn tree_positions(&self) -> Vec<(String, [f32; 2])> {
        let nodes = &content::get().tree.nodes;
        let minx = nodes.iter().map(|n| n.x).fold(f32::INFINITY, f32::min);
        let maxx = nodes.iter().map(|n| n.x).fold(f32::NEG_INFINITY, f32::max);
        let miny = nodes.iter().map(|n| n.y).fold(f32::INFINITY, f32::min);
        let maxy = nodes.iter().map(|n| n.y).fold(f32::NEG_INFINITY, f32::max);
        // The panel is wider than tall: spread the tree more horizontally so
        // neighbouring nodes keep their gaps.
        let sx = 250.0 / (maxx - minx).max(1.0);
        let sy = 205.0 / (maxy - miny).max(1.0);
        nodes
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    [28.0 + (n.x - minx) * sx, 65.0 + (n.y - miny) * sy],
                )
            })
            .collect()
    }
    fn tree_hit(&self, x: f32, y: f32) -> Option<String> {
        self.tree_positions()
            .into_iter()
            .find(|(id, p)| {
                let half = tree_node_size(id) / 2.0 + 1.0;
                (x - p[0]).abs() < half && (y - p[1]).abs() < half
            })
            .map(|(id, _)| id)
    }
    fn draw_tree(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Fill {
            rect: [7.0, 28.0, 295.0, 262.0],
            color: palette(1),
        });
        out.push(text(
            [16.0, 34.0, 275.0, 16.0],
            format!(
                "PASSIVES / {} POINTS / [P] CLOSE",
                self.game().character.progress.points
            ),
            palette(8),
            Align::Center,
        ));
        let points = self.tree_positions();
        let tree = &content::get().tree;
        for (a, b) in tree.edges() {
            let pa = points.iter().find(|(id, _)| id == &a.id).unwrap().1;
            let pb = points.iter().find(|(id, _)| id == &b.id).unwrap().1;
            let steps = ((pa[0] - pb[0]).abs().max((pa[1] - pb[1]).abs()) / 2.0).max(1.0) as usize;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                out.push(Draw::Fill {
                    rect: [
                        pa[0] + (pb[0] - pa[0]) * t,
                        pa[1] + (pb[1] - pa[1]) * t,
                        2.0,
                        2.0,
                    ],
                    color: if progress_lit(self.game(), &a.id, &b.id) {
                        palette(8)
                    } else {
                        palette(2)
                    },
                });
            }
        }
        let progress = &self.game().character.progress;
        for (id, p) in points {
            let Some(node) = tree.node(&id) else { continue };
            let allocated = progress.is_allocated(&id);
            let reachable = !allocated && progress.can_allocate(tree, &id).is_ok();
            let selected = self.tree_selection.as_deref() == Some(id.as_str());
            let size = tree_node_size(&id);
            let rect = [
                (p[0] - size / 2.0).round(),
                (p[1] - size / 2.0).round(),
                size,
                size,
            ];
            let border = if selected {
                WHITE
            } else if allocated {
                palette(8)
            } else if reachable {
                hex(0xf2c94c)
            } else {
                palette(4)
            };
            let fill = if allocated { palette(2) } else { palette(1) };
            let icon_tint = if allocated {
                palette(8)
            } else if reachable {
                hex(0xf2c94c)
            } else {
                palette(3)
            };
            out.push(Draw::Fill {
                rect,
                color: border,
            });
            out.push(Draw::Fill {
                rect: [rect[0] + 1.0, rect[1] + 1.0, size - 2.0, size - 2.0],
                color: fill,
            });
            match node.kind {
                NodeKind::Small => out.push(Draw::Fill {
                    rect: [rect[0] + 2.0, rect[1] + 2.0, size - 4.0, size - 4.0],
                    color: if allocated {
                        palette(8)
                    } else if reachable {
                        hex(0xf2c94c)
                    } else {
                        palette(2)
                    },
                }),
                NodeKind::Start => out.push(Draw::Fill {
                    rect: [rect[0] + 3.0, rect[1] + 3.0, size - 6.0, size - 6.0],
                    color: icon_tint,
                }),
                NodeKind::Notable => out.push(tinted(
                    [rect[0] + 1.0, rect[1] + 1.0, 8.0, 8.0],
                    tree_node_icon(node),
                    icon_tint,
                )),
                NodeKind::Keystone => out.push(tinted(
                    [rect[0] + 1.0, rect[1] + 1.0, 16.0, 16.0],
                    tree_node_icon(node),
                    icon_tint,
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payoff_gems_say_what_they_cash_in() {
        let gem = Loot::Gem(Gem::new("shatter", 1, GemRarity::Common));
        let lines = tip_lines(&gem);
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("Cashes in Chill or Freeze")),
            "{lines:?}"
        );
        let plain = tip_lines(&Loot::Gem(Gem::new("fireball", 1, GemRarity::Common)));
        assert!(!plain.iter().any(|l| l.starts_with("Cashes in")));
    }

    #[test]
    fn enemy_intents_show_their_numbers() {
        use megatower_sim::combat::spawn;
        let content = content::get();
        let rat = spawn(&content.enemies[0], 1, false);
        assert_eq!(intent_amount(&rat), "3-5");
        let warden = content
            .enemies
            .iter()
            .find(|e| e.id == "stair_warden")
            .unwrap();
        let boss = [spawn(warden, 3, false)];
        let first = intent_amount(&boss[0]);
        assert!(
            first.contains('-'),
            "the warden opens with an attack: {first}"
        );
        let slime = spawn(
            content
                .enemies
                .iter()
                .find(|e| e.id == "acid_slime")
                .unwrap(),
            1,
            false,
        );
        let mut slime = slime;
        slime.fighter.guard = 0.0;
        assert!(intent_amount(&slime).contains('-'));
    }
    fn fixture() -> (Labyrinth, UiState, Rects) {
        let mut run = Run::new(2, 3);
        run.pick_starter(0);
        (
            Labyrinth {
                run: Some(run),
                ..default()
            },
            UiState {
                screen: Screen::Playing,
                ..default()
            },
            Rects::from_layout(&megatower_assets::ui_layout::recovered_layout()),
        )
    }

    #[test]
    fn selected_slots_use_one_outline_frame_and_both_rings_share_a_valid_frame() {
        let (mut session, ui, rects) = fixture();
        session.selection = Some(Selection::Worn(EquipSlot::Weapon));
        let draws = session.draw(&ui, &rects);
        let outlines: Vec<_> = draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Image {
                    path, region, rect, ..
                } if path.ends_with("/item_slot_outline.png") => Some((region, rect)),
                _ => None,
            })
            .collect();
        assert_eq!(outlines.len(), 1);
        assert_eq!(*outlines[0].0, Some([0.0, 0.0, 36.0, 36.0]));
        assert_eq!([outlines[0].1[2], outlines[0].1[3]], [36.0, 36.0]);
        let ring_frames = draws
            .iter()
            .filter(|draw| {
                matches!(draw,
            Draw::Image {path,region:Some([x,_,_,_]),..}
            if path.ends_with("/equipment_slots.png") && *x==192.0)
            })
            .count();
        assert_eq!(ring_frames, 2);
        assert!(
            draws
                .iter()
                .all(|draw| !matches!(draw,Draw::Image {path,..} if path.ends_with("/socket.png")))
        );
        let first_slot_y = rects.market[0][1];
        assert!(
            draws
                .iter()
                .any(|draw| matches!(draw,Draw::Text {text,rect,size,..}
            if text.contains("linked sockets") && *size==12.0 && rect[1]+rect[3]<first_slot_y))
        );
    }

    #[test]
    fn the_map_arrow_points_where_you_face() {
        use megatower_sim::labyrinth::Dir;
        let tip = |dir| {
            // The thinnest end of the head: the one 1x1 rectangle.
            arrow_rects(dir)
                .into_iter()
                .find(|r| r[2] == 1.0 && r[3] == 1.0)
                .unwrap()
        };
        assert_eq!(tip(Dir::North), [3.0, 0.0, 1.0, 1.0]);
        assert_eq!(tip(Dir::South), [3.0, 9.0, 1.0, 1.0]);
        assert_eq!(tip(Dir::East), [9.0, 3.0, 1.0, 1.0]);
        assert_eq!(tip(Dir::West), [0.0, 3.0, 1.0, 1.0]);
        for dir in Dir::ALL {
            let area: f32 = arrow_rects(dir).iter().map(|r| r[2] * r[3]).sum();
            assert_eq!(area, 1.0 + 3.0 + 5.0 + 7.0 + 3.0 * 6.0, "{dir:?}");
        }
    }

    #[test]
    fn every_room_button_sits_inside_the_taller_action_area() {
        let (mut session, _, rects) = fixture();
        let [ax, ay, aw, ah] = ACTION_AREA;
        assert!(ay + ah <= 360.0 && ay > 200.0);
        for kind in [
            RoomKind::Start,
            RoomKind::Fight,
            RoomKind::Merchant,
            RoomKind::Shrine,
            RoomKind::Event,
            RoomKind::GemAltar { uses_left: 1 },
            RoomKind::Stairs { boss: false },
        ] {
            session.game_mut().debug_walk_to_kind(|k| {
                std::mem::discriminant(&k) == std::mem::discriminant(&kind)
            });
            for c in session.choices(&rects) {
                // The tree button lives in the left panel.
                if matches!(c.action, Action::Tree) {
                    continue;
                }
                let [x, y, w, h] = c.rect;
                assert!(
                    x >= ax && y >= ay && x + w <= ax + aw && y + h <= ay + ah,
                    "{kind:?} {:?} at {:?} leaves {ACTION_AREA:?}",
                    c.action,
                    c.rect
                );
            }
        }
    }

    #[test]
    fn movement_is_an_inverted_t_with_ahead_over_back() {
        let (w, _) = walk_button(Turn::Ahead);
        let (a, _) = walk_button(Turn::Left);
        let (s, _) = walk_button(Turn::Back);
        let (d, _) = walk_button(Turn::Right);
        assert_eq!(w[0], s[0], "W sits right above S");
        assert!(w[1] + w[3] < s[1], "W is above the bottom row");
        assert_eq!(a[1], s[1]);
        assert_eq!(d[1], s[1]);
        assert!(
            a[0] + a[2] <= s[0] && s[0] + s[2] <= d[0],
            "A, S, D in a row"
        );
    }

    #[test]
    fn there_is_one_map_button() {
        let (mut session, _, rects) = fixture();
        for kind in [
            RoomKind::Start,
            RoomKind::Fight,
            RoomKind::Stairs { boss: false },
        ] {
            session.game_mut().debug_walk_to_kind(|k| {
                std::mem::discriminant(&k) == std::mem::discriminant(&kind)
            });
            if session.game().mode != Mode::Map {
                continue;
            }
            let maps = session
                .choices(&rects)
                .iter()
                .filter(|c| matches!(c.action, Action::Map))
                .count();
            assert_eq!(maps, 1, "{kind:?}");
        }
    }

    #[test]
    fn movement_pass_and_skill_buttons_have_no_tooltip() {
        let shown = |session: &Labyrinth, ui: &mut UiState, rects: &Rects, at: [f32; 2]| {
            ui.cursor = Some(at);
            session
                .draw(ui, rects)
                .into_iter()
                .filter_map(|d| match d {
                    Draw::Text { text, .. } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let (mut session, mut ui, rects) = fixture();
        // The floor-map button keeps its tooltip, as a control.
        let button = session
            .choices(&rects)
            .into_iter()
            .find(|c| matches!(c.action, Action::Map))
            .unwrap()
            .rect;
        let map = shown(
            &session,
            &mut ui,
            &rects,
            [button[0] + button[2] / 2.0, button[1] + button[3] / 2.0],
        );
        assert!(map.iter().any(|t| t.contains("Floor map")), "{map:?}");
        // Directions: nothing pops up over the walk buttons.
        for i in 0..4 {
            let at = [325.0 + i as f32 * 44.0 + 10.0, 320.0];
            let text = shown(&session, &mut ui, &rects, at);
            assert!(
                !text
                    .iter()
                    .any(|t| t.contains("room") || t.contains("Unexplored")),
                "{text:?}"
            );
        }
        // In a fight, pass and the skill buttons are silent too.
        session
            .game_mut()
            .debug_walk_to_kind(|k| k == RoomKind::Fight);
        assert_eq!(session.game().mode, Mode::Combat);
        for at in [[335.0, 320.0], [390.0, 320.0]] {
            let text = shown(&session, &mut ui, &rects, at);
            assert!(
                !text
                    .iter()
                    .any(|t| t.contains("Pass turn") || t.contains("mana")),
                "{text:?}"
            );
        }
    }

    #[test]
    fn each_action_strip_has_non_overlapping_controls() {
        let (mut session, _, rects) = fixture();
        for kind in [
            RoomKind::Start,
            RoomKind::Fight,
            RoomKind::Merchant,
            RoomKind::Shrine,
            RoomKind::Event,
            RoomKind::GemAltar { uses_left: 1 },
        ] {
            session.game_mut().debug_walk_to_kind(|k| {
                std::mem::discriminant(&k) == std::mem::discriminant(&kind)
            });
            let choices = session.choices(&rects);
            for (i, a) in choices.iter().enumerate() {
                for b in choices.iter().skip(i + 1) {
                    let overlap = a.rect[0] < b.rect[0] + b.rect[2]
                        && b.rect[0] < a.rect[0] + a.rect[2]
                        && a.rect[1] < b.rect[1] + b.rect[3]
                        && b.rect[1] < a.rect[1] + a.rect[3];
                    assert!(!overlap, "{:?} overlaps {:?}", a.action, b.action);
                }
            }
        }
    }
    #[test]
    fn selecting_a_bag_gem_then_the_original_weapon_cell_sockets_it() {
        let (mut session, mut ui, rects) = fixture();
        let id = content::get().gems.supports[0].id.clone();
        let index = session.game().character.inventory.len();
        session
            .game_mut()
            .character
            .inventory
            .push(Loot::Gem(Gem::new(&id, 1, GemRarity::Common)));
        session.selection = Some(Selection::Bag(index));
        let weapon = rects.equipment[5];
        ui.cursor = Some([weapon[0] + 17.0, weapon[1] + 17.0]);
        session.pointer(
            &mut ui,
            &rects,
            &FrameInput {
                click: true,
                ..default()
            },
        );
        let sockets = session
            .game()
            .character
            .equipped(EquipSlot::Weapon)
            .unwrap()
            .sockets
            .as_ref()
            .unwrap();
        assert!(sockets.gems.iter().any(|g| g.id == id));
        let at = sockets.gems.iter().position(|g| g.id == id).unwrap();
        session.act(&mut ui, Action::Unsocket(EquipSlot::Weapon, at));
        assert!(
            session
                .game()
                .character
                .inventory
                .iter()
                .any(|loot| matches!(loot,Loot::Gem(g)if g.id==id))
        );
    }
    #[test]
    fn original_merchant_cells_buy_and_lock_the_megatower_sims_stock() {
        let (mut session, mut ui, rects) = fixture();
        session
            .game_mut()
            .debug_walk_to_kind(|k| k == RoomKind::Merchant);
        session.game_mut().character.gold = 1000;
        let rect = in_action_area(rects.merchant[0]);
        ui.cursor = Some([rect[0] + 17.0, rect[1] + 17.0]);
        session.pointer(
            &mut ui,
            &rects,
            &FrameInput {
                alt_click: true,
                ..default()
            },
        );
        assert!(session.game().shop().unwrap().locked[0]);
        let before = session.game().character.inventory.len();
        session.pointer(
            &mut ui,
            &rects,
            &FrameInput {
                click: true,
                ..default()
            },
        );
        assert_eq!(session.game().character.inventory.len(), before + 1);
        assert!(session.game().shop().unwrap().stock[0].is_none());
    }
    #[test]
    fn the_altar_action_upgrades_socketed_gems_without_changing_the_rules() {
        let (mut session, mut ui, _) = fixture();
        session
            .game_mut()
            .debug_walk_to_kind(|k| matches!(k, RoomKind::GemAltar { .. }));
        assert_eq!(session.game().mode, Mode::Altar);
        session.act(
            &mut ui,
            Action::Upgrade(GemSpot::Socket(EquipSlot::Weapon, 0)),
        );
        let gem = &session
            .game()
            .character
            .equipped(EquipSlot::Weapon)
            .unwrap()
            .sockets
            .as_ref()
            .unwrap()
            .gems[0];
        assert_eq!(gem.rarity, GemRarity::Uncommon);
    }
}

/// "Rf 2  Au 1  As 0  Rl 1  Rg 0": the orbs in stock, in key order.
fn orb_summary(orbs: &megatower_sim::orbs::Orbs) -> String {
    use megatower_sim::orbs::Orb;
    let tag = |o: Orb| match o {
        Orb::Reforge => "Rf",
        Orb::Augment => "Au",
        Orb::Ascend => "As",
        Orb::Relink => "Rl",
        Orb::Regret => "Rg",
    };
    Orb::ALL
        .into_iter()
        .filter(|&o| orbs.count(o) > 0)
        .map(|o| format!("{} {}", tag(o), orbs.count(o)))
        .collect::<Vec<_>>()
        .join("  ")
}
