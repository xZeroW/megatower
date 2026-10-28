//! Catalog data for the library and profile screens, combined with what the
//! saved profile has discovered, and the run -> profile bridge.

use super::*;

#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn sync_profile_system(mut slot: ResMut<GameSlot>, mut ui: ResMut<UiRes>) {
    if let Some(game) = slot.0.as_mut() {
        sync_profile(game, &mut ui.0);
    }
}

/// `UserData` hooks: the profile learns the items and enemies a run meets
/// and its deepest floor. Practice runs never touch the profile.
pub(super) fn sync_profile(game: &mut Game, ui: &mut UiState) {
    use megatower_core::run::game::Encounter;
    let practice = game.memory.game_mode == megatower_core::game_mode::GameMode::Practice;
    let mut changed = false;
    for encounter in game.encounters.drain(..) {
        if practice {
            continue;
        }
        changed |= match encounter {
            Encounter::Enemy(id) => ui.profile.encounter_enemy(&id),
            Encounter::Item(id) => ui.profile.encounter_item(&id),
        };
    }
    if !practice {
        let name = game.player(game.local()).adventurer.name.clone();
        changed |= ui.profile.set_floor_record(&name, game.memory.floor_number);
    }
    if changed {
        ui.profile.save();
    }
}

/// Catalog data the library and profile screens draw from.
pub(super) struct LibraryCatalogs {
    adventurer_sources: Vec<(String, Option<String>)>,
    item_sources: Vec<(String, Option<String>)>,
    /// Enemy id and its passives.
    enemy_sources: Vec<(String, Vec<String>)>,
    /// Name, portrait, in `ProfileState` (name-sorted) order.
    adventurers: Vec<(String, Option<String>)>,
    passive_names: Vec<(String, String)>,
}

pub(super) fn catalogs() -> &'static LibraryCatalogs {
    static CATALOGS: std::sync::OnceLock<LibraryCatalogs> = std::sync::OnceLock::new();
    CATALOGS.get_or_init(|| {
        let raw = megatower_core::items::catalog::RawItemCatalog::recovered();
        let order: Vec<String> = raw.0.iter().map(|d| d.id.clone()).collect();
        let items = raw.initialize(&order, false).expect("item catalog");
        let adventurers = megatower_core::adventurers::AdventurerCatalog::recovered();
        let enemies = megatower_core::enemies::EnemyCatalog::recovered(false);
        let content = megatower_core::character::content::CharacterContent::recovered();
        let names: Vec<String> = adventurers.list.iter().map(|a| a.name.clone()).collect();
        LibraryCatalogs {
            adventurer_sources: adventurers
                .list
                .iter()
                .map(|a| (a.name.clone(), a.passive.clone()))
                .collect(),
            item_sources: items
                .list
                .iter()
                .filter_map(|id| items.get(id))
                .map(|d| (d.id.clone(), d.passive.clone()))
                .collect(),
            enemy_sources: enemies
                .list
                .iter()
                .map(|e| (e.id.clone(), e.passives.iter().flatten().cloned().collect()))
                .collect(),
            adventurers: library_state::sorted_adventurer_order(&names)
                .into_iter()
                .map(|i| {
                    (
                        adventurers.list[i].name.clone(),
                        adventurers.list[i].portrait.clone(),
                    )
                })
                .collect(),
            passive_names: content
                .passives
                .iter()
                .map(|p| (p.id.clone(), p.name.clone()))
                .collect(),
        }
    })
}

/// `update_passives`: grouped passives in insertion order, `Some(translated
/// name)` once any source is known, else `None` ("???").
pub(super) fn library_passives(
    profile: &megatower_core::profile_store::ProfileStore,
) -> Vec<Option<String>> {
    let c = catalogs();
    library_state::group_passives(
        &c.adventurer_sources,
        &c.item_sources,
        &c.enemy_sources,
        &profile.item_set(),
        &profile.enemy_set(),
    )
    .into_iter()
    .map(|(passive, _, unknown)| {
        (!unknown).then(|| {
            let name = c
                .passive_names
                .iter()
                .find(|(id, _)| *id == passive)
                .map_or(passive.as_str(), |(_, n)| n.as_str());
            megatower_core::translation::t(name, "passive-name")
        })
    })
    .collect()
}

/// `update_synergies`: every specialization with a synergy set, in item-set
/// order. Icons are the sets' own; the result takes the specialization color.
pub(super) fn library_synergies() -> &'static [screens::SynergyEntry] {
    static SYNERGIES: std::sync::OnceLock<Vec<screens::SynergyEntry>> = std::sync::OnceLock::new();
    SYNERGIES.get_or_init(|| {
        let content = megatower_core::character::content::CharacterContent::recovered();
        let mut entries = vec![];
        for set in &content.item_sets {
            for spec in content
                .specializations
                .iter()
                .filter(|s| s.original_item_set.as_deref() == Some(set.id.as_str()))
            {
                let Some(synergy_id) = &spec.synergy_item_set else {
                    continue;
                };
                let find = |id: &str| content.item_sets.iter().find(|s| s.id == id);
                let (Some(synergy), Some(color)) =
                    (find(synergy_id), spec.color(&content.item_sets).ok())
                else {
                    continue;
                };
                let (Some(main_icon), Some(orig_icon)) = (&synergy.icon, &set.icon) else {
                    continue;
                };
                entries.push(screens::SynergyEntry {
                    main: (main_icon.clone(), synergy.color),
                    synergy: (orig_icon.clone(), set.color),
                    result: (orig_icon.clone(), color),
                });
            }
        }
        entries
    })
}

/// `ProfileState.update_adventurers`: adventurers by name, each with the
/// border its floor record earns.
pub(super) fn profile_entries(
    profile: &megatower_core::profile_store::ProfileStore,
) -> Vec<screens::ProfileEntry> {
    catalogs()
        .adventurers
        .iter()
        .map(|(name, portrait)| screens::ProfileEntry {
            name: name.clone(),
            portrait: portrait.clone(),
            border: megatower_core::definitions::BorderType::from_floor(
                true,
                profile.floor_record(name),
            )
            .texture(),
        })
        .collect()
}
