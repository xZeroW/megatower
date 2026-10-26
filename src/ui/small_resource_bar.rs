//! `small_resource_bar.gd` fixed-width mapping.

/// Source `_process` changes width only for these exact maximum values.
pub fn width_for_maximum(maximum: f64, current_width: f32) -> f32 {
    match maximum {
        40.0 => 31.0,
        30.0 => 24.0,
        20.0 => 17.0,
        10.0 => 10.0,
        _ => current_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_maximum_values_match_source_widths() {
        assert_eq!(width_for_maximum(40.0, 1.0), 31.0);
        assert_eq!(width_for_maximum(30.0, 1.0), 24.0);
        assert_eq!(width_for_maximum(20.0, 1.0), 17.0);
        assert_eq!(width_for_maximum(10.0, 1.0), 10.0);
        assert_eq!(width_for_maximum(40.1, 12.0), 12.0);
    }
}
