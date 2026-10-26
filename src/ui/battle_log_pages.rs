//! `battle_log_tab_container.gd` paging/state logic: current/last battle and
//! phantom page maps, page navigation, button enablement, repeat merging
//! and the visible-line window. Label creation/scroll sizing is shell work.

use std::collections::BTreeMap;

/// One piece of a log line (a `BBContainerData` with its text color).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub text: String,
    pub color: [f32; 4],
}

impl Piece {
    pub fn new(text: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

/// A log line; `None` models an invalid entry, which the source skips.
pub type Line = Vec<Option<Piece>>;
pub type Pages = BTreeMap<i32, Vec<Line>>;

pub const LINE_HEIGHT: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageType {
    CurrentBattle,
    LastBattle,
}

#[derive(Clone, Debug)]
pub struct BattleLogPages {
    pub last_phantom: Pages,
    pub last: Pages,
    pub phantom: Pages,
    pub current: Pages,
    pub page_type: PageType,
    pub is_phantom: bool,
    pub turn_page: i32,
    pub last_log: Line,
    pub repeats: i32,
}

impl Default for BattleLogPages {
    fn default() -> Self {
        Self {
            last_phantom: Pages::new(),
            last: Pages::new(),
            phantom: Pages::new(),
            current: Pages::new(),
            page_type: PageType::CurrentBattle,
            is_phantom: false,
            turn_page: 1,
            last_log: vec![],
            repeats: 1,
        }
    }
}

impl BattleLogPages {
    /// `_on_next_page_button_pressed`.
    pub fn next_page(&mut self) {
        self.turn_page += 1;
        match self.page_type {
            PageType::CurrentBattle => {
                if !self.is_phantom && !self.current.contains_key(&self.turn_page) {
                    self.turn_page -= 1;
                    return;
                }
                if self.is_phantom && !self.phantom.contains_key(&self.turn_page) {
                    self.is_phantom = false;
                    self.turn_page = 1;
                }
            }
            PageType::LastBattle => {
                if !self.is_phantom && !self.last.contains_key(&self.turn_page) {
                    if !self.phantom.is_empty() {
                        self.page_type = PageType::CurrentBattle;
                        self.is_phantom = true;
                        self.turn_page = 1;
                        return;
                    }
                    if !self.current.is_empty() {
                        self.page_type = PageType::CurrentBattle;
                        self.turn_page = 1;
                        return;
                    }
                }
                if self.is_phantom && !self.last_phantom.contains_key(&self.turn_page) {
                    if !self.last.is_empty() {
                        self.is_phantom = false;
                        self.turn_page = 1;
                        return;
                    }
                    if !self.phantom.is_empty() {
                        self.page_type = PageType::CurrentBattle;
                        self.is_phantom = true;
                        self.turn_page = 1;
                        return;
                    }
                    if !self.current.is_empty() {
                        self.page_type = PageType::CurrentBattle;
                        self.turn_page = 1;
                    }
                }
            }
        }
    }

    /// `_on_last_page_button_pressed`.
    pub fn previous_page(&mut self) {
        self.turn_page -= 1;
        match self.page_type {
            PageType::CurrentBattle => {
                if !self.is_phantom && !self.current.contains_key(&self.turn_page) {
                    if !self.phantom.is_empty() {
                        self.turn_page = self.phantom.len() as i32;
                        self.is_phantom = true;
                        return;
                    }
                    if !self.last.is_empty() {
                        self.turn_page = self.last.len() as i32;
                        self.page_type = PageType::LastBattle;
                        self.is_phantom = false;
                        return;
                    }
                    if !self.last_phantom.is_empty() {
                        self.turn_page = self.last_phantom.len() as i32;
                        self.page_type = PageType::LastBattle;
                        self.is_phantom = true;
                        return;
                    }
                    self.turn_page += 1;
                    return;
                }
                if self.is_phantom {
                    if !self.last.is_empty() {
                        self.turn_page = self.last.len() as i32;
                        self.page_type = PageType::LastBattle;
                        self.is_phantom = false;
                        return;
                    }
                    if !self.last_phantom.is_empty() {
                        self.turn_page = self.last_phantom.len() as i32;
                        self.page_type = PageType::LastBattle;
                        self.is_phantom = true;
                        return;
                    }
                    self.turn_page += 1;
                }
            }
            PageType::LastBattle => {
                if !self.is_phantom && !self.last.contains_key(&self.turn_page) {
                    if !self.last_phantom.is_empty() {
                        self.turn_page = self.last_phantom.len() as i32;
                        self.is_phantom = true;
                        return;
                    }
                    self.turn_page += 1;
                    return;
                }
                if self.is_phantom && !self.last_phantom.contains_key(&self.turn_page) {
                    self.turn_page += 1;
                }
            }
        }
    }

    /// `process_buttons`: (next_disabled, last_disabled). The source tests
    /// the size of `last_phantom` as an int truthiness in two places; same here.
    pub fn buttons_disabled(&self) -> (bool, bool) {
        let (mut next, mut last) = (false, false);
        let prev = self.turn_page - 1;
        match self.page_type {
            PageType::CurrentBattle => {
                if !self.is_phantom && !self.current.contains_key(&(self.turn_page + 1)) {
                    next = true;
                }
                if !self.is_phantom
                    && !self.current.contains_key(&prev)
                    && self.phantom.is_empty()
                    && self.last.is_empty()
                    && self.last_phantom.is_empty()
                {
                    last = true;
                }
                if self.is_phantom
                    && !self.phantom.contains_key(&prev)
                    && self.last.is_empty()
                    && self.last_phantom.is_empty()
                {
                    last = true;
                }
            }
            PageType::LastBattle => {
                if !self.is_phantom
                    && !self.last.contains_key(&prev)
                    && self.last_phantom.is_empty()
                {
                    last = true;
                }
                if self.is_phantom && !self.last_phantom.contains_key(&prev) {
                    last = true;
                }
            }
        }
        (next, last)
    }

    pub fn reset_for_phantom(&mut self) {
        self.phantom = std::mem::take(&mut self.current);
        self.turn_page = self.phantom.len() as i32;
        self.is_phantom = true;
    }

    pub fn reset_for_new_battle(&mut self) {
        self.last_phantom = std::mem::take(&mut self.phantom);
        self.last = std::mem::take(&mut self.current);
        self.is_phantom = false;
        self.turn_page = 1;
    }

    /// `update_new_turn` page follow.
    pub fn update_new_turn(&mut self, new_turn: i32) {
        if !self.is_phantom
            && self.page_type == PageType::CurrentBattle
            && self.turn_page == new_turn - 1
        {
            self.turn_page = new_turn;
        }
    }

    /// `add_to_log`: identical consecutive text merges into the previous
    /// line with an " xN" suffix.
    pub fn add_to_log(&mut self, line: Line, turn: i32) {
        let page = self.current.entry(turn).or_default();
        if !self.last_log.is_empty() {
            let id = |l: &Line| {
                l.iter()
                    .flatten()
                    .map(|p| p.text.as_str())
                    .collect::<String>()
            };
            if id(&self.last_log) == id(&line) {
                self.repeats += 1;
                let mut merged = self.last_log.clone();
                merged.push(Some(Piece::new(format!(" x{}", self.repeats), [1.0; 4])));
                page.pop();
                page.push(merged);
                return;
            }
        }
        page.push(line.clone());
        self.last_log = line;
        self.repeats = 1;
    }

    /// `get_visible_bbs` over the current-battle page: lines in
    /// `[start, start + visible)` that exist.
    pub fn visible_lines(&self, scroll_vertical: i32, view_height: f32) -> Vec<&Line> {
        let visible = (view_height / LINE_HEIGHT as f32) as i32;
        let start = scroll_vertical.div_euclid(LINE_HEIGHT);
        let Some(page) = self.current.get(&self.turn_page) else {
            return vec![];
        };
        (start..start + visible)
            .filter_map(|i| usize::try_from(i).ok().and_then(|i| page.get(i)))
            .collect()
    }

    /// Lines of the page being viewed (current/last battle, phantom or not).
    pub fn current_page_lines(&self) -> &[Line] {
        let pages = match (self.page_type, self.is_phantom) {
            (PageType::LastBattle, true) => &self.last_phantom,
            (PageType::LastBattle, false) => &self.last,
            (PageType::CurrentBattle, true) => &self.phantom,
            (PageType::CurrentBattle, false) => &self.current,
        };
        pages.get(&self.turn_page).map_or(&[], Vec::as_slice)
    }

    /// `update_page` label state: (battle label key, dead suffix).
    pub fn page_labels(&self) -> (&'static str, bool) {
        let key = match self.page_type {
            PageType::CurrentBattle => "current-battle",
            PageType::LastBattle => "last-battle",
        };
        (key, self.is_phantom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str) -> Line {
        vec![Some(Piece::new(s, [1.0; 4]))]
    }

    #[test]
    fn repeats_merge_and_pagination() {
        let mut p = BattleLogPages::default();
        p.add_to_log(line("hit"), 1);
        p.add_to_log(line("hit"), 1);
        p.add_to_log(line("hit"), 1);
        assert_eq!(p.current[&1].len(), 1);
        assert_eq!(
            p.current[&1][0].last().unwrap().as_ref().unwrap().text,
            " x3"
        );
        p.add_to_log(line("miss"), 2);
        assert_eq!(p.repeats, 1);
        p.next_page();
        assert_eq!(p.turn_page, 2);
        p.next_page();
        assert_eq!(p.turn_page, 2, "no page 3: stays");
        assert_eq!(p.buttons_disabled(), (true, false));

        p.reset_for_phantom();
        assert!(p.is_phantom && p.turn_page == 2 && p.current.is_empty());
        // Source quirk: a phantom page always leaves for earlier battles
        // (or restores the page) even when an earlier phantom page exists.
        p.previous_page();
        assert_eq!(p.turn_page, 2);

        p.reset_for_new_battle();
        assert_eq!((p.last.len(), p.last_phantom.len(), p.turn_page), (0, 2, 1));
        p.add_to_log(line("new"), 1);
        p.previous_page();
        assert_eq!(
            (p.page_type, p.is_phantom, p.turn_page),
            (PageType::LastBattle, true, 2)
        );
        p.next_page();
        // the source leaves is_phantom set in this branch
        assert_eq!(
            (p.page_type, p.is_phantom, p.turn_page),
            (PageType::CurrentBattle, true, 1)
        );
    }

    #[test]
    fn window() {
        let mut p = BattleLogPages::default();
        for i in 0..10 {
            p.add_to_log(line(&i.to_string()), 1);
        }
        assert_eq!(p.visible_lines(32, 48.0).len(), 3);
        assert_eq!(p.visible_lines(144, 160.0).len(), 1);
        p.update_new_turn(2);
        assert_eq!(p.turn_page, 2);
    }
}
