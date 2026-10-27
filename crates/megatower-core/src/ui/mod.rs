//! Bevy front end over `run::Game`. Gameplay runs to completion inside
//! `Game`; this layer plays back its event timeline, translates input into
//! game actions and draws the HUD from `compose`.
//!
//! Each frame runs the chained [`UiSystems`]: input is collected, the debug
//! script may act, the current screen updates and composes its draw list, the
//! run's side effects (profile, sound) are applied, and the draw list is
//! turned into UI nodes.

pub mod adventurer_ui;
pub mod battle_log_pages;
pub mod battle_speed;
pub mod compose;
pub mod containers;
pub mod death_screen;
pub mod difficulty_progress;
pub mod dynamic_slots;
pub mod effect_container;
pub mod effect_holder;
pub mod enemy_container;
pub mod enemy_holder;
pub mod gameplay_canvas;
pub mod hover;
pub mod hover_manager;
pub mod hover_rules;
pub mod hud_rules;
pub mod item_visual;
pub mod library;
pub mod library_planner;
pub mod library_state;
pub mod lobby;
pub mod market_xp;
pub mod memory_selection;
pub mod model;
pub mod navigation;
pub mod options_screen;
pub mod panels;
pub mod popups;
pub mod popups2;
pub mod resource_bar;
pub mod screen_flash;
pub mod screens;
pub mod small_resource_bar;
pub mod stat_label;
pub mod stats_scroll;
pub mod synergy;
pub mod tab_button;
pub mod turn_timer;
pub mod widgets;
pub mod widgets_text;

mod draw;
mod frame;
mod input;
mod library_data;
mod overlays;
mod window_options;

use crate::{
    gameplay_input::{GameAction, GameplayCommand, InputContext, commands_for_actions},
    godot_layout::recovered_layout,
};
use bevy::{prelude::*, window::PrimaryWindow};
use bevy_auto_plugin::prelude::*;
use compose::{
    Action, Align, Draw, action_buttons, compose, hit_slot, market_container, new_run_rect,
};
#[cfg(test)]
use frame::{adventurer_cells, select_draws};
pub(crate) use frame::{refresh, run_action};
pub use input::FrameInput;
use megatower_game::combat::turn::TurnType;
use megatower_game::run::{
    events::{FlashKind, Fx, Popup},
    game::Game,
    gameplay::NewRun,
    state::Player,
};
use model::*;
use panels::{DropAction, drop_targets, format_log};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
#[auto_add_plugin(plugin = crate::app::GamePlugin)]
pub struct UiPlugin;

/// The run in progress, if any.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = UiPlugin)]
pub struct GameSlot(pub Option<Game>);

#[derive(Resource)]
#[auto_insert_resource(plugin = UiPlugin, insert(UiRes(UiState::default())))]
pub struct UiRes(pub UiState);

/// Rectangles of the Godot-resolved gameplay canvas.
#[derive(Resource)]
#[auto_insert_resource(plugin = UiPlugin, insert(RectsRes(Rects::from_layout(&recovered_layout()))))]
pub struct RectsRes(pub Rects);

/// The per-frame UI pipeline, run in declaration order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(plugin = UiPlugin, schedule = Update, chain)]
pub enum UiSystems {
    Input,
    Debug,
    Screens,
    Effects,
    Render,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godot_layout::recovered_layout;
    use megatower_game::run::gameplay::NewRun;

    /// Compose the HUD at every step of bot-driven runs, in every tab, with the
    /// stash open and an item dragged: no panics, and drawing is deterministic.
    #[test]
    fn hud_composes_for_every_state_of_bot_runs() {
        let rects = Rects::from_layout(&recovered_layout());
        for (n, name) in ["Fren", "Pyry", "Elys", "Tyr", "Amon", "Jace"]
            .into_iter()
            .enumerate()
        {
            let mut game = Game::new(NewRun::default_for(name, 100 + n as u64));
            let mut ui = UiState::default();
            ui.screen = Screen::Playing;
            for step in 0..400 {
                game.tick(0.016);
                ui.tab = [Tab::Profile, Tab::Stats, Tab::Log][step % 3];
                ui.stash_open = step % 7 == 0;
                ui.cursor = Some([(step * 13 % 640) as f32, (step * 29 % 360) as f32]);
                let local = game.local();
                let inventory = game.state(local).inventory;
                ui.dragged = if step % 5 == 0 {
                    Some(megatower_game::inventory::Slot {
                        container: Some(inventory),
                        index: (step % 20) as i32,
                    })
                } else {
                    None
                };
                let first = compose(&mut game, &ui, &rects);
                let second = compose(&mut game, &ui, &rects);
                assert_eq!(first, second, "{name} step {step}");
                let _ = action_buttons(&mut game, &rects);
                if game.memory.is_game_ended {
                    break;
                }
                // Bot: buy, equip, fight/advance.
                let market = market_container(&game).unwrap();
                for i in 0..4 {
                    game.try_to_buy_item(market, i);
                }
                for index in 0..20 {
                    game.alt_press(megatower_game::inventory::Slot {
                        container: Some(inventory),
                        index,
                    });
                }
                game.primary_action();
                for event in game.take_events() {
                    if let Fx::Log(entry) = event.fx {
                        let _ = format_log(&game, &entry);
                    }
                }
            }
        }
    }

    #[test]
    fn market_item_tooltips_follow_the_source_structure() {
        let mut game = Game::new(NewRun::default_for("Fren", 5));
        let market = market_container(&game).unwrap();
        let mut seen = 0;
        for i in 0..4 {
            let slot = megatower_game::inventory::Slot {
                container: Some(market),
                index: i,
            };
            let Some(item) = slot.get_item(&game.world) else {
                continue;
            };
            let tip = compose::item_tip(&game, item).expect("valid market item has a tooltip");
            // The first row is the rarity-prefixed translated name; stat rows
            // follow with a leading amount segment.
            let name: String = tip.rows[0]
                .iter()
                .filter_map(|s| match s {
                    compose::Seg::Text(t, _) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert!(!name.is_empty() && name != "???", "{name}");
            for row in &tip.rows[1..] {
                assert!(!row.is_empty());
            }
            assert!(tip.price.is_some(), "market items show their price");
            seen += 1;
        }
        assert!(seen > 0);
        game.tick(0.016);
    }

    #[test]
    fn every_status_effect_has_a_tooltip_with_footer() {
        let game = Game::new(NewRun::default_for("Fren", 5));
        for policy in &game.rules.status_policies.0 {
            let tip =
                compose::status_tip(&game, policy.id).unwrap_or_else(|| panic!("{:?}", policy.id));
            let footer: String = tip
                .rows
                .last()
                .unwrap()
                .iter()
                .filter_map(|s| match s {
                    compose::Seg::Text(t, _) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert!(footer.contains(", "), "{:?}: {footer}", policy.id);
        }
    }

    #[test]
    fn select_screen_lists_all_adventurers() {
        let rects = Rects::from_layout(&recovered_layout());
        let catalog = megatower_game::adventurers::AdventurerCatalog::recovered();
        let draws = select_draws(&rects, &catalog, 0);
        let names = draws
            .iter()
            .filter(|d| matches!(d, Draw::Text { text, .. } if text == "JACE"))
            .count();
        assert_eq!(names, 1);
        assert_eq!(adventurer_cells(11).len(), 11);
    }
}
