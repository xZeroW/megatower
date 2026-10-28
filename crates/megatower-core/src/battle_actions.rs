//! `BattleActions` singleton: popup text/color and screen-shake data for the 16
//! recovered actions. Hover/BB rendering is not implemented.

use crate::run::events::Popup;
use crate::translation;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BattleActionId {
    ArmorBreak,
    Backstab,
    Block,
    Cleanse,
    CounterAttack,
    CriticalStrike,
    Dodge,
    Guard,
    Hit,
    Immolate,
    LuckyAttack,
    Miss,
    MultiAttack,
    OmniCrit,
    Parry,
    Polymorph,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BattleAction {
    pub id: BattleActionId,
    pub name: String,
    pub icon: Option<String>,
    pub color: [f32; 4],
    pub use_limit: i32,
    pub popup_color: [f32; 4],
    pub popup_text: String,
    pub popup_delay: f64,
    pub screen_shake: bool,
}

impl BattleAction {
    /// `get_action_popup_label_data`: translated popup text, uppercased.
    pub fn popup(&self) -> Popup {
        Popup::new(
            translation::t(&self.popup_text, "popup").to_uppercase(),
            self.popup_color,
        )
    }
}

pub fn action(id: BattleActionId) -> &'static BattleAction {
    static ACTIONS: OnceLock<Vec<BattleAction>> = OnceLock::new();
    ACTIONS
        .get_or_init(|| {
            crate::ron_data::from_str(
                include_str!("../../../assets/data/content/battle-actions.ron"),
                "battle-actions.ron",
            )
        })
        .iter()
        .find(|action| action.id == id)
        .expect("all battle actions exist")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popups_use_translation_and_recovered_color() {
        let miss = action(BattleActionId::Miss).popup();
        assert_eq!(miss.text, "MISSED");
        assert!((miss.color[0] - 0.729412).abs() < 1e-6);
        assert!(!action(BattleActionId::Miss).screen_shake);
    }
}
