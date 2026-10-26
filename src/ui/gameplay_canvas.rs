//! Source-local rules from `gameplay_canvas_layer.gd` that do not require a
//! Godot scene tree. Bevy presentation binds these results to its own nodes.

/// `_on_stats_scroll_container_search_changed` visibility result, in source
/// display order. Godot's `to_lower`/`contains` becomes Unicode lowercase
/// matching; an empty query leaves every label visible.
pub fn stat_search_visibility<'a>(
    names: impl IntoIterator<Item = &'a str>,
    query: &str,
) -> Vec<bool> {
    let query = query.to_lowercase();
    names
        .into_iter()
        .map(|name| query.is_empty() || name.to_lowercase().contains(&query))
        .collect()
}

/// `GameplayCanvasLayer._process`: the parent armor holder is visible iff at
/// least one child is visible.
pub fn armor_holder_visible(children_visible: impl IntoIterator<Item = bool>) -> bool {
    children_visible.into_iter().any(|visible| visible)
}

/// `update_armor_info`'s presentation values. Magic Shield overrides the
/// normal armor color, including when active armor is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmorView {
    pub visible: bool,
    pub active_armor: f64,
    pub armor: f64,
    pub color: [f32; 4],
}

pub fn armor_view(active_armor: f64, armor: f64, magic_shield: bool) -> ArmorView {
    ArmorView {
        visible: armor != 0.0,
        active_armor,
        armor,
        color: if magic_shield {
            [0.5411765, 0.6941177, 1.0, 1.0]
        } else {
            [0.2901961, 0.3294118, 0.3843137, 1.0]
        },
    }
}

/// `get_item_slot` / `get_item_texture_rect` bounds rule.
pub fn indexed<T>(items: &[T], index: i32) -> Option<&T> {
    usize::try_from(index)
        .ok()
        .and_then(|index| items.get(index))
}

/// `update_item_slots`: a locally dragged item is hidden only from its own
/// container and only on non-mobile platforms.
pub fn displayed_item<T: Copy + Eq>(
    item: Option<T>,
    dragged: Option<T>,
    same_container: bool,
    mobile: bool,
) -> Option<T> {
    if !mobile && same_container && item.is_some() && item == dragged {
        None
    } else {
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_rules_match_the_recovered_local_branches() {
        assert_eq!(
            stat_search_visibility(["Max Health", "Armor"], "HEA"),
            [true, false]
        );
        assert!(armor_holder_visible([false, true]));
        assert!(!armor_holder_visible([false, false]));
        assert_eq!(
            armor_view(4.0, 0.0, true).color,
            [0.5411765, 0.6941177, 1.0, 1.0]
        );
        assert!(!armor_view(4.0, 0.0, true).visible);
        assert_eq!(indexed(&[3], -1), None);
        assert_eq!(displayed_item(Some(7), Some(7), true, false), None);
        assert_eq!(displayed_item(Some(7), Some(7), false, false), Some(7));
    }
}
