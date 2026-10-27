//! The Library's Items tab: the search box, rarity slider and Build Planner of
//! `library_state.tscn`. Library items are real `Item` entities in a private
//! `Game` world, so borders, stats and tooltips come from the same code as the
//! run screen.

use super::{
    compose::{
        Align, Draw, Seg, TipView, draw_tip, image, item_draws, item_tip, palette, text, tinted,
    },
    model::Rects,
    screens::{button_rect, layout_draws},
};
use bevy::prelude::Entity;
use megatower_assets::godot_layout::{Layout, Visual};
use megatower_core::items::{instance::Item, rarity::ItemRarity};
use megatower_core::profile_store::{BUILD_SIZE, BuildItem, ProfileStore};
use megatower_core::run::{game::Game, gameplay::NewRun};
use std::collections::HashMap;

const COLUMNS: usize = 9;
const CELL: f32 = 34.0;
const GRID_ORIGIN: [f32; 2] = [29.0, 85.0];
const VISIBLE_ROWS: usize = 5;
const MAX_RARITY: i32 = 8;

const BUILD_COLUMNS: usize = 5;
const BUILD_PITCH: f32 = 38.0;
const BUILD_VISIBLE_ROWS: usize = 5;
const BUILD_WIDTH: f32 = 186.0;

#[derive(Default)]
pub struct LibraryUi {
    pub search: String,
    pub focused: bool,
    pub rarity: i32,
    pub dragging: bool,
    pub build_scroll: usize,
    game: Option<Box<Game>>,
    /// One entity per `Items.LIST` entry; `None` while undiscovered.
    entities: Vec<Option<Entity>>,
    known: usize,
    build_cache: HashMap<(String, i32), Entity>,
}

pub struct Pointer {
    pub cursor: Option<[f32; 2]>,
    pub click: bool,
    pub held: bool,
    pub wheel_rows: i32,
}

#[derive(Default)]
pub struct Typed {
    pub text: String,
    pub backspace: bool,
}

fn contains(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn find<'a>(layout: &'a Layout, tail: &str) -> Option<&'a Visual> {
    layout.visuals.iter().find(|v| v.path.ends_with(tail))
}

impl LibraryUi {
    /// Create the private world and one item per discovered `Items.LIST`
    /// entry (`LibraryState._ready`).
    fn ensure(&mut self, profile: &ProfileStore) {
        if self.game.is_none() {
            self.game = Some(Box::new(Game::new(NewRun::default_for("Fren", 1))));
        }
        if self.known == profile.encountered_items.len() && !self.entities.is_empty() {
            return;
        }
        let rarity = ItemRarity::from_index(self.rarity).unwrap_or(ItemRarity::Common);
        let game = self.game.as_mut().expect("created above");
        for entity in self.entities.drain(..).flatten() {
            game.world.despawn(entity);
        }
        let seen = profile.item_set();
        for id in game.items.list.clone() {
            let entity = seen
                .contains(&id)
                .then(|| game.items.get(&id).cloned())
                .flatten()
                .map(|def| {
                    let floor = def.spawn_floor;
                    let e = game.new_item(def, floor, 0);
                    if let Some(mut item) = game.world.get_mut::<Item>(e) {
                        item.set_rarity(rarity);
                    }
                    e
                });
            self.entities.push(entity);
        }
        game.encounters.clear();
        self.known = profile.encountered_items.len();
    }

    /// `process_rarity_slider`: every library item takes the new rarity.
    fn set_rarity(&mut self, rarity: i32) {
        self.rarity = rarity.clamp(0, MAX_RARITY);
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let rarity = ItemRarity::from_index(self.rarity).unwrap_or(ItemRarity::Common);
        for entity in self.entities.iter().flatten() {
            if let Some(mut item) = game.world.get_mut::<Item>(*entity) {
                item.set_rarity(rarity);
            }
        }
    }

    fn game(&self) -> &Game {
        self.game.as_deref().expect("ensure() ran")
    }

    /// The text `Items.update_search` matches against: the item's tooltip
    /// lines, which carry its name, stats, passive and ability.
    fn haystack(&self, entity: Entity) -> String {
        item_tip(self.game(), entity).map_or_else(String::new, |tip| {
            tip.rows
                .iter()
                .flatten()
                .filter_map(|seg| match seg {
                    Seg::Text(t, _) => Some(t.as_str()),
                    Seg::Icon(..) => None,
                })
                .collect::<String>()
                .to_lowercase()
        })
    }

    /// Indices into `Items.LIST` shown for the current search; undiscovered
    /// items are hidden while searching.
    pub fn visible(&self) -> Vec<usize> {
        let needle = self.search.to_lowercase();
        (0..self.entities.len())
            .filter(|i| {
                needle.is_empty()
                    || self.entities[*i].is_some_and(|e| self.haystack(e).contains(&needle))
            })
            .collect()
    }

    fn build_entity(&mut self, item: &BuildItem) -> Option<Entity> {
        let key = (item.id.clone(), item.rarity);
        if let Some(e) = self.build_cache.get(&key) {
            return Some(*e);
        }
        let game = self.game.as_mut()?;
        let def = game.items.get(&item.id).cloned()?;
        let floor = def.spawn_floor;
        let e = game.new_item(def, floor, item.rarity);
        game.encounters.clear();
        self.build_cache.insert(key, e);
        Some(e)
    }
}

pub fn max_scroll(visible: usize) -> usize {
    visible.div_ceil(COLUMNS).saturating_sub(VISIBLE_ROWS)
}

fn item_cell(index_in_view: usize, scroll: usize) -> Option<[f32; 4]> {
    let row = index_in_view / COLUMNS;
    (row >= scroll && row < scroll + VISIBLE_ROWS).then(|| {
        [
            GRID_ORIGIN[0] + (index_in_view % COLUMNS) as f32 * CELL,
            GRID_ORIGIN[1] + (row - scroll) as f32 * CELL,
            CELL,
            CELL,
        ]
    })
}

fn build_rows(items: &[Option<BuildItem>]) -> usize {
    let last = items.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
    4.max(last.div_ceil(BUILD_COLUMNS))
        .min(BUILD_SIZE / BUILD_COLUMNS)
}

fn build_panel(layout: &Layout) -> Option<[f32; 4]> {
    find(layout, "BuildScrollContainer/MarginContainer/GenericPanel").map(|v| v.rect)
}

fn build_cell(layout: &Layout, slot: usize, scroll: usize) -> Option<[f32; 4]> {
    let panel = build_panel(layout)?;
    let row = slot / BUILD_COLUMNS;
    (row >= scroll && row < scroll + BUILD_VISIBLE_ROWS).then(|| {
        [
            panel[0] + (panel[2] - BUILD_WIDTH) / 2.0 + (slot % BUILD_COLUMNS) as f32 * BUILD_PITCH,
            panel[1] + 5.0 + (row - scroll) as f32 * BUILD_PITCH,
            CELL,
            CELL,
        ]
    })
}

fn slider_track(layout: &Layout) -> Option<[f32; 4]> {
    find(layout, "/RaritySlider/NinePatchRect").map(|v| v.rect)
}

fn search_box(layout: &Layout) -> Option<[f32; 4]> {
    find(layout, "/SearchLineEdit/GenericPanel").map(|v| v.rect)
}

pub fn build_max_scroll(rows: usize) -> usize {
    rows.saturating_sub(BUILD_VISIBLE_ROWS)
}

/// One frame of input. Returns true when the profile changed (save it).
pub fn handle(
    ui: &mut LibraryUi,
    profile: &mut ProfileStore,
    layout: &Layout,
    grid_scroll: &mut usize,
    p: &Pointer,
    typed: &Typed,
) -> bool {
    ui.ensure(profile);
    let mut changed = false;
    if ui.focused {
        if typed.backspace && ui.search.pop().is_some() {
            *grid_scroll = 0;
        }
        let before = ui.search.len();
        ui.search
            .extend(typed.text.chars().filter(|c| !c.is_control()));
        if ui.search.len() != before {
            *grid_scroll = 0;
        }
    }
    // Slider dragging continues until release.
    // A fresh click ends any previous drag before it is evaluated.
    if p.click {
        ui.dragging = false;
    }
    if ui.dragging {
        if let (Some(track), Some([x, _]), true) = (slider_track(layout), p.cursor, p.held) {
            let frac = ((x - track[0]) / track[2]).clamp(0.0, 1.0);
            let value = (frac * MAX_RARITY as f32).round() as i32;
            if value != ui.rarity {
                ui.set_rarity(value);
            }
        } else {
            ui.dragging = false;
        }
    }
    let panel = build_panel(layout);
    if let (Some(panel), Some([x, y])) = (panel, p.cursor)
        && p.wheel_rows != 0
        && contains(panel, x, y)
    {
        let rows = build_rows(profile.selected());
        ui.build_scroll = (ui.build_scroll as i32 + p.wheel_rows)
            .clamp(0, build_max_scroll(rows) as i32) as usize;
    }
    let (true, Some([x, y])) = (p.click, p.cursor) else {
        return changed;
    };
    ui.focused = search_box(layout).is_some_and(|r| contains(r, x, y));
    if let Some(t) = slider_track(layout)
        && contains([t[0] - 6.0, t[1] - 6.0, t[2] + 12.0, t[3] + 12.0], x, y)
    {
        ui.dragging = true;
        let frac = ((x - t[0]) / t[2]).clamp(0.0, 1.0);
        ui.set_rarity((frac * MAX_RARITY as f32).round() as i32);
    }
    // Library item -> first free slot of the selected build.
    let visible = ui.visible();
    for (view_idx, list_idx) in visible.iter().enumerate() {
        if item_cell(view_idx, *grid_scroll).is_some_and(|r| contains(r, x, y))
            && let Some(entity) = ui.entities[*list_idx]
        {
            let game = ui.game();
            let id = game
                .world
                .get::<Item>(entity)
                .and_then(|i| i.resource.as_ref().map(|d| d.id.clone()));
            if let Some(id) = id {
                changed |= profile.add_to_build(BuildItem {
                    id,
                    rarity: ui.rarity,
                });
            }
        }
    }
    // Build slot -> removed.
    let rows = build_rows(profile.selected());
    for slot in 0..rows * BUILD_COLUMNS {
        if build_cell(layout, slot, ui.build_scroll).is_some_and(|r| contains(r, x, y)) {
            changed |= profile.remove_from_build(slot);
        }
    }
    if button_rect(layout, "LastPageButton").is_some_and(|r| contains(r, x, y)) {
        profile.previous_build();
        ui.build_scroll = 0;
        changed = true;
    }
    if button_rect(layout, "NextPageButton").is_some_and(|r| contains(r, x, y))
        && profile.selected_has_items()
    {
        profile.next_build();
        ui.build_scroll = 0;
        changed = true;
    }
    if find(layout, "/DeleteBuildButton").is_some_and(|v| contains(v.rect, x, y)) {
        profile.clear_selected();
        ui.build_scroll = 0;
        changed = true;
    }
    changed
}

pub fn draws(
    ui: &mut LibraryUi,
    profile: &mut ProfileStore,
    layout: &Layout,
    grid_scroll: usize,
    cursor: Option<[f32; 2]>,
    rects: &Rects,
    blink_on: bool,
) -> Vec<Draw> {
    ui.ensure(profile);
    let selected_has_items = profile.selected_has_items();
    let selected_idx = profile.selected_build;
    let mut dimmed = vec!["BestiaryButton", "SynergiesButton", "PassivesButton"];
    if !selected_has_items {
        dimmed.push("NextPageButton");
    }
    if selected_idx == 0 {
        dimmed.push("LastPageButton");
    }
    // The slider grabber and the build label are dynamic.
    let visuals = layout
        .visuals
        .iter()
        .filter(|v| !v.path.ends_with("/GrabberTextureRect"))
        .cloned()
        .map(|mut v| {
            if v.path.ends_with("/SelectedBuildLabel") {
                v.text = Some(format!("BUILD {}", selected_idx + 1));
            }
            v
        })
        .collect();
    let mut out = layout_draws(
        &Layout {
            viewport: layout.viewport,
            visuals,
        },
        &dimmed,
    );

    // Search box text with a caret while focused.
    if let Some(r) = search_box(layout) {
        let mut shown = ui.search.clone();
        if ui.focused && blink_on {
            shown.push('|');
        }
        out.push(text(
            [r[0] + 5.0, r[1] + 3.0, r[2] - 10.0, 16.0],
            shown,
            palette(0),
            Align::Left,
        ));
    }
    // Rarity slider grabber.
    if let Some(t) = slider_track(layout) {
        let x = t[0] - 4.0 + t[2] * ui.rarity as f32 / MAX_RARITY as f32;
        out.push(tinted(
            [x, t[1] - 3.0, 8.0, 10.0],
            "assets/textures/ui/slider_grabber.png",
            palette(4),
        ));
    }

    let mut tooltip: Option<TipView> = None;
    // Item grid.
    let visible = ui.visible();
    for (view_idx, list_idx) in visible.iter().enumerate() {
        let Some(cell) = item_cell(view_idx, grid_scroll) else {
            continue;
        };
        match ui.entities[*list_idx] {
            Some(entity) => {
                item_draws(ui.game(), entity, cell, false, &mut out);
                if cursor.is_some_and(|[x, y]| contains(cell, x, y)) {
                    tooltip = item_tip(ui.game(), entity);
                }
            }
            None => out.push(image(
                [cell[0] + 1.0, cell[1] + 1.0, 32.0, 32.0],
                crate::item_visual::UNKNOWN_ITEM,
            )),
        }
    }
    // Build slots.
    let items: Vec<Option<BuildItem>> = profile.selected().to_vec();
    for slot in 0..build_rows(&items) * BUILD_COLUMNS {
        let Some(cell) = build_cell(layout, slot, ui.build_scroll) else {
            continue;
        };
        out.push(tinted(cell, "assets/textures/ui/item_slot.png", palette(4)));
        if let Some(Some(item)) = items.get(slot)
            && let Some(entity) = ui.build_entity(item)
        {
            item_draws(ui.game(), entity, cell, false, &mut out);
            if cursor.is_some_and(|[x, y]| contains(cell, x, y)) {
                tooltip = item_tip(ui.game(), entity);
            }
        }
    }
    if let (Some(tip), Some(c)) = (tooltip, cursor) {
        draw_tip(&mut out, rects, c, tip);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_knowing(names: &[&str]) -> ProfileStore {
        let game = Game::new(NewRun::default_for("Fren", 1));
        let ids = game
            .items
            .list
            .iter()
            .filter(|id| names.iter().any(|n| id.contains(n)))
            .cloned()
            .collect();
        ProfileStore::new(ids)
    }

    fn click(x: f32, y: f32) -> Pointer {
        Pointer {
            cursor: Some([x, y]),
            click: true,
            held: true,
            wheel_rows: 0,
        }
    }

    #[test]
    fn search_matches_tooltip_text_and_hides_undiscovered_items() {
        let profile = profile_knowing(&["steak", "zefrot"]);
        let mut ui = LibraryUi::default();
        ui.ensure(&profile);
        assert_eq!(ui.visible().len(), ui.entities.len());
        ui.search = "STEAK".into();
        let hits = ui.visible();
        assert_eq!(hits.len(), 1);
        let id = ui.game().items.list[hits[0]].clone();
        assert!(id.contains("steak"), "{id}");
        ui.search = "no such item".into();
        assert!(ui.visible().is_empty());
    }

    #[test]
    fn rarity_slider_and_build_planner_round_trip_through_clicks() {
        let layout = megatower_assets::godot_layout::library_layout();
        let mut profile = profile_knowing(&[""]);
        let mut ui = LibraryUi::default();
        let mut scroll = 0;
        let track = slider_track(&layout).unwrap();
        // Click 5/8 along the slider.
        let x = track[0] + track[2] * 5.0 / 8.0;
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &click(x, track[1] + 2.0),
            &Typed::default(),
        );
        assert_eq!(ui.rarity, 5);
        // Consumables and essentials keep their own rarity; pick a gear item.
        let gear = (0..ui.entities.len())
            .find(|i| {
                ui.entities[*i].is_some_and(|e| {
                    ui.game()
                        .world
                        .get::<Item>(e)
                        .unwrap()
                        .can_increase_rarity()
                })
            })
            .unwrap();
        let entity = ui.entities[gear].unwrap();
        assert_eq!(
            ui.game().world.get::<Item>(entity).unwrap().rarity() as i32,
            5
        );
        // Its cell adds it to the build at that rarity.
        let id = ui.game().items.list[gear].clone();
        let cell = item_cell(gear, 0).unwrap();
        let changed = handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &click(cell[0] + 5.0, cell[1] + 5.0),
            &Typed::default(),
        );
        assert!(changed);
        let first = profile.selected()[0].clone().unwrap();
        assert_eq!((first.id, first.rarity), (id, 5));
        // Clicking the filled build slot removes it again.
        let slot = build_cell(&layout, 0, 0).unwrap();
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &click(slot[0] + 5.0, slot[1] + 5.0),
            &Typed::default(),
        );
        assert!(!profile.selected_has_items());
    }

    #[test]
    fn typing_only_edits_the_search_box_while_it_has_focus() {
        let layout = megatower_assets::godot_layout::library_layout();
        let mut profile = profile_knowing(&["steak"]);
        let mut ui = LibraryUi::default();
        let mut scroll = 3;
        let typed = |t: &str| Typed {
            text: t.into(),
            backspace: false,
        };
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &Pointer {
                cursor: None,
                click: false,
                held: false,
                wheel_rows: 0,
            },
            &typed("ignored"),
        );
        assert_eq!(ui.search, "");
        let r = search_box(&layout).unwrap();
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &click(r[0] + 5.0, r[1] + 5.0),
            &Typed::default(),
        );
        assert!(ui.focused);
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &Pointer {
                cursor: None,
                click: false,
                held: false,
                wheel_rows: 0,
            },
            &typed("ste"),
        );
        assert_eq!((ui.search.as_str(), scroll), ("ste", 0));
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &Pointer {
                cursor: None,
                click: false,
                held: false,
                wheel_rows: 0,
            },
            &Typed {
                text: String::new(),
                backspace: true,
            },
        );
        assert_eq!(ui.search, "st");
        // Clicking elsewhere drops focus.
        handle(
            &mut ui,
            &mut profile,
            &layout,
            &mut scroll,
            &click(5.0, 5.0),
            &Typed::default(),
        );
        assert!(!ui.focused);
    }

    #[test]
    fn build_grid_grows_in_rows_and_scrolls() {
        let mut build = vec![None; BUILD_SIZE];
        assert_eq!(build_rows(&build), 4);
        build[22] = Some(BuildItem {
            id: "x".into(),
            rarity: 0,
        });
        assert_eq!(build_rows(&build), 5);
        build[34] = Some(BuildItem {
            id: "x".into(),
            rarity: 0,
        });
        assert_eq!(build_rows(&build), 7);
        assert_eq!(build_max_scroll(7), 2);
        assert_eq!(max_scroll(199), 18);
    }
}
