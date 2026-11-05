//! The prompt after "Enter the Tower" when a saved run exists: pick it up
//! where the last room left off, or start a new one.
use super::*;
use crate::compose::{self, Align, Draw, WHITE, palette, text};

const CONTINUE: [f32; 4] = [170.0, 120.0, 300.0, 30.0];
const NEW_RUN: [f32; 4] = [170.0, 162.0, 300.0, 30.0];
const BACK: [f32; 4] = [270.0, 322.0, 100.0, 26.0];

impl Labyrinth {
    pub(crate) fn continue_frame(
        &mut self,
        ui: &mut UiState,
        rects: &Rects,
        keys: &ButtonInput<KeyCode>,
        click: bool,
    ) -> Vec<Draw> {
        if !self.has_save() {
            ui.continue_open = false;
        }
        if let (true, Some([x, y])) = (click, ui.cursor) {
            if contains(CONTINUE, x, y) {
                ui.continue_open = false;
                if self.resume() {
                    ui.screen = Screen::Playing;
                    ui.timeline = Timeline::default();
                    ui.presented_room = None;
                    ui.transition = None;
                    ui.death_at = None;
                    ui.dragged = None;
                    ui.popups.clear();
                }
            } else if contains(NEW_RUN, x, y) {
                ui.continue_open = false;
                ui.screen = Screen::Select;
            } else if contains(BACK, x, y) {
                ui.continue_open = false;
            }
        }
        if keys.just_pressed(KeyCode::Escape) {
            ui.continue_open = false;
        }
        let mut out = vec![Draw::Fill {
            rect: [0.0, 0.0, 640.0, 360.0],
            color: palette(1),
        }];
        out.push(text(
            [120.0, 60.0, 400.0, 24.0],
            "SAVED RUN",
            WHITE,
            Align::Center,
        ));
        if let Some(info) = &self.saved {
            out.push(text(
                [120.0, 88.0, 400.0, 13.0],
                format!("Floor {}  |  level {}", info.floor, info.level),
                palette(8),
                Align::Center,
            ));
        }
        compose::button(rects, CONTINUE, "Continue", None, true, 4, &mut out);
        compose::button(
            rects,
            NEW_RUN,
            "New run (abandons the save)",
            None,
            true,
            4,
            &mut out,
        );
        compose::button(rects, BACK, "Back", None, true, 4, &mut out);
        out
    }
}
