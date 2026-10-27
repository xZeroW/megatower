//! Numeric rules translated from `classes/utils/math.gd`.
//! Randomness is supplied by the caller; this does not substitute a new RNG
//! algorithm for Godot's seeded streams.

use bevy::math::{DVec2, Rect};

/// Source truncates increasingly coarse lower digits for very large values.
pub fn big_round(value: f64) -> f64 {
    let unit = if value >= 1e16 {
        1e15
    } else if value >= 1e13 {
        1e12
    } else if value >= 1e10 {
        1e9
    } else if value >= 1e7 {
        1e6
    } else if value >= 1e4 {
        1e3
    } else {
        return value;
    };
    (value / unit).floor() * unit
}

pub fn bell(weight: f64, position: f64) -> f64 {
    let position = (position + 0.1) * 10.0;
    let p1 = std::f64::consts::E / (0.01 * (2.0 * std::f64::consts::PI).sqrt());
    0.1 + p1.powf(-(weight - position).powi(2) / (100.0 * position))
}

pub fn diminished_stat(floor: i32, smoothness: f64, growth: f64) -> f64 {
    if floor <= 0 {
        return 0.0;
    }
    (((f64::from(floor) + smoothness).ln() - smoothness.ln()) * growth).ceil()
}

pub fn refine(base: f64, floor: i32) -> f64 {
    if base.abs() < 0.00001 {
        return 0.0;
    }
    (base * (1.0 + 1.05 * f64::from(floor) / (base + 1.0)))
        .max(0.0)
        .ceil()
}

pub fn percentage(maximum: f64, value: f64) -> f64 {
    if maximum <= 0.0 {
        return 0.0;
    }
    (value / maximum * 100.0).round()
}

pub fn point_to_rect_distance(point: DVec2, rect: Rect) -> f64 {
    let distance = (rect.min.as_dvec2() - point)
        .max(DVec2::ZERO)
        .max(point - rect.max.as_dvec2());
    distance.length()
}

pub fn hausdorff_distance(a: Rect, b: Rect) -> f64 {
    fn directed(a: Rect, b: Rect) -> f64 {
        let min = a.min.as_dvec2();
        let max = a.max.as_dvec2();
        [min, DVec2::new(max.x, min.y), DVec2::new(min.x, max.y), max]
            .into_iter()
            .map(|point| point_to_rect_distance(point, b))
            .fold(0.0, f64::max)
    }
    directed(a, b).max(directed(b, a))
}

/// Source returns a normalized *sign vector* times the gap length, not the
/// shortest displacement vector. Unequal diagonal gaps must retain that quirk.
pub fn rect_distance(a: Rect, b: Rect) -> DVec2 {
    let gap = (b.min.as_dvec2() - a.max.as_dvec2()).max(DVec2::ZERO)
        - (a.min.as_dvec2() - b.max.as_dvec2()).max(DVec2::ZERO);
    let direction = DVec2::new(
        if gap.x == 0.0 { 0.0 } else { gap.x.signum() },
        if gap.y == 0.0 { 0.0 } else { gap.y.signum() },
    );
    direction.normalize_or_zero() * gap.length()
}

pub fn log_10(value: f64) -> f64 {
    value.ln() * std::f64::consts::LOG10_E
}

/// `sample` is the next source-compatible RNG `randf()` result in [0, 1].
/// Do not clamp chance: the recovered function does not do so either.
pub fn rand_success(chance: f64, sample: f64) -> bool {
    sample * 100.0 < chance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_edges_match_source() {
        assert_eq!(diminished_stat(0, 25.0, 7.5), 0.0);
        assert_eq!(diminished_stat(1, 25.0, 7.5), 1.0);
        assert_eq!(refine(0.000001, 20), 0.0);
        assert_eq!(refine(10.0, 10), 20.0);
        assert_eq!(percentage(0.0, 10.0), 0.0);
        assert_eq!(percentage(200.0, 1.0), 1.0);
        assert!(!rand_success(50.0, 0.5));
        assert!(rand_success(50.0, 0.499));
        assert!(rand_success(101.0, 1.0));
    }

    #[test]
    fn distances_preserve_source_diagonal_direction() {
        let a = Rect::new(0.0, 0.0, 1.0, 1.0);
        let b = Rect::new(4.0, 5.0, 5.0, 6.0);
        assert_eq!(point_to_rect_distance(DVec2::new(0.5, 0.5), a), 0.0);
        assert_eq!(point_to_rect_distance(b.min.as_dvec2(), a), 5.0);
        let result = rect_distance(a, b);
        assert!((result.length() - 5.0).abs() < 1e-12);
        assert_eq!(result.x, result.y);
        assert_eq!(rect_distance(b, a), -result);
        assert_eq!(rect_distance(a, a), DVec2::ZERO);
        assert_eq!(hausdorff_distance(a, b), 41.0_f64.sqrt());
        assert_eq!(hausdorff_distance(a, a), 0.0);
    }
}
