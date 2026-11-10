//! The Legacy screen opened from the main menu: Legacy points, and the
//! unlocks they buy.
use super::*;
use crate::compose::{self, Align, Draw, WHITE, hex, palette, text};
use megatower_sim::legacy::UNLOCKS;

/// The first row of the left column (starters and head starts); content
/// unlocks fill the right one.
const ROW: [f32; 4] = [40.0, 76.0, 270.0, 20.0];
const COLUMN_GAP: f32 = 290.0;
const ROW_GAP: f32 = 23.0;
/// What the hovered unlock does.
const ABOUT: [f32; 4] = [20.0, 307.0, 600.0, 13.0];
const BACK: [f32; 4] = [270.0, 324.0, 100.0, 26.0];

fn row(i: usize) -> [f32; 4] {
    let content = UNLOCKS[i].reward.is_content();
    let line = UNLOCKS[..i]
        .iter()
        .filter(|u| u.reward.is_content() == content)
        .count();
    [
        ROW[0] + if content { COLUMN_GAP } else { 0.0 },
        ROW[1] + line as f32 * ROW_GAP,
        ROW[2],
        ROW[3],
    ]
}

impl Labyrinth {
    /// One frame of the screen: clicks buy unlocks, Escape or Back leaves.
    pub(crate) fn legacy_frame(
        &mut self,
        ui: &mut UiState,
        rects: &Rects,
        keys: &ButtonInput<KeyCode>,
        click: bool,
    ) -> Vec<Draw> {
        if let (true, Some([x, y])) = (click, ui.cursor) {
            if contains(BACK, x, y) {
                ui.legacy_open = false;
            } else if let Some(i) = (0..UNLOCKS.len()).find(|&i| contains(row(i), x, y))
                && self.legacy.buy(UNLOCKS[i].id)
            {
                self.save_legacy();
            }
        }
        if keys.just_pressed(KeyCode::Escape) {
            ui.legacy_open = false;
        }
        let mut out = vec![Draw::Fill {
            rect: [0.0, 0.0, 640.0, 360.0],
            color: palette(1),
        }];
        out.push(text(
            [120.0, 22.0, 400.0, 24.0],
            "LEGACY",
            WHITE,
            Align::Center,
        ));
        let next = self
            .legacy
            .next_milestone()
            .map(|m| format!("  |  next: floor {} (+{})", m.floor, m.points))
            .unwrap_or_default();
        out.push(text(
            [20.0, 44.0, 600.0, 13.0],
            format!(
                "{} points  |  best floor {}  |  runs {}{next}",
                self.legacy.points, self.legacy.best_floor, self.legacy.runs
            ),
            palette(8),
            Align::Center,
        ));
        let records: Vec<String> = megatower_sim::content::get()
            .gems
            .starters()
            .filter(|g| self.legacy.best_with(&g.id) > 0)
            .map(|g| format!("{} {}", g.name, self.legacy.best_with(&g.id)))
            .collect();
        if !records.is_empty() {
            out.push(text(
                [20.0, 58.0, 600.0, 13.0],
                format!("Best floors: {}", records.join(", ")),
                hex(0xffaa00),
                Align::Center,
            ));
        }
        for (i, u) in UNLOCKS.iter().enumerate() {
            let owned = self.legacy.owns(u.id);
            let label = if owned {
                format!("{} - owned", u.name)
            } else {
                format!("{} - {}", u.name, u.cost)
            };
            let affordable = !owned && self.legacy.points >= u.cost;
            // The rows are lower than a stock button, so the label is placed
            // here instead of by `compose::button`.
            let r = row(i);
            compose::button(rects, r, "", None, affordable, 4, &mut out);
            out.push(text(
                [r[0], r[1] + 2.0, r[2], 13.0],
                label,
                palette(if affordable { 1 } else { 4 }),
                Align::Center,
            ));
        }
        if let Some(u) = ui.cursor.and_then(|[x, y]| {
            UNLOCKS
                .iter()
                .enumerate()
                .find_map(|(i, u)| contains(row(i), x, y).then_some(u))
        }) {
            out.push(text(ABOUT, u.describe(), WHITE, Align::Center));
        }
        compose::button(rects, BACK, "Back", None, true, 4, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_above_the_back_button() {
        for (i, u) in UNLOCKS.iter().enumerate() {
            let r = row(i);
            assert!(r[1] + r[3] <= ABOUT[1], "{}", u.name);
            assert!(r[0] >= 0.0 && r[0] + r[2] <= 640.0);
            for j in 0..i {
                assert_ne!(row(j), r, "{} overlaps", u.name);
            }
        }
        const { assert!(ABOUT[1] + ABOUT[3] <= BACK[1]) };
    }

    #[test]
    fn every_unlock_says_what_it_does() {
        for u in UNLOCKS {
            assert!(u.describe().len() > 10, "{}", u.name);
        }
    }
}
