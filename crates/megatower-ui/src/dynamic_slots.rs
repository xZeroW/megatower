//! `dynamic_slot_container.gd` inventory-grid sizing.

/// Returns the source target count. The row calculation deliberately remains
/// fixed at five columns even when the GridContainer's configured `columns`
/// differs, because that is what the recovered script does.
pub fn target_slot_count(last_item_index: i32, min_rows: i32, columns: i32) -> i32 {
    let next_row = ((last_item_index + 1) as f32 / 5.0).ceil() as i32;
    (min_rows * columns).max(columns * next_row)
}

/// Computes the new child count and whether source code would instantiate or
/// free any slots.
pub fn update_slots(
    current_slots: i32,
    last_item_index: i32,
    min_rows: i32,
    columns: i32,
) -> (i32, bool) {
    let target = target_slot_count(last_item_index, min_rows, columns);
    (target, target != current_slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_keeps_minimum_then_grows_and_shrinks_in_full_rows() {
        assert_eq!(update_slots(0, -1, 4, 5), (20, true));
        assert_eq!(update_slots(20, 20, 4, 5), (25, true));
        assert_eq!(update_slots(25, 3, 4, 5), (20, true));
        assert_eq!(update_slots(20, 3, 4, 5), (20, false));
    }

    #[test]
    fn row_measurement_is_fixed_at_five_but_count_uses_grid_columns() {
        assert_eq!(target_slot_count(5, 1, 4), 8);
    }
}
