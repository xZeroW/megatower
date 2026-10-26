//! Market refresh quote rules from `Memory.get_market_refresh_price`,
//! `Math.big_round`, and `MarketManager.can_refresh_market`.

pub use crate::game_math::big_round;

/// Price returned by `MarketManager.get_refresh_price()` for an ordinary
/// market refresh. `base_gold_on_kill` is the result of the selected mode's
/// `get_gold_on_kill(null, floor_number)` call.
pub fn refresh_price(
    base_gold_on_kill: f64,
    floor_number: i32,
    room_index: i32,
    refresh_count: i32,
) -> f64 {
    if floor_number == 0 && room_index == -1 {
        return 2.0;
    }

    let base = base_gold_on_kill * 0.45;
    let growth = 1.0 + f64::from(refresh_count.min(3)) * 0.25;
    let mut price = base.powf(growth) + f64::from(refresh_count);
    price += (f64::from(floor_number) * 0.25).floor();
    if refresh_count > 3 {
        price *= f64::from(refresh_count - 2);
    }

    big_round(price.ceil())
}

/// The gates in `MarketManager.can_refresh_market()` after payment
/// affordability has been resolved by the character/economy system.
pub const fn can_refresh_market(
    price: f64,
    can_pay: bool,
    has_dragged_item: bool,
    player_died: bool,
) -> bool {
    price > 0.0 && can_pay && !has_dragged_item && !player_died
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Quote(f64, i32, i32, i32, f64);

    #[test]
    fn first_market_refresh_at_run_start_has_the_source_override_price() {
        assert_eq!(refresh_price(15.0, 0, -1, 0), 2.0);
    }

    #[test]
    fn normal_refresh_price_uses_floor_count_growth_and_late_multiplier() {
        assert_eq!(refresh_price(10.0, 1, 0, 0), 5.0);
        let expected = ((4.5_f64.powf(1.75) + 4.0) * 2.0).ceil();
        assert_eq!(refresh_price(10.0, 1, 0, 4), expected);
    }

    #[test]
    fn big_round_only_coarsens_exceptionally_large_quotes() {
        assert_eq!(big_round(9_999.0), 9_999.0);
        assert_eq!(big_round(12_345.0), 12_000.0);
        assert_eq!(big_round(1.234e10), 1.2e10);
    }

    #[test]
    fn refresh_requires_affordability_no_drag_and_a_living_player() {
        assert!(can_refresh_market(2.0, true, false, false));
        assert!(!can_refresh_market(0.0, true, false, false));
        assert!(!can_refresh_market(2.0, false, false, false));
        assert!(!can_refresh_market(2.0, true, true, false));
        assert!(!can_refresh_market(2.0, true, false, true));
    }

    #[test]
    fn refresh_quotes_match_recovered_market_path() {
        let rows: Vec<Quote> =
            serde_json::from_str(include_str!("../artifacts/market-quote-golden.json"))
                .expect("market quote oracle JSON");
        for Quote(base, floor, room, count, expected) in rows {
            assert_eq!(
                refresh_price(base, floor, room, count),
                expected,
                "base={base} floor={floor} room={room} count={count}"
            );
        }
    }
}
