//! `effect_container_holder.gd` slot-count and visibility planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectSlot {
    Hidden,
    InvalidVisible,
    Visible,
}
/// Existing children are only grown, never removed; every update hides first.
pub fn update_slots(existing: usize, effects_valid: &[bool]) -> Vec<EffectSlot> {
    let count = existing.max(effects_valid.len());
    (0..count)
        .map(|i| match effects_valid.get(i) {
            None => EffectSlot::Hidden,
            Some(false) => EffectSlot::InvalidVisible,
            Some(true) => EffectSlot::Visible,
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_grows_hides_and_keeps_invalid_entries_visible() {
        assert_eq!(
            update_slots(1, &[true, false]),
            vec![EffectSlot::Visible, EffectSlot::InvalidVisible]
        );
        assert_eq!(update_slots(3, &[]), vec![EffectSlot::Hidden; 3]);
    }
}
