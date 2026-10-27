//! Library/profile screen widgets: `build_planner_container`,
//! `lore_container`, `library_passive_container`, `equipment_slot_*`,
//! `bestiary_container`, `set_texture_rect`.

use megaloot_profile::records::Profile;

/// `BuildPlannerContainer` next-page button: fix builds, add a build when
/// the selection is on the last one, then select the next build.
pub fn next_build_page<B>(
    profile: &mut Profile<B>,
    is_empty: impl Fn(&B) -> bool,
    new_build: impl FnOnce() -> B,
) {
    profile.fix_builds(is_empty);
    if profile.builds.len() < profile.selected_build_idx + 2 {
        profile.builds.push(new_build());
    }
    profile.selected_build_idx = (profile.selected_build_idx + 1).min(profile.builds.len());
}

/// Last-page button: select the previous build (never below 0 when the
/// button is enabled), then fix builds.
pub fn previous_build_page<B>(profile: &mut Profile<B>, is_empty: impl Fn(&B) -> bool) {
    profile.selected_build_idx = profile.selected_build_idx.saturating_sub(1);
    profile.fix_builds(is_empty);
}

/// `process_page_arrow_buttons`: (next_disabled, last_disabled).
pub fn build_arrows_disabled(selected_build_has_items: bool, selected_idx: usize) -> (bool, bool) {
    (!selected_build_has_items, selected_idx == 0)
}

pub fn build_label(selected_idx: usize) -> String {
    format!("BUILD {}", selected_idx + 1)
}

/// `update_item_slots`: the dragged item's own slot shows empty (not on
/// mobile).
pub fn build_slot_hidden(
    dragging: bool,
    mobile: bool,
    same_container_resource: bool,
    dragged_index: usize,
    idx: usize,
) -> bool {
    dragging && !mobile && same_container_resource && dragged_index == idx
}

#[derive(Debug, PartialEq, Eq)]
pub struct ChapterButtons {
    /// Button labels in order; unlocked first, then "???".
    pub labels: Vec<String>,
    pub unlocked: usize,
    /// Chapter selected after `update_adventurer`.
    pub selected: Option<usize>,
}

/// `LoreContainer.update_adventurer`: only the first chapter unlocks, once
/// the adventurer's floor record is above 10.
pub fn chapter_buttons(
    chapter_count: usize,
    floor_record: i32,
    chapter_word: &str,
) -> ChapterButtons {
    if chapter_count == 0 {
        return ChapterButtons {
            labels: vec![],
            unlocked: 0,
            selected: None,
        };
    }
    let unlocked = usize::from(floor_record > 10);
    let mut labels: Vec<String> = (0..unlocked)
        .map(|i| format!("{chapter_word} {}", i + 1))
        .collect();
    labels.extend((0..chapter_count - unlocked).map(|_| "???".to_string()));
    ChapterButtons {
        labels,
        unlocked,
        selected: (unlocked > 0).then_some(0),
    }
}

/// `select_chapter`: indexes beyond the unlocked chapters are ignored.
pub fn select_chapter(new_idx: usize, unlocked: usize) -> Option<usize> {
    (new_idx < unlocked).then_some(new_idx)
}

/// `LibraryPassiveContainer` label brightness: dimmed when not hovered or
/// not interactable.
pub fn passive_brightness(base: f32, interactable: bool, hovered: bool) -> f32 {
    let mut b = base;
    if !interactable || hovered {
        b -= 0.5;
    }
    b.max(-0.75)
}

/// Source summary pieces appended to a passive's hover for its sources.
#[derive(Debug, PartialEq, Eq)]
pub enum SourceBit {
    Newline,
    Sources,
    Item(usize),
    Unknown,
    Comma,
}

pub fn passive_source_bits(sources: &[Option<usize>]) -> Vec<SourceBit> {
    let mut out = vec![];
    if !sources.is_empty() {
        out.extend([SourceBit::Newline, SourceBit::Newline, SourceBit::Sources]);
    }
    for (idx, s) in sources.iter().enumerate() {
        match s {
            Some(i) => out.push(SourceBit::Item(*i)),
            None => out.push(SourceBit::Unknown),
        }
        if idx != sources.len() - 1 {
            out.push(SourceBit::Comma);
        }
    }
    out
}

/// `EquipmentSlotContainer`/`EquipmentSlotPreviewContainer` highlight type:
/// highlight color while shown, primary color otherwise.
pub fn slot_highlight_color_type(highlighted: bool) -> megaloot_profile::records::GlobalColorType {
    if highlighted {
        megaloot_profile::records::GlobalColorType::Highlight
    } else {
        megaloot_profile::records::GlobalColorType::Primary
    }
}

/// `EquipmentSlotPreview.update_texture`.
pub fn preview_texture<'a>(socket_texture: &'a str, locked: bool) -> &'a str {
    if locked {
        "assets/textures/ui/equipment_lock.png"
    } else {
        socket_texture
    }
}

/// `BestiaryContainer.add_enemy`: unknown enemies use the placeholder.
pub fn bestiary_texture(enemy_texture: Option<&str>) -> String {
    enemy_texture
        .unwrap_or("assets/textures/enemies/unknown.png")
        .to_string()
}

/// `SetTextureRect` pop animation names.
pub fn set_pop_animation(error: bool) -> &'static str {
    if error { "pop_error" } else { "pop" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_paging() {
        let mut p = Profile::new(vec![1], vec![], "id".into());
        let empty = |b: &Vec<i32>| b.is_empty();
        next_build_page(&mut p, empty, Vec::new);
        assert_eq!((p.builds.len(), p.selected_build_idx), (2, 1));
        // The new second build is empty, so fix_builds on the next press drops it first.
        next_build_page(&mut p, empty, Vec::new);
        assert_eq!(p.builds.len(), 2);
        previous_build_page(&mut p, empty);
        assert_eq!(p.selected_build_idx, 0);
        assert_eq!(build_arrows_disabled(false, 0), (true, true));
        assert_eq!(build_label(0), "BUILD 1");
        assert!(build_slot_hidden(true, false, true, 3, 3));
        assert!(!build_slot_hidden(true, true, true, 3, 3));
    }

    #[test]
    fn lore_and_misc() {
        assert_eq!(
            chapter_buttons(3, 11, "Chapter"),
            ChapterButtons {
                labels: vec!["Chapter 1".into(), "???".into(), "???".into()],
                unlocked: 1,
                selected: Some(0)
            }
        );
        assert_eq!(chapter_buttons(2, 10, "Chapter").unlocked, 0);
        assert_eq!(chapter_buttons(0, 99, "Chapter").labels.len(), 0);
        assert_eq!(select_chapter(1, 1), None);
        assert_eq!(select_chapter(0, 1), Some(0));
        assert_eq!(passive_brightness(0.0, true, true), -0.5);
        assert_eq!(passive_brightness(0.0, false, false), -0.5);
        assert_eq!(passive_brightness(0.0, true, false), 0.0);
        assert_eq!(
            passive_source_bits(&[Some(1), None]),
            vec![
                SourceBit::Newline,
                SourceBit::Newline,
                SourceBit::Sources,
                SourceBit::Item(1),
                SourceBit::Comma,
                SourceBit::Unknown
            ]
        );
        assert!(passive_source_bits(&[]).is_empty());
        assert_eq!(
            preview_texture("a.png", true),
            "assets/textures/ui/equipment_lock.png"
        );
        assert_eq!(
            bestiary_texture(None),
            "assets/textures/enemies/unknown.png"
        );
        assert_eq!(set_pop_animation(true), "pop_error");
    }
}
