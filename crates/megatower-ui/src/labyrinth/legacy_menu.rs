//! The Legacy screen opened from the main menu: Legacy points, and the
//! unlocks they buy.
use super::*;
use crate::compose::{self, Align, Draw, WHITE, palette, text};
use megatower_sim::legacy::UNLOCKS;

const ROW: [f32; 4] = [120.0, 74.0, 400.0, 26.0];
const ROW_GAP: f32 = 32.0;
const BACK: [f32; 4] = [270.0, 322.0, 100.0, 26.0];

fn row(i: usize) -> [f32; 4] {
    [ROW[0], ROW[1] + i as f32 * ROW_GAP, ROW[2], ROW[3]]
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
        out.push(text(
            [120.0, 46.0, 400.0, 13.0],
            format!(
                "{} points  |  best floor {}  |  runs {}",
                self.legacy.points, self.legacy.best_floor, self.legacy.runs
            ),
            palette(8),
            Align::Center,
        ));
        for (i, u) in UNLOCKS.iter().enumerate() {
            let owned = self.legacy.owns(u.id);
            let label = if owned {
                format!("{} - owned", u.name)
            } else {
                format!("{} - {}", u.name, u.cost)
            };
            let affordable = !owned && self.legacy.points >= u.cost;
            compose::button(rects, row(i), &label, None, affordable, 4, &mut out);
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
        let last = row(UNLOCKS.len() - 1);
        assert!(last[1] + last[3] < BACK[1]);
    }
}
