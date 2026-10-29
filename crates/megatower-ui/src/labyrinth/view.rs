//! Populate Megatower's existing 640x360 HUD, not a second UI layout.
use super::*;
use crate::compose::{self, Align, Draw, Seg, TipView, WHITE, hex, image, palette, text, tinted};
use game_sim::{
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
/// The experience bar under the portrait (canvas pixels).
const XP_BAR: R = [16.0, 87.0, 36.0, 5.0];

fn intent_amount(enemy: &game_sim::combat::Enemy) -> String {
    use game_sim::combat::IntentKind;
    match &enemy.intent().kind {
        IntentKind::Attack(spec) => {
            let scale = enemy
                .fighter
                .mods
                .more(game_sim::modifier::Stat::Damage, spec.tags)
                * enemy.fighter.afflictions.damage_dealt_multiplier();
            let min: f64 = spec.base.iter().map(|(_, r)| r.min).sum();
            let max: f64 = spec.base.iter().map(|(_, r)| r.max).sum();
            format!("{:.0}-{:.0}", min * scale, max * scale)
        }
        IntentKind::Guard { armour } => format!("+{armour:.0}"),
        IntentKind::Heal { percent } => format!("{percent:.0}%"),
    }
}

fn intent_rect(cx: f32) -> R {
    let layout = megatower_assets::godot_layout::enemy_container_layout();
    // The source draws ailment icons 14px above this row. Put intent above
    // those, never at a hard-coded Y inside the health/armour panel.
    let statuses_y =
        compose::ENEMY_TOP + layout.rect("MarginContainer3/EffectContainerHolder")[1] - 14.0;
    [cx - 8.0, statuses_y - 20.0, 16.0, 16.0]
}
fn tip_lines(loot: &Loot) -> Vec<String> {
    match loot {
        Loot::Item(item) => {
            let mut lines = vec![
                item.name.clone(),
                format!("{:?}  /  Level {}", item.slot, item.level),
            ];
            lines.extend(
                item.implicit
                    .iter()
                    .chain(item.affixes.iter().flat_map(|a| a.mods.iter()))
                    .chain(item.fixed.iter())
                    .map(|m| format!("{:+.0} {:?} ({:?})", m.value, m.stat, m.kind)),
            );
            if let Some(s) = &item.sockets {
                lines.push(format!("{} linked sockets", s.links));
                lines.extend(s.gems.iter().map(|g| game_sim::run::gem_name(&g.id)));
            }
            lines.push("Right-click to equip / unequip".into());
            lines
        }
        Loot::Gem(g) => {
            let mut lines = vec![
                game_sim::run::gem_name(&g.id),
                format!("{:?} gem / Level {}", g.rarity, g.level),
            ];
            if let Some(def) = content::get().gems.active(&g.id) {
                lines.push(format!("{:.0} mana / {:?}", def.cost, def.targeting));
                lines.extend(
                    def.base
                        .iter()
                        .map(|(kind, r)| format!("{:.0}-{:.0} {kind:?}", r.min, r.max)),
                );
            } else if let Some(def) = content::get().gems.support(&g.id) {
                lines.push("Support gem".into());
                lines.extend(
                    def.mods
                        .iter()
                        .map(|m| format!("{:+.0} {:?}", m.value, m.stat)),
                );
            }
            lines.push("Select, then click socketed equipment".into());
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
                        game_sim::run::gem_name(id),
                        "Choose this first skill gem",
                    ));
                }
            }
            Mode::Map => {
                for (i, turn) in [Turn::Left, Turn::Ahead, Turn::Right, Turn::Back]
                    .into_iter()
                    .enumerate()
                {
                    let exit = game.floor.turns().into_iter().find(|(t, _)| *t == turn);
                    let mut c = choice(
                        [325.0 + i as f32 * 44.0, 313.0, 36.0, 29.0],
                        Action::Walk(turn),
                        match turn {
                            Turn::Left => "A",
                            Turn::Ahead => "W",
                            Turn::Right => "D",
                            Turn::Back => "S",
                        },
                        exit.as_ref()
                            .and_then(|(_, e)| e.known)
                            .map_or("Unexplored room", game_sim::run::room_name),
                    );
                    c.enabled = exit.is_some()
                        && game.floor.current_room().cleared
                        && self.view == View::Profile;
                    out.push(c);
                }
                if matches!(game.floor.current_room().kind, RoomKind::Stairs { .. }) {
                    out.push(choice(
                        [507.0, 313.0, 113.0, 29.0],
                        Action::Descend,
                        "Descend",
                        "Enter the next floor",
                    ));
                } else {
                    out.push(icon(
                        [583.0, 313.0, 29.0, 29.0],
                        Action::Map,
                        "assets/textures/icons/floor_icon.png",
                        "Floor map [M]",
                    ));
                }
            }
            Mode::Combat => {
                out.push(icon(
                    [325.0, 313.0, 29.0, 29.0],
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
                            [365.0 + i as f32 * 64.0, 313.0, 56.0, 29.0],
                            Action::Skill(i),
                            format!("{}", i + 1),
                            skill.name.clone(),
                        );
                        c.label.clear();
                        c.icon = Some(if skill.tags.has(game_sim::tags::Tag::Attack) {
                            "assets/textures/icons/attack_icon.png"
                        } else {
                            "assets/textures/icons/magic_damage_icon.png"
                        });
                        c.tip
                            .extend([format!("{cost:.0} {resource}"), skill.supports.join(", ")]);
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
                    rects.merchant_leave,
                    Action::Leave,
                    "assets/textures/ui/leave_icon.png",
                    "Leave merchant [Esc]",
                ));
                let mut refresh = icon(
                    rects.merchant_refresh,
                    Action::Refresh,
                    "assets/textures/icons/refresh_icon.png",
                    "Refresh unlocked gems",
                );
                if let Some(shop) = game.shop() {
                    refresh.tip.push(format!("{} gold", shop.refresh_cost()));
                    refresh.enabled = game.character.gold >= shop.refresh_cost();
                    for (i, (rect, gem)) in rects.merchant.iter().zip(&shop.stock).enumerate() {
                        if let Some(g) = gem {
                            let price = game_sim::merchant::gem_price(&content::get().gems, g);
                            let mut c =
                                choice(*rect, Action::Buy(i), "", game_sim::run::gem_name(&g.id));
                            c.tip = tip_lines(&Loot::Gem(g.clone()));
                            c.tip.push(format!("{price} gold / Right-click to lock"));
                            c.enabled = game.character.gold >= price;
                            out.push(c);
                        }
                    }
                }
                out.push(refresh);
            }
            Mode::Shrine(offers) => {
                out.push(icon(
                    rects.merchant_leave,
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
                        [380.0 + i as f32 * 76.0, 313.0, 68.0, 29.0],
                        Action::Boon(i),
                        format!("{}", i + 1),
                        name,
                    ));
                }
            }
            Mode::Event { id, outcome } => {
                if outcome.is_some() {
                    out.push(choice(
                        [425.0, 313.0, 100.0, 29.0],
                        Action::Leave,
                        "Continue",
                        "Leave event",
                    ));
                } else if let Some(event) = content::get().events.iter().find(|e| e.id == *id) {
                    for (i, c) in event.choices.iter().enumerate() {
                        out.push(choice(
                            [365.0 + i as f32 * 80.0, 313.0, 72.0, 29.0],
                            Action::Event(i),
                            format!("{}", i + 1),
                            c.label.clone(),
                        ));
                    }
                }
            }
            Mode::Altar => {
                out.push(icon(
                    rects.merchant_leave,
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
                        [420.0, 313.0, 125.0, 29.0],
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
            out.push(icon(
                rects.refresh,
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
                    match result {
                        Ok(()) => ui.audio.push(("assets/sfx/ui/rune_select.wav".into(), 5.0)),
                        Err(e) => self.game_mut().message = game_sim::run::gear_error(e).into(),
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
                    game_sim::run::gem_name(id),
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
                        game_sim::gems::Targeting::Single => "one".to_owned(),
                        game_sim::gems::Targeting::All => "all".to_owned(),
                        game_sim::gems::Targeting::Chain(n) => format!("chain {n}"),
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
            &megatower_assets::godot_layout::Layout {
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
            if let Some(e) = game.battle.as_ref().and_then(|b| b.enemies.get(i)) {
                let path = match e.intent().kind {
                    game_sim::combat::IntentKind::Attack(_) => {
                        "assets/textures/icons/attack_icon.png"
                    }
                    game_sim::combat::IntentKind::Guard { .. } => {
                        "assets/textures/icons/guard_icon.png"
                    }
                    game_sim::combat::IntentKind::Heal { .. } => {
                        "assets/textures/icons/max_health_icon.png"
                    }
                };
                let rect = intent_rect(cx);
                out.push(tinted(rect, path, palette(8)));
                out.push(text(
                    [rect[0] + 18.0, rect[1] + 2.0, 40.0, 13.0],
                    intent_amount(e),
                    palette(8),
                    Align::Left,
                ));
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
                if let Some(g) = gem {
                    loot_draw(&mut out, *rect, &Loot::Gem(g.clone()));
                    out.push(text(
                        [rect[0], rect[1] + 34.0, 34.0, 13.0],
                        game_sim::merchant::gem_price(&content::get().gems, g).to_string(),
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
                out.push(text(
                    [c.rect[0] + 3.0, c.rect[1], 12.0, 13.0],
                    (i + 1).to_string(),
                    palette(1),
                    Align::Left,
                ));
            }
        }
        if !game.message.is_empty() {
            let message: String = game.message.chars().take(50).collect();
            out.push(compose::nine_patch(
                [318.0, 273.0, 280.0, 16.0],
                &rects.small_panel,
                palette(1),
            ));
            out.push(text(
                [322.0, 272.0, 272.0, 16.0],
                message,
                palette(8),
                Align::Center,
            ));
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
                tooltip(&mut out, rects, cursor, c.tip.clone());
            } else if let Some(i) = compose::hit_enemy(&frame, cursor[0], cursor[1]).or_else(|| {
                frame.enemies.iter().enumerate().find_map(|(i, e)| {
                    e.as_ref()
                        .filter(|e| !e.out_of_combat)
                        .filter(|_| {
                            contains(
                                intent_rect(crate::frame::enemy_cx(&frame, i)),
                                cursor[0],
                                cursor[1],
                            )
                        })
                        .map(|_| i)
                })
            }) && let Some(e) = game.battle.as_ref().and_then(|b| b.enemies.get(i))
            {
                let mut lines = vec![
                    e.fighter.name.clone(),
                    format!("Next: {} ({})", e.intent().name, intent_amount(e)),
                ];
                lines.extend(
                    game_sim::ailment::Ailment::ALL
                        .into_iter()
                        .filter(|a| e.fighter.afflictions.has(*a))
                        .map(|a| format!("{a:?}")),
                );
                tooltip(&mut out, rects, cursor, lines);
            } else if contains(XP_BAR, cursor[0], cursor[1]) {
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
                            game_sim::passives::xp_to_next(progress.level)
                        ),
                        format!("{} passive points to spend", progress.points),
                    ],
                );
            } else if self.view == View::Tree
                && let Some(id) = self.tree_hit(cursor[0], cursor[1])
                && let Some(node) = content::get().tree.nodes.iter().find(|n| n.id == id)
            {
                let mut lines = vec![
                    node.name.clone(),
                    "Click to allocate / Right-click to refund".into(),
                ];
                lines.extend(
                    node.mods
                        .iter()
                        .map(|m| format!("{:+.0} {:?}", m.value, m.stat)),
                );
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
                    let mut lines = tip_lines(&Loot::Gem(g.clone()));
                    lines.push("Right-click to unsocket".into());
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
                &megatower_assets::godot_layout::death_screen_layout(),
                &format!("Died on floor {}", game.floor.number),
                None,
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
                .unwrap_or("assets/textures/portraits/fren.png"),
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
        let v = &frame.player;
        // Use the original animated bars, numerical panels and segmented mana
        // pip. Armour here is only game-sim's temporary guard, not a new rule.
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
            rects.bar_top[2] - compose::mana_width(v.max_mana) - 4.0
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
            let right = rects.mana[0] + rects.mana[2] + 2.0;
            let width = compose::mana_pip(right, health[1] - 1.0, v.max_mana, v.mana, out);
            compose::label_panel(
                rects,
                right - width / 2.0,
                health[1] - 9.0,
                &format!("{} / {}", v.mana, v.max_mana),
                hex(0x5b8def),
                out,
            );
        }
        // Experience toward the next level: a thin bar under the portrait.
        let progress = &game.character.progress;
        let next = game_sim::passives::xp_to_next(progress.level).max(1);
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
        out.push(Draw::Fill {
            rect: [7.0, 28.0, 295.0, 262.0],
            color: palette(1),
        });
        let game = self.game();
        let lines = if ui.tab == Tab::Log {
            self.log.iter().rev().take(14).cloned().collect::<Vec<_>>()
        } else {
            let f = game.character.fighter();
            vec![
                format!("Level {}", game.character.progress.level),
                format!("Life {:.0} / {:.0}", f.life, f.max_life),
                format!("Mana {:.0}", f.max_mana),
                format!("Armour {:.0}", f.defence().armour),
                format!("Evasion {:.0}%", f.evasion()),
                format!("{} passive points [P]", game.character.progress.points),
                "Map [M] / Equipment [I]".into(),
            ]
        };
        for (i, line) in lines.iter().enumerate() {
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
                game_sim::labyrinth::Dir::East,
                game_sim::labyrinth::Dir::South,
            ] {
                let known = floor.neighbor(p, dir).is_some_and(|n| {
                    room.visibility == Visibility::Visited
                        || floor.room(n).visibility == Visibility::Visited
                });
                if room.has_door(dir) && known {
                    out.push(Draw::Fill {
                        rect: if dir == game_sim::labyrinth::Dir::East {
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
            out.push(text(
                [x, y + 2.0, cell, 16.0],
                mark,
                if p == floor.current {
                    palette(1)
                } else {
                    WHITE
                },
                Align::Center,
            ));
            if p == floor.current {
                use game_sim::labyrinth::Dir;
                let notch = match floor.facing {
                    Dir::North => [x + cell * 0.3, y, cell * 0.4, 2.0],
                    Dir::South => [x + cell * 0.3, y + cell - 2.0, cell * 0.4, 2.0],
                    Dir::West => [x, y + cell * 0.3, 2.0, cell * 0.4],
                    Dir::East => [x + cell - 2.0, y + cell * 0.3, 2.0, cell * 0.4],
                };
                out.push(Draw::Fill {
                    rect: notch,
                    color: palette(1),
                });
            }
        }
    }

    fn tree_positions(&self) -> Vec<(String, [f32; 2])> {
        let nodes = &content::get().tree.nodes;
        let minx = nodes.iter().map(|n| n.x).fold(f32::INFINITY, f32::min);
        let maxx = nodes.iter().map(|n| n.x).fold(f32::NEG_INFINITY, f32::max);
        let miny = nodes.iter().map(|n| n.y).fold(f32::INFINITY, f32::min);
        let maxy = nodes.iter().map(|n| n.y).fold(f32::NEG_INFINITY, f32::max);
        let scale = (250.0 / (maxx - minx).max(1.0)).min(205.0 / (maxy - miny).max(1.0));
        nodes
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    [28.0 + (n.x - minx) * scale, 65.0 + (n.y - miny) * scale],
                )
            })
            .collect()
    }
    fn tree_hit(&self, x: f32, y: f32) -> Option<String> {
        self.tree_positions()
            .into_iter()
            .find(|(_, p)| (x - p[0]).abs() < 6.0 && (y - p[1]).abs() < 6.0)
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
                    color: palette(2),
                });
            }
        }
        for (id, p) in points {
            let allocated = self.game().character.progress.is_allocated(&id);
            out.push(Draw::Image {
                rect: [p[0] - 6.0, p[1] - 6.0, 12.0, 12.0],
                path: "assets/textures/ui/small_tree_node.png".into(),
                region: Some([if allocated { 13.0 } else { 0.0 }, 0.0, 13.0, 13.0]),
                tint: if allocated { palette(8) } else { palette(4) },
                slice: None,
                flip_x: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_intents_show_their_numbers() {
        use game_sim::combat::{encounter, spawn};
        let content = content::get();
        let rat = spawn(&content.enemies[0], 1, false);
        assert_eq!(intent_amount(&rat), "3-5");
        let mut rng = game_sim::rng::Rng::seeded(1);
        let boss = encounter(
            RoomKind::Stairs { boss: true },
            3,
            &content.enemies,
            &mut rng,
        )
        .unwrap();
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
            Rects::from_layout(&megatower_assets::godot_layout::recovered_layout()),
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
    fn enemy_intent_clears_both_status_icons_and_the_tallest_stats_panel() {
        let layout = megatower_assets::godot_layout::enemy_container_layout();
        let intent = intent_rect(474.0);
        let status_top =
            compose::ENEMY_TOP + layout.rect("MarginContainer3/EffectContainerHolder")[1] - 14.0;
        let panel_top = compose::ENEMY_TOP + layout.rect("MarginContainer2/Panel/ColorRect")[1];
        assert!(intent[1] + intent[3] < status_top);
        assert!(intent[1] + intent[3] < panel_top);
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
    fn original_merchant_cells_buy_and_lock_the_game_sims_stock() {
        let (mut session, mut ui, rects) = fixture();
        session
            .game_mut()
            .debug_walk_to_kind(|k| k == RoomKind::Merchant);
        session.game_mut().character.gold = 1000;
        let rect = rects.merchant[0];
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
