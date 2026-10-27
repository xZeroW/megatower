//! `enemy_container_holder.gd` visible-child horizontal layout.
pub fn layout(
    widths: &[f32],
    current: &[f32],
    separation: f32,
    battle_speed: f32,
    delta: f32,
) -> Vec<f32> {
    let total = widths.iter().sum::<f32>() + separation * (widths.len().saturating_sub(1) as f32);
    let t = (10.0 * battle_speed * delta).min(1.0);
    let mut cursor = 0.0;
    widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let target = cursor - total * 0.5;
            cursor += w + separation;
            current.get(i).copied().unwrap_or(0.0)
                + (target - current.get(i).copied().unwrap_or(0.0)) * t
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_layout_and_lerp_match() {
        assert_eq!(layout(&[], &[], 20.0, 1.0, 0.1), Vec::<f32>::new());
        assert_eq!(
            layout(&[10.0, 20.0], &[0.0, 0.0], 5.0, 1.0, 1.0),
            vec![-17.5, -2.5]
        );
    }
}
