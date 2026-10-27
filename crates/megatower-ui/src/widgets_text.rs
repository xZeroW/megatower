//! `generic_label`, `generic_rich_text_label`, `generic_v_scroll_bar`,
//! `generic_scroll_container`, `generic_line_edit`, `generic_drop_down`
//! rules without node plumbing.

use megatower_core::format::Rules;

/// `GenericLabel.get_rules`.
pub fn label_rules(
    ignore_plus: bool,
    use_prefix: bool,
    use_suffix: bool,
    is_percent: bool,
    is_whole: bool,
) -> Rules {
    Rules {
        prefix: use_prefix,
        suffix: use_suffix,
        percentage: is_percent,
        ignore_plus,
        whole: is_whole,
    }
}

/// `reload_label` font choice by locale: (font path, size).
pub fn label_font(locale: &str, ignore_override: bool) -> (&'static str, i32) {
    if ignore_override {
        return ("assets/fonts/monogram-extended.ttf", 16);
    }
    match locale {
        "japanese" | "koreana" => ("assets/fonts/Galmuri11.ttf", 12),
        "schinese" => ("assets/fonts/VonwaonBitmap-12px.ttf", 12),
        "tchinese" => ("assets/fonts/Cubic_11_1.100_R.ttf", 12),
        _ => ("assets/fonts/monogram-extended.ttf", 16),
    }
}

/// `move_toward`.
pub fn move_toward(from: f64, to: f64, delta: f64) -> f64 {
    if (to - from).abs() <= delta {
        to
    } else {
        from + (to - from).signum() * delta
    }
}

/// `GenericLabel.update_value`: the new displayed value, or `None` when the
/// label is already at its target (and not forced).
pub fn label_update_value(
    curr: f64,
    target: f64,
    delta: f64,
    update_speed: f64,
    force: bool,
) -> Option<f64> {
    if !force && target == curr {
        return None;
    }
    let mut speed = delta * (target - curr).abs() * update_speed;
    if force || update_speed == -1.0 {
        speed = 1.0;
    }
    let mut v = move_toward(curr, target, speed);
    if v >= target - 0.1 {
        v = target;
    }
    Some(v)
}

/// Paragraph grouping of `GenericRichTextLabel`. Items are opaque; `None`
/// entries are the source's newline markers.
#[derive(Debug, Default)]
pub struct RichText<T> {
    /// Paragraph contents (the first paragraph always exists).
    pub paragraphs: Vec<Vec<T>>,
    /// `bb_containers`: one entry per added piece, `false` for newlines.
    pub pieces: Vec<bool>,
}

impl<T> RichText<T> {
    pub fn new() -> Self {
        Self {
            paragraphs: vec![vec![]],
            pieces: vec![],
        }
    }

    /// `add_bb_container`: a newline starts a new paragraph unless nothing
    /// has been added yet.
    pub fn add(&mut self, text: &str, item: T) {
        if is_newline(text) {
            if !self.pieces.is_empty() {
                self.paragraphs.push(vec![]);
            }
            self.pieces.push(false);
            return;
        }
        self.paragraphs.last_mut().expect("paragraph").push(item);
        self.pieces.push(true);
    }

    pub fn clear(&mut self) {
        self.paragraphs = vec![vec![]];
        self.pieces.clear();
    }

    /// `remove_last_paragraph`: drops it; an empty result gets a fresh one.
    pub fn remove_last_paragraph(&mut self) {
        self.paragraphs.pop();
        if self.paragraphs.is_empty() {
            self.paragraphs.push(vec![]);
        }
    }
}

/// `text.c_unescape() == "\n"`.
pub fn is_newline(text: &str) -> bool {
    text == "\n" || text == "\\n"
}

pub const SCROLL_MIN_SIZE: i32 = 12;
pub const SCROLL_OFFSET: f32 = 2.0;

/// `GenericVScrollBar` geometry. `size_y` is the bar's own height.
#[derive(Clone, Copy, Debug)]
pub struct VScrollBar {
    pub size_y: f32,
    pub min_size: f32,
    pub max_size: f32,
    pub rect_height: f32,
}

impl VScrollBar {
    pub fn ratio(&self) -> f32 {
        self.min_size / self.max_size
    }
    /// `update_size`: (handle height, bar visible). NaN ratios keep the old
    /// height.
    pub fn update_size(&self) -> (Option<f32>, bool) {
        let new = (self.min_size - SCROLL_OFFSET * 2.0) * self.ratio();
        let h = (!new.is_nan()).then(|| (new as i32).max(SCROLL_MIN_SIZE) as f32);
        (h, self.max_size != self.min_size)
    }
    pub fn max_pos(&self) -> i32 {
        (self.size_y - self.rect_height - SCROLL_OFFSET * 2.0) as i32
    }
    /// `get_scroll` from the handle's y.
    pub fn scroll(&self, rect_y: f32) -> f32 {
        let curr = rect_y - SCROLL_OFFSET;
        (curr / self.max_pos() as f32) * (self.max_size - self.min_size)
    }
    /// Target clamp used every frame.
    pub fn clamp_target(&self, target: f32) -> f32 {
        target.clamp(
            SCROLL_OFFSET,
            self.size_y - self.rect_height - SCROLL_OFFSET,
        )
    }
    /// One easing step of the handle. Returns (new y, tick sound).
    pub fn ease(&self, y: f32, target: f32, last_y: f32, delta: f32) -> (f32, bool) {
        let mut d = delta * 17.5;
        if (y - target).abs() < 1.0 {
            d = 1.0;
        }
        let ny = y + (target - y) * d;
        let tick = (last_y / 10.0).round() as i32 != (ny / 10.0).round() as i32;
        (ny, tick)
    }
}

/// `GenericScrollContainer`: wheel steps and the resulting scroll_vertical.
pub fn wheel_target(target: f32, scroll_speed: i32, up: bool, down: bool) -> f32 {
    let mut t = target;
    if up {
        t -= scroll_speed as f32;
    }
    if down {
        t += scroll_speed as f32;
    }
    t
}

/// Right-stick scroll value rounding shared with the bar.
pub fn scroll_axis(axis_value: f32) -> f32 {
    (axis_value * 10.0).round() / 10.0
}

/// `GenericLineEdit._on_line_edit_text_changed`: char limit then caps lock.
pub fn line_edit_text(new_text: &str, char_limit: i32, caps_locked: bool) -> String {
    let mut t: String = if char_limit == -1 {
        new_text.to_string()
    } else {
        new_text.chars().take(char_limit.max(0) as usize).collect()
    };
    if caps_locked {
        t = t.to_uppercase();
    }
    t
}

/// `GenericDropDown.select(name)`: matches only indexes 1.. (index 0 can
/// never be chosen by name and is the fallback).
pub fn dropdown_select(selections: &[String], selection: &str) -> usize {
    let mut idx = 0;
    for (i, s) in selections.iter().enumerate() {
        if i != 0 && s == selection {
            idx = i;
        }
    }
    idx
}

/// `GenericDropDown` click handling: (selecting, chosen index).
pub fn dropdown_click(
    selecting: bool,
    main_hovered: bool,
    hovered_selection: Option<usize>,
    scroll_bar_hovered: bool,
) -> (bool, Option<usize>) {
    if main_hovered {
        return (!selecting, None);
    }
    let mut next = selecting;
    if !scroll_bar_hovered {
        next = false;
    }
    (next, hovered_selection)
}

/// Visible rows for the scroll area: the container height grows by each
/// selection until five are shown.
pub fn dropdown_scroll_height(selection_heights: &[f32]) -> f32 {
    selection_heights.iter().take(5).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use megatower_core::format;

    #[test]
    fn label_updates_and_rules() {
        let rules = label_rules(false, true, false, false, true);
        assert_eq!(format::number(5.0, rules), format::number(5.0, rules));
        assert_eq!(label_update_value(5.0, 5.0, 0.1, 2.5, false), None);
        assert_eq!(label_update_value(0.0, 100.0, 0.016, 2.5, true), Some(1.0));
        let v = label_update_value(0.0, 100.0, 0.016, 2.5, false).unwrap();
        assert!((v - 4.0).abs() < 1e-9);
        assert_eq!(
            label_update_value(99.95, 100.0, 0.016, 2.5, false),
            Some(100.0)
        );
        assert_eq!(
            label_font("koreana", false),
            ("assets/fonts/Galmuri11.ttf", 12)
        );
        assert_eq!(label_font("koreana", true).1, 16);
    }

    #[test]
    fn rich_text_paragraphs() {
        let mut r = RichText::new();
        r.add("\n", 0); // leading newline adds nothing visible
        r.add("a", 1);
        r.add("b", 2);
        r.add("\\n", 3);
        r.add("c", 4);
        assert_eq!(r.paragraphs, vec![vec![1, 2], vec![4]]);
        r.remove_last_paragraph();
        assert_eq!(r.paragraphs.len(), 1);
        r.remove_last_paragraph();
        assert_eq!(r.paragraphs.len(), 1);
    }

    #[test]
    fn scroll_bar_and_inputs() {
        let b = VScrollBar {
            size_y: 100.0,
            min_size: 100.0,
            max_size: 200.0,
            rect_height: 40.0,
        };
        assert_eq!(b.update_size(), (Some(48.0), true));
        assert_eq!(
            VScrollBar {
                max_size: 100.0,
                ..b
            }
            .update_size()
            .1,
            false
        );
        assert_eq!(b.max_pos(), 56);
        assert_eq!(b.scroll(2.0), 0.0);
        assert!((b.scroll(58.0) - 100.0).abs() < 1e-4);
        assert_eq!(b.clamp_target(500.0), 58.0);
        let (y, tick) = b.ease(2.0, 30.0, 2.0, 0.016);
        assert!(y > 2.0 && !tick || y > 2.0);
        assert_eq!(wheel_target(10.0, 10, true, true), 10.0);
        assert_eq!(line_edit_text("abcdef", 3, true), "ABC");
        assert_eq!(line_edit_text("abc", -1, false), "abc");
        let s = vec!["a".to_string(), "b".to_string()];
        assert_eq!(dropdown_select(&s, "a"), 0);
        assert_eq!(dropdown_select(&s, "b"), 1);
        assert_eq!(dropdown_click(true, true, None, false), (false, None));
        assert_eq!(
            dropdown_click(true, false, Some(2), false),
            (false, Some(2))
        );
        assert_eq!(dropdown_click(true, false, Some(2), true), (true, Some(2)));
        assert_eq!(dropdown_scroll_height(&[10.0; 7]), 50.0);
    }
}
